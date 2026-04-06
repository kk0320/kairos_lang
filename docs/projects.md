# Kairos Projects

## Manifest

Kairos projects use `kairos.toml`.

```toml
[package]
name = "assistant_briefing"
version = "1.0.0"
entry = "src/main.kai"

[build]
emit = ["ast", "ir", "prompt"]
```

Supported fields in 1.0:

- `package.name`
- `package.version`
- `package.entry`
- `build.emit`

## Manifest rules

Kairos 1.0 validates that:

- `package.name` is non-empty and uses lowercase ASCII letters, digits, and underscores
- `package.version` uses `MAJOR.MINOR.PATCH` with an optional prerelease suffix
- `package.entry` is a relative `.kai` path inside the project
- `build.emit` contains only supported targets: `ast`, `ir`, `prompt`

## Discovery rules

Kairos uses a small deterministic local project model:

1. Find `kairos.toml`.
2. Resolve `package.entry`.
3. Treat the parent directory of the entry file as the source root.
4. Load every `.kai` file under that source root in deterministic path order.
5. Index modules by their `module` declaration.
6. Resolve `use` imports against that module index.

## Resolution behavior

Current guarantees:

- unresolved imports are errors
- duplicate module names are errors
- import cycles are errors
- imported types and functions become available by name in the importing module
- conflicting imported names are errors

Current intentional limits:

- no selective imports
- no aliasing
- no visibility modifiers
- no registry or remote dependency fetching

## Bootstrapping projects

Create a new directory:

```powershell
cargo run --bin kairos -- new demo_project
cargo run --bin kairos -- new rules_demo --template rules
```

Initialize the current directory:

```powershell
cargo run --bin kairos -- init
cargo run --bin kairos -- init --template briefing
```

Scaffolding behavior:

- creates `kairos.toml` if missing
- creates starter source files if missing
- avoids overwriting existing files
- validates the generated project before reporting success
- normalizes package/module names when the directory name needs cleanup

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

Example:

```kai
module demo.project;
use demo.project.rules;
use demo.project.shared.text;
```

## Typical CLI workflow

```powershell
cargo run --bin kairos -- check path\to\project
cargo run --bin kairos -- ir path\to\project --json
cargo run --bin kairos -- prompt path\to\project
cargo run --bin kairos -- fmt path\to\project --check
cargo run --bin kairos -- run path\to\project --json
cargo run --bin kairos -- shell path\to\project
```

You can also pass a `.kai` file inside the project. Kairos will still load the surrounding project for `check`, `ir`, `prompt`, `run`, and `shell`.
