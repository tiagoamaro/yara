# examples/errors/

Every file here is *meant* to fail — they demonstrate what Yara's error output looks like at each pipeline stage (lex, parse, typecheck, runtime), per the root `CLAUDE.md` convention that every error is traceable to an exact line:column with a source excerpt and caret. Run any of them with `cargo run -- run examples/errors/<file>.yara`; each exits with status 1. Rendering itself (`print_error`/`render_snippet`) lives in `src/main.rs`, not in any compiler stage — see its doc comment.

## Status
All nine verified against the actual binary (2026-07-18) after `print_error`/`render_snippet` landed; output below is real, not illustrative.

## Files and their actual output

- `lex_error.yara` — `@` is not a recognized character:
  ```
  lex error: unexpected character `@`
    --> examples/errors/lex_error.yara:2:7
    |
  2 | x = 5 @ 3
    |       ^
  ```
- `parse_error.yara` — a function body missing its `end` (position is EOF, past the last line, so there's no source line to render — the header alone still pins it exactly):
  ```
  parse error: unexpected end of input, expected `end`
    --> examples/errors/parse_error.yara:4:1
  ```
- `type_error.yara` — `Int + Float` with no implicit coercion:
  ```
  type error: cannot apply `+` to `Integer` and `Float`
    --> examples/errors/type_error.yara:4:7
    |
  4 | z = x + y
    |       ^
  ```
- `undefined_variable.yara` — referencing a name that was never declared (caught by the typechecker, not at runtime):
  ```
  type error: undefined variable `mystery`
    --> examples/errors/undefined_variable.yara:2:7
    |
  2 | print(mystery)
    |       ^
  ```
- `array_out_of_bounds.yara` — indexing past an array's length (a runtime, not typecheck, error — length isn't known statically):
  ```
  runtime error: array index 10 out of bounds (length 3)
    --> examples/errors/array_out_of_bounds.yara:3:9
    |
  3 | print(xs[10])
    |         ^
  ```
- `runtime_error_stack_trace.yara` — the interesting one: `countdown` recurses until it divides by zero, and every call-stack frame gets its own `--> file:line:col` + source snippet, innermost first:
  ```
  runtime error: division by zero
    --> examples/errors/runtime_error_stack_trace.yara:5:7
    |
  5 |     1 / n
    |       ^
    in `countdown` at examples/errors/runtime_error_stack_trace.yara:7:5
    |
  7 |     countdown(n - 1)
    |     ^
    in `countdown` at examples/errors/runtime_error_stack_trace.yara:7:5
    |
  7 |     countdown(n - 1)
    |     ^
    in `countdown` at examples/errors/runtime_error_stack_trace.yara:7:5
    |
  7 |     countdown(n - 1)
    |     ^
    in `countdown` at examples/errors/runtime_error_stack_trace.yara:11:1
     |
  11 | countdown(3)
     | ^
  ```
  The bottom frame (`at 11:1`) is the original top-level call site; each frame above it is one level of recursion, down to the `1 / n` that actually failed.
- `class_unknown_field.yara` — accessing a field the class never declared (typecheck-time, via `check_field_access`):
  ```
  type error: class `Hello` has no field `missing`
    --> examples/errors/class_unknown_field.yara:11:8
     |
  11 | print(h.missing)
     |        ^
  ```
- `class_field_type_mismatch.yara` — assigning a `String` to an `Integer` field:
  ```
  type error: cannot assign `String` to field `count` of type `Integer`
    --> examples/errors/class_field_type_mismatch.yara:11:2
     |
  11 | h.count = "oops"
     |  ^
  ```
- `class_wrong_arg_count.yara` — calling `Hello.new` with two args when `initializer` takes one:
  ```
  type error: `Hello.new` expects 1 argument(s), found 2
    --> examples/errors/class_wrong_arg_count.yara:10:10
     |
  10 | h = Hello.new(5, 6)
     |          ^
  ```

## Gotchas
- `undefined_variable.yara` shows that referencing an undefined variable is a **typecheck-time** error, not a runtime one — Yara's typechecker tracks variable scope itself (see `src/typechecker/CLAUDE.md`), so this never reaches the interpreter.
- The caret is always a single `^`, not an underline spanning the whole offending token/expression — good enough to point at *where*, not yet at *how wide*. Revisit if imprecise pointing becomes confusing on longer tokens.
- **Known gap**: `print_error`'s source snippet is read from the *entry file* the user ran, not from whichever file a position actually originated in. Since `resolver` splices an imported file's statements into the importer before typechecking/interpretation ever run (see `src/resolver/CLAUDE.md`), an error whose line:column belongs to an *imported* file would render the wrong snippet (or an out-of-range one) — none of these examples exercise that combination, so it hasn't bitten yet, but it's a real latent bug until errors carry their own file path alongside line/column.
- Fixing the `Debug`-leaking `cannot apply \`Add\`` message (it now reads `cannot apply \`+\``) is exactly the kind of rough edge this folder exists to catch — if you add a new error path, run it for real and paste the actual output here rather than guessing.
