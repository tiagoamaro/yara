# parser/

Recursive-descent parser turning lexer tokens into `ast::` nodes.

## Status
TODO — not yet implemented.

## Requirements
- Parse function defs, var/const decls, expressions (arithmetic/comparison with correct precedence), `if`/`elsif`/`else`, `while`, `for ... in a..b`, base type annotations.
- Parse errors must report line:column of the offending token, with expected-vs-found message and source excerpt + caret.
- Accept both short/long type aliases interchangeably (`Int`/`Integer`, `Bool`/`Boolean`, `Str`/`String`).

## Gotchas
(none yet — update as discovered)
