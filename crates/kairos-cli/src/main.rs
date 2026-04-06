mod presentation;
mod scaffold;
mod shell;
mod workspace;

use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use scaffold::{create_new_project, init_project, ScaffoldReport, TemplateKind};
use shell::run_shell;
use workspace::{
    diagnostics_to_anyhow, format_project, load_program, parse_error_to_diagnostic,
    print_diagnostics, print_json, print_warning_summary, project_error_to_anyhow, read_source,
    LoadedWorkspace,
};

#[derive(Debug, Parser)]
#[command(name = "kairos", about = "Kairos language CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Check {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Fmt {
        path: PathBuf,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        stdout: bool,
    },
    Ast {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Ir {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Prompt {
        path: PathBuf,
    },
    Run {
        path: PathBuf,
        #[arg(long)]
        function: Option<String>,
        #[arg(long = "arg")]
        args: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    Shell {
        path: Option<PathBuf>,
    },
    New {
        name: String,
        #[arg(long, value_enum, default_value_t = TemplateKind::Default)]
        template: TemplateKind,
    },
    Init {
        #[arg(long, value_enum, default_value_t = TemplateKind::Default)]
        template: TemplateKind,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Check { path, json } => command_check(&path, json),
        Command::Fmt { path, check, stdout } => command_fmt(&path, check, stdout),
        Command::Ast { path, json } => command_ast(&path, json),
        Command::Ir { path, json } => command_ir(&path, json),
        Command::Prompt { path } => command_prompt(&path),
        Command::Run { path, function, args, json } => {
            command_run(&path, function.as_deref(), &args, json)
        }
        Command::Shell { path } => run_shell(path.as_deref()),
        Command::New { name, template } => command_new(&name, template),
        Command::Init { template } => command_init(template),
    }
}

fn command_check(path: &Path, json_output: bool) -> Result<()> {
    match LoadedWorkspace::load(path) {
        Ok(workspace) => {
            if json_output {
                print_json(&workspace.check_json())?;
            } else {
                match &workspace {
                    LoadedWorkspace::Standalone { analyzed, source_hint } => {
                        println!("OK: module `{}` validated", analyzed.program.module);
                        println!("Path: {}", source_hint.display());
                    }
                    LoadedWorkspace::Project { analyzed, focus_module, .. } => {
                        println!(
                            "OK: project `{}` validated",
                            analyzed.project.manifest.package.name
                        );
                        println!("Entry module: {}", analyzed.project.entry_module);
                        println!("Modules: {}", analyzed.project.modules.len());
                        if let Some(focus_module) = focus_module {
                            println!("Focused module: {focus_module}");
                        }
                        println!("Manifest: {}", analyzed.project.manifest_path.display());
                    }
                }
                print_warning_summary(workspace.warnings());
            }
            Ok(())
        }
        Err(diagnostics) => {
            if json_output {
                workspace::exit_with_json_error(&diagnostics)?;
            }
            print_diagnostics(&diagnostics);
            process::exit(1);
        }
    }
}

fn command_fmt(path: &Path, check: bool, stdout: bool) -> Result<()> {
    if check && stdout {
        bail!("`--check` and `--stdout` cannot be used together");
    }

    if is_project_root_input(path) {
        if stdout {
            bail!("`--stdout` requires a single file input");
        }
        let project = workspace_project(path)?;
        return format_project(&project, check);
    }

    let source = read_source(path)?;
    let program = kairos_parser::parse_source(&source)
        .map_err(|error| diagnostics_to_anyhow(&[parse_error_to_diagnostic(path, error)]))?;
    let formatted = kairos_formatter::format_program(&program);

    if stdout {
        print!("{formatted}");
        return Ok(());
    }

    if check {
        if source == formatted {
            println!("OK: `{}` is canonically formatted", path.display());
            return Ok(());
        }
        bail!(
            "`{}` is not canonically formatted. Run `kairos fmt {}`.",
            path.display(),
            path.display()
        );
    }

    if source == formatted {
        println!("No changes: `{}` is already formatted", path.display());
    } else {
        fs::write(path, formatted)
            .with_context(|| format!("failed to write `{}`", path.display()))?;
        println!("Formatted `{}`", path.display());
    }

    Ok(())
}

fn command_ast(path: &Path, json_output: bool) -> Result<()> {
    let _json_output = json_output;
    let value = if is_project_root_input(path) {
        let project = workspace_project(path)?;
        serde_json::to_value(&project)?
    } else {
        let program =
            load_program(path).map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?;
        serde_json::to_value(&program)?
    };
    print_json(&value)?;
    Ok(())
}

fn command_ir(path: &Path, json_output: bool) -> Result<()> {
    let _json_output = json_output;
    let workspace =
        LoadedWorkspace::load(path).map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?;
    let value = workspace.ir_value(None)?;
    print_json(&value)?;
    Ok(())
}

fn command_prompt(path: &Path) -> Result<()> {
    let workspace =
        LoadedWorkspace::load(path).map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?;
    print!("{}", workspace.prompt_text(None)?);
    Ok(())
}

fn command_run(
    path: &Path,
    function: Option<&str>,
    args: &[String],
    json_output: bool,
) -> Result<()> {
    let _json_output = json_output;
    let runtime_args =
        args.iter().map(|arg| workspace::parse_runtime_value(arg)).collect::<Result<Vec<_>>>()?;
    let workspace =
        LoadedWorkspace::load(path).map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?;
    let report = workspace.run(function, &runtime_args)?;
    print_json(&serde_json::to_value(&report)?)?;
    Ok(())
}

fn command_new(name: &str, template: TemplateKind) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to determine current directory")?;
    let report = create_new_project(&cwd, name, template)?;
    print_scaffold_report("Created", &report);
    Ok(())
}

fn command_init(template: TemplateKind) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to determine current directory")?;
    let report = init_project(&cwd, template)?;
    print_scaffold_report("Initialized", &report);
    Ok(())
}

fn print_scaffold_report(verb: &str, report: &ScaffoldReport) {
    println!(
        "{} Kairos project `{}` with template `{}`",
        verb,
        report.package_name,
        template_label(report.template)
    );
    println!("Root: {}", report.root.display());
    if !report.created.is_empty() {
        println!("Created:");
        for item in &report.created {
            println!("- {item}");
        }
    }
    if !report.skipped.is_empty() {
        println!("Skipped:");
        for item in &report.skipped {
            println!("- {item}");
        }
    }
    println!("Next steps:");
    println!("- `Set-Location \"{}\"`", report.root.display());
    println!("- `cargo run --bin kairos -- check .`");
    println!("- `cargo run --bin kairos -- shell .`");
}

fn template_label(template: TemplateKind) -> &'static str {
    match template {
        TemplateKind::Default => "default",
        TemplateKind::Briefing => "briefing",
        TemplateKind::Rules => "rules",
    }
}

fn workspace_project(path: &Path) -> Result<kairos_project::Project> {
    kairos_project::load_project(path).map_err(project_error_to_anyhow)
}

fn is_project_root_input(path: &Path) -> bool {
    path.is_dir() || path.file_name().is_some_and(|name| name == "kairos.toml")
}
