use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use kairos_ast::{Program, UseDecl, Visibility};
use kairos_semantic::{
    analyze_with_context, AnalysisContext, AnalyzedProgram, Diagnostic, DiagnosticLocation,
    ImportedFunction, ImportedType, ImportedTypeKind,
};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

pub type Result<T> = std::result::Result<T, ProjectError>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageManifest {
    pub package: PackageSection,
    #[serde(default)]
    pub build: BuildSection,
    #[serde(default)]
    pub dependencies: BTreeMap<String, DependencySpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencySpec {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDependency {
    pub alias: String,
    pub package: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPackage {
    pub name: String,
    pub version: String,
    pub entry_file: String,
    pub entry_module: String,
    pub is_root: bool,
    pub dependencies: Vec<PackageDependency>,
    #[serde(skip)]
    pub manifest: PackageManifest,
    #[serde(skip)]
    pub root: PathBuf,
    #[serde(skip)]
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectModule {
    pub package: String,
    pub module: String,
    pub relative_path: String,
    pub imports: Vec<String>,
    pub is_dependency: bool,
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
    pub packages: Vec<ProjectPackage>,
    pub modules: Vec<ProjectModule>,
    #[serde(skip)]
    package_index: BTreeMap<String, usize>,
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

#[derive(Debug)]
struct PackageLoadContext {
    project_root: PathBuf,
    manifest_to_package: BTreeMap<PathBuf, String>,
    package_to_manifest: BTreeMap<String, PathBuf>,
    packages: Vec<ProjectPackage>,
    modules: Vec<ProjectModule>,
    active_manifests: Vec<PathBuf>,
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
    pub fn package(&self, package: &str) -> Option<&ProjectPackage> {
        self.package_index.get(package).and_then(|index| self.packages.get(*index))
    }

    pub fn module(&self, module: &str) -> Option<&ProjectModule> {
        self.module_index.get(module).and_then(|index| self.modules.get(*index))
    }

    pub fn entry_module(&self) -> &ProjectModule {
        self.module(&self.entry_module).expect("entry module should exist in loaded project")
    }

    pub fn package_dependencies(&self, package: &str) -> &[PackageDependency] {
        self.package(package).map(|package| package.dependencies.as_slice()).unwrap_or(&[])
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
    let root = manifest_path.parent().map(Path::to_path_buf).ok_or_else(|| ProjectError {
        diagnostics: vec![project_error(
            "invalid_manifest_path",
            format!("manifest path `{}` has no parent directory", manifest_path.display()),
            Some(location_for_path(&manifest_path, None, None)),
        )],
    })?;

    let mut context = PackageLoadContext {
        project_root: root.clone(),
        manifest_to_package: BTreeMap::new(),
        package_to_manifest: BTreeMap::new(),
        packages: Vec::new(),
        modules: Vec::new(),
        active_manifests: Vec::new(),
    };

    let root_package = load_package_recursive(&mut context, &manifest_path, true)?;
    context.modules.sort_by(|left, right| {
        left.package
            .cmp(&right.package)
            .then(left.module.cmp(&right.module))
            .then(left.relative_path.cmp(&right.relative_path))
    });
    context.packages.sort_by(|left, right| {
        right
            .is_root
            .cmp(&left.is_root)
            .then(left.name.cmp(&right.name))
            .then(left.entry_file.cmp(&right.entry_file))
    });

    let root_package_index =
        context.packages.iter().position(|package| package.name == root_package).ok_or_else(
            || ProjectError {
                diagnostics: vec![project_error(
                    "missing_root_package",
                    format!("root package `{root_package}` was not retained in the project graph"),
                    Some(location_for_path(&manifest_path, None, None)),
                )],
            },
        )?;

    let root_package = context.packages[root_package_index].clone();
    let mut module_index = BTreeMap::new();
    let mut package_index = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for (index, package) in context.packages.iter().enumerate() {
        if let Some(previous_index) = package_index.insert(package.name.clone(), index) {
            let previous = &context.packages[previous_index];
            diagnostics.push(
                project_error(
                    "duplicate_package",
                    format!("package `{}` is loaded from more than one manifest", package.name),
                    Some(location_for_path(
                        &package.manifest_path,
                        None,
                        Some(package.name.clone()),
                    )),
                )
                .with_related(
                    "previous package declaration",
                    Some(location_for_path(
                        &previous.manifest_path,
                        None,
                        Some(previous.name.clone()),
                    )),
                ),
            );
        }
    }

    for (index, module) in context.modules.iter().enumerate() {
        if let Some(previous_index) = module_index.insert(module.module.clone(), index) {
            let previous = &context.modules[previous_index];
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
                    format!("previous module declaration in package `{}`", previous.package),
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

    let project = Project {
        root,
        manifest_path,
        manifest: root_package.manifest.clone(),
        entry_file: root_package.entry_file.clone(),
        entry_module: root_package.entry_module.clone(),
        packages: context.packages,
        modules: context.modules,
        package_index,
        module_index,
    };

    validate_import_graph(&project)?;
    Ok(project)
}

fn load_package_recursive(
    context: &mut PackageLoadContext,
    manifest_path: &Path,
    is_root: bool,
) -> Result<String> {
    let manifest_path = manifest_path.canonicalize().map_err(|error| ProjectError {
        diagnostics: vec![project_error(
            "manifest_not_found",
            format!("failed to resolve manifest `{}`: {error}", manifest_path.display()),
            Some(location_for_path(manifest_path, None, None)),
        )],
    })?;

    if let Some(package) = context.manifest_to_package.get(&manifest_path) {
        return Ok(package.clone());
    }

    if let Some(start) = context.active_manifests.iter().position(|active| active == &manifest_path)
    {
        let cycle = context.active_manifests[start..]
            .iter()
            .chain(std::iter::once(&manifest_path))
            .map(|path| normalize_relative_path(&path.to_string_lossy()))
            .collect::<Vec<_>>();
        return Err(ProjectError {
            diagnostics: vec![project_error(
                "dependency_cycle",
                format!("dependency cycle detected: {}", cycle.join(" -> ")),
                Some(location_for_path(&manifest_path, None, None)),
            )],
        });
    }

    let manifest = read_manifest(&manifest_path)?;
    validate_manifest_contents(&manifest, &manifest_path)?;
    let root = manifest_path.parent().map(Path::to_path_buf).ok_or_else(|| ProjectError {
        diagnostics: vec![project_error(
            "invalid_manifest_path",
            format!("manifest path `{}` has no parent directory", manifest_path.display()),
            Some(location_for_path(&manifest_path, None, None)),
        )],
    })?;

    if let Some(previous_manifest) = context.package_to_manifest.get(&manifest.package.name) {
        return Err(ProjectError {
            diagnostics: vec![project_error(
                "duplicate_package",
                format!(
                    "package `{}` is already loaded from another manifest",
                    manifest.package.name
                ),
                Some(location_for_path(&manifest_path, None, Some(manifest.package.name.clone()))),
            )
            .with_related(
                "previous package declaration",
                Some(location_for_path(
                    previous_manifest,
                    None,
                    Some(manifest.package.name.clone()),
                )),
            )],
        });
    }

    context.active_manifests.push(manifest_path.clone());

    let entry_file = root.join(&manifest.package.entry);
    if !entry_file.is_file() {
        context.active_manifests.pop();
        return Err(ProjectError {
            diagnostics: vec![project_error(
                "missing_entry",
                format!("manifest entry `{}` does not exist", manifest.package.entry),
                Some(DiagnosticLocation {
                    path: Some(relative_path_from(&context.project_root, &entry_file)),
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
        context.active_manifests.pop();
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
        let relative_path = relative_path_from(&context.project_root, &file);
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
                package: manifest.package.name.clone(),
                module: program.module.clone(),
                relative_path,
                imports: program.uses.clone(),
                is_dependency: !is_root,
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

    if !diagnostics.is_empty() {
        context.active_manifests.pop();
        return Err(ProjectError { diagnostics });
    }

    let entry_relative_path = relative_path_from(&context.project_root, &entry_file);
    let entry_module = modules
        .iter()
        .find(|module| module.path == entry_file)
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

    let mut dependencies = Vec::new();
    for (alias, dependency) in &manifest.dependencies {
        let dependency_manifest_path = resolve_dependency_manifest_path(&root, &dependency.path)?;
        let dependency_package = load_package_recursive(context, &dependency_manifest_path, false)?;
        dependencies.push(PackageDependency {
            alias: alias.clone(),
            package: dependency_package,
            path: normalize_relative_path(&dependency.path),
        });
    }
    dependencies.sort_by(|left, right| left.alias.cmp(&right.alias));

    context.active_manifests.pop();
    context.manifest_to_package.insert(manifest_path.clone(), manifest.package.name.clone());
    context.package_to_manifest.insert(manifest.package.name.clone(), manifest_path.clone());
    context.modules.extend(modules);
    context.packages.push(ProjectPackage {
        name: manifest.package.name.clone(),
        version: manifest.package.version.clone(),
        entry_file: entry_relative_path,
        entry_module,
        is_root,
        dependencies,
        manifest,
        root,
        manifest_path,
    });

    Ok(context.packages.last().expect("package should exist").name.clone())
}

fn read_manifest(manifest_path: &Path) -> Result<PackageManifest> {
    let manifest_source = fs::read_to_string(manifest_path).map_err(|error| ProjectError {
        diagnostics: vec![project_error(
            "manifest_read_failed",
            format!("failed to read manifest `{}`: {error}", manifest_path.display()),
            Some(location_for_path(manifest_path, None, None)),
        )],
    })?;

    toml::from_str::<PackageManifest>(&manifest_source).map_err(|error| ProjectError {
        diagnostics: vec![project_error(
            "manifest_parse_failed",
            format!("failed to parse manifest `{}`: {error}", manifest_path.display()),
            Some(location_for_path(manifest_path, None, None)),
        )],
    })
}

fn resolve_dependency_manifest_path(package_root: &Path, dependency_path: &str) -> Result<PathBuf> {
    let dependency_root = package_root.join(dependency_path);
    let manifest_path = if dependency_root.is_dir() {
        dependency_root.join("kairos.toml")
    } else {
        dependency_root
    };

    if manifest_path.is_file() {
        Ok(manifest_path)
    } else {
        Err(ProjectError {
            diagnostics: vec![project_error(
                "missing_dependency_manifest",
                format!(
                    "dependency path `{dependency_path}` does not contain a `kairos.toml` manifest"
                ),
                Some(location_for_path(&manifest_path, None, None)),
            )],
        })
    }
}

pub fn analyze_project(project: &Project) -> Result<AnalyzedProject> {
    let mut modules = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    for module in &project.modules {
        let (imported_types, imported_functions) = match resolve_import_bindings(project, module) {
            Ok(bindings) => bindings,
            Err(diagnostics) => {
                errors.extend(diagnostics);
                continue;
            }
        };
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

    let mut seen_dependency_paths = BTreeSet::new();
    for (alias, dependency) in &manifest.dependencies {
        if !is_valid_package_name(alias) {
            diagnostics.push(project_error(
                "invalid_dependency_name",
                format!(
                    "dependency key `{alias}` must start with a lowercase ASCII letter and use only lowercase ASCII letters, digits, and underscores"
                ),
                Some(location_for_path(
                    manifest_path,
                    None,
                    Some(format!("dependencies.{alias}")),
                )),
            ));
        }

        if dependency.path.trim().is_empty() {
            diagnostics.push(project_error(
                "invalid_dependency_path",
                format!("dependency `{alias}` must declare a non-empty `path`"),
                Some(location_for_path(
                    manifest_path,
                    None,
                    Some(format!("dependencies.{alias}.path")),
                )),
            ));
            continue;
        }

        let dependency_path = Path::new(&dependency.path);
        if dependency_path.is_absolute() {
            diagnostics.push(project_error(
                "invalid_dependency_path",
                format!("dependency `{alias}` path must be relative to the package root"),
                Some(location_for_path(
                    manifest_path,
                    None,
                    Some(format!("dependencies.{alias}.path")),
                )),
            ));
        }

        let normalized_path = normalize_relative_path(&dependency.path);
        if !seen_dependency_paths.insert(normalized_path.clone()) {
            diagnostics.push(project_error(
                "duplicate_dependency_path",
                format!(
                    "manifest `dependencies` references path `{normalized_path}` more than once"
                ),
                Some(location_for_path(
                    manifest_path,
                    None,
                    Some(format!("dependencies.{alias}.path")),
                )),
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
            let Some(imported_module) = project.module(import) else {
                diagnostics.push(project_error(
                    "unresolved_import",
                    format!("module `{}` imports unresolved module `{import}`", module.module),
                    Some(location_for_relative_path(
                        &module.relative_path,
                        Some(module.module.clone()),
                        Some(import.clone()),
                    )),
                ));
                continue;
            };

            if !module_can_access_package(project, &module.package, &imported_module.package) {
                let manifest_path = project
                    .package(&module.package)
                    .map(|package| package.manifest_path.clone())
                    .unwrap_or_else(|| project.manifest_path.clone());
                diagnostics.push(
                    project_error(
                        "undeclared_dependency_import",
                        format!(
                            "module `{}` imports `{import}` from package `{}` without declaring it as a direct dependency",
                            module.module, imported_module.package
                        ),
                        Some(location_for_relative_path(
                            &module.relative_path,
                            Some(module.module.clone()),
                            Some(import.clone()),
                        )),
                    )
                    .with_related(
                        format!(
                            "declare package `{}` in `[dependencies]` of package `{}`",
                            imported_module.package, module.package
                        ),
                        Some(location_for_path(&manifest_path, None, None)),
                    ),
                );
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

fn resolve_import_bindings(
    project: &Project,
    module: &ProjectModule,
) -> std::result::Result<(Vec<ImportedType>, Vec<ImportedFunction>), Vec<Diagnostic>> {
    let mut types = Vec::new();
    let mut functions = Vec::new();
    let mut diagnostics = Vec::new();

    for import in &module.program.imports {
        let Some(imported_module) = project.module(&import.module) else {
            continue;
        };

        if !import.items.is_empty() {
            resolve_selective_import(
                module,
                imported_module,
                import,
                &mut types,
                &mut functions,
                &mut diagnostics,
            );
            continue;
        }

        if let Some(alias) = &import.alias {
            import_module_namespace(module, imported_module, alias, &mut types, &mut functions);
        } else {
            import_module_members(module, imported_module, &mut types, &mut functions);
        }
    }

    if diagnostics.is_empty() {
        Ok((types, functions))
    } else {
        Err(diagnostics)
    }
}

fn resolve_selective_import(
    module: &ProjectModule,
    imported_module: &ProjectModule,
    import: &UseDecl,
    types: &mut Vec<ImportedType>,
    functions: &mut Vec<ImportedFunction>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &import.items {
        let local_name = item.alias.clone().unwrap_or_else(|| item.name.clone());
        let mut matched_any = false;
        let mut imported_any = false;

        for function in &imported_module.program.functions {
            if function.name != item.name {
                continue;
            }
            matched_any = true;
            if symbol_is_visible(module, imported_module, function.visibility) {
                imported_any = true;
                functions.push(ImportedFunction {
                    name: local_name.clone(),
                    module: imported_module.module.clone(),
                    params: function.params.clone(),
                    return_type: function.return_type.clone(),
                });
            }
        }

        for schema in &imported_module.program.schemas {
            if schema.name != item.name {
                continue;
            }
            matched_any = true;
            if symbol_is_visible(module, imported_module, schema.visibility) {
                imported_any = true;
                types.push(ImportedType {
                    name: local_name.clone(),
                    module: imported_module.module.clone(),
                    kind: ImportedTypeKind::Schema,
                });
            }
        }

        for enum_decl in &imported_module.program.enums {
            if enum_decl.name != item.name {
                continue;
            }
            matched_any = true;
            if symbol_is_visible(module, imported_module, enum_decl.visibility) {
                imported_any = true;
                types.push(ImportedType {
                    name: local_name.clone(),
                    module: imported_module.module.clone(),
                    kind: ImportedTypeKind::Enum,
                });
            }
        }

        for alias in &imported_module.program.type_aliases {
            if alias.name != item.name {
                continue;
            }
            matched_any = true;
            if symbol_is_visible(module, imported_module, alias.visibility) {
                imported_any = true;
                types.push(ImportedType {
                    name: local_name.clone(),
                    module: imported_module.module.clone(),
                    kind: ImportedTypeKind::Alias(alias.target.clone()),
                });
            }
        }

        if !matched_any {
            diagnostics.push(project_error(
                "unresolved_import_item",
                format!(
                    "module `{}` imports `{}` from `{}` but that symbol does not exist",
                    module.module, item.name, import.module
                ),
                Some(location_for_relative_path(
                    &module.relative_path,
                    Some(module.module.clone()),
                    Some(item.name.clone()),
                )),
            ));
        } else if !imported_any {
            diagnostics.push(
                project_error(
                    "private_import",
                    format!(
                        "module `{}` cannot import internal symbol `{}` from package `{}`",
                        module.module, item.name, imported_module.package
                    ),
                    Some(location_for_relative_path(
                        &module.relative_path,
                        Some(module.module.clone()),
                        Some(item.name.clone()),
                    )),
                )
                .with_related(
                    "mark the symbol `pub` or move the import into the same package",
                    Some(location_for_relative_path(
                        &imported_module.relative_path,
                        Some(imported_module.module.clone()),
                        Some(item.name.clone()),
                    )),
                ),
            );
        }
    }
}

fn import_module_namespace(
    module: &ProjectModule,
    imported_module: &ProjectModule,
    alias: &str,
    types: &mut Vec<ImportedType>,
    functions: &mut Vec<ImportedFunction>,
) {
    for function in &imported_module.program.functions {
        if symbol_is_visible(module, imported_module, function.visibility) {
            functions.push(ImportedFunction {
                name: format!("{alias}::{}", function.name),
                module: imported_module.module.clone(),
                params: function.params.clone(),
                return_type: function.return_type.clone(),
            });
        }
    }

    for schema in &imported_module.program.schemas {
        if symbol_is_visible(module, imported_module, schema.visibility) {
            types.push(ImportedType {
                name: format!("{alias}::{}", schema.name),
                module: imported_module.module.clone(),
                kind: ImportedTypeKind::Schema,
            });
        }
    }

    for enum_decl in &imported_module.program.enums {
        if symbol_is_visible(module, imported_module, enum_decl.visibility) {
            types.push(ImportedType {
                name: format!("{alias}::{}", enum_decl.name),
                module: imported_module.module.clone(),
                kind: ImportedTypeKind::Enum,
            });
        }
    }

    for alias_decl in &imported_module.program.type_aliases {
        if symbol_is_visible(module, imported_module, alias_decl.visibility) {
            types.push(ImportedType {
                name: format!("{alias}::{}", alias_decl.name),
                module: imported_module.module.clone(),
                kind: ImportedTypeKind::Alias(alias_decl.target.clone()),
            });
        }
    }
}

fn import_module_members(
    module: &ProjectModule,
    imported_module: &ProjectModule,
    types: &mut Vec<ImportedType>,
    functions: &mut Vec<ImportedFunction>,
) {
    for function in &imported_module.program.functions {
        if symbol_is_visible(module, imported_module, function.visibility) {
            functions.push(ImportedFunction {
                name: function.name.clone(),
                module: imported_module.module.clone(),
                params: function.params.clone(),
                return_type: function.return_type.clone(),
            });
        }
    }

    for schema in &imported_module.program.schemas {
        if symbol_is_visible(module, imported_module, schema.visibility) {
            types.push(ImportedType {
                name: schema.name.clone(),
                module: imported_module.module.clone(),
                kind: ImportedTypeKind::Schema,
            });
        }
    }

    for enum_decl in &imported_module.program.enums {
        if symbol_is_visible(module, imported_module, enum_decl.visibility) {
            types.push(ImportedType {
                name: enum_decl.name.clone(),
                module: imported_module.module.clone(),
                kind: ImportedTypeKind::Enum,
            });
        }
    }

    for alias_decl in &imported_module.program.type_aliases {
        if symbol_is_visible(module, imported_module, alias_decl.visibility) {
            types.push(ImportedType {
                name: alias_decl.name.clone(),
                module: imported_module.module.clone(),
                kind: ImportedTypeKind::Alias(alias_decl.target.clone()),
            });
        }
    }
}

fn symbol_is_visible(
    importing_module: &ProjectModule,
    imported_module: &ProjectModule,
    visibility: Visibility,
) -> bool {
    importing_module.package == imported_module.package || matches!(visibility, Visibility::Public)
}

fn module_can_access_package(project: &Project, package: &str, imported_package: &str) -> bool {
    package == imported_package
        || project
            .package_dependencies(package)
            .iter()
            .any(|dependency| dependency.package == imported_package)
}

fn relative_path_from(base: &Path, path: &Path) -> String {
    let Ok(base) = base.canonicalize() else {
        return normalize_relative_path(&path.to_string_lossy());
    };
    let Ok(path) = path.canonicalize() else {
        return normalize_relative_path(&path.to_string_lossy());
    };

    let base_components = base.components().collect::<Vec<_>>();
    let path_components = path.components().collect::<Vec<_>>();

    let mut shared = 0;
    while shared < base_components.len()
        && shared < path_components.len()
        && base_components[shared] == path_components[shared]
    {
        shared += 1;
    }

    if shared == 0 && matches!(base_components.first(), Some(Component::Prefix(_))) {
        return normalize_relative_path(&path.to_string_lossy());
    }

    let mut relative = PathBuf::new();
    for _ in shared..base_components.len() {
        relative.push("..");
    }
    for component in &path_components[shared..] {
        relative.push(component.as_os_str());
    }

    if relative.as_os_str().is_empty() {
        ".".to_string()
    } else {
        normalize_relative_path(&relative.to_string_lossy())
    }
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

    #[test]
    fn loads_project_with_local_dependency() {
        let tempdir = tempdir().expect("tempdir should exist");
        let shared_root = tempdir.path().join("shared_rules");
        let app_root = tempdir.path().join("reuse_app");
        fs::create_dir_all(shared_root.join("src")).expect("shared src should create");
        fs::create_dir_all(app_root.join("src")).expect("app src should create");

        fs::write(
            shared_root.join("kairos.toml"),
            "[package]\nname = \"shared_rules\"\nversion = \"2.0.0\"\nentry = \"src/main.kai\"\n",
        )
        .expect("shared manifest should write");
        fs::write(
            shared_root.join("src/main.kai"),
            "module shared.rules;\n\nfn main() -> Str\ndescribe \"shared\"\ntags [\"shared\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"ok\";\n}\n",
        )
        .expect("shared main should write");
        fs::write(
            shared_root.join("src/api.kai"),
            "module shared.rules.api;\n\npub fn classify(score: Int) -> Str\ndescribe \"shared\"\ntags [\"shared\"]\nrequires [score >= 0, score <= 100]\nensures [len(result) > 0]\n{\n  if score >= 50 {\n    return \"MEDIUM\";\n  } else {\n    return \"LOW\";\n  }\n}\n",
        )
        .expect("shared api should write");

        fs::write(
            app_root.join("kairos.toml"),
            "[package]\nname = \"reuse_app\"\nversion = \"2.0.0\"\nentry = \"src/main.kai\"\n\n[dependencies]\nshared_rules = { path = \"../shared_rules\" }\n",
        )
        .expect("app manifest should write");
        fs::write(
            app_root.join("src/main.kai"),
            "module demo.reuse_app;\nuse shared.rules.api as rules_api;\n\nfn main() -> Str\ndescribe \"app\"\ntags [\"app\"]\nrequires []\nensures [len(result) > 0]\n{\n  return rules_api::classify(72);\n}\n",
        )
        .expect("app main should write");

        let project = load_project(&app_root).expect("project should load");
        assert_eq!(project.packages.len(), 2);
        assert_eq!(project.modules.len(), 3);
        assert_eq!(project.package_dependencies("reuse_app").len(), 1);

        let analyzed = analyze_project(&project).expect("project should analyze");
        assert!(analyzed.warnings.is_empty());
    }

    #[test]
    fn rejects_private_import_from_local_dependency() {
        let tempdir = tempdir().expect("tempdir should exist");
        let shared_root = tempdir.path().join("shared_rules");
        let app_root = tempdir.path().join("reuse_app");
        fs::create_dir_all(shared_root.join("src")).expect("shared src should create");
        fs::create_dir_all(app_root.join("src")).expect("app src should create");

        fs::write(
            shared_root.join("kairos.toml"),
            "[package]\nname = \"shared_rules\"\nversion = \"2.0.0\"\nentry = \"src/main.kai\"\n",
        )
        .expect("shared manifest should write");
        fs::write(
            shared_root.join("src/main.kai"),
            "module shared.rules;\n\nfn main() -> Str\ndescribe \"shared\"\ntags [\"shared\"]\nrequires []\nensures [len(result) > 0]\n{\n  return \"ok\";\n}\n",
        )
        .expect("shared main should write");
        fs::write(
            shared_root.join("src/api.kai"),
            "module shared.rules.api;\n\nfn internal_label(score: Int) -> Str\ndescribe \"shared\"\ntags [\"internal\"]\nrequires [score >= 0, score <= 100]\nensures [len(result) > 0]\n{\n  return \"LOW\";\n}\n",
        )
        .expect("shared api should write");

        fs::write(
            app_root.join("kairos.toml"),
            "[package]\nname = \"reuse_app\"\nversion = \"2.0.0\"\nentry = \"src/main.kai\"\n\n[dependencies]\nshared_rules = { path = \"../shared_rules\" }\n",
        )
        .expect("app manifest should write");
        fs::write(
            app_root.join("src/main.kai"),
            "module demo.reuse_app;\nuse shared.rules.api::{internal_label};\n\nfn main() -> Str\ndescribe \"app\"\ntags [\"app\"]\nrequires []\nensures [len(result) > 0]\n{\n  return internal_label(10);\n}\n",
        )
        .expect("app main should write");

        let project = load_project(&app_root).expect("project should load");
        let error = analyze_project(&project).expect_err("analysis should fail");
        assert!(error.to_string().contains("private_import"));
    }
}
