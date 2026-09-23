# src/translations/

Parses vocabulary files into a `Vocabulary` (`mod.rs`) and holds the English message catalog (`messages.rs`). One `Rc<Vocabulary>` is threaded through `Lexer`/`Parser`/`resolver`/`TypeChecker`/`Interpreter` via their `with_vocabulary` constructors, so a single file controls keyword, type, builtin and method spellings plus error-message prose for a whole run.

## Status
Implemented. `parse_vocabulary(text) -> Result<Vocabulary, TranslationError>` is what `main.rs` calls for `--vocabulary <path>` (alias `--keywords <path>`). `parse_keyword_file` is the older keywords-only parser; only its own tests call it now.

## Design
- Sections: `[keywords]`, `[types]`, `[builtins]`, `[methods]`, `[messages]`. Lines before the first header default to `[keywords]`, so a header-less keyword file still parses.
- `Vocabulary::english()` derives every canonical name from the existing registries (`default_keywords`, `primitive_type_names` plus aliases/array names/`Array`/`Ptr`, `BUILTINS` plus `print`, `METHODS` plus `new`), so this module keeps no second list of names that could drift.
- A file only overrides what it mentions; everything else keeps its English spelling or message. A file translating just `if` is valid.
- Each non-message section keeps an inbound map (localized -> canonical, used by the typechecker/interpreter to resolve source spellings) and an outbound map (canonical -> localized, used to render names back into messages: `type_name`, `localized_method_names`, `bool_word`).
- Translating a keyword removes its old spelling, so the old and new spelling are never both valid.
- `TranslationError`s: unknown canonical name, empty right-hand side (`if =`), and a localized spelling already claimed by a different canonical name in the same section. `TranslationError.line` is 1-indexed within the vocabulary file, since the file is parsed before the program is read.
- File format: `#` line comments, blank lines ignored, `canonical = localized` split on the first `=`. Hand-written in `std`, no `serde`, since `Cargo.toml` has no `[dependencies]`.

## Gotchas
- Identifiers, string contents, `initializer`, and import paths are never translatable.
- `full_bundled_portuguese_file_parses` (test) reads `translations/pt.vocab` off disk via `CARGO_MANIFEST_DIR`; renaming that file breaks the test.

## Message catalog (`messages.rs`)
`MESSAGES` holds 128 `(key, English template)` pairs namespaced by stage: `type/` 74, `runtime/` 31, `diag/` 8, `resolve/` 6, `lex/` 5, `parse/` 4. `Vocabulary::msg(key, args)` returns the `[messages]` override if present, else the English template, with `{0}`, `{1}`, ... substituted.
- Converted: every `format!`-built error message in `lexer/`, `parser/`, `resolver/`, `typechecker/` and `interpreter/`, plus the renderer's stage labels and call-stack `in`/`at` words (`diag/` keys, see `src/diagnostics/CLAUDE.md`).
- Still hardcoded English: a few fixed-string messages built with `.to_string()`, not `format!`: `typechecker/statements.rs` (`for` range bounds), `lexer/mod.rs` (unexpected `!`), `parser/mod.rs` and `parser/statements.rs` (unexpected EOF expecting `end`, which duplicates the existing `parse/unexpected-eof-expected-end` key; invalid assignment target), `interpreter/calls.rs` and `interpreter/methods.rs` (nil `deref`/`set_deref`/`free`, `pop` on empty array). Lex/parse/translation-stage labels also stay English because `main.rs`'s `stage` renders with plain `diagnostics::render`.
- Not error prose, so not catalogued: stack-frame labels (`{class}.new`, `{class}#{method}` in `interpreter/classes.rs` and `typechecker/classes.rs`), the `Ptr<{inner}>` type-name builder in `parser::parse_type_annotation`, test-only strings.
- `parser::TokenKind` has a hand-written `impl Display` (`src/lexer/mod.rs`) reproducing `#[derive(Debug)]` output, so `expected {0}, found {1}` sites can pass `tok.kind.to_string()` to `msg` without changing rendered text.
- Every English template must reproduce the pre-conversion string byte-for-byte. Guarded by `tests/golden/*.stderr`, each module's message assertions, and localization tests (`lexer::localized_vocabulary_translates_lexer_errors`, `interpreter::localized_vocabulary_translates_runtime_error_messages`, `::localized_vocabulary_falls_back_to_english_for_untranslated_keys`, the `typechecker` vocabulary tests, `tests/vocabulary_end_to_end.rs`).
