# Kairos Build Order

## Must-complete order for Codex

1. Create Rust workspace.
2. Implement AST types.
3. Implement parser for:
   - module
   - use
   - context
   - schema
   - enum
   - type alias
   - function declarations with metadata
   - expressions and basic statements
4. Implement semantic analyzer.
5. Implement KIR lowering.
6. Implement CLI commands:
   - check
   - ast
   - ir
   - prompt
   - fmt
   - run
7. Add examples and fixtures.
8. Add integration tests.
9. Add CI.
10. Add GitHub metadata and documentation.

## Acceptance criteria
- a valid `.kai` file parses cleanly
- an invalid `.kai` file fails with useful diagnostics
- AST JSON output is stable
- KIR JSON output is stable
- `kairos prompt` produces deterministic markdown
- `kairos run` executes the minimal deterministic subset
- repository is push-ready for GitHub
