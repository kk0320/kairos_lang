# Formatter Rules

## Goals
- deterministic output
- no style debates
- easier AST diffing
- easier LLM consumption

## Rules
1. Preserve a single `module` declaration at the top.
2. Group `use` declarations directly under `module`.
3. Insert one blank line between top-level declarations.
4. Canonical function metadata order:
   - `describe`
   - `tags`
   - `requires`
   - `ensures`
5. Use double quotes for strings.
6. Use trailing commas for multiline collections.
7. Indent block contents by 2 spaces in `.kai`.
8. Avoid inline comments after code in formatted output.
9. End files with a newline.
