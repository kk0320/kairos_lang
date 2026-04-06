use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use kairos_ast::Program;
use kairos_semantic::{
    analyze_with_context, AnalysisContext, AnalyzedProgram, Diagnostic, DiagnosticLocation,
    ImportedFunction, ImportedType, ImportedTypeKind,
};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

pub type Result<T> = std::result::Result<T, ProjectError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub package: PackageSection,
    #[serde(default)]
    pub build: BuildSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSection {
    pub name: String,
    pub version: String,
    pub entry: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildSection {
    #[serde(default)]
    pub emit: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectModule {
    pub module: String,
    pub relative_path: String,
    pub imports: Vec<String>,
    pub program: Program,
    #[serde(skip)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(skip)]
    pub root: PathBuf,
    #[serde(skip)]
    pub manifest_path: PathBuf,
    pub manifest: PackageManifest,
    pub entry_file: String,
    pub entry_module: String,
    pub modules: Vec<ProjectModule>,
    #[serde(skip)]
    module_index: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct AnalyzedModule {
    pub relative_path: String,
    pub analyzed: AnalyzedProgram,
}

#[derive(Debug, Clone)]
pub struct AnalyzedProject {
    pub project: Project,
    pub modules: Vec<AnalyzedModule>,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct ProjectError {
    pub diagnostics: Vec<Diagnostic>,
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "[{}] {}", diagnostic.code, diagnostic.message)?;
        }
        Ok(())
    }
}

impl Error for ProjectError {}

impl Project {
    pub fn module(&self, module: &str) -> Option<&ProjectModule> {
        self.module_index.get(module).and_then(|index| self.modules.get(*index))
    }

    pub fn entry_module(&self) -> &ProjectModule {
        self.module(&self.entry_module).expect("entry module should exist in loaded project")
    }
}

pub fn find_manifest(start: &Path) -> Option<PathBuf> {
    let mut current =
        if start.is_dir() { start.to_path_buf() } else { start.parent()?.to_path_buf() };

    loop {
        let candidate = current.join("kairos.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub fn load_project(path: &Path) -> Result<Project> {
    let manifest_path = resolve_manifest_path(path)?;
    let manifest_source = fs::read_to_string(&manifest_path).map_err(|error| ProjectError {
        diagnostics: vec![project_error(
            "manifest_read_failed",
            format!("failed to read manifest `{}`: {error}", manifest_path.display()),
            Some(location_for_path(&manifest_path, None, None)),
        )],
    })?;
    let manifest =
        toml::from_str::<PackageManifest>(&manifest_source).map_err(|error| ProjectError {
            diagnostics: vec![project_error(
                "manifest_parse_failed",
                format!("failed to parse manifest `{}`: {error}", manifest_path.display()),
                Some(location_for_path(&manifest_path, None, None)),
            )],
        })?;

    validate_manifest_contents(&manifest, &manifest_path)?;

    let root = manifest_path.parent().map(Path::to_path_buf).ok_or_else(|| ProjectError {
        diagnostics: vec![project_error(
            "invalid_manifest_path",
            format!("manifest path `{}` has no parent directory", manifest_path.display()),
            Some(location_for_path(&manifest_path, None, None)),
        )],
    })?;

    let entry_file = root.join(&manifest.package.entry);
    if !entry_file.is_file() {
        return Err(ProjectError {
            diagnostics: vec![project_error(
                "missing_entry",
                format!("manifest entry `{}` does not exist", manifest.package.entry),
                Some(DiagnosticLocation {
                    path: Some(normalize_relative_path(&manifest.package.entry)),
                    module: None,
                    symbol: None,
                    line: None,
                    column: None,
                }),
            )],
        });
    }

    let source_root = entry_file.parent().ok_or_else(|| ProjectError {
        diagnostics: vec![project_error(
            "invalid_entry",
            format!(
                "manifest entry `{}` must live inside a source directory",
                manifest.package.entry
            ),
            Some(location_for_path(&entry_file, None, None)),
        )],
    })?;

    let files = collect_kai_files(source_root);
    if files.is_empty() {
        return Err(ProjectError {
            diagnostics: vec![project_error(
                "missing_sources",
                format!("no `.kai` source files were found under `{}`", source_root.display()),
                Some(location_for_path(source_root, None, None)),
            )],
        });
    }

    let mut modules = Vec::new();
    let mut diagnostics = Vec::new();

    for file in files {
        let relative_path = relative_to_root(&root, &file);
        let source = match fs::read_to_string(&file) {
            Ok(source) => source,
            Err(error) => {
                diagnostics.push(project_error(
                    "source_read_failed",
                    format!("failed to read `{}`: {error}", file.display()),
                    Some(location_for_relative_path(&relative_path, None, None)),
                ));
                continue;
            }
        };

        match kairos_parser::parse_source(&source) {
            Ok(program) => modules.push(ProjectModule {
                module: program.module.clone(),
                relative_path,
                imports: program.uses.clone(),
                program,
                path: file,
            }),
            Err(error) => diagnostics.push(Diagnostic {
                severity: kairos_semantic::Severity::Error,
                code: "parse_error",
                message: error.message,
                location: Some(DiagnosticLocation {
                    path: Some(relative_path),
                    module: None,
                    symbol: None,
                    line: Some(error.line),
                    column: Some(error.column),
                }),
                related: Vec::new(),
            }),
        }
    }

    modules.sort_by(|left, right| {
        left.module.cmp(&right.module).then(left.relative_path.cmp(&right.relative_path))
    });

    let mut module_index = BTreeMap::new();
    let mut duplicate_locations = BTreeMap::<String, String>::new();
    for (index, module) in modules.iter().enumerate() {
        if let Some(previous_index) = module_index.insert(module.module.clone(), index) {
            let previous = &modules[previous_index];
            duplicate_locations.insert(previous.module.clone(), previous.relative_path.clone());
            diagnostics.push(
                project_error(
                    "duplicate_module",
                    format!("module `{}` is declared by more than one source file", module.module),
                    Some(location_for_relative_path(
                        &module.relative_path,
                        Some(module.module.clone()),
                        Some(module.module.clone()),
                    )),
                )
                .with_related(
                    "previous module declaration",
                    Some(location_for_relative_path(
                        &previous.relative_path,
                        Some(previous.module.clone()),
                        Some(previous.module.clone()),
                    )),
                ),
            );
        }
    }

    if !diagnostics.is_empty() {
        return Err(ProjectError { diagnostics });
    }

    let entry_relative_path = relative_to_root(&root, &entry_file);
    let entry_module = modules
        .iter()
        .find(|module| module.relative_path == entry_relative_path)
        .map(|module| module.module.clone())
        .ok_or_else(|| ProjectError {
            diagnostics: vec![project_error(
                "missing_entry_module",
                format!(
                    "entry file `{}` was not part of the discovered project modules",
                    entry_relative_path
                ),
                Some(location_for_relative_path(&entry_relative_path, None, None)),
            )],
        })?;

    let project = Project {
        root,
        manifest_path,
        manifest,
        entry_file: entry_relative_path,
        entry_module,
        modules,
        module_index,
    };

    validate_import_graph(&project)?;
    Ok(project)
}

pub fn analyze_project(project: &Project) -> Result<AnalyzedProject> {
    let mut modules = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    for module in &project.modules {
        let imported_types = gather_imported_types(project, module);
        let imported_functions = gather_imported_functions(project, module);
        let context = AnalysisContext {
            file_path: Some(module.relative_path.clone()),
            module: Some(module.module.clone()),
            imported_types,
            imported_functions,
        };

        match analyze_with_context(module.program.clone(), &context) {
            Ok(analyzed) => {
                warnings.extend(analyzed.warnings.clone());
                modules
                    .push(AnalyzedModule { relative_path: module.relative_path.clone(), analyzed });
            }
            Err(error) => errors.extend(error.diagnostics),
        }
    }

    if errors.is_empty() {
        Ok(AnalyzedProject { project: project.clone(), modules, warnings })
    } else {
        Err(ProjectError { diagnostics: errors })
    }
}

fn resolve_manifest_path(path: &Path) -> Result<PathBuf> {
    let manifest_path = if path.is_dir() {
        path.join("kairos.toml")
    } else if path.file_name().is_some_and(|name| name == "kairos.toml") {
        path.to_path_buf()
    } else if path.extension().is_some_and(|extension| extension == "kai") {
        find_manifest(path).ok_or_else(|| ProjectError {
            diagnostics: vec![project_error(
                "manifest_not_found",
                format!("no `kairos.toml` manifest was found above `{}`", path.display()),
                Some(location_for_path(path, None, None)),
            )],
        })?
    } else {
        return Err(ProjectError {
            diagnostics: vec![project_error(
                "unsupported_input",
                format!(
                    "expected a project directory, `kairos.toml`, or `.kai` file, got `{}`",
                    path.display()
                ),
                Some(location_for_path(path, None, None)),
            )],
        });
    };

    if manifest_path.is_file() {
        Ok(manifest_path)
    } else {
        Err(ProjectError {
            diagnostics: vec![project_error(
                "manifest_not_found",
                format!("could not find manifest `{}`", manifest_path.display()),
                Some(location_for_path(&manifest_path, None, None)),
            )],
        })
    }
}

fn validate_manifest_contents(manifest: &PackageManifest, manifest_path: &Path) -> Result<()> {
    let mut diagnostics = Vec::new();
    if manifest.package.name.trim().is_empty() {
        diagnostics.push(project_error(
            "invalid_package_name",
            "manifest `package.name` must not be empty",
            Some(location_for_path(manifest_path, None, Some("package.name".to_string()))),
        ));
    } else if !is_valid_package_name(&manifest.package.name) {
        let suggestion = normalize_package_name(&manifest.package.name);
        let mut diagnostic = project_error(
            "invalid_package_name",
            "manifest `package.name` must start with a lowercase ASCII letter and use only lowercase ASCII letters, digits, and underscores",
            Some(location_for_path(manifest_path, None, Some("package.name".to_string()))),
        );
        if suggestion != manifest.package.name {
            diagnostic = diagnostic.with_related(
                format!("suggested package name: `{suggestion}`"),
                Some(location_for_path(manifest_path, None, Some("package.name".to_string()))),
            );
        }
        diagnostics.push(diagnostic);
    }

    if manifest.package.version.trim().is_empty() {
        diagnostics.push(project_error(
            "invalid_package_version",
            "manifest `package.version` must not be empty",
            Some(location_for_path(manifest_path, None, Some("package.version".to_string()))),
        ));
    } else if !is_valid_version_string(&manifest.package.version) {
        diagnostics.push(project_error(
            "invalid_package_version",
            "manifest `package.version` must use `MAJOR.MINOR.PATCH` with an optional prerelease suffix",
            Some(location_for_path(manifest_path, None, Some("package.version".to_string()))),
        ));
    }

    if manifest.package.entry.trim().is_empty() {
        diagnostics.push(project_error(
            "invalid_entry",
            "manifest `package.entry` must not be empty",
            Some(location_for_path(manifest_path, None, Some("package.entry".to_string()))),
        ));
    } else if !manifest.package.entry.ends_with(".kai") {
        diagnostics.push(project_error(
            "invalid_entry",
            "manifest `package.entry` must point to a `.kai` source file",
            Some(location_for_path(manifest_path, None, Some("package.entry".to_string()))),
        ));
    } else if Path::new(&manifest.package.entry).is_absolute() {
        diagnostics.push(project_error(
            "invalid_entry",
            "manifest `package.entry` must be a relative path inside the project root",
            Some(location_for_path(manifest_path, None, Some("package.entry".to_string()))),
        ));
    } else if Path::new(&manifest.package.entry)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        diagnostics.push(project_error(
            "invalid_entry",
            "manifest `package.entry` must not traverse outside the project root",
            Some(location_for_path(manifest_path, None, Some("package.entry".to_string()))),
        ));
    }

    let allowed_emit_targets = ["ast", "ir", "prompt"];
    let mut seen_emit_targets = BTreeSet::new();
    for emit in &manifest.build.emit {
        if !allowed_emit_targets.contains(&emit.as_str()) {
            diagnostics.push(project_error(
                "invalid_build_emit",
                format!(
                    "manifest `build.emit` contains unsupported target `{emit}`; expected one of: {}",
                    allowed_emit_targets.join(", ")
                ),
                Some(location_for_path(manifest_path, None, Some("build.emit".to_string()))),
            ));
        }

        if !seen_emit_targets.insert(emit.clone()) {
            diagnostics.push(project_error(
                "duplicate_build_emit",
                format!("manifest `build.emit` contains duplicate target `{emit}`"),
                Some(location_for_path(manifest_path, None, Some("build.emit".to_string()))),
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(ProjectError { diagnostics })
    }
}

fn collect_kai_files(root: &Path) -> Vec<PathBuf> {
    let mut files = WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "kai")
                .then(|| entry.path().to_path_buf())
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn validate_import_graph(project: &Project) -> Result<()> {
    let mut diagnostics = Vec::new();
    for module in &project.modules {
        for import in &module.imports {
            if project.module(import).is_none() {
                diagnostics.push(project_error(
                    "unresolved_import",
                    format!("module `{}` imports unresolved module `{import}`", module.module),
                    Some(location_for_relative_path(
                        &module.relative_path,
                        Some(module.module.clone()),
                        Some(import.clone()),
                    )),
                ));
            }
        }
    }

    let mut visited = BTreeSet::new();
    let mut stack = Vec::<String>::new();
    let mut active = BTreeSet::new();
    let mut reported_cycles = BTreeSet::new();

    for module in &project.modules {
        detect_cycle(
            project,
            &module.module,
            &mut visited,
            &mut active,
            &mut stack,
            &mut reported_cycles,
            &mut diagnostics,
        );
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(ProjectError { diagnostics })
    }
}

fn detect_cycle(
    project: &Project,
    module_name: &str,
    visited: &mut BTreeSet<String>,
    active: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    reported_cycles: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if active.contains(module_name) {
        if let Some(start) = stack.iter().position(|item| item == module_name) {
            let cycle = stack[start..]
                .iter()
                .chain(std::iter::once(&stack[start]))
                .cloned()
                .collect::<Vec<_>>();
            let key = cycle.join(" -> ");
            if reported_cycles.insert(key.clone()) {
                let module = project.module(module_name).expect("cycle module should exist");
                diagnostics.push(project_error(
                    "import_cycle",
                    format!("import cycle detected: {}", cycle.join(" -> ")),
                    Some(location_for_relative_path(
                        &module.relative_path,
                        Some(module.module.clone()),
                        Some(module.module.clone()),
                    )),
                ));
            }
        }
        return;
    }

    if !visited.insert(module_name.to_string()) {
        return;
    }

    active.insert(module_name.to_string());
    stack.push(module_name.to_string());

    if let Some(module) = project.module(module_name) {
        for import in &module.imports {
            if project.module(import).is_some() {
                detect_cycle(project, import, visited, active, stack, reported_cycles, diagnostics);
            }
        }
    }

    stack.pop();
    active.remove(module_name);
}

fn gather_imported_types(project: &Project, module: &ProjectModule) -> Vec<ImportedType> {
    let mut types = Vec::new();
    for import in &module.imports {
        let Some(imported_module) = project.module(import) else {
            continue;
        };
        for schema in &imported_module.program.schemas {
            types.push(ImportedType {
                name: schema.name.clone(),
                module: imported_module.module.clone(),
                kind: ImportedTypeKind::Schema,
            });
        }
        for enum_decl in &imported_module.program.enums {
            types.push(ImportedType {
                name: enum_decl.name.clone(),
                module: imported_module.module.clone(),
                kind: ImportedTypeKind::Enum,
            });
        }
        for alias in &imported_module.program.type_aliases {
            types.push(ImportedType {
                name: alias.name.clone(),
                module: imported_module.module.clone(),
                kind: ImportedTypeKind::Alias(alias.target.clone()),
            });
        }
    }
    types
}

fn gather_imported_functions(project: &Project, module: &ProjectModule) -> Vec<ImportedFunction> {
    let mut functions = Vec::new();
    for import in &module.imports {
        let Some(imported_module) = project.module(import) else {
            continue;
        };
        for function in &imported_module.program.functions {
            functions.push(ImportedFunction {
                name: function.name.clone(),
                module: imported_module.module.clone(),
                params: function.params.clone(),
                return_type: function.return_type.clone(),
            });
        }
    }
    functions
}

fn relative_to_root(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    normalize_relative_path(&relative.to_string_lossy())
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn is_valid_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_lowercase())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn normalize_package_name(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }

    let mut normalized = normalized.trim_matches('_').to_string();
    if normalized.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        normalized.insert(0, 'k');
    }
    if normalized.is_empty() {
        "kairos_app".to_string()
    } else {
        normalized
    }
}

fn is_valid_version_string(version: &str) -> bool {
    let (core, prerelease) = match version.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (version, None),
    };

    let core_parts = core.split('.').collect::<Vec<_>>();
    if core_parts.len() != 3
        || core_parts
            .iter()
            .any(|part| part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return false;
    }

    prerelease.is_none_or(|suffix| {
        !suffix.is_empty()
            && suffix.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-')
    })
}

fn location_for_path(
    path: &Path,
    module: Option<String>,
    symbol: Option<String>,
) -> DiagnosticLocation {
    DiagnosticLocation {
        path: Some(normalize_relative_path(&path.to_string_lossy())),
        module,
        symbol,
        line: None,
        column: None,
    }
}

fn location_for_relative_path(
    path: &str,
    module: Option<String>,
    symbol: Option<String>,
) -> DiagnosticLocation {
    DiagnosticLocation { path: Some(path.to_string()), module, symbol, line: None, column: None }
}

fn project_error(
    code: &'static str,
    message: impl Into<String>,
    location: Option<DiagnosticLocation>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(code, message);
    if let Some(location) = location {
        diagnostic = diagnostic.with_location(location);
    }
    diagnostic
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{analyze_project, load_project};

    #[test]
    fn loads_multi_file_project() {
        let tempdir = tempdir().expect("tempdir should exist");
        fs::create_dir_all(tempdir.path().join("src/shared")).expect("source tree should create");
        fs::write(
            tempdir.path().join("kairos.toml"),
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\nentry = \"src/main.kai\"\n",
        )
        .expect("manifest should write");
        fs::write(
            tempdir.path().join("src/main.kai"),
            "module demo.main;\nuse demo.shared.text;\n\nfn hello() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return greeting();\n}\n",
        )
        .expect("main should write");
        fs::write(
            tempdir.path().join("src/shared/text.kai"),
            "module demo.shared.text;\n\nfn greeting() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"hi\";\n}\n",
        )
        .expect("shared module should write");

        let project = load_project(tempdir.path()).expect("project should load");
        assert_eq!(project.modules.len(), 2);
        assert_eq!(project.entry_module, "demo.main");

        let analyzed = analyze_project(&project).expect("project should analyze");
        assert!(analyzed.warnings.is_empty());
    }

    #[test]
    fn rejects_unresolved_imports() {
        let tempdir = tempdir().expect("tempdir should exist");
        fs::create_dir_all(tempdir.path().join("src")).expect("source tree should create");
        fs::write(
            tempdir.path().join("kairos.toml"),
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\nentry = \"src/main.kai\"\n",
        )
        .expect("manifest should write");
        fs::write(
            tempdir.path().join("src/main.kai"),
            "module demo.main;\nuse demo.missing;\n\nfn hello() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"hi\";\n}\n",
        )
        .expect("main should write");

        let error = load_project(tempdir.path()).expect_err("project should fail");
        assert!(error.to_string().contains("unresolved module"));
    }

    #[test]
    fn rejects_duplicate_module_names() {
        let tempdir = tempdir().expect("tempdir should exist");
        fs::create_dir_all(tempdir.path().join("src/nested")).expect("source tree should create");
        fs::write(
            tempdir.path().join("kairos.toml"),
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\nentry = \"src/main.kai\"\n",
        )
        .expect("manifest should write");
        fs::write(
            tempdir.path().join("src/main.kai"),
            "module demo.main;\n\nfn main() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"hi\";\n}\n",
        )
        .expect("main should write");
        fs::write(
            tempdir.path().join("src/nested/duplicate.kai"),
            "module demo.main;\n\nfn helper() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"hi\";\n}\n",
        )
        .expect("duplicate should write");

        let error = load_project(tempdir.path()).expect_err("project should fail");
        assert!(error.to_string().contains("duplicate_module"));
    }

    #[test]
    fn rejects_import_cycles() {
        let tempdir = tempdir().expect("tempdir should exist");
        fs::create_dir_all(tempdir.path().join("src")).expect("source tree should create");
        fs::write(
            tempdir.path().join("kairos.toml"),
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\nentry = \"src/main.kai\"\n",
        )
        .expect("manifest should write");
        fs::write(
            tempdir.path().join("src/main.kai"),
            "module demo.main;\nuse demo.helper;\n\nfn main() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return helper();\n}\n",
        )
        .expect("main should write");
        fs::write(
            tempdir.path().join("src/helper.kai"),
            "module demo.helper;\nuse demo.main;\n\nfn helper() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"hi\";\n}\n",
        )
        .expect("helper should write");

        let error = load_project(tempdir.path()).expect_err("project should fail");
        assert!(error.to_string().contains("import_cycle"));
    }

    #[test]
    fn rejects_manifest_with_empty_package_name() {
        let tempdir = tempdir().expect("tempdir should exist");
        fs::create_dir_all(tempdir.path().join("src")).expect("source tree should create");
        fs::write(
            tempdir.path().join("kairos.toml"),
            "[package]\nname = \"\"\nversion = \"1.0.0\"\nentry = \"src/main.kai\"\n",
        )
        .expect("manifest should write");
        fs::write(
            tempdir.path().join("src/main.kai"),
            "module demo.main;\n\nfn main() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"hi\";\n}\n",
        )
        .expect("main should write");

        let error = load_project(tempdir.path()).expect_err("project should fail");
        assert!(error.to_string().contains("invalid_package_name"));
    }

    #[test]
    fn rejects_manifest_with_invalid_version_format() {
        let tempdir = tempdir().expect("tempdir should exist");
        fs::create_dir_all(tempdir.path().join("src")).expect("source tree should create");
        fs::write(
            tempdir.path().join("kairos.toml"),
            "[package]\nname = \"demo\"\nversion = \"1.0\"\nentry = \"src/main.kai\"\n",
        )
        .expect("manifest should write");
        fs::write(
            tempdir.path().join("src/main.kai"),
            "module demo.main;\n\nfn main() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"hi\";\n}\n",
        )
        .expect("main should write");

        let error = load_project(tempdir.path()).expect_err("project should fail");
        assert!(error.to_string().contains("invalid_package_version"));
    }

    #[test]
    fn rejects_manifest_with_parent_relative_entry() {
        let tempdir = tempdir().expect("tempdir should exist");
        fs::create_dir_all(tempdir.path().join("src")).expect("source tree should create");
        fs::write(
            tempdir.path().join("kairos.toml"),
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\nentry = \"../main.kai\"\n",
        )
        .expect("manifest should write");
        fs::write(
            tempdir.path().join("src/main.kai"),
            "module demo.main;\n\nfn main() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"hi\";\n}\n",
        )
        .expect("main should write");

        let error = load_project(tempdir.path()).expect_err("project should fail");
        assert!(error.to_string().contains("must not traverse outside the project root"));
    }

    #[test]
    fn rejects_manifest_with_invalid_emit_target() {
        let tempdir = tempdir().expect("tempdir should exist");
        fs::create_dir_all(tempdir.path().join("src")).expect("source tree should create");
        fs::write(
            tempdir.path().join("kairos.toml"),
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\nentry = \"src/main.kai\"\n\n[build]\nemit = [\"ast\", \"binary\"]\n",
        )
        .expect("manifest should write");
        fs::write(
            tempdir.path().join("src/main.kai"),
            "module demo.main;\n\nfn main() -> Str\ndescribe \"demo\"\ntags [\"demo\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"hi\";\n}\n",
        )
        .expect("main should write");

        let error = load_project(tempdir.path()).expect_err("project should fail");
        assert!(error.to_string().contains("invalid_build_emit"));
    }
}
