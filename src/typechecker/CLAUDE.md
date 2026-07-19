# typechecker/

Static type-checking pass over the parsed AST, before interpretation.

## Status
Implemented. `TypeChecker::new().check_program(&[Stmt]) -> Result<(), TypeError>`.

## Design
- `Type` enum: `Integer, Float, Boolean, String, Nil` — the canonical (post-alias-normalization) names from `ast::TypeAnnotation`.
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
