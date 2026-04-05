# Kairos

Kairos is a Rust-based programming language and tooling workspace for `.kai` files.

Tagline: *Code the right answer at the right moment.*

Kairos is intentionally AI-first. The language is designed so source code stays readable to humans, explicit for downstream LLM systems, and stable when exported as machine-readable structures.

## Current status

The repository now includes a working single-file Kairos pipeline:

- lexer and recursive-descent parser for `.kai`
- canonical AST in Rust with stable JSON export
- semantic analysis with duplicate-definition checks, scope validation, undefined identifier detection, basic type checks, and metadata validation
- KIR lowering with stable JSON output and SHA-256 source hashes
- deterministic prompt export for LLM context pipelines
- canonical formatter for the supported syntax
- minimal interpreter for the deterministic subset, including `requires` and `ensures` checks
- CLI integration tests against bundled examples

## Supported CLI

The workspace builds a CLI binary named `kairos`.

```powershell
cargo run --bin kairos -- check examples\hello_context\src\main.kai
cargo run --bin kairos -- ast examples\hello_context\src\main.kai --json
cargo run --bin kairos -- ir examples\video_context\src\main.kai --json
cargo run --bin kairos -- prompt examples\video_context\src\main.kai
cargo run --bin kairos -- fmt examples\hello_context\src\main.kai --check
cargo run --bin kairos -- run examples\risk_rules\src\main.kai --function classify --arg 72 --json
```

CLI behavior:

- `kairos check <file> [--json]` parses and validates a file
- `kairos ast <file> --json` prints AST JSON
- `kairos ir <file> --json` prints KIR JSON
- `kairos prompt <file>` prints deterministic markdown for LLM/system-context use
- `kairos fmt <file>` rewrites the file in canonical style
- `kairos fmt <file> --check` fails if formatting changes would be applied
- `kairos fmt <file> --stdout` prints formatted source instead of rewriting
- `kairos run <file> [--function <name>] [--arg <value> ...] [--json]` runs the interpreter subset

If `run` is called without `--function`, Kairos executes `main()` when present, otherwise all zero-argument functions in declaration order. Files with only parameterized functions require `--function`.

## Supported language subset

The implemented subset matches the published single-file MVP direction:

- `module` and `use`
- `context { ... }`
- `schema`, `enum`, and `type`
- `fn` declarations with `describe`, `tags`, `requires`, and `ensures`
- literals, identifiers, calls, lists, objects, and binary expressions
- `let`, `return`, `if`, and `else if`
- builtin runtime helpers: `len`, `concat`, `abs`, `min`, `max`

Semantic validation currently enforces:

- duplicate type, function, field, variant, and local binding detection
- duplicate parameter and context-key detection
- undefined identifiers and unknown function calls
- type existence checks for declared types
- boolean contract checks for `requires` and `ensures`
- return-type checks and full-path return validation
- string-only `tags`
- constant-only `context` values

Unknown `context` keys are currently accepted with warnings so the repo stays practical without inventing undocumented syntax for custom keys.

## Example

```kai
module demo.hello_context;

context {
  goal: "Provide the smallest valid Kairos example";
  audience: "LLM";
  domain: "demo";
  assumptions: [
    "This file is used for smoke testing.",
  ];
}

fn hello() -> Str
describe "Return a static greeting"
tags ["demo", "hello"]
requires []
ensures [len(result) > 0]
{
  return "Hello from Kairos";
}
```

## Build and validation

```powershell
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

These commands pass in the current repository state.

## Repository layout

```text
crates/
  kairos-ast
  kairos-parser
  kairos-semantic
  kairos-ir
  kairos-interpreter
  kairos-formatter
  kairos-cli
docs/
examples/
specs/
tests/
```

## Limitations

Kairos is still intentionally narrow in this phase:

- single-file modules only
- no package resolution or multi-file compilation
- no mutation-heavy execution model
- no networking, filesystem access, subprocesses, randomness, or wall-clock APIs
- no LLVM backend, LSP, or editor plugin in this repository phase

## License

Kairos is licensed under MIT. See [LICENSE](LICENSE).
