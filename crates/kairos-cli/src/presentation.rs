use std::fmt::Write;

use kairos_interpreter::{ExecutionReport, RuntimeValue};

use crate::workspace::{
    DependencyRecord, DoctorCheck, DoctorReport, DoctorStatus, ModuleRecord, TestOutcome,
    TestReport,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSnapshot {
    pub mode: String,
    pub source: String,
    pub root: String,
    pub package: Option<String>,
    pub entry: Option<String>,
    pub modules: Option<usize>,
    pub packages: Option<usize>,
    pub dependencies: Option<usize>,
    pub focus: Option<String>,
    pub watch: String,
}

pub fn render_shell_banner(version: &str, snapshot: &ShellSnapshot) -> String {
    let mut output = String::new();
    output.push_str(" _  __     _                 \n");
    output.push_str("| |/ /__ _(_)_ __ ___  ___   \n");
    output.push_str("| ' // _` | | '__/ _ \\/ __|  \n");
    output.push_str("| . \\ (_| | | | | (_) \\__ \\  \n");
    output.push_str("|_|\\_\\__,_|_|_|  \\___/|___/  \n");
    output.push('\n');
    output.push_str("AI-first programming language shell\n");
    output.push('\n');
    output.push_str(&format!("version: v{version}\n"));
    output.push_str(&format!("mode: {}\n", snapshot.mode));
    output.push_str(&format!("source: {}\n", snapshot.source));
    if let Some(package) = &snapshot.package {
        output.push_str(&format!("project: {package}\n"));
    }
    if let Some(entry) = &snapshot.entry {
        output.push_str(&format!("entry: {entry}\n"));
    }
    if let Some(modules) = snapshot.modules {
        output.push_str(&format!("modules: {modules}\n"));
    }
    if let Some(packages) = snapshot.packages {
        output.push_str(&format!("packages: {packages}\n"));
    }
    if let Some(dependencies) = snapshot.dependencies {
        output.push_str(&format!("dependencies: {dependencies}\n"));
    }
    if let Some(focus) = &snapshot.focus {
        output.push_str(&format!("focus: {focus}\n"));
    }
    output.push_str(&format!("root: {}\n", snapshot.root));
    output.push_str(&format!("watch: {}\n", snapshot.watch));
    output.push('\n');
    output.push_str("Tips:\n");
    output.push_str(":help\n");
    output.push_str(":status\n");
    output.push_str(":check\n");
    output.push_str(":run main\n");
    output.push_str(":ir\n");
    output.push_str(":modules\n");
    output.push_str(":deps\n");
    output.push_str(":prompt\n");
    output.push_str(":reload\n");
    output.push_str(":watch\n");
    output.push_str(":clear\n");
    output.push_str(":quit\n");
    output
}

pub fn render_shell_help() -> &'static str {
    "Kairos shell commands:\n\
:help                      Show this help text\n\
:status                    Show the current shell status\n\
:load <path>               Load a project, manifest, or `.kai` file\n\
:check                     Reload and validate the current target\n\
:ast [selector]            Print AST JSON for the current target or selected module\n\
:ir [selector]             Print KIR JSON for the current target or selected module\n\
:prompt [selector]         Print prompt context for the current target or selected module\n\
:run [function] [args...]  Run the current target with optional function and args\n\
:modules                   List loaded modules\n\
:deps                      List direct local dependencies for the loaded project\n\
:reload                    Reload the current target from disk\n\
:watch                     Start session watch mode\n\
:unwatch                   Stop session watch mode\n\
:clear                     Clear the terminal and redraw the banner\n\
:quit                      Exit the shell\n\
\n\
Examples:\n\
:load examples/assistant_briefing\n\
:run classify 72\n\
:run demo.decision_bundle.labels::label_for 72\n\
\n\
Argument parsing:\n\
- shell `:run` arguments follow the same rules as `kairos run`\n\
- JSON values are accepted directly, for example `72`, `true`, `[1,2]`, or `{\"ok\":true}`"
}

