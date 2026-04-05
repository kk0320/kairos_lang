# Kairos Roadmap

## Current state

The repository has completed the practical single-file MVP foundation:

- Rust workspace builds cleanly
- lexer and parser for `.kai`
- AST, semantics, KIR lowering, prompt export, formatter, and interpreter
- CLI integration tests against bundled examples
- CI-compatible `fmt`, `clippy`, `build`, and `test` workflows

## Phase 1: Current MVP scope

Completed in this repository phase:

- single-file module parsing
- semantic validation for the supported subset
- deterministic formatting
- AST JSON export
- KIR JSON export
- prompt export
- interpreter execution for the deterministic subset
- example-backed tests

Still intentionally out of scope inside Phase 1:

- multi-file resolution
- package metadata beyond simple example fixtures
- explicit custom-context-key syntax
- richer type system features
- advanced standard library growth

## Phase 2: Tooling

Planned next:

- richer diagnostics with source spans in structured outputs
- language server protocol support
- VS Code integration
- syntax-highlighting and tree-sitter support
- improved formatter heuristics for larger nested expressions

## Phase 3: Ecosystem

- real `kairos.toml` project handling
- multi-file projects
- module resolution
- package discovery and versioning
- richer standard library and documentation site

## Phase 4: Additional backends

- Python or TypeScript export if the KIR contract proves stable
- optional code generation backends
- LLVM only if the language semantics and KIR are mature enough to justify it
