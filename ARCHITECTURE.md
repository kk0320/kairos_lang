# Kairos Architecture

## Product shape

Kairos 2.0 is an AI-first language platform and terminal-native toolchain for deterministic `.kai` projects.

Its architecture stays intentionally focused:

- explicit local packages
- deterministic module and dependency loading
- stable machine-readable outputs
- human-readable shell and CLI workflows
- no hidden networked/package side effects

## End-to-end pipeline

1. Project discovery
   - resolve a `.kai` file, project directory, or `kairos.toml`
   - find the active manifest when the input belongs to a project
2. Manifest validation
   - validate `package.name`, `package.version`, `package.entry`, `dependencies`, and `build.emit`
   - ensure local paths stay inside the explicit local package model
3. Source discovery
   - treat the parent directory of `package.entry` as the package source root
   - enumerate every `.kai` file under that root in deterministic path order
4. Local dependency loading
   - load sibling local packages through `[dependencies]`
   - detect dependency cycles and duplicate package/module conflicts
5. Parsing
   - lex and parse source into the canonical AST
6. Project resolution
   - index modules by `module` declaration
   - validate unresolved imports, package boundary issues, duplicate modules, and import cycles
7. Semantic analysis
   - validate names, contracts, test signatures, types, imported symbols, visibility, and context metadata
8. Lowering
   - lower analyzed modules/projects into stable KIR
9. Backends
   - AST JSON
   - KIR JSON
   - prompt/context markdown
   - deterministic interpreter execution
   - deterministic project-native tests
   - deterministic doctor reports
   - canonical formatter output
10. Terminal layer
   - human-readable command summaries
   - shell banner/status/help rendering
   - reload/watch notifications
   - scaffolding feedback

## Crate responsibilities

### `kairos-ast`

Defines the canonical syntax tree and shared rendering helpers for expressions, imports, and type references.

### `kairos-parser`

Owns the lexer and parser for the supported Kairos grammar, including:

- `pub`
- `test fn`
- module aliases
- selective imports
- qualified names through `::`

### `kairos-project`

Owns the local project/package model:

- `kairos.toml` loading
- manifest validation
- local path dependency loading
- deterministic source enumeration
- package/module indexing
- unresolved-import, visibility, dependency-boundary, and cycle validation
- project-wide semantic entry preparation

### `kairos-semantic`

Validates the executable and descriptive subset:

- duplicate definitions
- undefined identifiers
- duplicate locals and parameters
- imported symbol ambiguity
- type references
- function contracts
- `test fn` signatures
- context key/value shape
- return types and return-path behavior

Diagnostics stay structured around stable fields:

- `severity`
- `code`
- `message`
- `location`
- `related`

### `kairos-ir`

Defines Kairos IR and lowers analyzed modules/projects into a stable machine-facing contract.

In 2.0, project KIR now also carries:

- package graph information
- dependency metadata
- explicit import binding metadata
- visibility and test flags

### `kairos-interpreter`

Executes the supported deterministic subset:

- literals and identifiers
- `let`
- `return`
- arithmetic/comparison/boolean operators
- `if / else`
- user-defined function calls
- deterministic builtin helpers
- project-aware imported function calls, including alias/selective import resolution

It also enforces `requires` before execution and `ensures` after execution.

### `kairos-formatter`

Prints canonical Kairos source for single files or root packages.

### `kairos-cli`

Provides the public user surface:

- `check`
- `fmt`
- `ast`
- `ir`
- `prompt`
- `run`
- `test`
- `doctor`
- `shell`
- `new`
- `init`

Internally the CLI is split into:

1. `workspace.rs`
   shared loading, selection, diagnostics, doctor/test reporting, and output helpers
2. `presentation.rs`
   shell banners, help text, status blocks, dependency lists, and report rendering
3. `shell.rs`
   interactive session state, commands, reload, watch mode, and dependency introspection
4. `scaffold.rs`
   project bootstrap logic and template generation

## Shell architecture

The shell is intentionally line-oriented rather than full-screen.

That keeps it:

- Windows-friendly
- deterministic
- easy to reason about
- easy to test
- aligned with the existing CLI backend

The shell does not maintain a separate execution model. Commands such as `:check`, `:prompt`, `:ir`, `:run`, `:deps`, `:reload`, and `:watch` call into the same real project/parser/semantic/KIR/runtime layers used by top-level CLI commands.

## Stability boundaries

Kairos 2.0 treats these surfaces as stable:

- CLI command names
- AST JSON structure
- KIR JSON structure
- prompt export structure
- diagnostic JSON field names
- test/doctor JSON field names
- deterministic project and local dependency loading rules
- scaffolding template shape

## Intentional non-goals in 2.0

The current architecture still does not include:

- remote dependencies or registry support
- async runtime features
- user-program networking or filesystem access
- full-screen TUI complexity
- full LSP/editor integration
- broader non-deterministic runtime behavior

Those remain post-2.0 roadmap work rather than partially hidden features.
