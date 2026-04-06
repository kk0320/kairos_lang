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
use presentation::{render_doctor_report, render_execution_report, render_test_report};
use scaffold::{create_new_project, init_project, ScaffoldReport, TemplateKind};
use shell::run_shell;
use workspace::{
    diagnostics_to_anyhow, format_project, load_program, parse_error_to_diagnostic,
    print_diagnostics, print_json, print_warning_summary, project_error_to_anyhow, read_source,
    DoctorCheck, DoctorReport, DoctorStatus, LoadedWorkspace,
};

#[derive(Debug, Parser)]
#[command(
    name = "kairos",
    version,
    about = "AI-first programming language and terminal-native toolchain for deterministic `.kai` projects",
    long_about = "Kairos is an AI-first programming language and terminal-native toolchain for deterministic `.kai` projects.\n\nUse Kairos to validate source, inspect stable AST/KIR JSON, generate prompt context, run deterministic functions, open the interactive shell, and scaffold new local projects.",
    after_help = "Examples:\n  kairos check examples\\assistant_briefing --json\n  kairos test examples\\decision_bundle\n  kairos doctor examples\\package_reuse_demo\n  kairos prompt examples\\assistant_briefing\n  kairos run examples\\decision_bundle --function classify --arg 72 --json\n  kairos shell examples\\assistant_briefing\n  kairos new demo_project --template briefing",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(
        about = "Validate a `.kai` file or Kairos project",
        long_about = "Parse and semantically validate a standalone `.kai` file or a whole Kairos project rooted by `kairos.toml`. When a file belongs to a project, Kairos validates it with full project/module resolution."
    )]
    Check {
        #[arg(
            value_name = "PATH",
            help = "Path to a `.kai` file, project directory, or `kairos.toml`"
        )]
        path: PathBuf,
        #[arg(
            long,
            help = "Emit stable JSON status or diagnostics instead of human-readable summaries"
        )]
        json: bool,
    },
    #[command(
        about = "Apply canonical formatting",
        long_about = "Format one `.kai` file or every discovered module in a Kairos project using Kairos's deterministic canonical style."
    )]
    Fmt {
        #[arg(
            value_name = "PATH",
            help = "Path to a `.kai` file, project directory, or `kairos.toml`"
        )]
        path: PathBuf,
        #[arg(
            long,
            help = "Fail instead of rewriting files when formatting changes would be needed"
        )]
        check: bool,
        #[arg(long, help = "Print formatted output to stdout; only valid for single-file input")]
        stdout: bool,
    },
    #[command(
        about = "Print AST JSON",
        long_about = "Print stable AST JSON for a standalone `.kai` file or a project root. The `--json` flag is retained for compatibility; AST output is always JSON."
    )]
    Ast {
        #[arg(
            value_name = "PATH",
            help = "Path to a `.kai` file, project directory, or `kairos.toml`"
        )]
        path: PathBuf,
        #[arg(long, help = "Retained for compatibility; AST output is always JSON")]
        json: bool,
    },
    #[command(
        about = "Print KIR JSON",
        long_about = "Print stable KIR JSON for a standalone `.kai` file or a project root. The `--json` flag is retained for compatibility; KIR output is always JSON."
    )]
    Ir {
        #[arg(
            value_name = "PATH",
            help = "Path to a `.kai` file, project directory, or `kairos.toml`"
        )]
        path: PathBuf,
        #[arg(long, help = "Retained for compatibility; KIR output is always JSON")]
        json: bool,
    },
    #[command(
        about = "Generate prompt/context markdown",
        long_about = "Render deterministic prompt/context markdown for a standalone `.kai` file or an entire Kairos project."
    )]
    Prompt {
        #[arg(
            value_name = "PATH",
            help = "Path to a `.kai` file, project directory, or `kairos.toml`"
        )]
        path: PathBuf,
    },
    #[command(
        about = "Run Kairos-native project tests",
        long_about = "Discover and run deterministic `test fn` cases in a standalone `.kai` file or a Kairos project. Project mode runs tests from the root package only."
    )]
    Test {
        #[arg(
            value_name = "PATH",
            help = "Path to a `.kai` file, project directory, or `kairos.toml`"
        )]
        path: PathBuf,
        #[arg(long, help = "Filter discovered tests by substring match")]
        filter: Option<String>,
        #[arg(long, help = "Emit stable JSON test output")]
        json: bool,
    },
    #[command(
        about = "Inspect project and environment health",
        long_about = "Report whether the current directory or supplied path resolves to a healthy Kairos target, including manifest, module, dependency, and analysis status."
    )]
    Doctor {
        #[arg(
            value_name = "PATH",
            help = "Optional path to a `.kai` file, project directory, or `kairos.toml`"
        )]
        path: Option<PathBuf>,
        #[arg(long, help = "Emit stable JSON doctor output")]
        json: bool,
    },
    #[command(
        about = "Run deterministic Kairos code",
        long_about = "Execute the supported deterministic interpreter subset for a file or project. Use `--json` for machine-readable output and omit it for a concise human summary."
    )]
    Run {
        #[arg(
            value_name = "PATH",
            help = "Path to a `.kai` file, project directory, or `kairos.toml`"
        )]
        path: PathBuf,
        #[arg(
            long,
            help = "Target a function by name, or `module.path::function_name` inside a project"
        )]
        function: Option<String>,
        #[arg(
            long = "arg",
            help = "Pass a runtime argument. JSON values are accepted, or bare text is treated as a string"
        )]
        args: Vec<String>,
        #[arg(
            long,
            help = "Emit stable JSON execution output instead of a human-readable summary"
        )]
        json: bool,
    },
    #[command(
        about = "Open the Kairos interactive shell",
        long_about = "Launch the Kairos interactive shell. With no path, the shell auto-detects the surrounding project when started inside a Kairos workspace; otherwise it starts in unloaded mode."
    )]
    Shell {
        #[arg(
            value_name = "PATH",
            help = "Optional project directory, `kairos.toml`, or `.kai` file to load immediately"
        )]
        path: Option<PathBuf>,
    },
    #[command(
        about = "Create a new Kairos project directory",
        long_about = "Scaffold a new Kairos project directory with a validated manifest, starter source files, and a minimal README. Generated projects validate immediately."
    )]
    New {
        #[arg(value_name = "NAME", help = "Directory name for the new project")]
        name: String,
        #[arg(long, value_enum, default_value_t = TemplateKind::Default, help = "Starter project template to generate")]
        template: TemplateKind,
    },
    #[command(
        about = "Initialize the current directory as a Kairos project",
        long_about = "Create `kairos.toml`, starter source files, and a README in the current directory without overwriting existing files. Generated projects validate immediately."
    )]
    Init {
        #[arg(long, value_enum, default_value_t = TemplateKind::Default, help = "Starter project template to generate")]
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
        Command::Test { path, filter, json } => command_test(&path, filter.as_deref(), json),
        Command::Doctor { path, json } => command_doctor(path.as_deref(), json),
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
    let runtime_args =
        args.iter().map(|arg| workspace::parse_runtime_value(arg)).collect::<Result<Vec<_>>>()?;
    let workspace =
        LoadedWorkspace::load(path).map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?;
    let report = workspace.run(function, &runtime_args)?;
    if json_output {
        print_json(&serde_json::to_value(&report)?)?;
    } else {
        println!("{}", render_execution_report(&report));
    }
    Ok(())
}

