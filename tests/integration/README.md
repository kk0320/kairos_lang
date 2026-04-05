# Integration Tests

The active CLI integration tests live in [crates/kairos-cli/tests/cli.rs](../../crates/kairos-cli/tests/cli.rs).

They cover:

- `check`
- `ast`
- `ir`
- `prompt`
- `fmt`
- `run`

The `tests/fixtures` directory remains available for parser and semantic fixtures that are shared across crates.
