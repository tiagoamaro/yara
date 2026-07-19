# typechecker/

Static type-checking pass over the parsed AST, before interpretation.

## Status
Implemented. `TypeChecker::new().check_program(&[Stmt]) -> Result<(), TypeError>`.

## Design
- `Type` enum: `Integer, Float, Boolean, String, Nil, Array(Box<Type>)` — the canonical (post-alias-normalization) names from `ast::TypeAnnotation`.
- **Arrays**: no generic `Array<T>` syntax — each element type gets its own concrete annotation name instead (`IntArray`, `FloatArray`, `BoolArray`, `StringArray`, resolved in `Type::from_annotation_name`), Pascal-array style. No array-of-array annotation exists, so nested collections can't be typed (the parser *would* accept `xs[0][1]` syntactically, but `check_expr`'s `Expr::Index` arm would reject indexing into whatever non-array type `xs[0]` turns out to be). `examples/data_structures/graph.yara` works around this by using an edge-list of two parallel `IntArray`s instead of an adjacency-list-of-lists.
- Array literal `[1, 2, 3]`: element type inferred from the first element, all others must match (`TypeError` "array elements must share one type"). Empty literal `[]` can't infer anything — `check_expr` returns a sentinel `Array(Nil)`, and `VarDecl`/`ConstDecl` special-cases that sentinel as compatible with any declared array annotation (so `xs: IntArray = []` works), then stores the *declared* type, not the sentinel, so the variable's real element type is known from then on.
- `arr[i]` (`Expr::Index`): index must be `Integer`; result type is the array's element type.
- Array builtins (`len`, `push`, `get`, `set`, `pop`) are checked in `check_array_builtin`, called from `Expr::Call` before falling through to user-defined function lookup — same ad-hoc pattern as `print`, not real functions in the `functions` table. `push`/`set` require the value argument's type to match the array's element type exactly.
- Function signatures collected in a pre-pass (`collect_function_signatures`) before checking bodies, so forward references / call order doesn't matter.
- Scoping: `Vec<Scope>` stack, `push_scope`/`pop_scope` around function bodies and `for` loops; `lookup_var` searches innermost-out.
- No implicit numeric coercion: `Int + Float` is a type error (`check_binary_op`). `String + String` is the only cross-type special case (concatenation).
- Comparisons (`< > <= >=`) require matching numeric operands and yield `Boolean`; `==`/`!=` require matching types of any kind.
- `if`/`elsif`/`while` conditions must be exactly `Boolean`.
- `for x in a..b` requires both bounds `Integer`; loop var bound as `Integer` in a pushed scope.
- Function return-type check compares the declared return type against the type of the body's final expression (implicit last-expr return) or an explicit trailing `return expr` — see `check_body_return_type`. A trailing `if`/`elsif`/`else` is itself treated as a tail expression (`check_tail_stmt`): each branch's own tail type is computed recursively (`check_body_return_type` on the branch), and all branches present must agree via `combine_tail_types` (mismatch is a `TypeError`, "branches of `if` return different types"). If there's no `else`, the `if`'s tail type is `None` (can't guarantee every path yields a value), which skips the declared-vs-actual comparison rather than erroring — needed for e.g. `factorial`'s `if n <= 1 ... else ... end` as the whole body.
- Unary negation (`-x`): operand must be `Integer` or `Float`; result is the same type. No other unary operator exists.
- `print(...)` is a built-in call recognized ad hoc in `check_expr`'s `Expr::Call` arm (accepts any args, returns `Nil`) — not in the `functions` table. Add real built-ins the same way until a stdlib design exists.
- Errors (`TypeError`) carry line/column same shape as `LexError`/`ParseError`: `Display` = `"{line}:{column}: {message}"`.

## Gotchas
- `stmt_line`/`stmt_column` free functions exist only to report the function-def's own position for return-type mismatches (matching on `Stmt::FunctionDef` again would move it) — reuse if similar position-of-outer-stmt lookups are needed later.
- Unknown type names in annotations (e.g. a typo) surface as `TypeError` at check time, not parse time — parser accepts any identifier as a type name.
