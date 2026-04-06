# Language Overview

Kairos is a structured AI-first language for logic, validation, prompt-adjacent workflows, and deterministic local package reuse.

## Design goals

- explicit intent
- low ambiguity
- deterministic execution and tooling output
- strong machine-readable structure
- readable source for both humans and LLMs
- practical terminal-native workflows

## What Kairos 2.0 supports

- multi-file local projects through `kairos.toml`
- local path-based package dependencies
- explicit modules with `module` and `use`
- `pub` visibility for cross-package reuse
- module alias and selective import forms
- `context` metadata blocks
- `schema`, `enum`, and `type` declarations
- function metadata with `describe`, `tags`, `requires`, and `ensures`
- `test fn` for project-native deterministic tests
- project-aware AST, KIR, prompt, formatter, runtime, shell, test, and doctor flows
- local scaffolding through `new` and `init`

## Why the language is AI-first

Kairos source is expected to carry meaning directly in the language:

- goal
- audience
- domain
- assumptions
- function intent
- preconditions
- postconditions
- explicit public/internal boundaries

That lets downstream systems consume code as structured meaning instead of guessing from naming conventions, comments, or repository folklore.

## Terminal-native philosophy

Kairos includes a shell, reload, watch, scaffolding, doctor, and test workflow, but the language stays deterministic.

The shell is not a separate toy mode. It uses the same project loader, parser, semantic analysis, KIR lowering, prompt export, testing, and interpreter pipeline as the top-level CLI. Terminal workflows therefore stay aligned with the machine-readable outputs used by downstream tools.

## Project and package model

Kairos intentionally uses a small explicit package model:

- every local package is rooted by `kairos.toml`
- the manifest entry path defines the package source root
- each source file declares exactly one `module`
- sibling local packages can be reused through `[dependencies]`
- imports stay explicit and deterministic
- cross-package access requires `pub`

This is deliberate. Kairos prefers trustworthy local behavior over hidden remote resolution or implicit package magic.

## Deterministic outputs

Kairos emits stable artifacts that downstream AI systems can rely on:

- AST JSON for syntax structure
- KIR JSON for normalized machine-facing structure
- prompt markdown for LLM system/context generation
- structured diagnostics for validation tooling
- deterministic execution reports
- deterministic test reports
- deterministic doctor reports

## Practical stdlib

The built-in deterministic helpers focus on AI/rules scripting needs:

- string normalization and search
- list inspection, sorting, uniqueness, and boolean aggregation
- object key lookup for small record-like values
- numeric comparison and clamping helpers

This subset is intentionally enough for prompt shaping, decision logic, and validation-heavy automation without introducing ambient side effects.

## Current limitations

Kairos 2.0 intentionally stays focused:

- one local dependency graph at a time
- no remote package registry or download model
- no external side effects in the interpreter
- no full-screen TUI or editor protocol layer
- no advanced type inference or macro system
- no rich semantic spans for every diagnostic yet