fn command_test(path: &Path, filter: Option<&str>, json_output: bool) -> Result<()> {
    let workspace =
        LoadedWorkspace::load(path).map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?;
    let report = workspace.test_report(filter)?;
    if json_output {
        print_json(&serde_json::to_value(&report)?)?;
    } else {
        println!("{}", render_test_report(&report));
    }

    if report.failed == 0 {
        Ok(())
    } else if json_output {
        process::exit(1);
    } else {
        bail!("{} Kairos test(s) failed", report.failed);
    }
}

fn command_doctor(path: Option<&Path>, json_output: bool) -> Result<()> {
    let workspace = match path {
        Some(path) => Some(
            LoadedWorkspace::load(path)
                .map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?,
        ),
        None => {
            let cwd = std::env::current_dir().context("failed to determine current directory")?;
            LoadedWorkspace::auto_detect(&cwd)
                .map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?
        }
    };

    let report = match workspace {
        Some(workspace) => workspace.doctor_report(),
        None => {
            let cwd = std::env::current_dir().context("failed to determine current directory")?;
            DoctorReport {
                status: DoctorStatus::Warning,
                target: "none",
                root: workspace::canonical_display_path(&cwd),
                package: None,
                entry_module: None,
                module_count: 0,
                package_count: 0,
                dependency_count: 0,
                checks: vec![
                    DoctorCheck {
                        name: "workspace_load".to_string(),
                        status: DoctorStatus::Warning,
                        message: "no Kairos project or `.kai` file was detected from the current directory".to_string(),
                    },
                    DoctorCheck {
                        name: "next_step".to_string(),
                        status: DoctorStatus::Ok,
                        message: "run `kairos init`, `kairos new <name>`, or pass an explicit path".to_string(),
                    },
                ],
                warnings: Vec::new(),
            }
        }
    };

    if json_output {
        print_json(&serde_json::to_value(&report)?)?;
    } else {
        println!("{}", render_doctor_report(&report));
    }
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
    if !report.notes.is_empty() {
        println!("Notes:");
        for note in &report.notes {
            println!("- {note}");
        }
    }
    println!("Next steps:");
    println!("- `Set-Location \"{}\"`", report.root.display());
    println!("- `cargo run --bin kairos -- check .`");
    println!("- `cargo run --bin kairos -- test .`");
    println!("- `cargo run --bin kairos -- doctor .`");
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
