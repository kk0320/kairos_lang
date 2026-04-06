# Language Overview

Kairos is a structured language for logic that should be easy to read, validate, serialize, and reuse as AI context.

## Design goals

- explicit intent
- low ambiguity
- deterministic tool output
- strong machine-readable structure
- readable source for both humans and LLMs
- practical terminal-native workflows

## What the current implementation supports

- multi-file projects through `kairos.toml`
- modules with `module` and explicit `use`
- `context` metadata blocks
- `schema`, `enum`, and `type` declarations
- function metadata with `describe`, `tags`, `requires`, and `ensures`
- deterministic statement/expression subset for local execution
- project-aware AST, KIR, prompt, formatter, and runtime flows
- interactive shell and project scaffolding workflows

## Why the language is AI-first

Kairos source is expected to carry:

- goal
- audience
- domain
- assumptions
- function intent
- preconditions
- postconditions

That lets downstream systems consume code as structured meaning instead of guessing from naming conventions or comments alone.

## Terminal-native philosophy

Kairos v0.5 adds a shell, reload, watch, and scaffolding workflow, but the language stays deterministic.

The shell is not a separate toy mode. It uses the real project loader, parser, semantic analysis, KIR lowering, prompt export, and interpreter pipeline. That means terminal workflows stay aligned with the machine-readable outputs used by downstream tools.

## Project and module model

Kairos intentionally uses a small, explicit project model:

- every project is rooted by `kairos.toml`
- the manifest entry path defines the source root
- each source file declares exactly one `module`
- `use demo.shared.rules;` imports an entire module by explicit path
- imported types and functions become available by name inside the importing module
- ambiguous imported names are errors instead of implicit precedence rules

This is deliberate. Kairos prefers project determinism and readable semantics over clever resolution rules.

## Deterministic outputs

Kairos emits stable artifacts that downstream AI systems can rely on:

- AST JSON for syntax structure
- KIR JSON for normalized machine-facing structure
- prompt markdown for LLM system/context generation
- structured diagnostics for validation tooling
- deterministic interpreter reports for supported subsets

## Practical stdlib

The built-in deterministic helpers focus on AI/rules scripting needs:

- string normalization and search
- list inspection and boolean aggregation
- object key lookup for small record-like values
- numeric comparison and clamping helpers

This is enough to support prompt shaping, decision logic, and schema-adjacent validation without introducing side effects.

## Current limitations

The repository intentionally stays focused on a practical language core:

- one local project root at a time
- no package registry or remote dependency model
- no selective imports or visibility keywords yet
- no external side effects in the interpreter
- no full-screen TUI or editor protocol layer yet
- no rich semantic spans for every diagnostic yet
