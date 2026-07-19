# lexer/

Tokenizes Yara source text into a `Vec<Token>` (or streaming iterator).

## Status
Implemented. `Lexer::new(source).tokenize() -> Result<Vec<Token>, LexError>`.

## Design
- `Token { kind: TokenKind, line, column }`, 1-indexed position from the start of the token.
- `TokenKind` covers literals (Int/Float/Str/Bool), identifiers, keywords (`def end if elsif else while for in const return nil true false`), operators (`+ - * / == != < > <= >= = := : .. ( ) ,`), `Eof`.
- Comments (`#...`) and whitespace skipped in `skip_whitespace_and_comments`.
- String literals support `\n \t \" \\` escapes; unterminated string/char errors report line:column via `LexError`.
- Type-alias normalization lives in `normalize_type_alias()` (`Int`->`Integer`, `Bool`->`Boolean`, `Str`->`String`) — NOT applied during lexing itself (identifiers stay raw `Ident(String)`); parser/typechecker should call this helper when resolving type annotations, so `Int` and `Integer` compare equal downstream.
- Tests in `mod.rs` (`cargo test`) cover tokenizing a function def, line/column tracking, literals, comments, range operator, unterminated string, alias normalization.

## Gotchas
- `!` alone (not `!=`) is a lex error — no unary `!`/`not` operator defined yet; add here if boolean negation syntax is decided.
- Range operator is `..` only (no `...`); single `.` is a lex error since Yara has no field-access dot yet.
