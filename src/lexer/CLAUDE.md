# lexer/

Tokenizes Yara source text into a `Vec<Token>` (or streaming iterator).

## Status
Implemented. `Lexer::new(source).tokenize() -> Result<Vec<Token>, LexError>`, or `Lexer::with_vocabulary(source, Rc<Vocabulary>)` to also localize error-message prose (see Design below) — `::new()`/`::with_keywords()` are lighter-weight constructors that keep error messages in English regardless of any keyword translation.

## Design
- `Token { kind: TokenKind, line, column }`, 1-indexed position from the start of the token.
- `TokenKind` covers literals (Int/Float/Str/Bool), identifiers, keywords (`def end if elsif else while for in const return nil true false`), operators (`+ - * / == != < > <= >= = := : .. ( ) ,`), `Eof`. Also carries a hand-written `impl Display` that reproduces `#[derive(Debug)]`'s exact rendering (tuple variants `Ident("foo")`, unit variants `Plus`) — used by `parser/`'s "expected X, found `{:?}`"-shaped errors so they can build a `vocab.msg` argument via `.to_string()` without changing the rendered text.
- Comments (`#...`) and whitespace skipped in `skip_whitespace_and_comments`.
- String literals support `\n \t \" \\` escapes; unterminated string/char errors report line:column via `LexError`.
- Keyword vocabulary is a single `KEYWORDS: &[(&str, KeywordToken)]` const; `canonical_name`, `all`, and `default_keywords` all derive from it (no parallel lists to keep in sync).
- Type-alias normalization is NOT in the lexer — it moved to `src/types.rs` (`types::normalize_type_alias`, `Int`->`Integer`, `Bool`->`Boolean`, `Str`->`String`) so the parser no longer imports from the lexer just to canonicalize a type name. Not applied during lexing itself either way (identifiers stay raw `Ident(String)`).
- `Lexer` carries a `vocab: Rc<Vocabulary>` field (defaulting to `Vocabulary::english()` for `new`/`with_keywords`) so every `LexError.message` (invalid float/integer literal, invalid escape sequence, unexpected character, unterminated string) is built via `vocab.msg("lex/...", &args)` rather than an inline `format!` — a translated vocabulary's `[messages]` section can override any of these. `main.rs` and `resolver::resolve` (for imported files) both construct via `Lexer::with_vocabulary` so this reaches every lexed file in a run, not just the entry file.
- Tests in `mod.rs` (`cargo test`) cover tokenizing a function def, line/column tracking, literals, comments, range operator, unterminated string, alias normalization, `TokenKind::Display` matching derived `Debug`, and a `[messages]`-overridden vocabulary translating a lexer error end-to-end.

## Gotchas
- `!` alone (not `!=`) is a lex error — no unary `!`/`not` operator defined yet; add here if boolean negation syntax is decided.
- Range operator is `..` only (no `...`); single `.` is a lex error since Yara has no field-access dot yet.
