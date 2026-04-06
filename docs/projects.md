# Kairos Projects

## Manifest

Kairos projects use `kairos.toml`.

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

Supported fields in 2.0:

- `package.name`
- `package.version`
- `package.entry`
- `dependencies.<name>.path`
- `build.emit`

## Manifest rules

Kairos validates that:

- `package.name` is non-empty and uses lowercase ASCII letters, digits, and underscores
- `package.version` uses `MAJOR.MINOR.PATCH` with an optional prerelease suffix
- `package.entry` is a relative `.kai` path inside the package root
- dependency keys use the same naming rules as packages
- dependency paths are relative and local
- `build.emit` contains only supported targets: `ast`, `ir`, `prompt`

## Discovery rules

Kairos uses a deterministic local package model:

1. Find `kairos.toml`.
2. Resolve `package.entry`.
3. Treat the parent directory of the entry file as the package source root.
4. Load every `.kai` file under that source root in deterministic path order.
5. Resolve local `[dependencies]` recursively through relative `path` entries.
6. Index modules by their `module` declaration.
7. Resolve `use` imports against the combined local package graph.

## Resolution behavior

Current guarantees:

- unresolved imports are errors
- unresolved selective import items are errors
- duplicate module names are errors
- dependency cycles are errors
- imported names remain deterministic
- conflicting imported names are errors
- cross-package imports require a direct declared dependency
- cross-package symbol access requires `pub`

Current intentional limits:

- local path-based dependencies only
- no remote registry or download behavior
- no version solver
- no package publishing workflow yet

## Visibility and imports

Supported forms:

```kai
use shared.rules_lib.api;
use shared.rules_lib.api as rules_api;
use shared.rules_lib.text::{headline, headline as shared_headline};
```

Visibility applies to:

- `pub fn`
- `pub schema`
- `pub enum`
- `pub type`

Current rule of thumb:

- same-package imports may use internal symbols
- cross-package imports may use only `pub` symbols

## Testing model

Kairos 2.0 adds first-class project-native tests through `test fn`.

Rules:

- tests are deterministic
- tests must declare zero parameters
- tests must return `Bool`
- project test discovery runs only root-package tests by default

## Recommended layout

```text
my_project/
  kairos.toml
  src/
    main.kai
    rules.kai
    shared/
      text.kai
```

Reusable sibling package:

```text
shared_rules_lib/
  kairos.toml
  src/
    main.kai
    api.kai
    text.kai
```

## Typical CLI workflow

```powershell
cargo run --bin kairos -- check path\to\project
cargo run --bin kairos -- test path\to\project
cargo run --bin kairos -- doctor path\to\project
cargo run --bin kairos -- ir path\to\project --json
cargo run --bin kairos -- prompt path\to\project
cargo run --bin kairos -- shell path\to\project
```

You can also pass a `.kai` file inside the project. Kairos still loads the surrounding project for `check`, `ir`, `prompt`, `run`, `test`, `doctor`, and `shell`.
