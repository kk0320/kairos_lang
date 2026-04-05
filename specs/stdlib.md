# Minimal Standard Library for v0.1

Only include a tiny deterministic subset.

## String
- `len(Str) -> Int`
- `concat(Str, Str) -> Str`

## Numeric
- `abs(Int) -> Int`
- `min(Int, Int) -> Int`
- `max(Int, Int) -> Int`

## Collection
- `len(List<T>) -> Int`

## Notes
Do not add:
- filesystem
- networking
- randomness
- wall clock time
- subprocess execution

v0.1 should stay deterministic and safe.
