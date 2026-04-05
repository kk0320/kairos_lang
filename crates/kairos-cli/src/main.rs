use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use kairos_interpreter::{run, run_project, RuntimeValue};
use kairos_project::{analyze_project, find_manifest, load_project, AnalyzedProject, Project};
use kairos_semantic::{AnalyzedProgram, Diagnostic, DiagnosticLocation, Severity};
use serde_json::{json, Value};

#[derive(Debug, Parser)]
#[command(name = "kairos", about = "Kairos language CLI")]
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
    }
}

fn command_check(path: &Path, json_output: bool) -> Result<()> {
    match load_analyzed_input(path) {
        Ok(AnalyzedInput::Standalone { path, analyzed }) => {
            if json_output {
                print_json(&json!({
                    "status": "ok",
                    "kind": "module",
                    "path": normalize_display_path(&path),
                    "module": analyzed.program.module,
                    "warnings": diagnostics_to_json(&analyzed.warnings),
                }))?;
            } else {
                println!("OK: module `{}` validated", analyzed.program.module);
                println!("Path: {}", path.display());
                print_warning_summary(&analyzed.warnings);
            }
            Ok(())
        }
        Ok(AnalyzedInput::Project { analyzed, focus_module }) => {
            if json_output {
                print_json(&json!({
                    "status": "ok",
                    "kind": "project",
                    "package": analyzed.project.manifest.package.name,
                    "entry_module": analyzed.project.entry_module,
                    "module_count": analyzed.project.modules.len(),
                    "focus_module": focus_module,
                    "warnings": diagnostics_to_json(&analyzed.warnings),
                }))?;
            } else {
                println!("OK: project `{}` validated", analyzed.project.manifest.package.name);
                println!("Entry module: {}", analyzed.project.entry_module);
                println!("Modules: {}", analyzed.project.modules.len());
                if let Some(module) = focus_module {
                    println!("Focused module: {module}");
                }
                println!("Manifest: {}", analyzed.project.manifest_path.display());
                print_warning_summary(&analyzed.warnings);
            }
            Ok(())
        }
        Err(diagnostics) => {
            if json_output {
                print_json(&json!({
                    "status": "error",
                    "diagnostics": diagnostics_to_json(&diagnostics),
                }))?;
                process::exit(1);
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
        let project = load_project(path).map_err(project_error_to_anyhow)?;
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
    if is_project_root_input(path) {
        let project = load_project(path).map_err(project_error_to_anyhow)?;
        print_json(&serde_json::to_value(&project)?)?;
    } else {
        let program =
            load_program(path).map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?;
        print_json(&serde_json::to_value(&program)?)?;
    }
    Ok(())
}

fn command_ir(path: &Path, json_output: bool) -> Result<()> {
    let _json_output = json_output;
    match load_analyzed_input(path).map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))? {
        AnalyzedInput::Standalone { analyzed, .. } => {
            print_json(&serde_json::to_value(kairos_ir::lower(&analyzed))?)?;
        }
        AnalyzedInput::Project { analyzed, focus_module, .. } => {
            if let Some(module_name) = focus_module {
                let module = analyzed
                    .modules
                    .iter()
                    .find(|module| module.analyzed.program.module == module_name)
                    .expect("focused module should exist in analyzed project");
                print_json(&serde_json::to_value(kairos_ir::lower(&module.analyzed))?)?;
            } else {
                print_json(&serde_json::to_value(kairos_ir::lower_project(&analyzed))?)?;
            }
        }
    }
    Ok(())
}

fn command_prompt(path: &Path) -> Result<()> {
    match load_analyzed_input(path).map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))? {
        AnalyzedInput::Standalone { analyzed, .. } => {
            print!("{}", kairos_ir::render_prompt(&kairos_ir::lower(&analyzed)));
        }
        AnalyzedInput::Project { analyzed, focus_module, .. } => {
            if let Some(module_name) = focus_module {
                let module = analyzed
                    .modules
                    .iter()
                    .find(|module| module.analyzed.program.module == module_name)
                    .expect("focused module should exist in analyzed project");
                print!("{}", kairos_ir::render_prompt(&kairos_ir::lower(&module.analyzed)));
            } else {
                print!(
                    "{}",
                    kairos_ir::render_project_prompt(&kairos_ir::lower_project(&analyzed))
                );
            }
        }
    }
    Ok(())
}

