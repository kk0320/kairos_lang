# Kairos CLI

The CLI binary is named `kairos`.

During local development, the simplest way to invoke it is:

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

If a `.kai` file lives inside a Kairos project, `check`, `ir`, `prompt`, and `run` validate it with full project/module resolution rather than treating it as isolated source.

## Commands

### `kairos check <file-or-project> [--json]`

Parse and semantically validate a standalone file or a whole project.

Examples:

```powershell
cargo run --bin kairos -- check examples\hello_context\src\main.kai
cargo run --bin kairos -- check examples\assistant_briefing
cargo run --bin kairos -- check examples\assistant_briefing --json
```

JSON diagnostics are stable and include:

- `code`
- `severity`
- `message`
- `location`
- `related`

### `kairos fmt <file-or-project> [--check] [--stdout]`

Apply canonical formatting.

- default behavior rewrites the file or project in place
- `--check` fails when formatting changes would be needed
- `--stdout` prints formatted source without rewriting and is only valid for single-file input

Examples:

```powershell
cargo run --bin kairos -- fmt examples\hello_context\src\main.kai --check
cargo run --bin kairos -- fmt examples\hello_context\src\main.kai --stdout
cargo run --bin kairos -- fmt examples\assistant_briefing --check
```

### `kairos ast <file-or-project> --json`

Print AST JSON.

- file input returns a single-module AST
- project input returns manifest plus discovered module ASTs

```powershell
cargo run --bin kairos -- ast examples\hello_context\src\main.kai --json
cargo run --bin kairos -- ast examples\assistant_briefing --json
```

### `kairos ir <file-or-project> --json`

Print KIR JSON.

- file input returns module KIR
- project input returns project KIR
- a file inside a project is validated with project resolution, then emits the focused module KIR

KIR includes normalized context data, type declarations, metadata, bodies, imports, and a SHA-256 source hash. Project KIR adds package metadata and a stable project hash.

```powershell
cargo run --bin kairos -- ir examples\video_context\src\main.kai --json
cargo run --bin kairos -- ir examples\decision_bundle --json
```

### `kairos prompt <file-or-project>`

Print deterministic markdown for downstream LLM/system-context use.

- file input returns module prompt context
- project input returns project prompt context with package and module sections

```powershell
cargo run --bin kairos -- prompt examples\video_context\src\main.kai
cargo run --bin kairos -- prompt examples\assistant_briefing
```

### `kairos run <file-or-project> [--function <name>] [--arg <value> ...] [--json]`

Execute the deterministic interpreter subset.

Argument parsing rules:

- JSON values such as `72`, `true`, `"hello"`, `[1, 2]`, and `{"ok": true}` are accepted
- bare non-JSON text is treated as a string

Entrypoint behavior:

1. `--function` runs the named function
2. otherwise `main()` in the focused module is used when present and zero-argument
3. otherwise all zero-argument functions in the focused module run in declaration order

For project runs, `--function` also accepts `module.path::function_name` when you want to target a non-entry module explicitly.

Examples:

```powershell
cargo run --bin kairos -- run examples\hello_context\src\main.kai --json
cargo run --bin kairos -- run examples\risk_rules\src\main.kai --function classify --arg 72 --json
cargo run --bin kairos -- run examples\decision_bundle --function classify --arg 72 --json
cargo run --bin kairos -- run examples\stdlib_playbook --json
```

## Exit behavior

- parse, semantic, formatting, and runtime failures return non-zero exit codes
- `check --json`, `ast --json`, `ir --json`, and `run --json` produce stable JSON output suitable for tooling
- project errors report deterministic module/file context where available
