# Kairos

Kairos is a Rust-based programming language and tooling workspace for `.kai` files.

Tagline: *Code the right answer at the right moment.*

Kairos is intentionally AI-first. The language is designed so source code stays readable to humans, explicit for downstream LLM systems, and stable when exported as machine-readable structures.

## Current status

Kairos v0.2 is a working, reviewable foundation for small AI-first projects:

- lexer and recursive-descent parser for `.kai`
- canonical AST with stable JSON export
- project loading via `kairos.toml`
- deterministic multi-file module resolution with `use`
- semantic analysis with structured diagnostics
- KIR lowering for modules and whole projects
- deterministic prompt export for downstream LLM workflows
- canonical formatter for files and project roots
- deterministic interpreter with contract checks
- CLI integration tests against bundled examples and project fixtures

## Quick start

The workspace builds a CLI binary named `kairos`.

```powershell
cargo build --workspace
cargo test --workspace

cargo run --bin kairos -- check examples\assistant_briefing
cargo run --bin kairos -- check examples\hello_context\src\main.kai --json
cargo run --bin kairos -- ast examples\assistant_briefing --json
cargo run --bin kairos -- ir examples\decision_bundle --json
cargo run --bin kairos -- prompt examples\assistant_briefing
cargo run --bin kairos -- fmt examples\assistant_briefing --check
cargo run --bin kairos -- run examples\decision_bundle --function classify --arg 72 --json
cargo run --bin kairos -- run examples\stdlib_playbook --json
```

## Project model

Kairos projects are rooted by `kairos.toml`.

```toml
[package]
name = "assistant_briefing"
version = "0.2.0"
entry = "src/main.kai"

[build]
emit = ["ast", "ir", "prompt"]
```

Current v0.2 rules:

- the entry file must point to a `.kai` source file
- the parent directory of the entry file is treated as the project source root
- every `.kai` file under that source root is loaded deterministically
- modules are resolved by `module` declaration and imported with `use demo.shared.text;`
- duplicate module names, unresolved imports, and import cycles are hard errors

Imported functions and types are brought into scope by module name. Kairos v0.2 does not yet have selective imports or explicit visibility keywords.

## Supported CLI

Kairos supports these primary flows:

- `kairos check <file-or-project> [--json]`
- `kairos fmt <file-or-project> [--check] [--stdout]`
- `kairos ast <file-or-project> --json`
- `kairos ir <file-or-project> --json`
- `kairos prompt <file-or-project>`
- `kairos run <file-or-project> [--function <name>] [--arg <value> ...] [--json]`

Key behavior:

- `check`, `ir`, `prompt`, and `run` are project-aware when the input is a project root or a file inside a project
- `ast` prints file AST for a file input and project AST for a project input
- `fmt` formats one file or every discovered module in a project
- `run` executes `main()` when available, otherwise all zero-argument functions in the selected module, unless `--function` is provided
- project runs also accept `module.path::function_name` for explicit cross-module entry selection

## Supported language subset

The current implementation supports:

- `module` and `use`
- `context { ... }`
- `schema`, `enum`, and `type`
- `fn` declarations with `describe`, `tags`, `requires`, and `ensures`
- literals, identifiers, calls, lists, objects, and binary expressions
- `let`, `return`, `if`, and `else if`
- project-aware imports across multiple `.kai` files

The practical deterministic stdlib in v0.2 includes:

- string helpers: `len`, `concat`, `contains`, `starts_with`, `ends_with`, `trim`, `upper`, `lower`
- list helpers: `join`, `first`, `last`, `all`, `any`
- object helpers: `has_key`, `get_str`, `get_int`, `keys`
- numeric helpers: `abs`, `min`, `max`, `clamp`

Semantic validation currently enforces:

- duplicate type, function, field, variant, module, and local binding detection
- duplicate parameter and context-key detection
- unresolved identifiers, functions, modules, and imports
- import ambiguity for conflicting imported names
- type existence checks for declared types
- boolean contract checks for `requires` and `ensures`
- malformed context values
- return-type checks and full-path return validation
- stable diagnostic payloads with `code`, `severity`, `message`, `location`, and related notes

Unknown `context` keys are still accepted with warnings so custom metadata remains practical without inventing undocumented syntax.

## Example projects

- `examples/hello_context`: smallest single-module smoke test
- `examples/video_context`: type declarations and prompt export
- `examples/risk_rules`: deterministic single-file rule execution
- `examples/assistant_briefing`: multi-file AI-context project
- `examples/decision_bundle`: multi-file rules/decision engine project
- `examples/stdlib_playbook`: multi-file stdlib demonstration

## Build and validation

```powershell
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

These commands pass in the current repository state.

## Repository layout

```text
crates/
  kairos-ast
  kairos-parser
  kairos-project
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

- source discovery is limited to one project source root per manifest
- no selective imports, visibility modifiers, or package registry
- no mutation-heavy execution model
- no networking, filesystem access, subprocesses, randomness, or wall-clock APIs for user programs
- no advanced type inference, macros, async runtime, LLVM backend, or editor protocol layer yet
- semantic diagnostics include file/module/symbol context and parse spans, but not full rich AST spans for every semantic error yet

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [ROADMAP.md](ROADMAP.md)
- [docs/cli.md](docs/cli.md)
- [docs/language-overview.md](docs/language-overview.md)
- [docs/syntax.md](docs/syntax.md)
- [docs/projects.md](docs/projects.md)

## License

Kairos is licensed under MIT. See [LICENSE](LICENSE).
