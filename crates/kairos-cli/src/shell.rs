use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{bail, Context, Result};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{
    presentation::{
        clear_screen_sequence, render_module_list, render_shell_banner, render_shell_help,
        render_shell_status, ShellSnapshot,
    },
    workspace::{
        diagnostics_to_anyhow, normalize_display_path, parse_runtime_value, print_json,
        LoadedWorkspace, ModuleRecord,
    },
};

pub fn run_shell(path: Option<&Path>) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to determine current directory")?;
    let initial_workspace = match path {
        Some(path) => Some(
            LoadedWorkspace::load(path)
                .map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?,
        ),
        None => LoadedWorkspace::auto_detect(&cwd)
            .map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?,
    };

    let mut session = ShellSession::new(cwd, initial_workspace);
    session.print_banner()?;
    session.run()
}

#[derive(Debug)]
struct ShellSession {
    state: Arc<Mutex<ShellState>>,
    watch: Option<ActiveWatch>,
}

#[derive(Debug)]
struct ShellState {
    cwd: PathBuf,
    workspace: Option<LoadedWorkspace>,
    watch_status: String,
}

#[derive(Debug)]
struct ActiveWatch {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ShellCommand {
    Help,
    Status,
    Load(String),
    Check,
    Ast(Option<String>),
    Ir(Option<String>),
    Prompt(Option<String>),
    Run { function: Option<String>, args: Vec<String> },
    Modules,
    Reload,
    Watch,
    Unwatch,
    Clear,
    Quit,
}

impl ShellSession {
    fn new(cwd: PathBuf, workspace: Option<LoadedWorkspace>) -> Self {
        let watch_status = "off".to_string();
        Self {
            state: Arc::new(Mutex::new(ShellState { cwd, workspace, watch_status })),
            watch: None,
        }
    }

