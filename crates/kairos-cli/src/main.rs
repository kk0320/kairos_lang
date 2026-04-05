use std::{fs, path::Path, path::PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use kairos_interpreter::{run, RuntimeValue};
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
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Fmt {
        file: PathBuf,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        stdout: bool,
    },
    Ast {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Ir {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Prompt {
        file: PathBuf,
    },
    Run {
        file: PathBuf,
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
        Command::Check { file, json } => command_check(&file, json),
        Command::Fmt { file, check, stdout } => command_fmt(&file, check, stdout),
        Command::Ast { file, json } => command_ast(&file, json),
        Command::Ir { file, json } => command_ir(&file, json),
        Command::Prompt { file } => command_prompt(&file),
        Command::Run { file, function, args, json } => {
            command_run(&file, function.as_deref(), &args, json)
        }
    }
}

fn command_check(file: &Path, json_output: bool) -> Result<()> {
    let analyzed = load_analyzed(file)?;
    if json_output {
        let warnings = analyzed
            .warnings
            .iter()
            .map(|warning| {
                json!({
                    "code": warning.code,
                    "severity": format!("{:?}", warning.severity).to_lowercase(),
                    "message": warning.message,
                })
            })
            .collect::<Vec<_>>();
        print_json(&json!({
            "status": "ok",
            "module": analyzed.program.module,
            "warnings": warnings,
        }))?;
    } else {
        println!("OK: module `{}` validated", analyzed.program.module);
        if analyzed.warnings.is_empty() {
            println!("Warnings: none");
        } else {
            println!("Warnings:");
            for warning in &analyzed.warnings {
                println!("- [{}] {}", warning.code, warning.message);
            }
        }
    }
    Ok(())
}

fn command_fmt(file: &Path, check: bool, stdout: bool) -> Result<()> {
    if check && stdout {
        bail!("`--check` and `--stdout` cannot be used together");
    }

    let source = read_source(file)?;
    let program = kairos_parser::parse_source(&source)?;
    let formatted = kairos_formatter::format_program(&program);

    if stdout {
        print!("{formatted}");
        return Ok(());
    }

    if check {
        if source == formatted {
            println!("OK: `{}` is canonically formatted", file.display());
            return Ok(());
        }
        bail!(
            "`{}` is not canonically formatted. Run `kairos fmt {}`.",
            file.display(),
            file.display()
        );
    }

    if source == formatted {
        println!("No changes: `{}` is already formatted", file.display());
    } else {
        fs::write(file, formatted)
            .with_context(|| format!("failed to write `{}`", file.display()))?;
        println!("Formatted `{}`", file.display());
    }

    Ok(())
}

fn command_ast(file: &Path, json_output: bool) -> Result<()> {
    let program = load_program(file)?;
    let _json_output = json_output;
    print_json(&serde_json::to_value(&program)?)?;
    Ok(())
}

fn command_ir(file: &Path, json_output: bool) -> Result<()> {
    let ir = load_ir(file)?;
    let _json_output = json_output;
    print_json(&serde_json::to_value(&ir)?)?;
    Ok(())
}

fn command_prompt(file: &Path) -> Result<()> {
    let ir = load_ir(file)?;
    print!("{}", kairos_ir::render_prompt(&ir));
    Ok(())
}

fn command_run(
    file: &Path,
    function: Option<&str>,
    args: &[String],
    json_output: bool,
) -> Result<()> {
    let ir = load_ir(file)?;
    let runtime_args =
        args.iter().map(|arg| parse_runtime_value(arg)).collect::<Result<Vec<_>>>()?;
    let report = run(&ir, function, &runtime_args)?;
    let _json_output = json_output;
    print_json(&serde_json::to_value(&report)?)?;
    Ok(())
}

fn load_program(file: &Path) -> Result<kairos_ast::Program> {
    let source = read_source(file)?;
    let program = kairos_parser::parse_source(&source)
        .with_context(|| format!("failed to parse `{}`", file.display()))?;
    Ok(program)
}

fn load_analyzed(file: &Path) -> Result<kairos_semantic::AnalyzedProgram> {
    let program = load_program(file)?;
    let analyzed = kairos_semantic::analyze(program)
        .with_context(|| format!("semantic validation failed for `{}`", file.display()))?;
    Ok(analyzed)
}

fn load_ir(file: &Path) -> Result<kairos_ir::KirProgram> {
    let analyzed = load_analyzed(file)?;
    Ok(kairos_ir::lower(&analyzed))
}

fn read_source(file: &Path) -> Result<String> {
    fs::read_to_string(file).with_context(|| format!("failed to read `{}`", file.display()))
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
