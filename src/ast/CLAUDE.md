# ast/

AST node type definitions shared by parser, typechecker, interpreter.

## Status
Implemented. `Expr`, `Stmt`, `TypeAnnotation`, `Param`, `BinOp` defined.

## Design
- Every `Expr` variant and `Stmt` variant carries `line`/`column`. `Expr::line()`/`Expr::column()` helpers dispatch across variants.
- `TypeAnnotation.name` is always the canonical (alias-normalized) form — parser calls `lexer::normalize_type_alias` before constructing it, so typechecker/interpreter never see `Int`/`Bool`/`Str`, only `Integer`/`Boolean`/`String`.
- `Stmt::If` models `elsif` as `Vec<(Expr, Vec<Stmt>)>`, separate from optional `else_body`.
- No unary operators yet (no `!`, no unary `-`) — add a variant here plus lexer/parser support if introduced.
- Function bodies use Ruby-style implicit last-expression return; `Stmt::Return` also exists for explicit early return.

## Gotchas
- `BinOp` has no precedence info attached — precedence lives entirely in the parser's grammar (comparison > additive > multiplicative).
