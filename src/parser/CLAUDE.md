# parser/

Recursive-descent parser turning lexer tokens into `ast::` nodes.

## Status
Implemented. `Parser::new(tokens).parse_program() -> Result<Vec<Stmt>, ParseError>`.

## Design
- Precedence climbing via chained methods: `parse_comparison` -> `parse_additive` -> `parse_multiplicative` -> `parse_primary`. No unary operators yet.
- `parse_ident_stmt` disambiguates `x = expr` / `x: Type = expr` var-decl from a bare expression statement (e.g. a call `foo()`) by checkpointing `self.pos` and rewinding if no `:`/`=` follows the identifier.
- `parse_block(terminators)` collects statements until one of the given terminator token kinds (`end`/`elsif`/`else`) or errors on unexpected EOF with the position of the EOF token.
- Type aliases normalized here (not in lexer) via `lexer::normalize_type_alias`, in `parse_type_annotation`.
- `ParseError` mirrors `LexError` shape: `message`, `line`, `column`, `Display` impl `"{line}:{column}: {message}"`.
- Tests in `mod.rs` cover: function defs, var decl (inferred/explicit/aliased types), if/elsif/else, while, for-range, call expr statement, operator precedence, const decl, error position reporting.

## Gotchas
- `advance()` clamps at the last token (`Eof`) rather than panicking past the end — relies on `tokenize()` always appending a trailing `Eof`.
- `check()` compares `std::mem::discriminant`, so it matches only the token *kind* shape, not payload (e.g. any `Ident(_)` matches `Ident("".into())` used as a placeholder in terminator lists) — fine since terminators used so far carry no payload.