fn command_run(
    path: &Path,
    function: Option<&str>,
    args: &[String],
    json_output: bool,
) -> Result<()> {
    let runtime_args =
        args.iter().map(|arg| parse_runtime_value(arg)).collect::<Result<Vec<_>>>()?;

    let report = match load_analyzed_input(path)
        .map_err(|diagnostics| diagnostics_to_anyhow(&diagnostics))?
    {
        AnalyzedInput::Standalone { analyzed, .. } => {
            run(&kairos_ir::lower(&analyzed), function, &runtime_args)?
        }
        AnalyzedInput::Project { analyzed, focus_module, .. } => {
            let default_module = focus_module.as_deref().unwrap_or(&analyzed.project.entry_module);
            run_project(
                &kairos_ir::lower_project(&analyzed),
                default_module,
                function,
                &runtime_args,
            )?
        }
    };

    let _json_output = json_output;
    print_json(&serde_json::to_value(&report)?)?;
    Ok(())
}

enum AnalyzedInput {
    Standalone { path: PathBuf, analyzed: Box<AnalyzedProgram> },
    Project { analyzed: Box<AnalyzedProject>, focus_module: Option<String> },
}

fn load_analyzed_input(path: &Path) -> std::result::Result<AnalyzedInput, Vec<Diagnostic>> {
    if is_project_root_input(path) {
        let project = load_project(path).map_err(|error| error.diagnostics)?;
        let analyzed = Box::new(analyze_project(&project).map_err(|error| error.diagnostics)?);
        return Ok(AnalyzedInput::Project { analyzed, focus_module: None });
    }

    if is_kai_file(path) {
        if let Some(manifest_path) = find_manifest(path) {
            let project = load_project(&manifest_path).map_err(|error| error.diagnostics)?;
            let focus_module = project
                .modules
                .iter()
                .find(|module| module.path == path)
                .map(|module| module.module.clone());
            let analyzed = Box::new(analyze_project(&project).map_err(|error| error.diagnostics)?);
            return Ok(AnalyzedInput::Project { analyzed, focus_module });
        }
    }

    let program = load_program(path)?;
    let analyzed = Box::new(kairos_semantic::analyze(program).map_err(|error| error.diagnostics)?);
    Ok(AnalyzedInput::Standalone { path: path.to_path_buf(), analyzed })
}

fn load_program(path: &Path) -> std::result::Result<kairos_ast::Program, Vec<Diagnostic>> {
    let source = read_source(path).map_err(|error| {
        vec![Diagnostic {
            severity: Severity::Error,
            code: "source_read_failed",
            message: error.to_string(),
            location: Some(location_for_path(path)),
            related: Vec::new(),
        }]
    })?;

    kairos_parser::parse_source(&source)
        .map_err(|error| vec![parse_error_to_diagnostic(path, error)])
}

fn format_project(project: &Project, check: bool) -> Result<()> {
    let mut changed = Vec::new();

    for module in &project.modules {
        let source = read_source(&module.path)?;
        let formatted = kairos_formatter::format_program(&module.program);
        if source != formatted {
            changed.push(module.relative_path.clone());
            if !check {
                fs::write(&module.path, formatted)
                    .with_context(|| format!("failed to write `{}`", module.path.display()))?;
            }
        }
    }

    if check {
        if changed.is_empty() {
            println!("OK: project `{}` is canonically formatted", project.manifest.package.name);
            return Ok(());
        }

        bail!(
            "project `{}` is not canonically formatted:\n{}",
            project.manifest.package.name,
            changed.iter().map(|path| format!("- {path}")).collect::<Vec<_>>().join("\n")
        );
    }

    if changed.is_empty() {
        println!("No changes: project `{}` is already formatted", project.manifest.package.name);
    } else {
        println!(
            "Formatted project `{}` ({} file(s))",
            project.manifest.package.name,
            changed.len()
        );
        for path in changed {
            println!("- {path}");
        }
    }

    Ok(())
}

fn diagnostics_to_json(diagnostics: &[Diagnostic]) -> Vec<Value> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "code": diagnostic.code,
                "severity": format!("{:?}", diagnostic.severity).to_lowercase(),
                "message": diagnostic.message,
                "location": diagnostic.location,
                "related": diagnostic.related,
            })
        })
        .collect()
}

