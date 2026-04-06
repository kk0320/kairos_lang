# Kairos Architecture

## Product shape

Kairos 1.0 is an AI-first language and terminal-native toolchain for deterministic `.kai` projects.

Its architecture is intentionally narrow:

- explicit local projects
- deterministic module loading
- stable machine-readable outputs
- human-readable shell and CLI workflows
- no hidden networked/package side effects

## End-to-end pipeline

1. Project discovery
   - resolve a `.kai` file, project directory, or `kairos.toml`
   - find the active manifest when the input belongs to a project
2. Manifest validation
   - validate `package.name`, `package.version`, `package.entry`, and `build.emit`
   - ensure the entry path stays inside the local project model
3. Source discovery
   - treat the parent directory of `package.entry` as the source root
   - enumerate every `.kai` file under that root in deterministic path order
4. Parsing
   - lex and parse source into the canonical AST
5. Project resolution
   - index modules by `module` declaration
   - validate unresolved imports, duplicate modules, and import cycles
6. Semantic analysis
   - validate names, contracts, types, imported symbols, and context metadata
7. Lowering
   - lower analyzed modules/projects into stable KIR
8. Backends
   - AST JSON
   - KIR JSON
   - prompt/context markdown
   - deterministic interpreter execution
   - canonical formatter output
9. Terminal layer
   - human-readable command summaries
   - shell banner/status/help rendering
   - reload/watch notifications
   - scaffolding feedback

## Crate responsibilities

### `kairos-ast`

Defines the canonical syntax tree and shared rendering helpers for expressions and type references.

### `kairos-parser`

Owns the lexer and parser for the supported Kairos grammar.

### `kairos-project`

Owns the local project model:

- `kairos.toml` loading
- manifest validation
- source-root discovery
- deterministic source enumeration
- module indexing
- unresolved-import and cycle validation
- project-wide semantic entry preparation

### `kairos-semantic`

Validates the executable and descriptive subset:

- duplicate definitions
- undefined identifiers
- duplicate locals and parameters
- duplicate imported names
- type references
- function contracts
- context key/value shape
- return types and return-path behavior

Diagnostics are structured around stable fields:

- `severity`
- `code`
- `message`
- `location`
- `related`

### `kairos-ir`

Defines Kairos IR and lowers analyzed modules/projects into a stable machine-facing contract.

### `kairos-interpreter`

Executes the supported deterministic subset:

- literals and identifiers
- `let`
- `return`
- arithmetic/comparison/boolean operators
- `if / else`
- user-defined function calls
- deterministic builtin helpers
- project-aware imported function calls

It also enforces `requires` before execution and `ensures` after execution.

### `kairos-formatter`

Prints canonical Kairos source for single files or whole projects.

### `kairos-cli`

Provides the public user surface:

- `check`
- `fmt`
- `ast`
- `ir`
- `prompt`
- `run`
- `shell`
- `new`
- `init`

Internally the CLI is split into:

1. `workspace.rs`
   shared loading, selection, diagnostics, and output helpers
2. `presentation.rs`
   shell banners, help text, status blocks, and execution rendering
3. `shell.rs`
   interactive session state, commands, reload, and watch mode
4. `scaffold.rs`
   project bootstrap logic and template generation

## Shell architecture

The shell is intentionally line-oriented rather than full-screen.

That keeps it:

- Windows-friendly
- easy to reason about
- deterministic
- easy to test
- aligned with the existing CLI backend

The shell does not maintain a separate execution model. Commands such as `:check`, `:prompt`, `:ir`, `:reload`, and `:run` call into the same real project/parser/semantic/KIR/runtime layers used by top-level CLI commands.

Watch mode is session-only and built around a small filesystem notification layer. On relevant `.kai` or `kairos.toml` changes, Kairos reloads and revalidates the current target and prints a concise terminal update.

## Project model

Kairos 1.0 intentionally keeps the project model explicit and local:

- one `kairos.toml` per local project
- one source root derived from the entry file
- whole-module imports through `use`
- deterministic local resolution only

This is a product decision as much as a technical one. Kairos prioritizes trustworthy local behavior over implicit package resolution or remote dependency features.

## Stability boundaries

Kairos 1.0 treats these surfaces as stable enough for public usage:

- CLI command names
- AST JSON structure
- KIR JSON structure
- prompt export structure
- diagnostic JSON field names
- deterministic project loading rules
- scaffolding templates and local manifest shape

## Intentional non-goals in 1.0

The current architecture does not include:

- remote dependencies or package registry support
- selective imports or visibility modifiers
- user-program networking or filesystem access
- async runtime features
- full-screen TUI complexity
- full LSP/editor integration
- general-purpose system scripting ambitions

Those remain post-1.0 roadmap work rather than partially hidden features.
