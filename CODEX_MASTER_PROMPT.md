# Codex Master Prompt for Building Kairos

You are building a new programming language repository called **Kairos**.

## Identity
- Language name: `Kairos`
- Source extension: `.kai`
- CLI binary name: `kairos`
- Tagline: `Code the right answer at the right moment`

## Core concept
Kairos is **not** a general-purpose language first.
Kairos is an **AI-first programming language / DSL** for code that must be:
- easy for LLMs to read,
- easy to convert into structured context,
- deterministic to parse,
- explicit about purpose, assumptions, preconditions, and postconditions.

## Architectural mandate
Implement the language as a **Rust workspace** with these crates:
- `kairos-ast`
- `kairos-parser`
- `kairos-semantic`
- `kairos-ir`
- `kairos-interpreter`
- `kairos-formatter`
- `kairos-cli`

Do **not** start with LLVM.
LLVM is a future phase only.

The first release target is **v0.1.0-alpha**.

## Mandatory compiler pipeline
Implement:
1. parsing
2. semantic analysis
3. KIR lowering
4. JSON exporters
5. prompt exporter
6. minimal interpreter

## CLI commands that must exist
- `kairos check <file>`
- `kairos fmt <file>`
- `kairos ast <file> --json`
- `kairos ir <file> --json`
- `kairos prompt <file>`
- `kairos run <file>`

## Language features required in MVP
Top-level:
- module declarations
- use declarations
- context blocks
- schema declarations
- enum declarations
- type aliases
- function declarations

Function metadata:
- `describe "..."`  (required for public/exported functions)
- `tags [...]`
- `requires [...]`
- `ensures [...]`

Statements:
- `let`
- `return`
- `if / else`
- expression statements

Expressions:
- literals
- identifiers
- lists
- object literals
- binary operators
- function calls
- parentheses

## AI-first conventions
Preserve these rules:
- syntax must be low-ambiguity
- formatter must output canonical style
- AST and KIR JSON must be deterministic
- prompt exporter must generate stable markdown
- every symbol should be easy for another LLM to understand
- function intent must be explicit via `describe`

## Example syntax target
```kai
module demo.video_context;

context {
  goal: "Turn a technical video into a reusable system-context prompt";
  audience: "LLM";
  domain: "programming-languages";
  assumptions: [
    "The source explains how custom programming languages are built.",
    "The output must preserve compiler pipeline knowledge."
  ];
}

schema CompileStep {
  name: Str,
  description: Str,
}

fn summarize_stage_count(count: Int) -> Str
describe "Return a short status line about the number of compile stages"
tags ["demo", "summary", "compiler"]
requires [count > 0]
ensures [len(result) > 0]
{
  return "Compile stages captured.";
}
```

## File creation requirements
Create all repository files completely.
Do not leave TODO-only placeholders unless the file is explicitly a roadmap or future extension stub.
Provide full contents for every file you create or edit.

## Repository files that must exist
- `README.md`
- `ARCHITECTURE.md`
- `ROADMAP.md`
- `CONTRIBUTING.md`
- `Cargo.toml`
- `.gitignore`
- `.editorconfig`
- `.github/workflows/ci.yml`
- `specs/kairos.ebnf`
- `specs/kairos-ir.schema.json`
- `specs/formatter-rules.md`
- `docs/language-overview.md`
- `docs/syntax.md`
- `docs/ai-first-design.md`
- `docs/cli.md`
- `examples/hello_context/src/main.kai`
- `examples/video_context/src/main.kai`
- `examples/risk_rules/src/main.kai`
- tests and fixtures

## Parser implementation guidance
Use a Rust-native parsing strategy such as:
- `pest`, or
- `chumsky`

Pick one and stay consistent.

## Error handling guidance
Use strong diagnostics.
The CLI should fail with human-readable messages and non-zero exit codes.

## Testing requirements
Add:
- parser tests
- semantic tests
- formatter tests
- CLI smoke tests
- golden tests for AST, KIR, and prompt output where practical

## Documentation requirements
Docs must explain:
- why Kairos exists,
- why it is AI-first,
- how syntax works,
- how KIR works,
- how to run the CLI,
- how to contribute.

## Style requirements
- Keep code explicit and boring.
- Avoid hidden magic.
- Prefer stable data structures.
- Prefer clarity over abstraction.
- No shortened code excerpts.
- When you edit a file, show the **full file contents**.

## GitHub readiness
Make the repo ready for GitHub push:
- issue templates
- PR template
- CI
- open-source license
- useful README
- examples

## Completion condition
The task is complete only when:
- the workspace structure exists,
- all required files exist,
- the CLI surface is implemented,
- examples are included,
- tests exist,
- the docs are coherent,
- the repo is GitHub-ready.
