use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

use anyhow::{bail, Context, Result};
use kairos_interpreter::{run, run_project, ExecutionReport, RuntimeValue};
use kairos_project::{analyze_project, find_manifest, load_project, AnalyzedProject, Project};
use kairos_semantic::{AnalyzedProgram, Diagnostic, DiagnosticLocation, Severity};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub enum LoadedWorkspace {
    Standalone { source_hint: PathBuf, analyzed: Box<AnalyzedProgram> },
    Project { source_hint: PathBuf, analyzed: Box<AnalyzedProject>, focus_module: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRecord {
    pub package: Option<String>,
    pub module: String,
    pub relative_path: String,
    pub is_entry: bool,
    pub is_focus: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DependencyRecord {
    pub alias: String,
    pub package: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestCaseRecord {
    pub package: Option<String>,
    pub module: String,
    pub function: String,
    pub display_name: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestOutcome {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TestResultRecord {
    pub outcome: TestOutcome,
    pub module: String,
    pub function: String,
    pub display_name: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<RuntimeValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TestReport {
    pub status: &'static str,
    pub target: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<TestResultRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Ok,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub status: DoctorStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoctorReport {
    pub status: DoctorStatus,
    pub target: &'static str,
    pub root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_module: Option<String>,
    pub module_count: usize,
    pub package_count: usize,
    pub dependency_count: usize,
    pub checks: Vec<DoctorCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Value>,
}

pub struct ProjectModuleSelection<'a> {
    pub analyzed: &'a AnalyzedProgram,
}

impl LoadedWorkspace {
    pub fn load(path: &Path) -> std::result::Result<Self, Vec<Diagnostic>> {
        if is_project_root_input(path) {
            let project = load_project(path).map_err(|error| error.diagnostics)?;
            let analyzed = Box::new(analyze_project(&project).map_err(|error| error.diagnostics)?);
            return Ok(Self::Project {
                source_hint: path.to_path_buf(),
                analyzed,
                focus_module: None,
            });
        }

        if is_kai_file(path) {
            if let Some(manifest_path) = find_manifest(path) {
                let project = load_project(&manifest_path).map_err(|error| error.diagnostics)?;
                let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                let focus_module = project
                    .modules
                    .iter()
                    .find(|module| {
                        module.path.canonicalize().unwrap_or_else(|_| module.path.clone())
                            == canonical_path
                    })
                    .map(|module| module.module.clone());
                let analyzed =
                    Box::new(analyze_project(&project).map_err(|error| error.diagnostics)?);
                return Ok(Self::Project {
                    source_hint: path.to_path_buf(),
                    analyzed,
                    focus_module,
                });
            }
        }

        let program = load_program(path)?;
        let analyzed =
            Box::new(kairos_semantic::analyze(program).map_err(|error| error.diagnostics)?);
        Ok(Self::Standalone { source_hint: path.to_path_buf(), analyzed })
    }

    pub fn auto_detect(start: &Path) -> std::result::Result<Option<Self>, Vec<Diagnostic>> {
        if let Some(manifest_path) = find_manifest(start) {
            Self::load(&manifest_path).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn reload(&self) -> std::result::Result<Self, Vec<Diagnostic>> {
        Self::load(self.source_hint())
    }

    pub fn source_hint(&self) -> &Path {
        match self {
            Self::Standalone { source_hint, .. } | Self::Project { source_hint, .. } => source_hint,
        }
    }

    pub fn root_path(&self) -> &Path {
        match self {
            Self::Standalone { source_hint, .. } => source_hint,
            Self::Project { analyzed, .. } => &analyzed.project.root,
        }
    }

    pub fn display_root(&self) -> String {
        canonical_display_path(self.root_path())
    }

    pub fn package_name(&self) -> Option<&str> {
        match self {
            Self::Standalone { .. } => None,
            Self::Project { analyzed, .. } => Some(&analyzed.project.manifest.package.name),
        }
    }

    pub fn entry_module(&self) -> Option<&str> {
        match self {
            Self::Standalone { analyzed, .. } => Some(&analyzed.program.module),
            Self::Project { analyzed, .. } => Some(&analyzed.project.entry_module),
        }
    }

    pub fn focus_module(&self) -> Option<&str> {
        match self {
            Self::Standalone { analyzed, .. } => Some(&analyzed.program.module),
            Self::Project { focus_module, .. } => focus_module.as_deref(),
        }
    }

    pub fn module_count(&self) -> usize {
        match self {
            Self::Standalone { .. } => 1,
            Self::Project { analyzed, .. } => analyzed.project.modules.len(),
        }
    }

    pub fn package_count(&self) -> usize {
        match self {
            Self::Standalone { .. } => 1,
            Self::Project { analyzed, .. } => analyzed.project.packages.len(),
        }
    }

    pub fn dependency_count(&self) -> usize {
        match self {
            Self::Standalone { .. } => 0,
            Self::Project { analyzed, .. } => analyzed
                .project
                .package(&analyzed.project.manifest.package.name)
                .map(|package| package.dependencies.len())
                .unwrap_or(0),
        }
    }

    pub fn mode_label(&self) -> &'static str {
        match self {
            Self::Standalone { .. } => "file-aware | deterministic",
            Self::Project { .. } => "project-aware | deterministic",
        }
    }

    pub fn target_label(&self) -> &'static str {
        match self {
            Self::Standalone { .. } => "file",
            Self::Project { .. } => "project",
        }
    }

    pub fn warnings(&self) -> &[Diagnostic] {
        match self {
            Self::Standalone { analyzed, .. } => &analyzed.warnings,
            Self::Project { analyzed, .. } => &analyzed.warnings,
        }
    }

    pub fn module_records(&self) -> Vec<ModuleRecord> {
        match self {
            Self::Standalone { source_hint, analyzed } => vec![ModuleRecord {
                package: None,
                module: analyzed.program.module.clone(),
                relative_path: normalize_display_path(source_hint),
                is_entry: true,
                is_focus: true,
            }],
            Self::Project { analyzed, focus_module, .. } => analyzed
                .project
                .modules
                .iter()
                .map(|module| ModuleRecord {
                    package: Some(module.package.clone()),
                    module: module.module.clone(),
                    relative_path: module.relative_path.clone(),
                    is_entry: module.module == analyzed.project.entry_module,
                    is_focus: focus_module.as_deref().is_some_and(|focus| module.module == focus),
                })
                .collect(),
        }
    }

    pub fn dependency_records(&self) -> Vec<DependencyRecord> {
        match self {
            Self::Standalone { .. } => Vec::new(),
            Self::Project { analyzed, .. } => analyzed
                .project
                .package(&analyzed.project.manifest.package.name)
                .map(|package| {
                    package
                        .dependencies
                        .iter()
                        .map(|dependency| DependencyRecord {
                            alias: dependency.alias.clone(),
                            package: dependency.package.clone(),
                            path: dependency.path.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub fn ast_value(&self, selector: Option<&str>) -> Result<Value> {
        match self {
            Self::Standalone { analyzed, .. } => {
                if let Some(selector) = selector {
                    let normalized = normalize_selector(selector);
                    if normalized != analyzed.program.module
                        && normalized != normalize_display_path(self.source_hint())
                    {
                        bail!("unknown module selector `{selector}`");
                    }
                }
                Ok(serde_json::to_value(&analyzed.program)?)
            }
            Self::Project { analyzed, focus_module, .. } => {
                if let Some(selection) =
                    select_project_module(analyzed, focus_module.as_deref(), selector)?
                {
                    Ok(serde_json::to_value(&selection.analyzed.program)?)
                } else {
                    Ok(serde_json::to_value(&analyzed.project)?)
                }
            }
        }
    }

    pub fn ir_value(&self, selector: Option<&str>) -> Result<Value> {
        match self {
            Self::Standalone { analyzed, .. } => {
                Ok(serde_json::to_value(kairos_ir::lower(analyzed))?)
            }
            Self::Project { analyzed, focus_module, .. } => {
                if let Some(selection) =
                    select_project_module(analyzed, focus_module.as_deref(), selector)?
                {
                    Ok(serde_json::to_value(kairos_ir::lower(selection.analyzed))?)
                } else {
                    Ok(serde_json::to_value(kairos_ir::lower_project(analyzed))?)
                }
            }
        }
    }

    pub fn prompt_text(&self, selector: Option<&str>) -> Result<String> {
        match self {
            Self::Standalone { analyzed, .. } => {
                Ok(kairos_ir::render_prompt(&kairos_ir::lower(analyzed)))
            }
            Self::Project { analyzed, focus_module, .. } => {
                if let Some(selection) =
                    select_project_module(analyzed, focus_module.as_deref(), selector)?
                {
                    Ok(kairos_ir::render_prompt(&kairos_ir::lower(selection.analyzed)))
                } else {
                    Ok(kairos_ir::render_project_prompt(&kairos_ir::lower_project(analyzed)))
                }
            }
        }
    }

    pub fn run(&self, function: Option<&str>, args: &[RuntimeValue]) -> Result<ExecutionReport> {
        match self {
            Self::Standalone { analyzed, .. } => {
                Ok(run(&kairos_ir::lower(analyzed), function, args)?)
            }
            Self::Project { analyzed, focus_module, .. } => {
                let default_module =
                    focus_module.as_deref().unwrap_or(&analyzed.project.entry_module);
                Ok(run_project(
                    &kairos_ir::lower_project(analyzed),
                    default_module,
                    function,
                    args,
                )?)
            }
        }
    }

    pub fn discover_tests(&self, filter: Option<&str>) -> Vec<TestCaseRecord> {
        let filter = filter.map(str::to_lowercase);
        let matches_filter = |name: &str| {
            filter.as_deref().is_none_or(|filter| name.to_lowercase().contains(filter))
        };

        match self {
            Self::Standalone { source_hint, analyzed } => analyzed
                .program
                .functions
                .iter()
                .filter(|function| function.is_test)
                .map(|function| TestCaseRecord {
                    package: None,
                    module: analyzed.program.module.clone(),
                    function: function.name.clone(),
                    display_name: format!("{}::{}", analyzed.program.module, function.name),
                    relative_path: normalize_display_path(source_hint),
                })
                .filter(|record| matches_filter(&record.display_name))
                .collect(),
            Self::Project { analyzed, .. } => {
                let root_package = analyzed.project.manifest.package.name.clone();
                analyzed
                    .project
                    .modules
                    .iter()
                    .zip(&analyzed.modules)
                    .filter(|(project_module, _)| project_module.package == root_package)
                    .flat_map(|(project_module, analyzed_module)| {
                        analyzed_module
                            .analyzed
                            .program
                            .functions
                            .iter()
                            .filter(|function| function.is_test)
                            .map(move |function| TestCaseRecord {
                                package: Some(project_module.package.clone()),
                                module: project_module.module.clone(),
                                function: function.name.clone(),
                                display_name: format!(
                                    "{}::{}",
                                    project_module.module, function.name
                                ),
                                relative_path: project_module.relative_path.clone(),
                            })
                    })
                    .filter(|record| matches_filter(&record.display_name))
                    .collect()
            }
        }
    }

    pub fn test_report(&self, filter: Option<&str>) -> Result<TestReport> {
        let cases = self.discover_tests(filter);
        let mut results = Vec::new();
        let mut passed = 0usize;
        let mut failed = 0usize;

        match self {
            Self::Standalone { analyzed, .. } => {
                let kir = kairos_ir::lower(analyzed);
                for case in &cases {
                    let outcome = run(&kir, Some(&case.function), &[]);
                    match outcome {
                        Ok(report) => {
                            let value = report
                                .results
                                .first()
                                .map(|result| result.value.clone())
                                .unwrap_or(RuntimeValue::Null);
                            if matches!(value, RuntimeValue::Boolean(true)) {
                                passed += 1;
                                results.push(TestResultRecord {
                                    outcome: TestOutcome::Passed,
                                    module: case.module.clone(),
                                    function: case.function.clone(),
                                    display_name: case.display_name.clone(),
                                    message: "test passed".to_string(),
                                    value: Some(value),
                                });
                            } else {
                                failed += 1;
                                results.push(TestResultRecord {
                                    outcome: TestOutcome::Failed,
                                    module: case.module.clone(),
                                    function: case.function.clone(),
                                    display_name: case.display_name.clone(),
                                    message: "test returned false".to_string(),
                                    value: Some(value),
                                });
                            }
                        }
                        Err(error) => {
                            failed += 1;
                            results.push(TestResultRecord {
                                outcome: TestOutcome::Failed,
                                module: case.module.clone(),
                                function: case.function.clone(),
                                display_name: case.display_name.clone(),
                                message: error.to_string(),
                                value: None,
                            });
                        }
                    }
                }
            }
            Self::Project { analyzed, .. } => {
                let kir = kairos_ir::lower_project(analyzed);
                for case in &cases {
                    let outcome = run_project(&kir, &case.module, Some(&case.function), &[]);
                    match outcome {
                        Ok(report) => {
                            let value = report
                                .results
                                .first()
                                .map(|result| result.value.clone())
                                .unwrap_or(RuntimeValue::Null);
                            if matches!(value, RuntimeValue::Boolean(true)) {
                                passed += 1;
                                results.push(TestResultRecord {
                                    outcome: TestOutcome::Passed,
                                    module: case.module.clone(),
                                    function: case.function.clone(),
                                    display_name: case.display_name.clone(),
                                    message: "test passed".to_string(),
                                    value: Some(value),
                                });
                            } else {
                                failed += 1;
                                results.push(TestResultRecord {
                                    outcome: TestOutcome::Failed,
                                    module: case.module.clone(),
                                    function: case.function.clone(),
                                    display_name: case.display_name.clone(),
                                    message: "test returned false".to_string(),
                                    value: Some(value),
                                });
                            }
                        }
                        Err(error) => {
                            failed += 1;
                            results.push(TestResultRecord {
                                outcome: TestOutcome::Failed,
                                module: case.module.clone(),
                                function: case.function.clone(),
                                display_name: case.display_name.clone(),
                                message: error.to_string(),
                                value: None,
                            });
                        }
                    }
                }
            }
        }

        let total = results.len();
        Ok(TestReport {
            status: if failed == 0 { "ok" } else { "failed" },
            target: self.target_label(),
            package: self.package_name().map(ToOwned::to_owned),
            total,
            passed,
            failed,
            results,
        })
    }

    pub fn doctor_report(&self) -> DoctorReport {
        let warnings = diagnostics_to_json(self.warnings());
        let mut checks = Vec::new();
        checks.push(DoctorCheck {
            name: "workspace_load".to_string(),
            status: DoctorStatus::Ok,
            message: format!("loaded {} target successfully", self.target_label()),
        });
        checks.push(DoctorCheck {
            name: "deterministic_mode".to_string(),
            status: DoctorStatus::Ok,
            message: self.mode_label().to_string(),
        });

        if let Some(entry_module) = self.entry_module() {
            checks.push(DoctorCheck {
                name: "entry_module".to_string(),
                status: DoctorStatus::Ok,
                message: format!("entry module `{entry_module}` is available"),
            });
        }

        if self.dependency_count() > 0 {
            checks.push(DoctorCheck {
                name: "dependencies".to_string(),
                status: DoctorStatus::Ok,
                message: format!("{} direct local dependencies resolved", self.dependency_count()),
            });
        }

        let status = if warnings.is_empty() {
            DoctorStatus::Ok
        } else {
            checks.push(DoctorCheck {
                name: "warnings".to_string(),
                status: DoctorStatus::Warning,
                message: format!("{} warning(s) were reported during analysis", warnings.len()),
            });
            DoctorStatus::Warning
        };

        DoctorReport {
            status,
            target: self.target_label(),
            root: self.display_root(),
            package: self.package_name().map(ToOwned::to_owned),
            entry_module: self.entry_module().map(ToOwned::to_owned),
            module_count: self.module_count(),
            package_count: self.package_count(),
            dependency_count: self.dependency_count(),
            checks,
            warnings,
        }
    }

    pub fn check_json(&self) -> Value {
        match self {
            Self::Standalone { source_hint, analyzed } => json!({
                "status": "ok",
                "kind": "module",
                "path": normalize_display_path(source_hint),
                "module": analyzed.program.module,
                "warnings": diagnostics_to_json(&analyzed.warnings),
            }),
            Self::Project { analyzed, focus_module, .. } => json!({
                "status": "ok",
                "kind": "project",
                "package": analyzed.project.manifest.package.name,
                "entry_module": analyzed.project.entry_module,
                "module_count": analyzed.project.modules.len(),
                "package_count": analyzed.project.packages.len(),
                "dependency_count": self.dependency_count(),
                "focus_module": focus_module,
                "warnings": diagnostics_to_json(&analyzed.warnings),
            }),
        }
    }
}

pub fn load_program(path: &Path) -> std::result::Result<kairos_ast::Program, Vec<Diagnostic>> {
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

pub fn format_project(project: &Project, check: bool) -> Result<()> {
    let mut changed = Vec::new();
    let root_package = project.manifest.package.name.clone();

    for module in project.modules.iter().filter(|module| module.package == root_package) {
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

pub fn diagnostics_to_json(diagnostics: &[Diagnostic]) -> Vec<Value> {
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

pub fn diagnostics_to_anyhow(diagnostics: &[Diagnostic]) -> anyhow::Error {
    anyhow::anyhow!(diagnostics.iter().map(render_diagnostic).collect::<Vec<_>>().join("\n"))
}

pub fn project_error_to_anyhow(error: kairos_project::ProjectError) -> anyhow::Error {
    diagnostics_to_anyhow(&error.diagnostics)
}

pub fn render_diagnostic(diagnostic: &Diagnostic) -> String {
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

pub fn print_diagnostics(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        eprintln!("{}", render_diagnostic(diagnostic));
    }
}

pub fn print_warning_summary(warnings: &[Diagnostic]) {
    if warnings.is_empty() {
        println!("Warnings: none");
    } else {
        println!("Warnings:");
        for warning in warnings {
            println!("- {}", render_diagnostic(warning));
        }
    }
}

pub fn render_location(location: &DiagnosticLocation) -> String {
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

pub fn parse_error_to_diagnostic(path: &Path, error: kairos_parser::ParseError) -> Diagnostic {
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

pub fn location_for_path(path: &Path) -> DiagnosticLocation {
    DiagnosticLocation {
        path: Some(normalize_display_path(path)),
        module: None,
        symbol: None,
        line: None,
        column: None,
    }
}

pub fn normalize_display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn canonical_display_path(path: &Path) -> String {
    let normalized = path
        .canonicalize()
        .map(|path| normalize_display_path(&path))
        .unwrap_or_else(|_| normalize_display_path(path));
    normalized.strip_prefix("//?/").map(ToOwned::to_owned).unwrap_or(normalized)
}

pub fn read_source(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))
}

pub fn parse_runtime_value(input: &str) -> Result<RuntimeValue> {
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

pub fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn exit_with_json_error(diagnostics: &[Diagnostic]) -> Result<()> {
    print_json(&json!({
        "status": "error",
        "diagnostics": diagnostics_to_json(diagnostics),
    }))?;
    process::exit(1);
}

fn select_project_module<'a>(
    analyzed: &'a AnalyzedProject,
    focus_module: Option<&'a str>,
    selector: Option<&str>,
) -> Result<Option<ProjectModuleSelection<'a>>> {
    let Some(selector) = selector.or(focus_module) else {
        return Ok(None);
    };

    let selector = normalize_selector(selector);
    let project = &analyzed.project;

    for (project_module, analyzed_module) in project.modules.iter().zip(&analyzed.modules) {
        let direct_match = selector == project_module.module
            || selector == normalize_selector(&project_module.relative_path);
        let absolute_match =
            selector == canonical_display_path(&project.root.join(&project_module.relative_path));
        if direct_match || absolute_match {
            return Ok(Some(ProjectModuleSelection { analyzed: &analyzed_module.analyzed }));
        }
    }

    bail!("unknown module selector `{selector}`")
}

fn is_project_root_input(path: &Path) -> bool {
    path.is_dir() || path.file_name().is_some_and(|name| name == "kairos.toml")
}

fn is_kai_file(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "kai")
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Warning => "warning",
        Severity::Error => "error",
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

fn normalize_selector(value: &str) -> String {
    value.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::LoadedWorkspace;

    #[test]
    fn auto_detects_project_from_nested_file_path() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/assistant_briefing/src/main.kai");
        let workspace = LoadedWorkspace::load(&path).expect("workspace should load");

        assert_eq!(workspace.package_name(), Some("assistant_briefing"));
        assert_eq!(workspace.focus_module(), Some("demo.assistant_briefing"));
    }

    #[test]
    fn reloads_standalone_workspace_after_source_change() {
        let tempdir = tempdir().expect("tempdir should exist");
        let file = tempdir.path().join("main.kai");
        fs::write(
            &file,
            "module demo.one;\n\nfn main() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"one\";\n}\n",
        )
        .expect("source should write");

        let loaded = LoadedWorkspace::load(&file).expect("workspace should load");
        fs::write(
            &file,
            "module demo.one;\n\nfn main() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"two\";\n}\n",
        )
        .expect("source should update");

        let reloaded = loaded.reload().expect("workspace should reload");
        let prompt = reloaded.prompt_text(None).expect("prompt should render");
        assert!(prompt.contains("demo.one"));
    }
}
