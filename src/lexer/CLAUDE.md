# lexer/

Tokenizes Yara source text into a `Vec<Token>` (or streaming iterator).

## Status
TODO — not yet implemented.

## Requirements
- Every `Token` must carry `line` and `column` (1-indexed) for diagnostics.
- Recognize: identifiers, keywords (`def`, `end`, `if`, `elsif`, `else`, `while`, `for`, `in`, `const`, `return`, `true`, `false`, `nil`), literals (Int, Float, String, Bool), operators (`+ - * / == != < > <= >= = := : .. ( ) , #`), line comments (`#...`).
- Normalize type-name aliases at this stage or in parser: `Int`=`Integer`, `Bool`=`Boolean`, `Str`=`String` (decide and document here once implemented).
- Lex errors must include line:column and a clear message (invalid char, unterminated string, etc).

## Gotchas
(none yet — update as discovered)
