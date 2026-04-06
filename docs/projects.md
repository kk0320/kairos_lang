# Kairos Projects

## Manifest

Kairos projects use `kairos.toml`.

```toml
[package]
name = "assistant_briefing"
version = "0.5.0-dev"
entry = "src/main.kai"

[build]
emit = ["ast", "ir", "prompt"]
```

Supported fields in v0.5:

- `package.name`
- `package.version`
- `package.entry`
- `build.emit`

## Discovery rules

Kairos currently uses a minimal deterministic project model:

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
- no registry or external dependency fetching

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

## CLI workflow

```powershell
cargo run --bin kairos -- check path\to\project
cargo run --bin kairos -- ir path\to\project --json
cargo run --bin kairos -- prompt path\to\project
cargo run --bin kairos -- fmt path\to\project --check
cargo run --bin kairos -- run path\to\project --json
cargo run --bin kairos -- shell path\to\project
```

You can also pass a `.kai` file inside the project. Kairos will still load the surrounding project for `check`, `ir`, `prompt`, `run`, and `shell`.
