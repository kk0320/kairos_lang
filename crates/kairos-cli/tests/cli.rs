use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../").join(path)
}

#[test]
fn check_reports_project_when_file_belongs_to_manifest() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("check")
        .arg(fixture("examples/hello_context/src/main.kai"))
        .assert()
        .success()
        .stdout(contains("OK: project `hello_context` validated"))
        .stdout(contains("Focused module: demo.hello_context"));
}

#[test]
fn help_describes_kairos_as_terminal_native_toolchain() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("terminal-native toolchain"))
        .stdout(contains("kairos shell examples\\assistant_briefing"))
        .stdout(contains("Create a new Kairos project directory"));
}

#[test]
fn check_accepts_project_root_json() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("check")
        .arg(fixture("examples/assistant_briefing"))
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"kind\": \"project\""))
        .stdout(contains("\"package\": \"assistant_briefing\""))
        .stdout(contains("\"module_count\": 3"));
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
fn ast_outputs_project_json_shape_for_project_root() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("ast")
        .arg(fixture("examples/assistant_briefing"))
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"entry_module\": \"demo.assistant_briefing\""))
        .stdout(contains("\"relative_path\": \"src/briefing.kai\""))
        .stdout(contains("\"module\": \"demo.assistant_briefing.policies\""));
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
fn ir_outputs_project_machine_readable_json() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("ir")
        .arg(fixture("examples/decision_bundle"))
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"package\""))
        .stdout(contains("\"entry_module\": \"demo.decision_bundle\""))
        .stdout(contains("\"module\": \"demo.decision_bundle.labels\""));
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
fn prompt_outputs_project_sections_for_project_root() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("prompt")
        .arg(fixture("examples/assistant_briefing"))
        .assert()
        .success()
        .stdout(contains("# Kairos Project Context"))
        .stdout(contains("## Package"))
        .stdout(contains("### demo.assistant_briefing.briefing"));
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
fn fmt_can_check_project_root() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("fmt")
        .arg(fixture("examples/assistant_briefing"))
        .arg("--check")
        .assert()
        .success()
        .stdout(contains("canonically formatted"));
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
fn run_supports_project_root_execution() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("run")
        .arg(fixture("examples/decision_bundle"))
        .arg("--function")
        .arg("classify")
        .arg("--arg")
        .arg("72")
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"module\": \"demo.decision_bundle\""))
        .stdout(contains("\"MEDIUM\""));
}

#[test]
fn run_human_mode_renders_summary() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("run")
        .arg(fixture("examples/decision_bundle"))
        .arg("--function")
        .arg("classify")
        .arg("--arg")
        .arg("72")
        .assert()
        .success()
        .stdout(contains("Kairos execution report"))
        .stdout(contains("- module: demo.decision_bundle"))
        .stdout(contains("classify => \"MEDIUM\""));
}

#[test]
fn run_supports_stdlib_project_execution() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("run")
        .arg(fixture("examples/stdlib_playbook"))
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"module\": \"demo.stdlib_playbook\""))
        .stdout(contains("Title: KAIROS PLATFORM | keys=owner,score,title"));
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

#[test]
fn check_reports_structured_import_failure() {
    let tempdir = tempdir().expect("tempdir should exist");
    fs::create_dir_all(tempdir.path().join("src")).expect("source tree should create");
    fs::write(
        tempdir.path().join("kairos.toml"),
        "[package]\nname = \"broken\"\nversion = \"1.0.0\"\nentry = \"src/main.kai\"\n",
    )
    .expect("manifest should write");
    fs::write(
        tempdir.path().join("src/main.kai"),
        "module demo.broken;\nuse demo.missing;\n\nfn main() -> Str\ndescribe \"broken\"\ntags [\"test\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"nope\";\n}\n",
    )
    .expect("source should write");

    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("check")
        .arg(tempdir.path())
        .arg("--json")
        .assert()
        .failure()
        .stdout(contains("\"status\": \"error\""))
        .stdout(contains("\"code\": \"unresolved_import\""));
}

#[test]
fn shell_launches_with_project_path_and_status() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("shell")
        .arg(fixture("examples/assistant_briefing"))
        .write_stdin(":status\n:quit\n")
        .assert()
        .success()
        .stdout(contains("AI-first programming language shell"))
        .stdout(contains("project: assistant_briefing"))
        .stdout(contains("Kairos shell status"));
}

#[test]
fn shell_auto_detects_project_from_current_directory() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .current_dir(fixture("examples/assistant_briefing"))
        .arg("shell")
        .write_stdin(":status\n:quit\n")
        .assert()
        .success()
        .stdout(contains("source: project"))
        .stdout(contains("project: assistant_briefing"));
}

