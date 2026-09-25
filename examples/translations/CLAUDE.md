# examples/translations/

Programs demonstrating Yara's full-vocabulary translation feature (`--vocabulary <path>`, `--keywords <path>` alias still works — see `ruby/lib/yara/CLAUDE.md`).

## Status
`kitchen_sink_pt.yara` verified end to end (2026-07-25):
```
ruby/bin/yara run examples/translations/kitchen_sink_pt.yara \
  --vocabulary translations/pt.vocab
```
runs clean and prints expected values for every section. `fatorial_pt.yara` runs clean standalone (no output). `hello_pt.yara` still verified:
```
ruby/bin/yara run examples/translations/hello_pt.yara --vocabulary translations/pt.vocab
```
produces identical output to `examples/objects/hello.yara` (`5`, `10`, `3.14159`, `12.56636`).

## Files
- `hello_pt.yara` — the same class/const/field/method program as `examples/objects/hello.yara`, now written entirely in Portuguese vocabulary: keywords (`classe`, `constante`, `funcao`, `fim`, ...), type names (`Inteiro`, `Flutuante`), the builtin `escreva` (`print`), and `.novo` (`.new`) — every translatable category `translations/pt.vocab` covers. `initializer` (a fixed method name, not a translated vocabulary entry) stays as-is.
- `fatorial_pt.yara` — recursive `fatorial` helper, PT keywords (`funcao`/`fim`/`se`/`senao`), imported by `kitchen_sink_pt.yara`. Pure definitions, no top-level prints (so it runs clean standalone).
- `kitchen_sink_pt.yara` — PT mirror of `examples/kitchen_sink.yara`, but self-contained in one file (every feature inlined rather than imported per-feature) that `importar`s only `fatorial_pt`; showcases consts, control flow, functions (+recursion via the import), loops, array/string/int/float/bool methods, pointers (alloc/deref/set_deref/free as builtin + method), a deterministic `coletar()` GC sweep, a class with `.novo`, and single-parent inheritance (`classe Cae < Animal`). The two PT exceptions (`initializer`, import paths) are noted in the file header.

## Gotchas
- Running this file *without* `--vocabulary translations/pt.vocab` fails at the lexer: `classe`/`funcao`/`fim`/etc. aren't recognized keywords in the default English vocabulary, so they lex as plain identifiers and the parser then chokes on the resulting nonsense token stream. The flag isn't optional for this particular file, unlike every other example in `examples/`.
- A runtime/type error written in this same Portuguese vocabulary, with its localized error message, lives under `examples/errors/runtime_error_pt.yara` instead (kept with the other deliberately-broken examples, not here).
