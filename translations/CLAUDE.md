# translations/

Bundled keyword-translation files, consumed by `yara run <file> --keywords <path>` (parsed by `src/translations::parse_keyword_file`). See `src/translations/CLAUDE.md` for the mechanism, `examples/translations/CLAUDE.md` for a runnable example.

## Status
`pt.keywords` (Portuguese) — the one bundled proof-of-concept, verified against `examples/translations/hello_pt.yara`.

## Files
- `pt.keywords` — translates all 15 keywords (`def`, `end`, `if`, `elsif`, `else`, `while`, `for`, `in`, `const`, `return`, `import`, `class`, `nil`, `true`, `false`) to Portuguese. Doesn't have to be exhaustive (see `src/translations/CLAUDE.md` on partial translation files), but is, as the reference example.

## Gotchas
- This is not a directory of every language — just the one bundled example proving the mechanism round-trips. Adding a new language means adding a new `<lang>.keywords` file here (format documented in `src/translations/CLAUDE.md`) and, ideally, a matching example under `examples/translations/`.
