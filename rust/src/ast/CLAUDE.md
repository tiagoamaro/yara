# ast/

AST node type definitions shared by parser, typechecker, interpreter.

## Status
Implemented. `Expr`, `Stmt`, `TypeAnnotation`, `Param`, `BinOp` defined.

## Design
- Every `Expr` variant and `Stmt` variant carries `line`/`column`. `Expr::line()`/`Expr::column()` and `Stmt::line()`/`Stmt::column()` helpers dispatch across variants (the `Stmt` ones delegate to the wrapped expression for `ExprStmt`). Use these instead of re-matching a node just to read its position.
- `TypeAnnotation.name` is always the canonical (alias-normalized) form — parser calls `types::normalize_type_alias` before constructing it, so typechecker/interpreter never see `Int`/`Bool`/`Str`, only `Integer`/`Boolean`/`String`.
- `Stmt::If` models `elsif` as `Vec<(Expr, Vec<Stmt>)>`, separate from optional `else_body`.
- Unary: `Expr::Unary { op: UnOp::Neg, expr, line, column }` for `-x`. Only negation exists — no `!`/`not` yet.
- Function bodies use Ruby-style implicit last-expression return; `Stmt::Return` also exists for explicit early return. A trailing `if`/`elsif`/`else` is *also* a valid tail expression (needed for idiomatic recursion, e.g. `factorial`) — see `typechecker::check_tail_stmt` / `interpreter::exec_tail_stmt`.
- `Stmt::Import { path, line, column }` — parsed as a normal statement but has no runtime/typecheck meaning of its own; `resolver::resolve_imports` splices it away before typechecking ever sees the program. Both `typechecker` and `interpreter` treat it as a no-op for exhaustiveness only.
- `Expr::ArrayLit { elements, line, column }` (`[1, 2, 3]`) and `Expr::Index { array, index, line, column }` (`arr[i]`, chainable). No array-of-array/nested collection AST node — `array` in `Index` is just any `Expr`, so `Index` *can* nest syntactically, but `typechecker::Type` has no array-of-array annotation to type it against (see `src/typechecker/CLAUDE.md`).
- **Classes**: `Stmt::ClassDef { name, parent, consts, fields, methods, line, column }` — `consts`/`methods` are `Vec<Stmt>` but only ever contain `Stmt::ConstDecl`/`Stmt::FunctionDef` respectively (reused rather than inventing near-duplicate variants); `fields` is `Vec<FieldDecl>` (a bare `name: Type` instance-var declaration, no value — the one place a "declaration with no value" exists in the AST). `parent: Option<String>` is the optional single-parent name from `class Child < Parent`; the AST only carries the name — merging inherited fields/methods ("flattening") happens downstream in the typechecker/interpreter, not here. `Stmt::FieldAssign { object, field, value, line, column }` is `object.field = value`. `Expr::FieldAccess { object, field, .. }` is `object.field` (read), `Expr::MethodCall { object, method, args, .. }` is `object.method(args)` — this one node also covers `ClassName.new(args)` construction; the typechecker/interpreter special-case a bare `Expr::Ident` `object` that names a known class (and isn't a bound variable) rather than the parser distinguishing it, since the parser has no semantic/class-table information.
- `Expr::shift_lines(&mut self, offset)` and `Stmt::shift_lines(&mut self, offset)` — recursively add `offset` to every `line` field in the tree (columns untouched), including nested bodies, params, fields, type annotations. Called by `resolver` when splicing an imported file into the virtual line space: each imported statement is offset so its `line` values map to the correct slot in the `SourceMap`. Enables post-import stages (typechecker, interpreter) to use `render_with_map` for correct multi-file error carets.

## Gotchas
- `BinOp` has no precedence info attached — precedence lives entirely in the parser's grammar (comparison > additive > multiplicative > unary).
- No record/struct type — `examples/data_structures/` builds linked lists/trees/graphs out of parallel `IntArray`s with integer "indices" standing in for pointers (arena style). `Ptr<T>` provides an opt-in pointer type for manual/GC memory management; pointers to instances are supported (`Ptr<Node>` legal, resolved via the class table) — see `examples/pointers/linked_list.yara` and `examples/pointers/circular_list.yara` for pointer-based list demos.
