# Kairos Roadmap

## Current state

The repository has completed the v0.2 practical language-core foundation:

- Rust workspace builds cleanly
- project-aware CLI, formatter, and interpreter
- multi-file loading through `kairos.toml`
- deterministic module resolution with import-cycle and duplicate-module checks
- stable AST, KIR, prompt, and diagnostic outputs
- expanded deterministic stdlib for strings, lists, objects, booleans, and numeric rules
- bundled single-file and multi-file examples
- CI-compatible `fmt`, `clippy`, `build`, and `test` workflows

## v0.1: MVP foundation

Completed:

- lexer and parser for `.kai`
- canonical AST
- semantic validation for the initial subset
- deterministic formatting
- AST JSON export
- KIR JSON export
- prompt export
- interpreter execution for the deterministic subset

## v0.2: Project-aware foundation

Completed:

- `kairos.toml` project loading
- multi-file module resolution
- unresolved-import, duplicate-module, and import-cycle diagnostics
- project-aware `check`, `ir`, `prompt`, `fmt`, and `run`
- project KIR and project prompt output
- expanded stdlib for practical rule execution
- example projects for AI context, rules, and stdlib usage

Still intentionally out of scope in v0.2:

- package registry or remote dependencies
- selective imports and visibility keywords
- rich semantic spans for every diagnostic
- advanced type inference and generics beyond the existing practical subset
- async, networking, filesystem APIs, or host-side side effects for user programs

## Next priority: v0.3

Recommended next:

- richer source spans for semantic diagnostics
- selective imports and explicit visibility rules
- clearer type shapes for objects and records
- project-level test fixtures and snapshot coverage for AST/KIR/diagnostics
- tree-sitter grammar and syntax-highlighting support

## Later phases

### Tooling

- language server protocol support
- VS Code integration
- formatter heuristics for larger nested expressions
- machine-readable diagnostic schema documentation

### Ecosystem

- package discovery beyond one local project
- package versioning
- standard library depth for data transformation and validation
- documentation site generation from KIR and prompt artifacts

### Additional backends

- Python or TypeScript export if the KIR contract proves stable
- optional code generation backends
- LLVM only if the language semantics and KIR are mature enough to justify it