pub fn render_shell_status(snapshot: &ShellSnapshot) -> String {
    let mut output = String::new();
    output.push_str("Kairos shell status\n");
    output.push_str(&format!("- mode: {}\n", snapshot.mode));
    output.push_str(&format!("- source: {}\n", snapshot.source));
    if let Some(package) = &snapshot.package {
        output.push_str(&format!("- project: {package}\n"));
    }
    if let Some(entry) = &snapshot.entry {
        output.push_str(&format!("- entry: {entry}\n"));
    }
    if let Some(modules) = snapshot.modules {
        output.push_str(&format!("- modules: {modules}\n"));
    }
    if let Some(packages) = snapshot.packages {
        output.push_str(&format!("- packages: {packages}\n"));
    }
    if let Some(dependencies) = snapshot.dependencies {
        output.push_str(&format!("- dependencies: {dependencies}\n"));
    }
    if let Some(focus) = &snapshot.focus {
        output.push_str(&format!("- focus: {focus}\n"));
    }
    output.push_str(&format!("- root: {}\n", snapshot.root));
    output.push_str(&format!("- watch: {}", snapshot.watch));
    output
}

pub fn render_module_list(records: &[ModuleRecord]) -> String {
    if records.is_empty() {
        return "No modules are currently loaded.".to_string();
    }

    let mut lines = vec!["Loaded modules".to_string()];
    for record in records {
        let mut markers = Vec::new();
        if record.is_entry {
            markers.push("entry");
        }
        if record.is_focus {
            markers.push("focus");
        }
        let marker_text =
            if markers.is_empty() { String::new() } else { format!(" [{}]", markers.join(", ")) };
        if let Some(package) = &record.package {
            lines.push(format!(
                "- {}{} -> {} (package: {})",
                record.module, marker_text, record.relative_path, package
            ));
        } else {
            lines.push(format!("- {}{} -> {}", record.module, marker_text, record.relative_path));
        }
    }
    lines.join("\n")
}

pub fn render_dependency_list(records: &[DependencyRecord]) -> String {
    if records.is_empty() {
        return "No direct local dependencies are loaded.".to_string();
    }

    let mut lines = vec!["Direct dependencies".to_string()];
    for record in records {
        lines.push(format!("- {} -> {} ({})", record.alias, record.package, record.path));
    }
    lines.join("\n")
}

pub fn clear_screen_sequence() -> &'static str {
    "\u{1b}[2J\u{1b}[H"
}

pub fn render_execution_report(report: &ExecutionReport) -> String {
    let mut output = String::new();
    output.push_str("Kairos execution report\n");
    output.push_str(&format!("- module: {}\n", report.module));
    if report.results.is_empty() {
        output.push_str("- results: none");
        return output;
    }

    for result in &report.results {
        output.push_str("- ");
        output.push_str(&result.function);
        output.push_str(" => ");
        output.push_str(&render_runtime_value(&result.value));
        output.push('\n');
    }

    output.pop();
    output
}

pub fn render_test_report(report: &TestReport) -> String {
    let mut output = String::new();
    output.push_str("Kairos test report\n");
    output.push_str(&format!("- target: {}\n", report.target));
    if let Some(package) = &report.package {
        output.push_str(&format!("- package: {package}\n"));
    }
    output.push_str(&format!(
        "- total: {} | passed: {} | failed: {}\n",
        report.total, report.passed, report.failed
    ));
    if report.results.is_empty() {
        output.push_str("- results: none");
        return output;
    }

    for result in &report.results {
        let outcome = match result.outcome {
            TestOutcome::Passed => "PASS",
            TestOutcome::Failed => "FAIL",
        };
        output.push_str(&format!("- [{outcome}] {} ({})", result.display_name, result.message));
        if let Some(value) = &result.value {
            output.push_str(&format!(" => {}", render_runtime_value(value)));
        }
        output.push('\n');
    }
    output.pop();
    output
}

pub fn render_doctor_report(report: &DoctorReport) -> String {
    let mut output = String::new();
    output.push_str("Kairos doctor report\n");
    output.push_str(&format!("- status: {}\n", doctor_status_label(&report.status)));
    output.push_str(&format!("- target: {}\n", report.target));
    output.push_str(&format!("- root: {}\n", report.root));
    if let Some(package) = &report.package {
        output.push_str(&format!("- package: {package}\n"));
    }
    if let Some(entry_module) = &report.entry_module {
        output.push_str(&format!("- entry_module: {entry_module}\n"));
    }
    output.push_str(&format!(
        "- package_count: {} | module_count: {} | dependency_count: {}\n",
        report.package_count, report.module_count, report.dependency_count
    ));
    output.push_str("Checks:\n");
    for check in &report.checks {
        output.push_str(&format!(
            "- [{}] {}\n",
            doctor_status_label(&check.status),
            render_check(check)
        ));
    }
    if !report.warnings.is_empty() {
        output.push_str("Warnings:\n");
        for warning in &report.warnings {
            output.push_str(&format!("- {}\n", warning["message"].as_str().unwrap_or("warning")));
        }
    }
    output.trim_end().to_string()
}

