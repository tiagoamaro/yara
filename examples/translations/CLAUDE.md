# examples/translations/

Programs demonstrating Yara's keyword-translation feature (`--keywords <path>`, see `src/translations/CLAUDE.md`).

## Status
`hello_pt.yara` verified end to end (2026-07-19):
```
cargo run -- run examples/translations/hello_pt.yara --keywords translations/pt.keywords
```
produces identical output to `examples/objects/hello.yara`.

## Files
- `hello_pt.yara` — the same class/const/field/method program as `examples/objects/hello.yara`, written with Portuguese keywords (`classe`, `constante`, `funcao`, `fim`, ...). Type names (`Integer`, `Float`), the method name `initializer`, and builtins (`new`, `print`) are unchanged — only the 15 reserved words in `lexer::KeywordToken` are ever translatable, see `src/translations/CLAUDE.md`'s Gotchas.

## Gotchas
- Running this file *without* `--keywords translations/pt.keywords` fails at the lexer: `classe`/`funcao`/`fim`/etc. aren't recognized keywords in the default English map, so they lex as plain identifiers and the parser then chokes on the resulting nonsense token stream. The flag isn't optional for this particular file, unlike every other example in `examples/`.
