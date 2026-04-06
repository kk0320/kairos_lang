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
- `pub`
- `test`

## Modules and imports

Every source file begins with a `module` declaration.

```kai
module demo.package_reuse_demo;
use shared.rules_lib.api as rules_api;
use shared.rules_lib.text::{headline as library_headline};
```

Supported import forms:

- `use package.module;`
- `use package.module as alias;`
- `use package.module::{name, other as alias};`

Current rules:

- one `module` declaration per file
- `use` declarations must appear directly below `module`
- module paths use `.` separators
- qualified imported names use `::`
- same-package imports may access internal symbols
- cross-package imports may access only `pub` symbols

## Visibility and tests

```kai
pub fn classify(score: Int) -> Str
describe "Classify a reusable score"
tags ["public", "rules"]
requires [score >= 0, score <= 100]
ensures [len(result) > 0]
{
  return "MEDIUM";
}

test fn classify_smoke() -> Bool
describe "Verify the reusable score classification"
tags ["test"]
requires []
ensures [result == true]
{
  return classify(72) == "MEDIUM";
}
```

Current test rules:

- `test fn` must declare zero parameters
- `test fn` must return `Bool`
- tests run deterministically through `kairos test`

## Project manifests

Projects are rooted by `kairos.toml`.

```toml
[package]
name = "package_reuse_demo"
version = "2.0.0"
entry = "src/main.kai"

[dependencies]
shared_rules = { path = "../shared_rules_lib" }

[build]
emit = ["ast", "ir", "prompt"]
```

Kairos treats the parent directory of `entry` as the package source root and loads every `.kai` file under that directory tree. Dependencies are loaded recursively through explicit local paths.

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

- negative values are currently written through expressions such as `0 - 1`
- object literals are supported for deterministic record-like data
- qualified imported names such as `rules_api::classify(72)` are supported

## Deterministic stdlib

The current builtin subset includes:

- `len`, `concat`, `contains`, `starts_with`, `ends_with`, `trim`, `upper`, `lower`, `normalize_space`
- `join`, `first`, `last`, `all`, `any`, `count`, `sort`, `unique`
- `has_key`, `get_str`, `get_int`, `get_bool`, `get_list`, `get_obj`, `keys`
- `abs`, `min`, `max`, `clamp`

## Canonical formatting

The formatter guarantees:

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
- root-package formatting in deterministic path order
