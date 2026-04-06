# Contributing to Kairos

## Principles

- keep syntax explicit
- prefer determinism over cleverness
- preserve stable AST/KIR/diagnostic contracts
- optimize for readability by both humans and LLMs
- document intentional limits rather than hiding partial features

## Development setup

### Windows PowerShell

```powershell
cargo build --workspace
cargo test --workspace
```

### Optional local install

```powershell
cargo install --path crates/kairos-cli
kairos --help
```

## Recommended workflow

1. Open an issue or write down the problem clearly.
2. Create a feature branch.
3. Add or update tests.
4. Run formatting and validation.
5. Update docs when external behavior changes.
6. Open a pull request.

## Validation commands

```powershell
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

## What to preserve

When changing Kairos, treat these as stable product surfaces unless there is a strong reason:

- CLI command names
- AST JSON shape
- KIR JSON shape
- diagnostic JSON fields
- prompt export structure
- deterministic project loading rules

## Commit style

- `feat(cli): polish shell help output`
- `fix(project): reject invalid manifest entry paths`
- `docs(readme): clarify local install flow`
