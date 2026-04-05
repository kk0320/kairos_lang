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
