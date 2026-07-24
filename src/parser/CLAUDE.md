# parser/

Recursive-descent parser turning lexer tokens into `ast::` nodes.

## Status
Implemented. `Parser::new(tokens).parse_program() -> Result<Vec<Stmt>, ParseError>`.

## Layout
- **`mod.rs`** (535 lines) — module doc, imports, `ParseError` + `Diagnostic` impl, `Parser` struct + constructor + `parse_program()` + core token helpers (`peek`/`check`/`advance`/`expect`/`expect_ident`), `parse_type_annotation` (handles `Ptr<T>` syntax), `parse_block`, `parse_comma_separated`, and all unit tests.
- **`stmts.rs`** (~290 lines) — statement parsing: `parse_stmt` (dispatcher), `parse_ident_stmt` (checkpoint/rewind disambiguator), `parse_import`, `parse_const_decl`, `parse_class`, `parse_function_def`, `parse_return`, `parse_if`, `parse_while`, `parse_for`, `parse_param`.
- **`exprs.rs`** (~210 lines) — expression parsing via precedence climbing: `parse_expr`, `parse_comparison`, `parse_additive`, `parse_multiplicative`, `parse_unary`, `parse_primary`, `parse_postfix`, `parse_primary_base`.

## Design
- Precedence climbing via chained methods: `parse_comparison` -> `parse_additive` -> `parse_multiplicative` -> `parse_unary` -> `parse_primary`. `parse_unary` handles prefix `-` (`UnOp::Neg`), right-recursive so `--x` parses (typechecker/interpreter will happily double-negate).
- `import "path"` parsed by `parse_import`: keyword then a required string-literal token, no `end`. See `resolver` for what actually happens with it.
- `parse_ident_stmt` disambiguates `x = expr` / `x: Type = expr` var-decl from a bare expression statement (e.g. a call `foo()`) by checkpointing `self.pos` and rewinding if no `:`/`=` follows the identifier.
- `parse_block(terminators)` collects statements until one of the given terminator token kinds (`end`/`elsif`/`else`) or errors on unexpected EOF with the position of the EOF token.
- Every bracketed comma-separated list — call args, method-call args, array literals, function params — goes through one `parse_comma_separated<T>(terminator, terminator_desc, parse_item)` helper. `parse_item` is a fn pointer (`Self::parse_expr` for expression lists, `Self::parse_param` for params), so the empty-list/trailing-item/close-bracket logic exists once. Trailing commas are rejected.
- Type aliases normalized here via `types::normalize_type_alias` (in `src/types.rs`, no longer in the lexer), in `parse_type_annotation`.
- `Ptr<T>` type syntax parsed in `parse_type_annotation`: if the type name is `Ptr`, expects `<`, recursively parses the inner-type annotation (supporting nesting like `Ptr<Ptr<Integer>>`), expects `>`, and encodes the result into `TypeAnnotation.name` as e.g. `"Ptr<Integer>"`. Inner types are alias-normalized by the recursion.
- `ParseError` mirrors `LexError` shape: `message`, `line`, `column`, `Display` impl `"{line}:{column}: {message}"`.
- Tests in `mod.rs` cover: function defs, var decl (inferred/explicit/aliased types), if/elsif/else, while, for-range, call expr statement, operator precedence, const decl, error position reporting, array literal + indexing, import, class parsing, `.new`/field access/method call/field assignment.
- Array literals (`[1, 2, 3]`), indexing (`arr[i]`), field access (`obj.field`), and method calls (`obj.method(args)`) are all parsed as postfix operators in `parse_primary`/`parse_primary_base`/`parse_postfix`: `parse_primary_base` builds the base expression (literal/ident/call/paren/`[...]` literal), then `parse_postfix` loops, wrapping it in `Expr::Index` for each trailing `[expr]` or `Expr::FieldAccess`/`Expr::MethodCall` for each trailing `.name`/`.name(args)` — so `a.b.c[0].d(1)` chains naturally even though most of that has no type-level meaning yet (see `ast::CLAUDE.md`).
- `class Name ... end` parsed by `parse_class`, a *restricted* statement loop (not `parse_stmt`/`parse_block`): only `const`, a bare `name: Type` field declaration, or `def` are accepted as class-body statements; anything else is a parse error naming what was expected.
- Assignment-target disambiguation in `parse_ident_stmt` now goes through a full `parse_expr()` first (handles `obj.field = value` alongside plain `x = value`), then matches on the resulting `Expr` variant: `Expr::Ident` -> `Stmt::VarDecl`, `Expr::FieldAccess` -> `Stmt::FieldAssign`, anything else with a following `=` -> "invalid assignment target" parse error (e.g. `1 = 2` or `f() = 2`).

## Gotchas
- `advance()` clamps at the last token (`Eof`) rather than panicking past the end — relies on `tokenize()` always appending a trailing `Eof`.
- `check()` compares `std::mem::discriminant`, so it matches only the token *kind* shape, not payload (e.g. any `Ident(_)` matches `Ident("".into())` used as a placeholder in terminator lists) — fine since terminators used so far carry no payload.
- `ClassName.new(args)` parses as an ordinary `Expr::MethodCall` — the parser has no idea "new" or "ClassName" are special; that dispatch happens in `typechecker`/`interpreter`, which check whether the call's `object` is a bare `Ident` naming a registered class.
- **Newlines are not statement terminators**, so a line beginning with a unary `-` is absorbed as a *binary* minus into the previous line's expression. E.g. `if n < 0` on one line then `-1` on the next parses as the condition `n < (0 - 1)` with an *empty* then-body, not a `-1` tail expression. A leading-`-` expression statement (or tail expression) is a latent gotcha; write `0 - 1` / parenthesize, or avoid a bare leading minus at the start of a statement. Not currently fixed (no newline/terminator token in the grammar).
