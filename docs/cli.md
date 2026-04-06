# Kairos CLI

The CLI binary is named `kairos`.

During local development, the simplest invocation is:

```powershell
cargo run --bin kairos -- <command> ...
```

## Input resolution

Kairos commands accept either:

- a single `.kai` file
- a project directory containing `kairos.toml`
- a direct path to `kairos.toml`

Behavior depends on the command:

- `check`, `ir`, `prompt`, and `run` are project-aware
- `ast` returns file AST for file inputs and project AST for project inputs
- `fmt` formats one file or every discovered project module
- `shell` auto-detects the surrounding project when started with no path inside a Kairos project directory

If a `.kai` file lives inside a Kairos project, `check`, `ir`, `prompt`, and `run` validate it with full project/module resolution rather than treating it as isolated source.

## Commands

### `kairos check <file-or-project> [--json]`

Parse and semantically validate a standalone file or a whole project.

```powershell
cargo run --bin kairos -- check examples\hello_context\src\main.kai
cargo run --bin kairos -- check examples\assistant_briefing --json
```

JSON diagnostics remain stable and include:

- `code`
- `severity`
- `message`
- `location`
- `related`

### `kairos fmt <file-or-project> [--check] [--stdout]`

Apply canonical formatting.

- default behavior rewrites the file or project in place
- `--check` fails when formatting changes would be needed
- `--stdout` is only valid for single-file input

```powershell
cargo run --bin kairos -- fmt examples\assistant_briefing --check
```

### `kairos ast <file-or-project> --json`

Print AST JSON.

- file input returns a single-module AST
- project input returns manifest plus discovered module ASTs

```powershell
cargo run --bin kairos -- ast examples\assistant_briefing --json
```

### `kairos ir <file-or-project> --json`

Print KIR JSON.

- file input returns module KIR
- project input returns project KIR
- a file inside a project is validated with project resolution, then emits the focused module KIR

```powershell
cargo run --bin kairos -- ir examples\decision_bundle --json
```

### `kairos prompt <file-or-project>`

Print deterministic markdown for downstream LLM/system-context use.

```powershell
cargo run --bin kairos -- prompt examples\assistant_briefing
```

### `kairos run <file-or-project> [--function <name>] [--arg <value> ...] [--json]`

Execute the deterministic interpreter subset.

Argument parsing rules:

- JSON values such as `72`, `true`, `"hello"`, `[1, 2]`, and `{"ok": true}` are accepted
- bare non-JSON text is treated as a string

For project runs, `--function` also accepts `module.path::function_name` when you want to target a non-entry module explicitly.

```powershell
cargo run --bin kairos -- run examples\decision_bundle --function classify --arg 72 --json
```

### `kairos shell [path]`

Start the Kairos interactive shell.

- with a project path: load that project
- with a `.kai` file: load that file or its surrounding project if present
- with no path inside a Kairos project directory: auto-detect the project
- with no path outside a project: start in unloaded mode and use `:load`

```powershell
cargo run --bin kairos -- shell examples\assistant_briefing
cargo run --bin kairos -- shell
```

The shell command set is documented in [docs/shell.md](shell.md).

### `kairos new <name> [--template <template>]`

Create a new Kairos project directory.

Available templates:

- `default`
- `briefing`
- `rules`

```powershell
cargo run --bin kairos -- new demo_project
cargo run --bin kairos -- new briefing_demo --template briefing
```

### `kairos init [--template <template>]`

Initialize the current directory as a Kairos project without overwriting existing files.

```powershell
cargo run --bin kairos -- init
cargo run --bin kairos -- init --template rules
```

## Exit behavior

- parse, semantic, formatting, and runtime failures return non-zero exit codes
- `check --json`, `ast --json`, `ir --json`, and `run --json` produce stable JSON output suitable for tooling
- shell mode is human-oriented and prints terminal summaries instead of machine-oriented JSON by default
