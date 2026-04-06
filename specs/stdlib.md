# Kairos Deterministic Standard Library

Kairos 2.0 includes a deliberately small builtin library aimed at AI-first scripting, prompt shaping, validation, and deterministic rule evaluation.

## String

- `len(Str) -> Int`
- `concat(Str, Str) -> Str`
- `contains(Str, Str) -> Bool`
- `starts_with(Str, Str) -> Bool`
- `ends_with(Str, Str) -> Bool`
- `trim(Str) -> Str`
- `upper(Str) -> Str`
- `lower(Str) -> Str`
- `normalize_space(Str) -> Str`

## List

- `len(List<T>) -> Int`
- `count(List<T>) -> Int`
- `join(List<Str>, Str) -> Str`
- `first(List<T>) -> T?`
- `last(List<T>) -> T?`
- `all(List<Bool>) -> Bool`
- `any(List<Bool>) -> Bool`
- `sort(List<Int|Float|Str>) -> List<_>`
- `unique(List<T>) -> List<T>`

## Object

- `contains(Object, Str) -> Bool`
- `has_key(Object, Str) -> Bool`
- `get_str(Object, Str) -> Str?`
- `get_int(Object, Str) -> Int?`
- `get_bool(Object, Str) -> Bool?`
- `get_list(Object, Str) -> List<Any>?`
- `get_obj(Object, Str) -> Object?`
- `keys(Object) -> List<Str>`

## Numeric

- `abs(Int) -> Int`
- `min(Int, Int) -> Int`
- `max(Int, Int) -> Int`
- `clamp(Int, Int, Int) -> Int`

## Design limits

Kairos 2.0 intentionally does not include builtin support for:

- filesystem access
- networking
- randomness
- wall-clock time
- subprocess execution
- environment-dependent behavior

The stdlib remains deterministic and side-effect free by design.
