# diagnostics/

Shared error *presentation* for every compiler stage. Owns how an error looks to
the user (rustc-style header + source snippet + caret); it does not own error
*data* — each stage keeps its own error type.

## Status
Implemented. Introduced in the modularization refactor (Phase 1) to replace the
old per-stage `match`-print-exit branches and the free `print_error`/`render_snippet`
that used to live in `main.rs`.

## Design
- `trait Diagnostic { kind() -> &str; message() -> &str; span() -> Span; frames() -> Vec<Frame> }`.
  Every stage error implements it *in its own module* (orphan-rule-fine, same crate):
  `LexError`, `ParseError`, `ResolveError`, `TypeError`, `RuntimeError`, `TranslationError`.
  The five stage types stay separate on purpose (locality + teaching) — this trait
  unifies only rendering, not the types.
- `kind()` is the label before the message (`"lex error"`, `"type error"`, …).
  `frames()` defaults to empty; only `RuntimeError` overrides it, returning its
  `call_stack` **reversed** (innermost first, the order the trace prints).
- `Span { line, column }` — just position, no file path. Lines are *virtual*
  after imports are resolved: the entry file occupies virtual lines 1..=N,
  and each imported file is appended via its own line-count offset.
  Built with `Span::new(line, column)` at each error site (stages still store loose
  `line`/`column`; no full-AST `Span` migration was done).
- `SourceMap` + `SourceFile { start_line, path, source }` — tracks the virtual
  line-space layout. Entry file is added first; `add_file(path, source)` appends
  each import, reserving its line count (minimum 1) and returning the offset
  used by the resolver when splicing. `lookup(line)` returns the file whose
  virtual-line range contains a given line. Same trick as rustc's SourceMap,
  with line numbers instead of byte offsets.
- `render_with_map(&dyn Diagnostic, &SourceMap) -> String` — resolves the primary
  span and every stack frame through the map: for each, look up which file and
  what local line, then fetch the source snippet from that file. Produces correct
  carets for post-import stages (typechecker, interpreter). Old `render(diag, path,
  source)` now delegates to `render_with_map` with a single-file map — output
  byte-identical for single-file programs (Phase 1 acceptance check still holds).
- `render(&dyn Diagnostic, path, source) -> String` builds the whole block:
  `kind: message`, `  --> path:line:column`, the snippet, then a frame line +
  snippet per `Frame`. Returned as a `String` (not printed) so it's unit-testable;
  `main.rs`'s `stage` helper is the only caller and does the `eprint!` + exit.
- `render_with_map_and_vocab(&dyn Diagnostic, &SourceMap, Option<&Vocabulary>)` —
  the actual implementation both `render` and `render_with_map` delegate to
  (each passing `vocab: None`). With `Some(vocab)`, localizes `diag.kind()`
  (via `localized_kind`, matching the six hardcoded English `kind()` strings —
  `"lex error"`, `"parse error"`, `"type error"`, `"runtime error"`,
  `"import error"`, `"keyword translation error"` — to catalog keys
  `diag/lex-error` … `diag/keyword-translation-error`) and the `in`/`at` words
  in each call-stack frame line (`diag/frame-in`/`diag/frame-at`), all in
  `src/translations/messages.rs::MESSAGES`. `main.rs`'s `stage_mapped` is the
  only caller that passes `Some(vocab)` (for the post-import stages); `stage`
  (lex/parse/translation errors) still calls plain `render`, so lex/parse-stage
  labels aren't localized yet — only what's reachable through `stage_mapped`.
  `kind()`'s trait signature is unchanged (still `&str`, still hardcoded
  English per stage) — only the *renderer* looks up a translation for it, kept
  this way so `render`/`render_with_map` (vocab: None) stay byte-identical to
  pre-localization output without touching every stage's error type.
- `render_snippet(source, line, column)` renders one gutter-aligned source line
  with a `^` caret; a line past EOF yields `""` rather than panicking.

## Gotchas
- `render` (single-file) and `render_with_map` (multi-file) produce output that is
  byte-for-byte identical to the pre-refactor `print_error` for single-file programs.
  If you change the format here, the `examples/errors/*` golden comparisons (and
  anyone eyeballing error output) will notice — that identity is the Phase 1
  acceptance check. Adding new span/frame fields or changing the caret character
  will break those comparisons.
- Post-import stages (typechecker, interpreter) use `render_with_map`; carets are
  now guaranteed correct even when errors originate from imported files (the
  imported-file snippet gap is FIXED). Translation errors are still rendered against
  the *keyword file* via plain `render` (single-file).
