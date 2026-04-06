# Kairos Roadmap

## Current state: v2.0

Kairos 2.0 is the current language-platform baseline.

Completed in 2.0:

- Rust workspace with clean build/test/fmt/clippy flows
- `.kai` parsing, AST, semantics, KIR, formatter, and deterministic interpreter
- project-aware workflows through `kairos.toml`
- deterministic multi-file module loading
- `pub` visibility, selective imports, and module aliases
- local path-based package reuse through `[dependencies]`
- structured diagnostics
- stable AST JSON, KIR JSON, prompt markdown, execution JSON, test JSON, and doctor JSON
- terminal-native shell with reload, watch, and dependency introspection
- local project scaffolding through `kairos new` and `kairos init`
- bundled examples and public-facing documentation

## Historical phases

### v0.1

- MVP parser, AST, semantics, formatter, KIR, prompt export, interpreter

### v0.2

- project loading
- multi-file module resolution
- project-aware `check`, `ir`, `prompt`, `fmt`, and `run`

### v0.5

- shell mode
- watch/reload workflows
- scaffolding templates
- terminal presentation layer

### v1.0

- release-ready local toolchain baseline
- installability, public docs, and 1.0 command/help polish

## Post-2.0 priorities

### v2.1 candidates

- richer semantic spans and source-mapped diagnostics
- more explicit visibility diagnostics and import suggestions
- package graph visualization and lightweight generated docs
- broader golden/snapshot protection for AST, KIR, diagnostics, test, and doctor output

### v2.2 candidates

- selective import filtering in `kairos test`
- shell session history
- additional deterministic validation-focused stdlib helpers
- richer record/schema ergonomics

## Deliberately out of scope after 2.0 unless product direction changes

- remote registry
- remote imports
- async runtime
- networking
- user-program file I/O
- full LSP/editor protocol stack
- full-screen TUI shell
- macro-heavy metaprogramming
- broad non-deterministic runtime features

Kairos should keep growing by sharpening its AI-first deterministic strengths, not by becoming an unfocused general-purpose toolchain all at once.