#[test]
fn shell_supports_modules_and_run_commands() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("shell")
        .arg(fixture("examples/decision_bundle"))
        .write_stdin(":modules\n:run classify 72\n:quit\n")
        .assert()
        .success()
        .stdout(contains("Loaded modules"))
        .stdout(contains("demo.decision_bundle.labels"))
        .stdout(contains("Kairos execution report"))
        .stdout(contains("classify => \"MEDIUM\""));
}

#[test]
fn shell_supports_dependency_listing() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("shell")
        .arg(fixture("examples/package_reuse_demo"))
        .write_stdin(":deps\n:quit\n")
        .assert()
        .success()
        .stdout(contains("Direct dependencies"))
        .stdout(contains("shared_rules -> shared_rules_lib"));
}

#[test]
fn shell_supports_reload_and_watch_toggle() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("shell")
        .arg(fixture("examples/assistant_briefing"))
        .write_stdin(":reload\n:watch\n:status\n:unwatch\n:quit\n")
        .assert()
        .success()
        .stdout(contains("OK: reloaded project `assistant_briefing`"))
        .stdout(contains("Watch mode enabled."))
        .stdout(contains("watch: active"))
        .stdout(contains("Watch mode disabled."));
}

#[test]
fn shell_can_load_project_from_unloaded_mode() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .current_dir(fixture("."))
        .arg("shell")
        .write_stdin(":load examples/assistant_briefing\n:status\n:quit\n")
        .assert()
        .success()
        .stdout(contains("source: none"))
        .stdout(contains("OK: loaded project `assistant_briefing`"))
        .stdout(contains("project: assistant_briefing"));
}

#[test]
fn new_scaffolds_project_that_validates() {
    let tempdir = tempdir().expect("tempdir should exist");
    let project_root = tempdir.path().join("temp_demo_project");

    Command::cargo_bin("kairos")
        .expect("binary should build")
        .current_dir(tempdir.path())
        .arg("new")
        .arg("temp_demo_project")
        .assert()
        .success()
        .stdout(contains("Created Kairos project"))
        .stdout(contains("temp_demo_project"));

    assert!(project_root.join("kairos.toml").is_file());
    assert!(project_root.join("src/main.kai").is_file());

    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("check")
        .arg(&project_root)
        .assert()
        .success()
        .stdout(contains("validated"));
}

#[test]
fn init_scaffolds_current_directory_that_validates() {
    let tempdir = tempdir().expect("tempdir should exist");

    Command::cargo_bin("kairos")
        .expect("binary should build")
        .current_dir(tempdir.path())
        .arg("init")
        .arg("--template")
        .arg("briefing")
        .assert()
        .success()
        .stdout(contains("Initialized Kairos project"));

    assert!(tempdir.path().join("kairos.toml").is_file());
    assert!(tempdir.path().join("src/main.kai").is_file());
    assert!(tempdir.path().join("src/briefing.kai").is_file());

    Command::cargo_bin("kairos")
        .expect("binary should build")
        .current_dir(tempdir.path())
        .arg("check")
        .arg(".")
        .assert()
        .success()
        .stdout(contains("validated"));
}

#[test]
fn check_reports_local_dependency_project_health() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("check")
        .arg(fixture("examples/package_reuse_demo"))
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"package\": \"package_reuse_demo\""))
        .stdout(contains("\"dependency_count\": 1"))
        .stdout(contains("\"package_count\": 2"));
}

#[test]
fn prompt_includes_local_dependency_modules() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("prompt")
        .arg(fixture("examples/package_reuse_demo"))
        .assert()
        .success()
        .stdout(contains("# Kairos Project Context"))
        .stdout(contains("shared.rules_lib.api"));
}

#[test]
fn test_command_runs_project_tests() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("test")
        .arg(fixture("examples/decision_bundle"))
        .assert()
        .success()
        .stdout(contains("Kairos test report"))
        .stdout(contains("classify_medium_case"))
        .stdout(contains("[PASS]"));
}

#[test]
fn test_command_supports_json_output() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("test")
        .arg(fixture("examples/package_reuse_demo"))
        .arg("--json")
        .assert()
        .success()
        .stdout(contains("\"status\": \"ok\""))
        .stdout(contains("\"display_name\": \"demo.package_reuse_demo::dependency_smoke\""));
}

#[test]
fn doctor_reports_project_health() {
    Command::cargo_bin("kairos")
        .expect("binary should build")
        .arg("doctor")
        .arg(fixture("examples/package_reuse_demo"))
        .assert()
        .success()
        .stdout(contains("Kairos doctor report"))
        .stdout(contains("dependency_count: 1"))
        .stdout(contains("dependencies: 1 direct local dependencies resolved"));
}

#[test]
fn doctor_reports_warning_when_no_project_is_detected() {
    let tempdir = tempdir().expect("tempdir should exist");

    Command::cargo_bin("kairos")
        .expect("binary should build")
        .current_dir(tempdir.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("status: warning"))
        .stdout(contains("no Kairos project or `.kai` file was detected"));
}
