# src/translations/

Parses keyword-translation files (`canonical = localized` lines) into the `HashMap<String, lexer::KeywordToken>` that `lexer::Lexer::with_keywords` consumes. This is the whole implementation of "write Yara's keywords in another language" — nothing outside the lexer needs to know translation exists, since parser/typechecker/interpreter only ever see `TokenKind`, never the source spelling of a keyword.

## Status
Implemented. `parse_keyword_file(text: &str) -> Result<HashMap<String, KeywordToken>, TranslationError>`. Wired into `main.rs` via a `--keywords <path>` flag on `yara run`.

## Design
- Starts from `lexer::default_keywords()` (the English map) and only *overrides* entries a translation file actually mentions — a file only needs to list the keywords it wants to change; anything omitted keeps its English spelling. This is why `translations/pt.keywords` doesn't need every keyword, and why a near-empty translation file (translate just `if`) is a valid, useful thing to write.
- Translating a keyword *removes* its old spelling from the map before inserting the new one (tracked via a `canonical_to_current_spelling` side map) — so a translated file can never leave both the old and new spelling simultaneously valid for the same keyword.
- Two error cases beyond "unknown canonical name" (a typo on the left-hand side): an empty right-hand side (`if =`), and a localized spelling already claimed by a *different* keyword (mapping two canonical names to the same word) — both are `TranslationError`s naming the offending line, not silently ignored or last-write-wins.
- `TranslationError.line` is 1-indexed *within the translation file*, not within whatever `.yara` program it'll be applied to — this whole parse happens before the target program is even read (see `main.rs::run_file`), so there's no source-program position to report yet.
- File format: `#` starts a line comment (same character as Yara itself, for consistency), blank lines ignored, everything else must be `canonical = localized` split on the first `=`. No JSON/TOML — this project has zero external dependencies (`Cargo.toml` has no `[dependencies]`), and pulling in `serde` just for this would break that; the whole parser here is ~40 lines of `std`.

## Gotchas
- Only reserved words (see `lexer::KeywordToken::all()`) are translatable. Type names (`Int`/`Integer`/`IntArray`/...), identifiers, string contents, and error messages are explicitly out of scope and stay in English regardless of any translation file — `examples/translations/hello_pt.yara` still says `Integer`/`Float`, `initializer`, `new`, `print`.
- `full_bundled_portuguese_file_parses` (test) reads `translations/pt.keywords` off disk via `CARGO_MANIFEST_DIR` — if that file moves or is renamed, this test needs updating too.