fn diagnostics_to_anyhow(diagnostics: &[Diagnostic]) -> anyhow::Error {
    anyhow::anyhow!(diagnostics.iter().map(render_diagnostic).collect::<Vec<_>>().join("\n"))
}

fn project_error_to_anyhow(error: kairos_project::ProjectError) -> anyhow::Error {
    diagnostics_to_anyhow(&error.diagnostics)
}

fn render_diagnostic(diagnostic: &Diagnostic) -> String {
    let mut rendered = format!(
        "{}[{}]: {}",
        severity_label(diagnostic.severity.clone()),
        diagnostic.code,
        diagnostic.message
    );

    if let Some(location) = &diagnostic.location {
        rendered.push_str(" (");
        rendered.push_str(&render_location(location));
        rendered.push(')');
    }

    if !diagnostic.related.is_empty() {
        for related in &diagnostic.related {
            rendered.push_str("\n  note: ");
            rendered.push_str(&related.message);
            if let Some(location) = &related.location {
                rendered.push_str(" (");
                rendered.push_str(&render_location(location));
                rendered.push(')');
            }
        }
    }

    rendered
}

fn print_diagnostics(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        eprintln!("{}", render_diagnostic(diagnostic));
    }
}

fn print_warning_summary(warnings: &[Diagnostic]) {
    if warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in warnings {
            println!("- {}", render_diagnostic(warning));
        }
    }
}

fn render_location(location: &DiagnosticLocation) -> String {
    let mut parts = Vec::new();
    if let Some(path) = &location.path {
        parts.push(path.clone());
    }
    if let Some(module) = &location.module {
        parts.push(format!("module {module}"));
    }
    if let Some(symbol) = &location.symbol {
        parts.push(format!("symbol {symbol}"));
    }
    if let (Some(line), Some(column)) = (location.line, location.column) {
        parts.push(format!("line {line}, column {column}"));
    }
    parts.join(", ")
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

fn parse_error_to_diagnostic(path: &Path, error: kairos_parser::ParseError) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: "parse_error",
        message: error.message,
        location: Some(DiagnosticLocation {
            path: Some(normalize_display_path(path)),
            module: None,
            symbol: None,
            line: Some(error.line),
            column: Some(error.column),
        }),
        related: Vec::new(),
    }
}

fn location_for_path(path: &Path) -> DiagnosticLocation {
    DiagnosticLocation {
        path: Some(normalize_display_path(path)),
        module: None,
        symbol: None,
        line: None,
        column: None,
    }
}

fn is_project_root_input(path: &Path) -> bool {
    path.is_dir() || path.file_name().is_some_and(|name| name == "kairos.toml")
}

fn is_kai_file(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "kai")
}

fn normalize_display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn read_source(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))
}

fn parse_runtime_value(input: &str) -> Result<RuntimeValue> {
    if let Ok(json_value) = serde_json::from_str::<Value>(input) {
        return json_to_runtime_value(json_value);
    }

    if let Ok(integer) = input.parse::<i64>() {
        return Ok(RuntimeValue::Integer(integer));
    }

    if let Ok(float) = input.parse::<f64>() {
        return Ok(RuntimeValue::Float(float));
    }

    match input {
        "true" => Ok(RuntimeValue::Boolean(true)),
        "false" => Ok(RuntimeValue::Boolean(false)),
        "null" => Ok(RuntimeValue::Null),
        _ => Ok(RuntimeValue::String(input.to_string())),
    }
}

fn json_to_runtime_value(value: Value) -> Result<RuntimeValue> {
    match value {
        Value::String(value) => Ok(RuntimeValue::String(value)),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                Ok(RuntimeValue::Integer(integer))
            } else if let Some(float) = number.as_f64() {
                Ok(RuntimeValue::Float(float))
            } else {
                bail!("unsupported numeric runtime value")
            }
        }
        Value::Bool(value) => Ok(RuntimeValue::Boolean(value)),
        Value::Array(values) => Ok(RuntimeValue::List(
            values.into_iter().map(json_to_runtime_value).collect::<Result<Vec<_>>>()?,
        )),
        Value::Object(values) => Ok(RuntimeValue::Object(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, json_to_runtime_value(value)?)))
                .collect::<Result<_>>()?,
        )),
        Value::Null => Ok(RuntimeValue::Null),
    }
}

fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
