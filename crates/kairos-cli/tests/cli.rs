use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../").join(path)
}

#[test]
fn check_reports_valid_module() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("check")
        .arg(fixture("examples/hello_context/src/main.kai"))
        .assert()
        .success()
        .stdout(contains("OK: module `demo.hello_context` validated"));
}

#[test]
fn ast_outputs_json_shape() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("ast")
        .arg(fixture("examples/hello_context/src/main.kai"))
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"module\": \"demo.hello_context\""));
}

#[test]
fn ir_outputs_machine_readable_json() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("ir")
        .arg(fixture("examples/video_context/src/main.kai"))
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"source_hash\""))
        .stdout(contains("\"functions\""))
        .stdout(contains("\"enums\""));
}

#[test]
fn prompt_outputs_deterministic_sections() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("prompt")
        .arg(fixture("examples/video_context/src/main.kai"))
        .assert()
        .success()
        .stdout(contains("# Kairos System Context"))
        .stdout(contains("## Functions"))
        .stdout(contains("## Notes for Downstream LLMs"));
}

#[test]
fn fmt_can_check_and_write_files() {
    let tempdir = tempdir().expect("tempdir should exist");
    let temp_file = tempdir.path().join("main.kai");
    fs::write(
        &temp_file,
        r#"module demo.fmt;
fn hello()->Str
describe "demo"
tags["a"]
requires[]
ensures[len(result)>0]
{return "ok";}
"#,
    )
    .expect("fixture should write");

    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("fmt")
        .arg(&temp_file)
        .arg("--check")
        .assert()
        .failure()
        .stderr(contains("not canonically formatted"));

    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("fmt")
        .arg(&temp_file)
        .assert()
        .success()
        .stdout(contains("Formatted"));

    let rewritten = fs::read_to_string(&temp_file).expect("formatted file should read");
    assert!(rewritten.contains("fn hello() -> Str"));
    assert!(rewritten.contains("tags [\"a\"]"));
}

#[test]
fn run_supports_argument_driven_execution() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("run")
        .arg(fixture("examples/risk_rules/src/main.kai"))
        .arg("--function")
        .arg("classify")
        .arg("--arg")
        .arg("72")
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"function\": \"classify\""))
        .stdout(contains("\"MEDIUM\""));
}

#[test]
fn run_requires_function_when_no_zero_arg_entry_exists() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("run")
        .arg(fixture("examples/risk_rules/src/main.kai"))
        .assert()
        .failure()
        .stderr(contains("no zero-argument functions are available"));
}
