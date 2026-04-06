# Changelog

## 2.0.0

- added `pub` visibility, selective imports, and import aliases
- added local path-based package reuse through `[dependencies]`
- added first-class `kairos test` and deterministic `test fn`
- added `kairos doctor` with human-readable and JSON health reports
- expanded the shell with dependency introspection through `:deps`
- expanded the deterministic stdlib with `normalize_space`, `count`, `sort`, `unique`, `get_bool`, `get_list`, and `get_obj`
- updated KIR and prompt exports to include package graph and import binding metadata
- added local dependency and test-oriented examples for v2.0

## 1.0.0

- stabilized the Kairos CLI and terminal workflow surface
- normalized versioning across workspace metadata, docs, examples, and generated manifests
- polished `kairos --help` and subcommand help text
- separated human-readable and machine-readable execution output for `kairos run`
- strengthened `kairos.toml` validation for package names, versions, entry paths, and emit targets
- improved project scaffolding messages and generated README content
- refreshed documentation for release-ready public usage
