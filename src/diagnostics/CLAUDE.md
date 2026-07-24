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
- `Span { line, column, file: Option<PathBuf> }`. `file` is always `None` today —
  it's the hook for the known imported-file-snippet gap (see root `CLAUDE.md` TODO):
  when errors start carrying their own file, `render` can prefer it with no further
  plumbing. Built with `Span::new(line, column)` at each error site (stages still
  store loose `line`/`column`; no full-AST `Span` migration was done).
- `render(&dyn Diagnostic, path, source) -> String` builds the whole block:
  `kind: message`, `  --> path:line:column`, the snippet, then a frame line +
  snippet per `Frame`. Returned as a `String` (not printed) so it's unit-testable;
  `main.rs`'s `stage` helper is the only caller and does the `eprint!` + exit.
- `render_snippet(source, line, column)` renders one gutter-aligned source line
  with a `^` caret; a line past EOF yields `""` rather than panicking.

## Gotchas
- `render` takes `path`/`source` from the caller, so the caret is only correct when
  the error's position belongs to that file. Translation errors are rendered against
  the *keyword file* (`main.rs` passes `kw_path`/`kw_text`); the imported-file gap
  remains until `Span.file` is populated.
- Output is byte-for-byte identical to the pre-refactor `print_error`. If you change
  the format here, the `examples/errors/*` golden comparisons (and anyone eyeballing
  error output) will notice — that identity is the Phase 1 acceptance check.
