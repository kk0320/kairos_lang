# Kairos CLI

The CLI binary is named `kairos`.

During local development:

```powershell
cargo run --bin kairos -- <command> ...
```

After a local install:

```powershell
cargo install --path crates/kairos-cli
kairos --help
```

## Accepted inputs

Kairos commands accept:

- a single `.kai` file
- a project directory containing `kairos.toml`
- a direct path to `kairos.toml`

If a `.kai` file belongs to a Kairos project, project-aware commands load the surrounding project instead of treating the file as isolated source.

## Commands

### `kairos check <path> [--json]`

Validate a standalone file or whole project.

### `kairos fmt <path> [--check] [--stdout]`

Apply canonical formatting.

- rewrites the file in place by default
- rewrites only the root package when project dependencies are present
- `--check` fails if changes would be needed
- `--stdout` is only valid for single-file input

### `kairos ast <path> [--json]`

Print AST JSON.

- file input returns a single-module AST
- project input returns the manifest plus discovered package/module ASTs
- `--json` is retained for compatibility; output is always JSON

### `kairos ir <path> [--json]`

Print stable KIR JSON.

- file input returns module KIR
- project input returns project KIR with package graph and import binding data
- `--json` is retained for compatibility; output is always JSON

### `kairos prompt <path>`

Print deterministic markdown intended for downstream LLM or system-context consumption.

### `kairos run <path> [--function <name>] [--arg <value> ...] [--json]`

Run the deterministic interpreter subset.

Argument parsing:

- JSON values such as `72`, `true`, `"hello"`, `[1, 2]`, and `{"ok": true}` are accepted directly
- bare non-JSON text is treated as a string

For project execution, `--function` also accepts `module.path::function_name`.

### `kairos test <path> [--filter <text>] [--json]`

Discover and run deterministic `test fn` cases.

- standalone file input runs tests from that file
- project input runs tests from the root package only
- `--filter` applies a substring match to the discovered test display names
- `--json` returns stable machine-readable results

### `kairos doctor [path] [--json]`

Inspect project and environment health.

- with a file or project path: validates that target and summarizes package/module/dependency state
- with no path: auto-detects the surrounding project from the current directory
- if no project is detected, Kairos reports a warning with next steps instead of failing noisily

### `kairos shell [path]`

Start the interactive shell.

- with a project path: load that project
- with a `.kai` file: load that file or its surrounding project
- with no path inside a project: auto-detect the project
- with no path outside a project: start unloaded and use `:load`

See [docs/shell.md](shell.md) for the shell command set.

### `kairos new <name> [--template <template>]`

Create a new Kairos project directory.

Templates:

- `default`
- `briefing`
- `rules`

Generated projects include a starter `test fn` and validate immediately.

### `kairos init [--template <template>]`

Initialize the current directory as a Kairos project without overwriting existing files.

## JSON behavior

Kairos keeps machine-oriented and human-oriented output separated:

- `check --json` returns stable status/diagnostic JSON
- `ast` and `ir` return stable JSON
- `run --json` returns stable execution JSON
- `test --json` returns stable test JSON
- `doctor --json` returns stable doctor JSON
- shell mode stays human-oriented by default

## Example flow

```powershell
cargo run --bin kairos -- check examples\package_reuse_demo --json
cargo run --bin kairos -- test examples\package_reuse_demo
cargo run --bin kairos -- doctor examples\package_reuse_demo
cargo run --bin kairos -- prompt examples\package_reuse_demo
cargo run --bin kairos -- run examples\package_reuse_demo --json
```