    fn run(&mut self) -> Result<()> {
        let stdin = io::stdin();

        loop {
            print!("kairos> ");
            io::stdout().flush().context("failed to flush shell prompt")?;

            let mut line = String::new();
            if stdin.read_line(&mut line).context("failed to read shell input")? == 0 {
                break;
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let command = match parse_shell_command(line) {
                Ok(command) => command,
                Err(error) => {
                    println!("error: {error}");
                    continue;
                }
            };

            let should_exit = self.execute(command)?;
            if should_exit {
                break;
            }
        }

        self.stop_watch()?;
        println!("Leaving Kairos shell.");
        Ok(())
    }

    fn execute(&mut self, command: ShellCommand) -> Result<bool> {
        match command {
            ShellCommand::Help => {
                println!("{}", render_shell_help());
            }
            ShellCommand::Status => {
                let snapshot = self.snapshot()?;
                println!("{}", render_shell_status(&snapshot));
            }
            ShellCommand::Load(path) => {
                let was_watching = self.watch.is_some();
                if was_watching {
                    self.stop_watch()?;
                }
                let absolute = self.resolve_input_path(&path)?;
                let loaded = LoadedWorkspace::load(&absolute)
                    .map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?;
                {
                    let mut state = self.state.lock().expect("shell state should lock");
                    state.workspace = Some(loaded);
                }
                println!("{}", self.reload_summary("loaded")?);
                if was_watching {
                    println!("Watch mode paused after `:load`. Run `:watch` to resume.");
                }
            }
            ShellCommand::Check => {
                self.reload_current("validated")?;
            }
            ShellCommand::Ast(selector) => {
                let value = {
                    let state = self.state.lock().expect("shell state should lock");
                    let workspace = state
                        .workspace
                        .as_ref()
                        .context("no Kairos project or file is currently loaded")?;
                    workspace.ast_value(selector.as_deref())?
                };
                print_json(&value)?;
            }
            ShellCommand::Ir(selector) => {
                let value = {
                    let state = self.state.lock().expect("shell state should lock");
                    let workspace = state
                        .workspace
                        .as_ref()
                        .context("no Kairos project or file is currently loaded")?;
                    workspace.ir_value(selector.as_deref())?
                };
                print_json(&value)?;
            }
            ShellCommand::Prompt(selector) => {
                let text = {
                    let state = self.state.lock().expect("shell state should lock");
                    let workspace = state
                        .workspace
                        .as_ref()
                        .context("no Kairos project or file is currently loaded")?;
                    workspace.prompt_text(selector.as_deref())?
                };
                println!("{text}");
            }
            ShellCommand::Run { function, args } => {
                let runtime_args =
                    args.iter().map(|arg| parse_runtime_value(arg)).collect::<Result<Vec<_>>>()?;
                let report = {
                    let state = self.state.lock().expect("shell state should lock");
                    let workspace = state
                        .workspace
                        .as_ref()
                        .context("no Kairos project or file is currently loaded")?;
                    workspace.run(function.as_deref(), &runtime_args)?
                };
                print_json(&serde_json::to_value(&report)?)?;
            }
            ShellCommand::Modules => {
                let records = self.module_records()?;
                println!("{}", render_module_list(&records));
            }
            ShellCommand::Reload => {
                self.reload_current("reloaded")?;
            }
            ShellCommand::Watch => {
                self.start_watch()?;
            }
            ShellCommand::Unwatch => {
                self.stop_watch()?;
                println!("Watch mode disabled.");
            }
            ShellCommand::Clear => {
                print!("{}", clear_screen_sequence());
                io::stdout().flush().context("failed to flush clear sequence")?;
                self.print_banner()?;
            }
            ShellCommand::Quit => return Ok(true),
        }

        Ok(false)
    }

    fn snapshot(&self) -> Result<ShellSnapshot> {
        let state = self.state.lock().expect("shell state should lock");
        Ok(state.snapshot())
    }

    fn print_banner(&self) -> Result<()> {
        let snapshot = self.snapshot()?;
        println!("{}", render_shell_banner(env!("CARGO_PKG_VERSION"), &snapshot));
        Ok(())
    }

    fn resolve_input_path(&self, input: &str) -> Result<PathBuf> {
        let candidate = PathBuf::from(input);
        if candidate.is_absolute() {
            return Ok(candidate);
        }

        let state = self.state.lock().expect("shell state should lock");
        let base = match &state.workspace {
            Some(LoadedWorkspace::Project { analyzed, .. }) => analyzed.project.root.clone(),
            Some(LoadedWorkspace::Standalone { source_hint, .. }) => {
                source_hint.parent().map(Path::to_path_buf).unwrap_or_else(|| state.cwd.clone())
            }
            None => state.cwd.clone(),
        };

        Ok(base.join(candidate))
    }

    fn module_records(&self) -> Result<Vec<ModuleRecord>> {
        let state = self.state.lock().expect("shell state should lock");
        let workspace =
            state.workspace.as_ref().context("no Kairos project or file is currently loaded")?;
        Ok(workspace.module_records())
    }

    fn reload_current(&mut self, verb: &str) -> Result<()> {
        let summary = {
            let mut state = self.state.lock().expect("shell state should lock");
            state.reload_current(verb)?
        };
        println!("{summary}");
        Ok(())
    }

    fn reload_summary(&self, verb: &str) -> Result<String> {
        let state = self.state.lock().expect("shell state should lock");
        let workspace =
            state.workspace.as_ref().context("no Kairos project or file is currently loaded")?;
        Ok(success_summary(workspace, verb))
    }

    fn start_watch(&mut self) -> Result<()> {
        if self.watch.is_some() {
            println!("Watch mode is already active.");
            return Ok(());
        }

        let watch_paths = {
            let state = self.state.lock().expect("shell state should lock");
            state.watch_paths()?
        };

        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::clone(&self.state);
        let thread_stop = Arc::clone(&stop);

        let handle = thread::spawn(move || {
            let (tx, rx) = mpsc::channel();
            let mut watcher = match RecommendedWatcher::new(
                move |result| {
                    let _ = tx.send(result);
                },
                Config::default(),
            ) {
                Ok(watcher) => watcher,
                Err(error) => {
                    print_watch_message(&format!("[watch] failed to start watcher: {error}"));
                    return;
                }
            };

            for (path, mode) in &watch_paths {
                if let Err(error) = watcher.watch(path, *mode) {
                    print_watch_message(&format!(
                        "[watch] failed to watch `{}`: {error}",
                        path.display()
                    ));
                    return;
                }
            }

            while !thread_stop.load(Ordering::Relaxed) {
                let Ok(result) = rx.recv_timeout(Duration::from_millis(250)) else {
                    continue;
                };

                let Ok(event) = result else {
                    print_watch_message("[watch] watcher delivered an invalid event");
                    continue;
                };

                let mut changed = collect_relevant_paths(&event.paths);
                while let Ok(result) = rx.recv_timeout(Duration::from_millis(120)) {
                    match result {
                        Ok(event) => changed.extend(collect_relevant_paths(&event.paths)),
                        Err(error) => {
                            print_watch_message(&format!("[watch] watcher error: {error}"));
                        }
                    }
                }

                if changed.is_empty() {
                    continue;
                }

                let message = {
                    let mut guard = state.lock().expect("shell state should lock");
                    match guard.reload_current("reloaded") {
                        Ok(summary) => format!(
                            "[watch] changed: {}\n{summary}",
                            changed.into_iter().collect::<Vec<_>>().join(", ")
                        ),
                        Err(error) => format!(
                            "[watch] changed: {}\n[watch] reload failed: {error}",
                            changed.into_iter().collect::<Vec<_>>().join(", ")
                        ),
                    }
                };

                print_watch_message(&message);
            }
        });

        {
            let mut state = self.state.lock().expect("shell state should lock");
            state.watch_status = "active".to_string();
        }
        self.watch = Some(ActiveWatch { stop, handle });
        println!("Watch mode enabled.");
        Ok(())
    }

    fn stop_watch(&mut self) -> Result<()> {
        if let Some(active_watch) = self.watch.take() {
            active_watch.stop.store(true, Ordering::Relaxed);
            let _ = active_watch.handle.join();
        }
        let mut state = self.state.lock().expect("shell state should lock");
        state.watch_status = "off".to_string();
        Ok(())
    }
}

impl ShellState {
    fn snapshot(&self) -> ShellSnapshot {
        match &self.workspace {
            Some(workspace) => ShellSnapshot {
                mode: workspace.mode_label().to_string(),
                source: workspace.target_label().to_string(),
                root: workspace.display_root(),
                package: workspace.package_name().map(ToOwned::to_owned),
                entry: workspace.entry_module().map(ToOwned::to_owned),
                modules: Some(workspace.module_count()),
                focus: workspace.focus_module().map(ToOwned::to_owned),
                watch: self.watch_status.clone(),
            },
            None => ShellSnapshot {
                mode: "unloaded | deterministic".to_string(),
                source: "none".to_string(),
                root: normalize_display_path(&self.cwd),
                package: None,
                entry: None,
                modules: None,
                focus: None,
                watch: self.watch_status.clone(),
            },
        }
    }

