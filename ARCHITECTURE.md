# Kairos Architecture

## Positioning

Kairos is an AI-first language toolchain for deterministic, machine-readable `.kai` programs that now also provides a terminal-native interactive workflow.

The primary outputs are:

- structural AST JSON
- stable KIR JSON
- structured diagnostics JSON
- prompt/context markdown for downstream LLM systems
- deterministic interpreter results
- terminal-native validation and reload feedback

## Implemented pipeline

1. Project discovery
   - resolves `kairos.toml`
   - determines the source root from the manifest entry path
   - discovers `.kai` files deterministically
2. Lexing and parsing
   - tokenizes `.kai` source
   - parses the Kairos subset into the canonical AST
3. Project resolution
   - indexes modules
   - validates unresolved imports, duplicate modules, and import cycles
4. Semantic analysis
   - validates symbols, metadata, context values, imports, and practical type constraints
5. Lowering
   - lowers analyzed modules or whole projects into stable KIR structures
6. Deterministic backends
   - AST JSON
   - KIR JSON
   - prompt export
   - interpreter execution
   - formatter output
7. Terminal workflow
   - CLI command summaries
   - shell startup rendering
   - shell reload and watch notifications
   - project scaffolding

## Crate responsibilities

### `kairos-ast`

Canonical syntax tree definitions plus expression/type rendering helpers used across the workspace.

### `kairos-parser`

Owns the lexer and parser for the supported Kairos grammar.

### `kairos-project`

Owns project discovery and module graph loading:

- `kairos.toml` parsing and basic manifest validation
- source root discovery
- `.kai` file enumeration
- module indexing
- import validation
- cycle detection
- project-wide semantic entry preparation

### `kairos-semantic`

Validates:

- duplicate definitions
- undefined identifiers
- duplicate locals and parameters
- duplicate imported names
- context key/value shape
- metadata shape
- type references
- module-aware symbol imports
- return-type and return-path correctness

Diagnostics carry stable fields for severity, code, message, location, and related notes.

### `kairos-ir`

Defines KIR and lowers analyzed modules or whole projects into a stable machine-facing contract.

KIR includes:

- module identity
- imports
- normalized context object
- schemas
- enums
- type aliases
- function signatures and metadata
- lowered statement/expression bodies
- SHA-256 source hash

Project KIR also includes package metadata, entry information, configured emit targets, and a stable project hash.

### `kairos-interpreter`

Executes the supported KIR subset:

- literals and identifiers
- `let`
- `return`
- arithmetic and comparison
- boolean operators
- `if / else`
- user-defined function calls
- deterministic builtin library
- project-aware imported function calls

It also evaluates `requires` before execution and `ensures` after execution.

### `kairos-formatter`

Prints a canonical source representation from the AST for single files and project roots.

### `kairos-cli`

Provides the user-facing workflow:

- machine-readable commands: `check`, `ast`, `ir`, `prompt`, `fmt`, `run`
- terminal-native shell: `shell`
- scaffolding: `new`, `init`

Internally, the CLI now has three distinct layers:

1. shared workspace loading and output helpers
2. terminal presentation helpers for banner/status/help rendering
3. shell/scaffold logic for interactive and bootstrap workflows

## Shell architecture

Kairos v0.5 keeps the shell deliberately simple:

- line-oriented input via `kairos>`
- no full-screen TUI
- command parser with quoted-argument support
- shared access to the real project/parser/semantic/runtime pipeline
- in-session watch state only

` :reload` and `:check` both use real project reloads instead of fake cached summaries.

` :watch` uses a small file-watching layer to monitor the current project root or standalone file directory, then reloads and revalidates in the same session.

## Scaffolding architecture

`kairos new` and `kairos init` are CLI-layer features, but they validate generated projects with the real loader and semantic pipeline before reporting success.

Templates remain intentionally small:

- `default`
- `briefing`
- `rules`

This keeps scaffolding honest, deterministic, and easy to review.

## Non-goals in this phase

The current architecture still excludes:

- package registries and external dependency fetching
- selective imports and visibility modifiers
- external I/O for user programs
- async or concurrency in the user language
- full-screen TUIs
- LLVM/codegen backends
- editor protocol integration

Those remain future roadmap work rather than hidden partial features.
