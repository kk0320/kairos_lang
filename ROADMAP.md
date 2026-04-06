# Kairos Roadmap

## Current state: v1.0

Kairos 1.0 is the first release-ready public baseline.

Completed in 1.0:

- Rust workspace with clean build/test/fmt/clippy flows
- `.kai` parsing, AST, semantics, KIR, formatter, and deterministic interpreter
- project-aware workflows through `kairos.toml`
- deterministic multi-file module loading with `use`
- structured diagnostics
- stable AST JSON, KIR JSON, prompt markdown, and execution JSON outputs
- terminal-native shell with reload and watch
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

## Post-1.0 priorities

### v1.1 candidates

- richer semantic spans and source-mapped diagnostics
- snapshot/golden coverage for AST, KIR, and diagnostics stability
- shell session history and a few quality-of-life shell improvements
- more explicit manifest/schema documentation

### v1.2 candidates

- selective imports and explicit visibility rules
- clearer object/record type modeling
- machine-readable diagnostic schema documentation
- additional deterministic validation-focused stdlib helpers

## Deliberately out of scope after 1.0 unless product direction changes

- package registry
- remote imports
- async runtime
- networking
- user-program file I/O
- full LSP/editor protocol stack
- full-screen TUI shell
- macro-heavy metaprogramming
- broad non-deterministic runtime features

Kairos should grow by sharpening its AI-first deterministic strengths, not by becoming an unfocused general-purpose toolchain all at once.
