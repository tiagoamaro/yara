# examples/

Sample `.yara` programs, used to exercise the language as each compiler stage lands.

## Status
All examples run end-to-end via `cargo run -- run examples/<file>.yara` (verified 2026-07-18, including `kitchen_sink.yara`).

## Files
- `hello.yara` — smallest possible program.
- `functions.yara` — function definitions and calls.
- `types.yara` — base type declarations (Int/Integer, Float, Bool/Boolean, Str/String).
- `control_flow.yara` — `if`/`elsif`/`else` used as a function's tail expression, unary negation.
- `loops.yara` — `for x in a..b` and `while`.
- `recursion.yara` — `factorial`, exercising a function calling itself and `if`/`else` as tail return.
- `constants.yara` — `const` decls and a function reading an outer const.
- `kitchen_sink.yara` — imports every other example file (`import "name"`, no `.yara` extension needed) to exercise the whole language in one run; demonstrates the `resolver` (see `src/resolver/CLAUDE.md`). Does not currently import `data_structures/*` (kept separate since those are more involved demos, not quick language-feature smoke tests).
- `data_structures/` — array-backed data structure demos (`list`, `stack`, `queue`, `linked_list`, `binary_tree`, `graph`); see its own `CLAUDE.md`.
- `errors/` — deliberately-failing programs showing rendered lex/parse/type/runtime error output, including a recursive call-stack trace; see its own `CLAUDE.md`.
- `objects/` — `class` declarations: const/instance-var/initializer/method, `.new` construction, field read/write; see its own `CLAUDE.md`.
- `pointers/` — manual memory management demo (`alloc`/`deref`/`set_deref`/`free` builtins). `basic.yara` exercises allocation, dereferencing, and explicit freeing. `leak.yara` demonstrates allocations never freed (allowed in manual mode; garbage collection added later for comparison); see its own `CLAUDE.md`.
- `translations/` — keyword-translation demo (`--keywords <path>`, Portuguese); see its own `CLAUDE.md`.

## Gotchas
- `import` paths are relative to the importing file's own directory and resolved at `yara run` time (not a build step) — running `kitchen_sink.yara` from a different working directory still works because resolution is relative to the file, not cwd.