fn render_check(check: &DoctorCheck) -> String {
    format!("{}: {}", check.name, check.message)
}

fn doctor_status_label(status: &DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Ok => "ok",
        DoctorStatus::Warning => "warning",
    }
}

fn render_runtime_value(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::String(value) => format!("{value:?}"),
        RuntimeValue::Integer(value) => value.to_string(),
        RuntimeValue::Float(value) => value.to_string(),
        RuntimeValue::Boolean(value) => value.to_string(),
        RuntimeValue::List(values) => {
            let rendered = values.iter().map(render_runtime_value).collect::<Vec<_>>().join(", ");
            format!("[{rendered}]")
        }
        RuntimeValue::Object(values) => {
            let mut rendered = String::from("{");
            for (index, (key, value)) in values.iter().enumerate() {
                if index > 0 {
                    rendered.push_str(", ");
                }
                let _ = write!(rendered, "{key:?}: {}", render_runtime_value(value));
            }
            rendered.push('}');
            rendered
        }
        RuntimeValue::Null => "null".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        render_execution_report, render_module_list, render_shell_banner, render_shell_status,
        ModuleRecord, ShellSnapshot,
    };
    use kairos_interpreter::{ExecutionReport, ExecutionResult, RuntimeValue};

    #[test]
    fn banner_includes_project_metadata() {
        let snapshot = ShellSnapshot {
            mode: "project-aware | deterministic".to_string(),
            source: "project".to_string(),
            root: "C:/demo".to_string(),
            package: Some("assistant_briefing".to_string()),
            entry: Some("demo.assistant_briefing".to_string()),
            modules: Some(3),
            packages: Some(1),
            dependencies: Some(0),
            focus: Some("demo.assistant_briefing".to_string()),
            watch: "off".to_string(),
        };

        let banner = render_shell_banner("2.0.0", &snapshot);
        assert!(banner.contains("KAIROS") || banner.contains("_  __"));
        assert!(banner.contains("assistant_briefing"));
        assert!(banner.contains("demo.assistant_briefing"));
    }

    #[test]
    fn status_renders_unloaded_mode_cleanly() {
        let snapshot = ShellSnapshot {
            mode: "unloaded | deterministic".to_string(),
            source: "none".to_string(),
            root: "C:/workspace".to_string(),
            package: None,
            entry: None,
            modules: None,
            packages: None,
            dependencies: None,
            focus: None,
            watch: "off".to_string(),
        };

        let status = render_shell_status(&snapshot);
        assert!(status.contains("unloaded | deterministic"));
        assert!(status.contains("C:/workspace"));
    }

    #[test]
    fn module_list_marks_entry_and_focus() {
        let rendered = render_module_list(&[
            ModuleRecord {
                package: Some("demo".to_string()),
                module: "demo.main".to_string(),
                relative_path: "src/main.kai".to_string(),
                is_entry: true,
                is_focus: true,
            },
            ModuleRecord {
                package: Some("demo".to_string()),
                module: "demo.shared".to_string(),
                relative_path: "src/shared.kai".to_string(),
                is_entry: false,
                is_focus: false,
            },
        ]);

        assert!(rendered.contains("[entry, focus]"));
        assert!(rendered.contains("demo.shared"));
    }

    #[test]
    fn execution_report_renders_human_summary() {
        let rendered = render_execution_report(&ExecutionReport {
            module: "demo.rules".to_string(),
            results: vec![ExecutionResult {
                function: "classify".to_string(),
                value: RuntimeValue::String("MEDIUM".to_string()),
            }],
        });

        assert!(rendered.contains("Kairos execution report"));
        assert!(rendered.contains("demo.rules"));
        assert!(rendered.contains("classify => \"MEDIUM\""));
    }
}
