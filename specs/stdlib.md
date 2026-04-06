# Kairos Deterministic Standard Library

Kairos 1.0 includes a deliberately small builtin library aimed at AI-first scripting, prompt shaping, and rule evaluation.

## String

- `len(Str) -> Int`
- `concat(Str, Str) -> Str`
- `contains(Str, Str) -> Bool`
- `starts_with(Str, Str) -> Bool`
- `ends_with(Str, Str) -> Bool`
- `trim(Str) -> Str`
- `upper(Str) -> Str`
- `lower(Str) -> Str`

## List

- `len(List<T>) -> Int`
- `join(List<Str>, Str) -> Str`
- `first(List<T>) -> T?`
- `last(List<T>) -> T?`
- `all(List<Bool>) -> Bool`
- `any(List<Bool>) -> Bool`

## Object

- `contains(Object, Str) -> Bool`
- `has_key(Object, Str) -> Bool`
- `get_str(Object, Str) -> Str?`
- `get_int(Object, Str) -> Int?`
- `keys(Object) -> List<Str>`

## Numeric

- `abs(Int) -> Int`
- `min(Int, Int) -> Int`
- `max(Int, Int) -> Int`
- `clamp(Int, Int, Int) -> Int`

## Design limits

Kairos 1.0 intentionally does not include builtin support for:

- filesystem access
- networking
- randomness
- wall-clock time
- subprocess execution
- environment-dependent behavior

The stdlib remains deterministic and side-effect free by design.
