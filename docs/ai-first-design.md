# AI-first Design

Kairos treats source code as something that must be read by:

1. humans,
2. compilers,
3. LLMs,
4. retrieval systems,
5. prompt pipelines.

Most languages optimize mainly for (1) and (2).  
Kairos explicitly optimizes for (3), (4), and (5) as well.

## Why this matters
Normal source code often hides critical meaning in:
- naming conventions,
- comments,
- undocumented assumptions,
- repository folklore.

Kairos makes those explicit with structured syntax.

## Core idea
The language itself should carry:
- goal
- audience
- assumptions
- domain
- function intent
- preconditions
- postconditions

That lets a downstream model consume the code as trusted structured context rather than guessing from surface syntax.
