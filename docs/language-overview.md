# Language Overview

Kairos is a structured language for logic that should be easy to read, validate, serialize, and reuse as AI context.

## Design goals

- explicit intent
- low ambiguity
- deterministic tool output
- strong machine-readable structure
- readable source for both humans and LLMs

## What the current implementation supports

- single-file modules with `module` and optional `use`
- `context` metadata blocks
- `schema`, `enum`, and `type` declarations
- function metadata with `describe`, `tags`, `requires`, and `ensures`
- a deterministic statement/expression subset for local execution

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

## Current limitations

The repository intentionally stays focused on a practical MVP:

- no multi-file compilation yet
- no package resolution
- no external side effects in the interpreter
- no advanced backend or editor protocol layer yet
