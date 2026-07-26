# translations/

Bundled vocabulary-translation files, consumed by `yara run <file> --vocabulary <path>` (`--keywords <path>` still works as an alias), parsed by `src/translations::parse_vocabulary`. See `src/translations/CLAUDE.md` for the mechanism, `examples/translations/CLAUDE.md` for a runnable example.

## Status
`pt.vocab` (Portuguese) — the one bundled proof-of-concept, verified against `examples/translations/hello_pt.yara` and `examples/errors/runtime_error_pt.yara`.

## Files
- `pt.vocab` — sectioned `[keywords]/[types]/[builtins]/[methods]/[messages]` file (format documented in `src/translations/CLAUDE.md`). `[keywords]` translates all 15 reserved words (`def`, `end`, `if`, `elsif`, `else`, `while`, `for`, `in`, `const`, `return`, `import`, `class`, `nil`, `true`, `false`). `[types]` covers every primitive/array type name plus `Ptr`. `[builtins]` covers all free-function builtins (`print`, `len`, `push`, `get`, `set`, `pop`, `alloc`, `deref`, `set_deref`, `free`, `collect`). `[methods]` covers every primitive method (`size`, `push`, `get`, `set`, `pop`, `is_empty`, `upper`, `lower`, `trim`, `to_i`, `to_f`, `to_s`, `abs`, `deref`, `set_deref`, `free`) plus the special `.new` constructor (`new = novo`). `[messages]` covers a large majority of the catalog in `src/translations/messages.rs` (128 keys) — not exhaustive, since message-catalog conversion itself isn't complete for every stage (see that file's own status notes); anything omitted falls back to English. Doesn't have to be exhaustive in any section (see `src/translations/CLAUDE.md` on partial vocabulary files), but is, as the reference example.

## Gotchas
- This is not a directory of every language — just the one bundled example proving the mechanism round-trips across all five sections. Adding a new language means adding a new `<lang>.vocab` file here (format documented in `src/translations/CLAUDE.md`) and, ideally, a matching example under `examples/translations/`.
- `full_bundled_portuguese_file_parses`-style tests read `pt.vocab` off disk via `CARGO_MANIFEST_DIR` — if this file moves or is renamed, those tests need updating too.
