# Contributing to Kairos

## Principles
- Keep syntax explicit.
- Prefer determinism over cleverness.
- Preserve stable AST/KIR contracts.
- Optimize for readability by both humans and LLMs.

## Workflow
1. Open an issue.
2. Create a feature branch.
3. Add or update tests.
4. Run formatting and checks.
5. Open a pull request.

## Development commands
```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

## Commit style
- `feat(parser): add function metadata parsing`
- `fix(cli): handle missing module declaration`
- `docs(syntax): clarify context block keys`
