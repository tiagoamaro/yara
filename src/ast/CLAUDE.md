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
- `Expr::ArrayLit { elements, line, column }` (`[1, 2, 3]`) and `Expr::Index { array, index, line, column }` (`arr[i]`, chainable). No array-of-array/nested collection AST node — `array` in `Index` is just any `Expr`, so `Index` *can* nest syntactically, but `typechecker::Type` has no array-of-array annotation to type it against (see `src/typechecker/CLAUDE.md`).
- **Classes**: `Stmt::ClassDef { name, consts, fields, methods, line, column }` — `consts`/`methods` are `Vec<Stmt>` but only ever contain `Stmt::ConstDecl`/`Stmt::FunctionDef` respectively (reused rather than inventing near-duplicate variants); `fields` is `Vec<FieldDecl>` (a bare `name: Type` instance-var declaration, no value — the one place a "declaration with no value" exists in the AST). `Stmt::FieldAssign { object, field, value, line, column }` is `object.field = value`. `Expr::FieldAccess { object, field, .. }` is `object.field` (read), `Expr::MethodCall { object, method, args, .. }` is `object.method(args)` — this one node also covers `ClassName.new(args)` construction; the typechecker/interpreter special-case a bare `Expr::Ident` `object` that names a known class (and isn't a bound variable) rather than the parser distinguishing it, since the parser has no semantic/class-table information.

## Gotchas
- `BinOp` has no precedence info attached — precedence lives entirely in the parser's grammar (comparison > additive > multiplicative > unary).
- No record/struct type and no pointers — `examples/data_structures/` builds linked lists/trees/graphs out of parallel `IntArray`s with integer "indices" standing in for pointers (arena style). See root `CLAUDE.md` TODO for a possible future opt-in pointer type. `class` (added later) is a separate, heavier-weight way to group state — the two features don't interact (no `Array<SomeClass>`, no arena-of-objects idiom explored yet).
