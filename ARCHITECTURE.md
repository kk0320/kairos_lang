# Kairos Architecture

## Positioning

Kairos is an AI-first language toolchain for deterministic, machine-readable `.kai` programs.

The primary outputs are not just execution artifacts. They are:

- structural AST JSON
- stable KIR JSON
- structured diagnostics JSON
- prompt/context markdown for downstream LLM systems
- deterministic interpreter results

## Implemented pipeline

1. Project discovery
   - resolves `kairos.toml`
   - determines the source root from the manifest entry path
   - discovers `.kai` files deterministically
2. Lexing
   - tokenizes `.kai` source
   - supports comments, strings, numeric literals, keywords, operators, and punctuation
3. Parsing
   - recursive-descent parser over the Kairos v0.2 subset
   - produces the canonical AST in `kairos-ast`
4. Project resolution
   - builds the module graph
   - validates duplicate modules, unresolved imports, and import cycles
5. Semantic analysis
   - validates symbols, metadata, context values, return paths, imports, and a practical type subset
6. Lowering
   - lowers analyzed modules or whole projects into stable KIR structures
7. Backends
   - CLI check output
   - AST JSON
   - KIR JSON
   - prompt export
   - interpreter execution
   - formatter output

## Crate responsibilities

### `kairos-ast`

Canonical syntax tree definitions plus expression/type rendering helpers used by multiple crates.

### `kairos-parser`

Owns the lexer and parser for the supported Kairos grammar.

### `kairos-project`

Owns project discovery and module graph loading:

- `kairos.toml` parsing
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

Warnings are preserved for non-fatal issues such as custom context keys. Diagnostics carry stable fields for severity, code, message, location, and related notes.

### `kairos-ir`

Defines KIR and lowers analyzed modules or whole projects into a stable machine-facing contract. KIR includes:

- module identity
- imports
- normalized context object
- schemas
- enums
- type aliases
- function signatures and metadata
- lowered statement/expression bodies
- SHA-256 source hash

Project KIR also includes:

- package name and version
- entry file and entry module
- configured emit targets
- stable project hash

This crate also renders the deterministic prompt exports used by `kairos prompt`.

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

Prints a canonical source representation from the AST. Formatting rules are deterministic and designed for low-noise diffs across single files and project roots.

### `kairos-cli`

Provides the user-facing workflow:

- `check`
- `fmt`
- `ast`
- `ir`
- `prompt`
- `run`

The CLI is project-aware:

- project roots and manifest paths load the whole module graph
- `.kai` files inside a project validate and run with project resolution
- file AST remains file-scoped for direct inspection

## Project model

Kairos v0.2 uses a deliberately small project model:

- one `kairos.toml` manifest per project
- one source root, derived from the entry file parent directory
- explicit `use module.path;` imports
- imported types and functions become available by name inside the importing module
- ambiguous imported names are rejected instead of guessed

This keeps project behavior deterministic and easy for tools to reason about.

## Semantic policy notes

The implementation currently follows the documented intent without inventing new syntax:

- `describe` is required on every function in the supported subset
- `tags` must contain string literals
- `requires` and `ensures` must evaluate to `Bool`
- `context` values must be compile-time constants
- unknown `context` keys are warnings, not hard errors, until an explicit custom-key syntax exists in the language docs
- imported module names must resolve deterministically before semantic analysis runs

## Execution model

`kairos run` behaves as follows:

1. Load a standalone module or resolve a project.
2. If `--function` is provided, execute that function with parsed CLI arguments.
3. Otherwise, execute `main()` in the focused module if it exists and takes no arguments.
4. Otherwise, execute all zero-argument functions in the focused module.
5. If no zero-argument entry exists, fail with a clear error.

This keeps execution deterministic while avoiding an undocumented mandatory entrypoint requirement.

## Non-goals in this phase

The current architecture deliberately excludes:

- package registries and external dependency fetching
- selective imports and visibility modifiers
- external I/O
- async or concurrency
- LLVM/codegen backends
- editor protocol integration

Those remain future roadmap work rather than hidden partial features.
