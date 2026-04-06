# Kairos CLI

The CLI binary is named `kairos`.

During local development, the most direct invocation is:

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

If a `.kai` file belongs to a Kairos project, project-aware commands will load the surrounding project rather than treating the file as isolated source.

## Commands

### `kairos check <path> [--json]`

Validate a standalone file or whole project.

Examples:

```powershell
cargo run --bin kairos -- check examples\hello_context
cargo run --bin kairos -- check examples\assistant_briefing --json
```

### `kairos fmt <path> [--check] [--stdout]`

Apply canonical formatting.

- rewrites the file/project in place by default
- `--check` fails if changes would be needed
- `--stdout` is only valid for single-file input

### `kairos ast <path> [--json]`

Print AST JSON.

- file input returns a single-module AST
- project input returns the manifest plus discovered module ASTs
- `--json` is retained for compatibility; output is always JSON

### `kairos ir <path> [--json]`

Print stable KIR JSON.

- file input returns module KIR
- project input returns project KIR
- `--json` is retained for compatibility; output is always JSON

### `kairos prompt <path>`

Print deterministic markdown intended for downstream LLM or system-context consumption.

### `kairos run <path> [--function <name>] [--arg <value> ...] [--json]`

Run the deterministic interpreter subset.

Argument parsing:

- JSON values such as `72`, `true`, `"hello"`, `[1, 2]`, and `{"ok": true}` are accepted directly
- bare non-JSON text is treated as a string

Output modes:

- default mode prints a concise human-readable execution summary
- `--json` prints stable machine-readable execution JSON

For project execution, `--function` also accepts `module.path::function_name`.

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

### `kairos init [--template <template>]`

Initialize the current directory as a Kairos project without overwriting existing files.

## JSON behavior

Kairos keeps machine-oriented and human-oriented output separated:

- `check --json` returns stable status/diagnostic JSON
- `ast` and `ir` return stable JSON
- `run --json` returns stable execution JSON
- shell mode stays human-oriented by default

## Exit behavior

- parse, semantic, manifest, formatting, and runtime failures return non-zero exit codes
- validation errors remain structured in JSON mode
- shell mode prints terminal summaries rather than machine-oriented JSON by default
