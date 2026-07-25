# examples/errors/

Every file here is *meant* to fail — they demonstrate what Yara's error output looks like at each pipeline stage (lex, parse, typecheck, runtime), per the root `CLAUDE.md` convention that every error is traceable to an exact line:column with a source excerpt and caret. Run any of them with `cargo run -- run examples/errors/<file>.yara`; each exits with status 1. Rendering itself (`render`/`render_with_map`/`render_snippet`) lives in `src/diagnostics/`, not in any compiler stage — `main.rs` only invokes it.

## Status
All sixteen verified against the actual binary (2026-07-25) after class inheritance landed; output below is real, not illustrative.

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
- `import_type_error.yara` — imports `import_type_error_helper.yara` and calls its `bad()`; the type error lives in the *helper*, and the snippet renders from the helper file with its own local line, not the entry file:
  ```
  type error: function `bad` declared to return `Integer`, but returns `String`
    --> examples/errors/import_type_error_helper.yara:1:1
    |
  1 | def bad(): Integer
    | ^
  ```
- `import_type_error_helper.yara` — the imported helper: `bad()` is declared `Integer` but returns a `String`. Fails at typecheck on its own too, which is why it has its own `Type` expected-stage entry in `tests/run_examples.rs`.
- `class_unassigned_field.yara` — a `Counter` class has an instance-var `count: Integer` but the class has no `initializer`, and a `bump()` method that assigns to `count`. The typechecker rejects this at the field's declaration — every instance-var must be assigned in `initializer`:
  ```
  type error: field `count` of class `Counter` is never assigned in `initializer` (it would be `Nil` at runtime, not `Integer`)
    --> examples/errors/class_unassigned_field.yara:2:3
    |
  2 |   count: Integer
    |   ^
  ```
- `class_inherited_field_unassigned.yara` — a `Dog < Animal` child's `initializer` never assigns the inherited `name` field; no `super` means the child must assign every inherited field itself. The error points at the field's *original* declaration in the parent:
  ```
  type error: field `name` of class `Dog` is never assigned in `initializer` (it would be `Nil` at runtime, not `String`)
    --> examples/errors/class_inherited_field_unassigned.yara:5:3
    |
  5 |   name: String
    |   ^
  ```
- `use_after_free.yara` — attempts to dereference a pointer after it has been freed (caught at runtime):
  ```
  runtime error: use after free: pointer ptr#0 was already freed
    --> examples/errors/use_after_free.yara:4:7
    |
  4 | print(deref(p))
    |       ^
  ```
- `double_free.yara` — attempts to free a pointer that was already freed (caught at runtime):
  ```
  runtime error: double free: pointer ptr#0 was already freed
    --> examples/errors/double_free.yara:4:1
    |
  4 | free(p)
    | ^
  ```
- `nil_pointer_deref.yara` — attempts to dereference a `nil` pointer (caught at runtime):
  ```
  runtime error: nil pointer dereference: `deref` on `nil`
    --> examples/errors/nil_pointer_deref.yara:4:7
    |
  4 | print(deref(p))
    |       ^
  ```

## Gotchas
- `undefined_variable.yara` shows that referencing an undefined variable is a **typecheck-time** error, not a runtime one — Yara's typechecker tracks variable scope itself (see `src/typechecker/CLAUDE.md`), so this never reaches the interpreter.
- The caret is always a single `^`, not an underline spanning the whole offending token/expression — good enough to point at *where*, not yet at *how wide*. Revisit if imprecise pointing becomes confusing on longer tokens.
- The `import_type_error` / `import_type_error_helper` pair demonstrates that errors in imported files now render their correct source snippets via `diagnostics::SourceMap` virtual-line resolution — the resolver assigns each imported file a disjoint range of virtual lines, shifts imported AST positions into it, and diagnostics map virtual lines back to (file, local line) during rendering.
- Fixing the `Debug`-leaking `cannot apply \`Add\`` message (it now reads `cannot apply \`+\``) is exactly the kind of rough edge this folder exists to catch — if you add a new error path, run it for real and paste the actual output here rather than guessing.
