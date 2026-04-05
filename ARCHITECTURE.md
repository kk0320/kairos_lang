# Kairos Architecture

## Positioning

Kairos is an AI-first language toolchain for deterministic, machine-readable `.kai` programs.

The primary outputs are not just execution artifacts. They are:

- structural AST JSON
- stable KIR JSON
- prompt/context markdown for downstream LLM systems
- deterministic interpreter results

## Implemented pipeline

1. Lexing
   - tokenizes `.kai` source
   - supports comments, strings, numeric literals, keywords, operators, and punctuation
2. Parsing
   - recursive-descent parser over the single-file Kairos subset
   - produces the canonical AST in `kairos-ast`
3. Semantic analysis
   - validates symbols, metadata, context values, return paths, and a practical type subset
4. Lowering
   - lowers AST into KIR with stable JSON-friendly structures
5. Backends
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

Owns the lexer and parser for the supported single-file grammar.

### `kairos-semantic`

Validates:

- duplicate definitions
- undefined identifiers
- duplicate locals and parameters
- context key/value shape
- metadata shape
- type references
- return-type and return-path correctness

Warnings are preserved for non-fatal issues such as custom context keys.

### `kairos-ir`

Defines KIR and lowers analyzed programs into a stable machine-facing contract. KIR includes:

- module identity
- imports
- normalized context object
- schemas
- enums
- type aliases
- function signatures and metadata
- lowered statement/expression bodies
- SHA-256 source hash

This crate also renders the deterministic prompt export used by `kairos prompt`.

### `kairos-interpreter`

Executes the supported KIR subset:

- literals and identifiers
- `let`
- `return`
- arithmetic and comparison
- boolean operators
- `if / else`
- user-defined function calls
- tiny builtin library

It also evaluates `requires` before execution and `ensures` after execution.

### `kairos-formatter`

Prints a canonical source representation from the AST. Formatting rules are deterministic and designed for low-noise diffs.

### `kairos-cli`

Provides the user-facing workflow:

- `check`
- `fmt`
- `ast`
- `ir`
- `prompt`
- `run`

## Semantic policy notes

The implementation currently follows the documented intent without inventing new syntax:

- `describe` is required on every function in the supported subset
- `tags` must contain string literals
- `requires` and `ensures` must evaluate to `Bool`
- `context` values must be compile-time constants
- unknown `context` keys are warnings, not hard errors, until an explicit custom-key syntax exists in the language docs

## Execution model

`kairos run` behaves as follows:

1. If `--function` is provided, execute that function with parsed CLI arguments.
2. Otherwise, execute `main()` if it exists and takes no arguments.
3. Otherwise, execute all zero-argument functions in declaration order.
4. If no zero-argument entry exists, fail with a clear error.

This keeps execution deterministic while avoiding an undocumented mandatory entrypoint requirement.

## Non-goals in this phase

The current architecture deliberately excludes:

- multi-file compilation
- package graphs
- external I/O
- async or concurrency
- LLVM/codegen backends
- editor protocol integration

Those remain future roadmap work rather than hidden partial features.
