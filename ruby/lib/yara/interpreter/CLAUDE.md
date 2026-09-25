# ruby/lib/yara/interpreter/

Tree-walk evaluator over a typechecked program: no bytecode, `eval_expr`/`exec_statement` walk the AST directly. `../interpreter.rb` holds `Instance`, `Pointer`, `RuntimeError`, `Interpreter` (`run_program`, `display`, `flatten_classes`, `with_frame`); `expressions.rb`, `statements.rb`, `calls.rb`, `classes.rb` and `methods.rb` reopen `Interpreter`.

## Design
- Yara values are plain Ruby values: `Integer`, `Float`, `true`/`false`, `String`, `nil`, `Array`, `Instance` (class name plus a field hash) and `Pointer` (a heap index). Arrays and instances are shared by reference, so a function's `push` or field assignment is visible to its caller; that is what the arena-style `examples/data_structures/` rely on. Strings are never mutated in place.
- `Interpreter.display` is how `print` and runtime messages show values: floats through `RustFormat.float_display`, arrays as `[a, b]`, instances as `#<Name>`, pointers as `ptr#N`, `nil` as `nil`.
- `run_program` registers functions and classes, flattens inheritance (parents' consts, fields and methods first, a child's own methods winning), then runs the top-level statements.
- `return` travels back as a `Return` value from `exec_statement` and `exec_block`; `exec_function_body` treats the last statement as the implicit return value, including a trailing `if`, the same rule the typechecker checks.
- Calls push a `Diagnostics::Frame` (`name`, `Name.new`, or `Name#method`) and a scope through `with_frame`, popped even when an error propagates. A `RuntimeError` copies the call stack when raised, so its trace shows every frame.
- `construct` evaluates consts in order (each can read earlier ones by bare name), sets instance variables to nil, then runs `initializer` if there is one. `run_method` implements implicit `self` by copying the fields into the method's scope and copying the same names back afterwards.
- The heap is an array of slots: a one-element array while allocated, nil once freed, and never reused, so use-after-free, double free and invalid pointers are runtime errors. `collect` is mark and sweep: every bound value in every scope is a root, marking follows array elements, instance fields and pointees (with an object-id set so cycles end), and unmarked allocated slots are freed; it returns how many.
- Integers stay within i64: `+ - * /`, negation and `abs` raise "integer overflow" past the bounds, and `/` truncates toward zero. `Float#to_i` saturates and maps NaN to 0. `String#to_i`/`to_f` accept exactly what Rust's `str::parse` does (sign, digits, `inf`/`nan` for floats), validated by hand.
- Primitive methods dispatch through `eval_<kind>_<name>`; free builtins through `eval_builtin_<name>`.
- Messages come from the catalog (`runtime/...` keys), with values shown by `display`.

## Gotchas
- mruby differences handled here: float negation multiplies by -1.0, because mruby's unary minus turns `-0.0` into `0.0`. `upper`/`lower` on non-ASCII text follow the runtime, so mruby leaves non-ASCII letters unchanged.
- Integer division by zero is an error; float division by zero gives `inf`/`NaN`.
- A field mutated by a nested function call made from a method is not copied back, since only the method's own scope is.
