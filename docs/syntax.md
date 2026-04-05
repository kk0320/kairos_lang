# Kairos Syntax

## Top-level declarations

The current parser supports:

- `module`
- `use`
- `context`
- `schema`
- `enum`
- `type`
- `fn`

## Modules and imports

Every source file begins with a `module` declaration.

```kai
module demo.assistant_briefing;
use demo.assistant_briefing.briefing;
use demo.assistant_briefing.policies;
```

Current project rules:

- one `module` declaration per file
- `use` declarations must appear directly below `module`
- imports use full module paths
- imported types and functions become available by name
- duplicate imported names are validation errors

## Project manifests

Projects are rooted by `kairos.toml`.

```toml
[package]
name = "assistant_briefing"
version = "0.2.0"
entry = "src/main.kai"

[build]
emit = ["ast", "ir", "prompt"]
```

Kairos currently treats the parent directory of `entry` as the source root and loads every `.kai` file under that directory tree.

## Function metadata

Metadata sits between a function signature and body.

```kai
fn classify(score: Int) -> Str
describe "Classify a score into a label"
tags ["demo", "classification"]
requires [score >= 0]
ensures [len(result) > 0]
{
  return "ok";
}
```

Rules enforced today:

- `describe` is required
- `tags` must be string literals
- `requires` and `ensures` must evaluate to `Bool`

## Context blocks

```kai
context {
  goal: "Summarize technical content";
  audience: "LLM";
  domain: "compiler-design";
  assumptions: [
    "Input is trusted.",
    "Output must be concise.",
  ];
}
```

Current validation:

- values must be compile-time constants
- `goal`, `audience`, and `domain` must be strings
- `assumptions` must be a list of strings
- unknown keys are warnings, not parse failures

## Statements and expressions

The current executable subset includes:

- `let`
- `return`
- `if / else if / else`
- literals
- identifiers
- function calls
- list literals
- object literals
- binary expressions

Notes on the current expression subset:

- negative values are currently written through expressions such as `0 - 1` rather than unary-minus literals
- object literals are supported for deterministic record-like data
- function calls are unqualified; module access is controlled by `use`

## Deterministic stdlib

The current builtin subset includes:

- `len`, `concat`, `contains`, `starts_with`, `ends_with`, `trim`, `upper`, `lower`
- `join`, `first`, `last`, `all`, `any`
- `has_key`, `get_str`, `get_int`, `keys`
- `abs`, `min`, `max`, `clamp`

## Canonical formatting

The formatter currently guarantees:

- one `module` declaration at the top
- `use` declarations directly below `module`
- one blank line between top-level sections
- metadata order:
  1. `describe`
  2. `tags`
  3. `requires`
  4. `ensures`
- trailing commas in multiline collections
- two-space indentation
- files ending with a newline
- project formatting iterates all discovered modules in deterministic path order