    fn reload_current(&mut self, verb: &str) -> Result<String> {
        let Some(current) = self.workspace.clone() else {
            bail!("no Kairos project or file is currently loaded");
        };

        let reloaded =
            current.reload().map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?;
        let summary = success_summary(&reloaded, verb);
        self.workspace = Some(reloaded);
        Ok(summary)
    }

    fn watch_paths(&self) -> Result<Vec<(PathBuf, RecursiveMode)>> {
        let Some(workspace) = &self.workspace else {
            bail!("load a project or `.kai` file before enabling watch mode");
        };

        match workspace {
            LoadedWorkspace::Project { analyzed, .. } => {
                Ok(vec![(analyzed.project.root.clone(), RecursiveMode::Recursive)])
            }
            LoadedWorkspace::Standalone { source_hint, .. } => {
                let parent =
                    source_hint.parent().map(Path::to_path_buf).unwrap_or_else(|| self.cwd.clone());
                Ok(vec![(parent, RecursiveMode::NonRecursive)])
            }
        }
    }
}

fn parse_shell_command(input: &str) -> Result<ShellCommand> {
    if !input.starts_with(':') {
        bail!("shell commands must start with `:`. Use `:help` for the command list");
    }

    let tokens = shlex::split(input).context("failed to parse shell input")?;
    if tokens.is_empty() {
        bail!("empty shell command");
    }

    let command = tokens[0].trim_start_matches(':');
    let args = &tokens[1..];

    match command {
        "help" if args.is_empty() => Ok(ShellCommand::Help),
        "status" if args.is_empty() => Ok(ShellCommand::Status),
        "load" if args.len() == 1 => Ok(ShellCommand::Load(args[0].clone())),
        "check" if args.is_empty() => Ok(ShellCommand::Check),
        "ast" if args.len() <= 1 => Ok(ShellCommand::Ast(args.first().cloned())),
        "ir" if args.len() <= 1 => Ok(ShellCommand::Ir(args.first().cloned())),
        "prompt" if args.len() <= 1 => Ok(ShellCommand::Prompt(args.first().cloned())),
        "run" => Ok(ShellCommand::Run {
            function: args.first().cloned(),
            args: args.iter().skip(1).cloned().collect(),
        }),
        "modules" if args.is_empty() => Ok(ShellCommand::Modules),
        "reload" if args.is_empty() => Ok(ShellCommand::Reload),
        "watch" if args.is_empty() => Ok(ShellCommand::Watch),
        "unwatch" if args.is_empty() => Ok(ShellCommand::Unwatch),
        "clear" if args.is_empty() => Ok(ShellCommand::Clear),
        "quit" if args.is_empty() => Ok(ShellCommand::Quit),
        _ => bail!("unknown shell command `{command}`"),
    }
}

fn success_summary(workspace: &LoadedWorkspace, verb: &str) -> String {
    let warnings = workspace.warnings().len();
    match workspace {
        LoadedWorkspace::Standalone { analyzed, source_hint, .. } => format!(
            "OK: {verb} module `{}` from `{}` (warnings: {warnings})",
            analyzed.program.module,
            source_hint.display(),
        ),
        LoadedWorkspace::Project { analyzed, focus_module, .. } => {
            let mut summary = format!(
                "OK: {verb} project `{}` (entry: {}, modules: {})",
                analyzed.project.manifest.package.name,
                analyzed.project.entry_module,
                analyzed.project.modules.len(),
            );
            if let Some(focus_module) = focus_module {
                summary.push_str(&format!(", focus: {focus_module}"));
            }
            summary.push_str(&format!(", warnings: {warnings}"));
            summary
        }
    }
}

fn collect_relevant_paths(paths: &[PathBuf]) -> Vec<String> {
    let mut changed = paths
        .iter()
        .filter_map(|path| {
            let normalized = normalize_display_path(path);
            let is_source = normalized.ends_with(".kai") || normalized.ends_with("kairos.toml");
            is_source.then_some(normalized)
        })
        .collect::<Vec<_>>();
    changed.sort();
    changed.dedup();
    changed
}

fn print_watch_message(message: &str) {
    println!("\n{message}");
    print!("kairos> ");
    let _ = io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::{parse_shell_command, ShellSession, ShellState};
    use crate::workspace::LoadedWorkspace;

    #[test]
    fn parses_run_command_with_args() {
        let parsed = parse_shell_command(":run classify 72 true").expect("command should parse");
        assert!(matches!(
            parsed,
            super::ShellCommand::Run { function: Some(_), args } if args == vec!["72", "true"]
        ));
    }

    #[test]
    fn parses_load_command_with_quoted_path() {
        let parsed = parse_shell_command(":load \"examples/assistant_briefing\"")
            .expect("command should parse");
        assert_eq!(parsed, super::ShellCommand::Load("examples/assistant_briefing".to_string()));
    }

    #[test]
    fn reload_current_updates_workspace() {
        let tempdir = tempdir().expect("tempdir should exist");
        let root = tempdir.path();
        fs::write(
            root.join("main.kai"),
            "module demo.live;\n\nfn main() -> Str\ndescribe \"live\"\ntags [\"live\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"one\";\n}\n",
        )
        .expect("source should write");
        let workspace =
            LoadedWorkspace::load(&root.join("main.kai")).expect("workspace should load");
        let mut state = ShellState {
            cwd: root.to_path_buf(),
            workspace: Some(workspace),
            watch_status: "off".to_string(),
        };

        fs::write(
            root.join("main.kai"),
            "module demo.live;\n\nfn main() -> Str\ndescribe \"live\"\ntags [\"live\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"two\";\n}\n",
        )
        .expect("source should update");

        let summary = state.reload_current("reloaded").expect("workspace should reload");
        assert!(summary.contains("demo.live"));
    }

    #[test]
    fn snapshot_reports_unloaded_mode() {
        let session = ShellSession::new(PathBuf::from("C:/demo"), None);
        let snapshot = session.snapshot().expect("snapshot should render");
        assert_eq!(snapshot.mode, "unloaded | deterministic");
        assert_eq!(snapshot.watch, "off");
    }
}
