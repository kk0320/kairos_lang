use crate::workspace::ModuleRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSnapshot {
    pub mode: String,
    pub source: String,
    pub root: String,
    pub package: Option<String>,
    pub entry: Option<String>,
    pub modules: Option<usize>,
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
    output.push_str(":modules\n");
    output.push_str(":prompt\n");
    output.push_str(":reload\n");
    output.push_str(":watch\n");
    output.push_str(":clear\n");
    output.push_str(":quit\n");
    output
}

pub fn render_shell_help() -> &'static str {
    "Kairos shell commands:\n\
:help                Show this help text\n\
:status              Show the current shell status\n\
:load <path>         Load a project, manifest, or `.kai` file\n\
:check               Reload and validate the current target\n\
:ast [selector]      Print AST JSON for the current target or selected module\n\
:ir [selector]       Print KIR JSON for the current target or selected module\n\
:prompt [selector]   Print prompt context for the current target or selected module\n\
:run [function] [args...]  Run the current target with optional function and args\n\
:modules             List loaded modules\n\
:reload              Reload the current target from disk\n\
:watch               Start session watch mode\n\
:unwatch             Stop session watch mode\n\
:clear               Clear the terminal and redraw the banner\n\
:quit                Exit the shell"
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
        lines.push(format!("- {}{} -> {}", record.module, marker_text, record.relative_path));
    }
    lines.join("\n")
}

pub fn clear_screen_sequence() -> &'static str {
    "\u{1b}[2J\u{1b}[H"
}

#[cfg(test)]
mod tests {
    use super::{
        render_module_list, render_shell_banner, render_shell_status, ModuleRecord, ShellSnapshot,
    };

    #[test]
    fn banner_includes_project_metadata() {
        let snapshot = ShellSnapshot {
            mode: "project-aware | deterministic".to_string(),
            source: "project".to_string(),
            root: "C:/demo".to_string(),
            package: Some("assistant_briefing".to_string()),
            entry: Some("demo.assistant_briefing".to_string()),
            modules: Some(3),
            focus: Some("demo.assistant_briefing".to_string()),
            watch: "off".to_string(),
        };

        let banner = render_shell_banner("0.5.0-dev", &snapshot);
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
                module: "demo.main".to_string(),
                relative_path: "src/main.kai".to_string(),
                is_entry: true,
                is_focus: true,
            },
            ModuleRecord {
                module: "demo.shared".to_string(),
                relative_path: "src/shared.kai".to_string(),
                is_entry: false,
                is_focus: false,
            },
        ]);

        assert!(rendered.contains("[entry, focus]"));
        assert!(rendered.contains("demo.shared"));
    }
}
