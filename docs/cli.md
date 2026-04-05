# Kairos CLI

The CLI binary is named `kairos`.

During local development, the simplest way to invoke it is:

```powershell
cargo run --bin kairos -- <command> ...
```

## Commands

### `kairos check <file> [--json]`

Parse and semantically validate a `.kai` file.

Examples:

```powershell
cargo run --bin kairos -- check examples\hello_context\src\main.kai
cargo run --bin kairos -- check examples\hello_context\src\main.kai --json
```

### `kairos fmt <file> [--check] [--stdout]`

Apply canonical formatting.

- default behavior rewrites the file in place
- `--check` fails when formatting changes would be needed
- `--stdout` prints formatted source without rewriting

Examples:

```powershell
cargo run --bin kairos -- fmt examples\hello_context\src\main.kai --check
cargo run --bin kairos -- fmt examples\hello_context\src\main.kai --stdout
```

### `kairos ast <file> --json`

Print AST JSON for the supported `.kai` subset.

```powershell
cargo run --bin kairos -- ast examples\hello_context\src\main.kai --json
```

### `kairos ir <file> --json`

Print KIR JSON. KIR includes normalized context data, type declarations, metadata, bodies, and a SHA-256 source hash.

```powershell
cargo run --bin kairos -- ir examples\video_context\src\main.kai --json
```

### `kairos prompt <file>`

Print deterministic markdown for downstream LLM/system-context use.

```powershell
cargo run --bin kairos -- prompt examples\video_context\src\main.kai
```

### `kairos run <file> [--function <name>] [--arg <value> ...] [--json]`

Execute the deterministic interpreter subset.

Argument parsing rules:

- JSON values such as `72`, `true`, `"hello"`, `[1, 2]`, and `{"ok": true}` are accepted
- bare non-JSON text is treated as a string

Entrypoint behavior:

1. `--function` runs the named function
2. otherwise `main()` is used when present and zero-argument
3. otherwise all zero-argument functions run in declaration order

Examples:

```powershell
cargo run --bin kairos -- run examples\hello_context\src\main.kai --json
cargo run --bin kairos -- run examples\risk_rules\src\main.kai --function classify --arg 72 --json
```

## Exit behavior

- parse, semantic, formatting, and runtime failures return non-zero exit codes
- `check --json`, `ast --json`, `ir --json`, and `run --json` produce stable JSON output suitable for tooling
