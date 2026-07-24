# parser/

Recursive-descent parser turning lexer tokens into `ast::` nodes.

## Status
Implemented. `Parser::new(tokens).parse_program() -> Result<Vec<Stmt>, ParseError>`.

## Design
- Precedence climbing via chained methods: `parse_comparison` -> `parse_additive` -> `parse_multiplicative` -> `parse_unary` -> `parse_primary`. `parse_unary` handles prefix `-` (`UnOp::Neg`), right-recursive so `--x` parses (typechecker/interpreter will happily double-negate).
- `import "path"` parsed by `parse_import`: keyword then a required string-literal token, no `end`. See `resolver` for what actually happens with it.
- `parse_ident_stmt` disambiguates `x = expr` / `x: Type = expr` var-decl from a bare expression statement (e.g. a call `foo()`) by checkpointing `self.pos` and rewinding if no `:`/`=` follows the identifier.
- `parse_block(terminators)` collects statements until one of the given terminator token kinds (`end`/`elsif`/`else`) or errors on unexpected EOF with the position of the EOF token.
- Type aliases normalized here via `types::normalize_type_alias` (in `src/types.rs`, no longer in the lexer), in `parse_type_annotation`.
- `ParseError` mirrors `LexError` shape: `message`, `line`, `column`, `Display` impl `"{line}:{column}: {message}"`.
- Tests in `mod.rs` cover: function defs, var decl (inferred/explicit/aliased types), if/elsif/else, while, for-range, call expr statement, operator precedence, const decl, error position reporting, array literal + indexing, import, class parsing, `.new`/field access/method call/field assignment.
- Array literals (`[1, 2, 3]`), indexing (`arr[i]`), field access (`obj.field`), and method calls (`obj.method(args)`) are all parsed as postfix operators in `parse_primary`/`parse_primary_base`/`parse_postfix`: `parse_primary_base` builds the base expression (literal/ident/call/paren/`[...]` literal), then `parse_postfix` loops, wrapping it in `Expr::Index` for each trailing `[expr]` or `Expr::FieldAccess`/`Expr::MethodCall` for each trailing `.name`/`.name(args)` — so `a.b.c[0].d(1)` chains naturally even though most of that has no type-level meaning yet (see `ast::CLAUDE.md`).
- `class Name ... end` parsed by `parse_class`, a *restricted* statement loop (not `parse_stmt`/`parse_block`): only `const`, a bare `name: Type` field declaration, or `def` are accepted as class-body statements; anything else is a parse error naming what was expected.
- Assignment-target disambiguation in `parse_ident_stmt` now goes through a full `parse_expr()` first (handles `obj.field = value` alongside plain `x = value`), then matches on the resulting `Expr` variant: `Expr::Ident` -> `Stmt::VarDecl`, `Expr::FieldAccess` -> `Stmt::FieldAssign`, anything else with a following `=` -> "invalid assignment target" parse error (e.g. `1 = 2` or `f() = 2`).

## Gotchas
- `advance()` clamps at the last token (`Eof`) rather than panicking past the end — relies on `tokenize()` always appending a trailing `Eof`.
- `check()` compares `std::mem::discriminant`, so it matches only the token *kind* shape, not payload (e.g. any `Ident(_)` matches `Ident("".into())` used as a placeholder in terminator lists) — fine since terminators used so far carry no payload.
- `ClassName.new(args)` parses as an ordinary `Expr::MethodCall` — the parser has no idea "new" or "ClassName" are special; that dispatch happens in `typechecker`/`interpreter`, which check whether the call's `object` is a bare `Ident` naming a registered class.
