# examples/translations/

Programs demonstrating Yara's full-vocabulary translation feature (`--vocabulary <path>`, `--keywords <path>` alias still works — see `src/translations/CLAUDE.md`).

## Status
`hello_pt.yara` verified end to end (2026-07-25):
```
cargo run -- run examples/translations/hello_pt.yara --vocabulary translations/pt.vocab
```
produces identical output to `examples/objects/hello.yara` (`5`, `10`, `3.14159`, `12.56636`).

## Files
- `hello_pt.yara` — the same class/const/field/method program as `examples/objects/hello.yara`, now written entirely in Portuguese vocabulary: keywords (`classe`, `constante`, `funcao`, `fim`, ...), type names (`Inteiro`, `Flutuante`), the builtin `escreva` (`print`), and `.novo` (`.new`) — every translatable category `translations/pt.vocab` covers. `initializer` (a fixed method name, not a translated vocabulary entry) stays as-is.

## Gotchas
- Running this file *without* `--vocabulary translations/pt.vocab` fails at the lexer: `classe`/`funcao`/`fim`/etc. aren't recognized keywords in the default English vocabulary, so they lex as plain identifiers and the parser then chokes on the resulting nonsense token stream. The flag isn't optional for this particular file, unlike every other example in `examples/`.
- A runtime/type error written in this same Portuguese vocabulary, with its localized error message, lives under `examples/errors/runtime_error_pt.yara` instead (kept with the other deliberately-broken examples, not here).
