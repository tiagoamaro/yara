# editors/vscode-yara/

Minimal VS Code extension providing TextMate-grammar syntax highlighting for `.yara` files. Not a Rust crate, not part of the `cargo build`/`cargo test`/`cargo fmt` checks that gate everything under `src/`.

## Status
Implemented: `package.json` (extension manifest), `language-configuration.json` (comments/brackets), `syntaxes/yara.tmLanguage.json` (the actual grammar). Not published to the Marketplace — local-install only (symlink into `~/.vscode/extensions/`, see this folder's own `README.md`).

## Design
- The grammar's keyword/type/operator lists are meant to mirror `src/lexer/mod.rs`'s `TokenKind` exactly (the `read_ident_or_keyword` keyword match, `normalize_type_alias`'s alias pairs, and the array type names from `typechecker::Type::from_annotation_name`) — if the language grows a new keyword or type alias, update both the lexer and this grammar together, or the editor and the compiler will silently disagree about what's a keyword. Types currently recognized: `Int`, `Integer`, `Float`, `Bool`, `Boolean`, `Str`, `String`, `Nil`, `IntArray`, `FloatArray`, `BoolArray`, `StringArray`, `Ptr`.
- Pattern order in `yara.tmLanguage.json`'s top-level `patterns` array matters: `class-decl`/`function-decl` (which capture `class Name`/`def name` as two separate scopes) are listed *before* the generic `keywords` pattern, so `class`/`def` get matched together with their following name rather than the bare-keyword pattern grabbing just the keyword and leaving the name to fall through as a plain identifier. TextMate/Oniguruma resolves overlapping patterns by list order at a given position, not by longest-match — get this order wrong and class/function names silently stop getting their own highlight color.
- No indentation-based folding config — `def`/`class`/`if`/`while`/`for` ... `end` blocks aren't given explicit fold markers. Not needed for highlighting; add a `folding` block to `language-configuration.json` if that's ever wanted.

## Gotchas
- TextMate grammars are regex over text, not aware of Yara's real grammar (`src/parser/`). Concretely: a capitalized identifier used as a class-name type annotation (`h: Hello`) only highlights as a type if it's one of the built-in names (`Int`, `IntArray`, etc.) — an arbitrary user class name falls through to a plain identifier, since the grammar has no way to know every `class` declared in the file without a real parse. Same limitation for `.name` after an expression: field access and method calls get identical highlighting, since telling them apart needs to know whether `name` is a method or a field, which isn't visible to regex.
- This extension has no tests and nothing here participates in the root `CLAUDE.md` convention of "run `cargo test`/`cargo fmt` before finishing a change" — verification is manual/visual (open example `.yara` files in VS Code and look).
