# examples/objects/

Demonstrates Yara's `class` feature, including single-parent inheritance — see
`src/typechecker/CLAUDE.md`/`src/interpreter/CLAUDE.md` for the full design.

## Status
`hello.yara` and `inheritance.yara` verified end to end (2026-07-25).

## Files
- `hello.yara` — a class with a const, an instance var, an `initializer`, and a method reading both; instantiation via `.new`, field read/write via `.field`/`.field = value`.
- `inheritance.yara` — `class Dog < Animal`: child adds its own field, inherits a parent field/assigns it in its own initializer (no `super`), and overrides a parent method.

## Gotchas
- Single parent only (`class Child < Parent`), no `super`, no override keyword (child member of the same name implicitly overrides parent's) — see `docs/syntax.md` Inheritance section.
- No class-level/static methods other than the special `.new`, no visibility modifiers (all fields/methods public).
- No `self` expression — a method can't call another method on its own instance without a receiver, so intra-class method calls aren't possible yet.
- See root `CLAUDE.md` TODO for pointers as a separate, still-not-implemented, deferred idea — classes don't touch that.
