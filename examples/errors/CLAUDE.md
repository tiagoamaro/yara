# examples/errors/

Every file here is *meant* to fail — they demonstrate what Yara's error output looks like at each pipeline stage (lex, parse, typecheck, runtime), per the root `CLAUDE.md` convention that every error is traceable to an exact line:column. Run any of them with `cargo run -- run examples/errors/<file>.yara`; each exits with status 1.

## Status
All nine verified against the actual binary (2026-07-18); output below is real, not illustrative.

## Files and their actual output

- `lex_error.yara` — `@` is not a recognized character:
  ```
  lex error: 2:7: unexpected character `@`
  ```
- `parse_error.yara` — a function body missing its `end`:
  ```
  parse error: 4:1: unexpected end of input, expected `end`
  ```
- `type_error.yara` — `Int + Float` with no implicit coercion:
  ```
  type error: 4:7: cannot apply `+` to `Integer` and `Float`
  ```
- `undefined_variable.yara` — referencing a name that was never declared (caught by the typechecker, not at runtime):
  ```
  type error: 2:7: undefined variable `mystery`
  ```
- `array_out_of_bounds.yara` — indexing past an array's length (a runtime, not typecheck, error — length isn't known statically):
  ```
  runtime error: error: array index 10 out of bounds (length 3)
    at 3:9
  ```
- `runtime_error_stack_trace.yara` — the interesting one: `countdown` recurses until it divides by zero, and `RuntimeError::Display` (see `src/interpreter/CLAUDE.md`) prints every call frame that led there, innermost first:
  ```
  runtime error: error: division by zero
    at 5:7
    in `countdown` at 7:5
    in `countdown` at 7:5
    in `countdown` at 7:5
    in `countdown` at 11:1
  ```
  The bottom frame (`at 11:1`) is the original top-level call site; each frame above it is one level of recursion, down to the `1 / n` that actually failed.
- `class_unknown_field.yara` — accessing a field the class never declared (typecheck-time, via `check_field_access`):
  ```
  type error: 11:8: class `Hello` has no field `missing`
  ```
- `class_field_type_mismatch.yara` — assigning a `String` to an `Integer` field:
  ```
  type error: 11:2: cannot assign `String` to field `count` of type `Integer`
  ```
- `class_wrong_arg_count.yara` — calling `Hello.new` with two args when `initializer` takes one:
  ```
  type error: 10:10: `Hello.new` expects 1 argument(s), found 2
  ```

## Gotchas
- `undefined_variable.yara` shows that referencing an undefined variable is a **typecheck-time** error, not a runtime one — Yara's typechecker tracks variable scope itself (see `src/typechecker/CLAUDE.md`), so this never reaches the interpreter.
- Fixing the `Debug`-leaking `cannot apply \`Add\`` message (it now reads `cannot apply \`+\``) is exactly the kind of rough edge this folder exists to catch — if you add a new error path, run it for real and paste the actual output here rather than guessing.
