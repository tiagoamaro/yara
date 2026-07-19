# ast/

AST node type definitions shared by parser, typechecker, interpreter.

## Status
Implemented. `Expr`, `Stmt`, `TypeAnnotation`, `Param`, `BinOp` defined.

## Design
- Every `Expr` variant and `Stmt` variant carries `line`/`column`. `Expr::line()`/`Expr::column()` helpers dispatch across variants.
- `TypeAnnotation.name` is always the canonical (alias-normalized) form — parser calls `lexer::normalize_type_alias` before constructing it, so typechecker/interpreter never see `Int`/`Bool`/`Str`, only `Integer`/`Boolean`/`String`.
- `Stmt::If` models `elsif` as `Vec<(Expr, Vec<Stmt>)>`, separate from optional `else_body`.
- Unary: `Expr::Unary { op: UnOp::Neg, expr, line, column }` for `-x`. Only negation exists — no `!`/`not` yet.
- Function bodies use Ruby-style implicit last-expression return; `Stmt::Return` also exists for explicit early return. A trailing `if`/`elsif`/`else` is *also* a valid tail expression (needed for idiomatic recursion, e.g. `factorial`) — see `typechecker::check_tail_stmt` / `interpreter::exec_tail_stmt`.
- `Stmt::Import { path, line, column }` — parsed as a normal statement but has no runtime/typecheck meaning of its own; `resolver::resolve_imports` splices it away before typechecking ever sees the program. Both `typechecker` and `interpreter` treat it as a no-op for exhaustiveness only.

## Gotchas
- `BinOp` has no precedence info attached — precedence lives entirely in the parser's grammar (comparison > additive > multiplicative > unary).
