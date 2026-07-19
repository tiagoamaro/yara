# examples/objects/

Demonstrates Yara's `class` feature (no inheritance yet — see `src/typechecker/CLAUDE.md`/`src/interpreter/CLAUDE.md` for the full design).

## Status
`hello.yara` verified end to end (2026-07-18).

## Files
- `hello.yara` — a class with a const, an instance var, an `initializer`, and a method reading both; instantiation via `.new`, field read/write via `.field`/`.field = value`.

## Gotchas
- No inheritance, no class-level/static methods other than the special `.new`, no visibility modifiers (all fields/methods public).
- See root `CLAUDE.md` TODO for pointers as a separate, still-not-implemented, deferred idea — classes don't touch that.
