# Kairos Roadmap

## Current state

The repository has completed the v0.5 terminal-native foundation:

- Rust workspace builds cleanly
- project-aware CLI, formatter, and interpreter
- multi-file loading through `kairos.toml`
- deterministic module resolution with import-cycle and duplicate-module checks
- stable AST, KIR, prompt, and diagnostic outputs
- expanded deterministic stdlib
- Kairos shell with reload and watch support
- project scaffolding through `new` and `init`
- bundled examples and CLI integration coverage

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

## v0.5: Terminal-native workflow

Completed:

- `kairos shell [path]`
- Kairos-branded shell startup banner and status blocks
- shell commands for status, modules, prompt, run, reload, and load
- session watch mode with `:watch` / `:unwatch`
- project scaffolding through `kairos new` and `kairos init`
- starter templates for `default`, `briefing`, and `rules`

Still intentionally out of scope in v0.5:

- package registry or remote dependencies
- full-screen TUI complexity
- auto-run on save by default
- JIT, GC redesign, async runtime, or networking
- user-program file I/O

## Next priority: v0.6

Recommended next:

- richer semantic spans for diagnostics
- selective imports and explicit visibility rules
- clearer object/record type shapes
- persisted shell history and small shell niceties
- snapshot coverage for AST/KIR/diagnostic stability

## Later phases

### Tooling

- language server protocol support
- VS Code integration
- formatter heuristics for larger nested expressions
- machine-readable diagnostic schema documentation

### Ecosystem

- package discovery beyond one local project
- package versioning
- richer data-validation standard library features
- documentation site generation from KIR and prompt artifacts

### Additional backends

- Python or TypeScript export if the KIR contract proves stable
- optional code generation backends
- LLVM only if the language semantics and KIR are mature enough to justify it
