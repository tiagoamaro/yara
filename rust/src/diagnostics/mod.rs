//! Shared error-rendering vocabulary for every compiler stage.
//!
//! Each stage (`lexer`, `parser`, `resolver`, `typechecker`, `interpreter`,
//! `translations`) keeps its *own* distinct error type — that locality is
//! deliberate, both for teaching and so a stage's errors carry exactly the
//! fields that stage needs. What they share is only how they're *presented* to
//! the user: a rustc-style header + source snippet + caret. This module owns
//! that presentation, behind one [`Diagnostic`] trait every error type
//! implements, so the CLI renders them all through a single path instead of a
//! per-stage `match`.

/// A source position for a diagnostic: 1-indexed line/column.
///
/// The line is a *virtual* line once imports are resolved: the resolver
/// assigns every file a disjoint line range in one shared line space (the
/// entry file keeps its natural lines; each imported file's statements are
/// shifted past everything registered before it — see [`SourceMap`]). A span
/// therefore never needs to carry a file path itself: [`SourceMap::lookup`]
/// recovers the file and local line from the virtual line alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// 1-indexed source line (virtual once imports are spliced; see above).
    pub line: usize,
    /// 1-indexed source column.
    pub column: usize,
}

impl Span {
    /// A span at `line`/`column`.
    pub fn new(line: usize, column: usize) -> Self {
        Span { line, column }
    }
}

/// One file registered in a [`SourceMap`]: where its virtual line range
/// starts, plus everything needed to render a snippet from it.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// First virtual line this file occupies (1 for the entry file).
    pub start_line: usize,
    /// Display path used in diagnostic headers.
    pub path: String,
    /// The file's full source text, for snippet rendering.
    pub source: String,
}

/// Maps virtual diagnostic lines back to (file, local line) — the same trick
/// rustc's `SourceMap` plays with byte offsets, done here with line numbers.
///
/// The entry file always occupies lines `1..=N` (so single-file programs are
/// completely unaffected: virtual line == local line). Each imported file is
/// appended after everything registered so far via [`SourceMap::add_file`],
/// which hands back the line offset the resolver must shift that file's AST
/// positions by ([`crate::ast::Stmt::shift_lines`]).
#[derive(Debug, Clone)]
pub struct SourceMap {
    /// Registered files, in registration order — `start_line` is strictly
    /// ascending, which is what makes [`SourceMap::lookup`]'s "last file at or
    /// before this line" scan correct.
    files: Vec<SourceFile>,
}

impl SourceMap {
    /// A map containing just the entry file, occupying virtual lines from 1.
    pub fn new(entry_path: &str, entry_source: &str) -> Self {
        SourceMap {
            files: vec![SourceFile {
                start_line: 1,
                path: entry_path.to_string(),
                source: entry_source.to_string(),
            }],
        }
    }

    /// Registers an imported file after every file added so far and returns
    /// the *line offset* its positions must be shifted by (`local + offset =
    /// virtual`). The reserved range is the file's own line count (at least 1,
    /// so even an empty file occupies a distinct range).
    pub fn add_file(&mut self, path: String, source: String) -> usize {
        let last = self.files.last().expect("source map always has entry file");
        let start_line = last.start_line + last.source.lines().count().max(1);
        self.files.push(SourceFile {
            start_line,
            path,
            source,
        });
        start_line - 1
    }

    /// Resolves a virtual line to the file containing it: the last registered
    /// file whose range starts at or before `line`. Lines before every range
    /// (only possible for line 0, which no 1-indexed position produces) fall
    /// back to the entry file.
    pub fn lookup(&self, line: usize) -> &SourceFile {
        self.files
            .iter()
            .rev()
            .find(|f| f.start_line <= line)
            .unwrap_or(&self.files[0])
    }
}

/// One entry in a diagnostic's call-stack trace: a function/method name and the
/// source position of its call site. Mirrors `interpreter::StackFrame`, but
/// lives here so the renderer depends on this module rather than on the
/// interpreter's internals.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The active function/method's name (e.g. `factorial`, `Hello#area`).
    pub name: String,
    /// The call-site position (not the position inside the callee).
    pub span: Span,
}

/// Anything the CLI can render as a rustc-style error. Every stage's error type
/// implements this; the renderer ([`render`]) only ever sees `&dyn Diagnostic`,
/// so adding a new stage means implementing this trait, not touching the CLI's
/// error-printing code.
pub trait Diagnostic {
    /// Short stage label shown before the message, e.g. `"lex error"`,
    /// `"type error"`, `"runtime error"`.
    fn kind(&self) -> &str;
    /// The human-readable message, *without* any position (the renderer adds
    /// the `file:line:column` header itself).
    fn message(&self) -> &str;
    /// The primary source position the error points at.
    fn span(&self) -> Span;
    /// Call-stack frames, innermost first (the order they should print in).
    /// Defaults to empty — only runtime errors carry a trace.
    fn frames(&self) -> Vec<Frame> {
        Vec::new()
    }
}

/// Renders a diagnostic rustc-style into a string (returned rather than printed
/// so it is unit-testable and the CLI decides where it goes): a header naming
/// `file:line:column`, the offending source line with a `^` caret under the
/// exact column, and — for a runtime error's trace — the same treatment for
/// every call-stack frame, innermost first.
///
/// `path`/`source` are the single file positions resolve against — the right
/// call for pre-import stages (lexing/parsing the entry file, translation
/// files). Once imports may have been spliced in, use [`render_with_map`],
/// which this delegates to via a single-file [`SourceMap`] (so both paths
/// share one formatting implementation and stay byte-identical).
pub fn render(diag: &dyn Diagnostic, path: &str, source: &str) -> String {
    render_with_map_and_vocab(diag, &SourceMap::new(path, source), None)
}

/// Like [`render`], but resolves every position (the primary span and each
/// call-stack frame) through `map`, so an error whose virtual line falls in
/// an imported file's range gets that file's path, local line, and snippet —
/// not the entry file's.
pub fn render_with_map(diag: &dyn Diagnostic, map: &SourceMap) -> String {
    render_with_map_and_vocab(diag, map, None)
}

/// Like [`render_with_map`], but when `vocab` is `Some`, localizes the stage
/// label (`diag.kind()`, e.g. `"lex error"`) and the `in`/`at` words in each
/// call-stack frame line through the message catalog (`diag/lex-error`,
/// `diag/parse-error`, ..., `diag/frame-in`, `diag/frame-at`). `vocab: None`
/// (used by [`render`] and [`render_with_map`]) keeps every word exactly as
/// `diag.kind()` returns it and hardcodes `in`/`at` — byte-identical to the
/// pre-localization output, which is what keeps `tests/golden/*.stderr` and
/// the renderer unit tests below passing unchanged.
pub fn render_with_map_and_vocab(
    diag: &dyn Diagnostic,
    map: &SourceMap,
    vocab: Option<&crate::translations::Vocabulary>,
) -> String {
    let span = diag.span();
    let mut out = String::new();
    let kind = localized_kind(diag.kind(), vocab);
    out.push_str(&format!("{}: {}\n", kind, diag.message()));
    let file = map.lookup(span.line);
    let local = span.line - file.start_line + 1;
    out.push_str(&format!("  --> {}:{}:{}\n", file.path, local, span.column));
    out.push_str(&render_snippet(&file.source, local, span.column));
    let (in_word, at_word) = match vocab {
        Some(v) => (v.msg("diag/frame-in", &[]), v.msg("diag/frame-at", &[])),
        None => ("in".to_string(), "at".to_string()),
    };
    for frame in diag.frames() {
        let file = map.lookup(frame.span.line);
        let local = frame.span.line - file.start_line + 1;
        out.push_str(&format!(
            "  {} `{}` {} {}:{}:{}\n",
            in_word, frame.name, at_word, file.path, local, frame.span.column
        ));
        out.push_str(&render_snippet(&file.source, local, frame.span.column));
    }
    out
}

/// Maps a stage's hardcoded English `kind()` label to its catalog key and
/// looks up `vocab`'s localized spelling; `vocab: None` or an unrecognized
/// `kind` (a synthetic test diagnostic, e.g.) passes `kind` through unchanged.
fn localized_kind(kind: &str, vocab: Option<&crate::translations::Vocabulary>) -> String {
    let Some(vocab) = vocab else {
        return kind.to_string();
    };
    let key = match kind {
        "lex error" => "diag/lex-error",
        "parse error" => "diag/parse-error",
        "type error" => "diag/type-error",
        "runtime error" => "diag/runtime-error",
        "import error" => "diag/import-error",
        "keyword translation error" => "diag/keyword-translation-error",
        _ => return kind.to_string(),
    };
    vocab.msg(key, &[])
}

/// Renders a single source line with a caret under `column`, gutter-aligned:
///
/// ```text
///   |
/// 4 | z = x + y
///   |       ^
/// ```
///
/// A `line` past the end of `source` yields an empty string (rather than
/// panicking) so an EOF-position error degrades gracefully.
pub fn render_snippet(source: &str, line: usize, column: usize) -> String {
    let Some(text) = source.lines().nth(line.saturating_sub(1)) else {
        return String::new();
    };
    let gutter = format!("{line}");
    let pad = " ".repeat(gutter.len());
    let caret_pad = " ".repeat(column.saturating_sub(1));
    format!("{pad} |\n{gutter} | {text}\n{pad} | {caret_pad}^\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The caret line should line up under the exact 1-indexed column passed
    /// in, not the start of the line.
    #[test]
    fn snippet_points_caret_at_column() {
        let source = "x = 5\ny = x @ 2\n";
        let snippet = render_snippet(source, 2, 7);
        assert_eq!(snippet, "  |\n2 | y = x @ 2\n  |       ^\n");
    }

    /// A line number past the end of the source (e.g. an EOF-position parse
    /// error) should render as an empty string rather than panicking.
    #[test]
    fn snippet_out_of_range_line_is_empty() {
        assert_eq!(render_snippet("only one line\n", 5, 1), "");
    }

    /// The left-hand gutter padding should widen to match the line number's
    /// digit count (e.g. 2 chars for line 10), keeping the `|` columns
    /// aligned across the blank/text/caret rows.
    #[test]
    fn snippet_gutter_width_matches_line_number_digits() {
        let source = "\n".repeat(9) + "tenth line";
        let snippet = render_snippet(&source, 10, 3);
        assert!(snippet.starts_with("   |\n10 | tenth line\n   |   ^\n"));
    }

    /// A stage error with no call stack renders header + snippet and nothing
    /// else; the `kind`/`message`/position all come through the trait.
    #[test]
    fn render_uses_trait_kind_message_and_span() {
        struct Dummy;
        impl Diagnostic for Dummy {
            fn kind(&self) -> &str {
                "type error"
            }
            fn message(&self) -> &str {
                "boom"
            }
            fn span(&self) -> Span {
                Span::new(2, 7)
            }
        }
        let rendered = render(&Dummy, "prog.yara", "x = 5\ny = x @ 2\n");
        assert_eq!(
            rendered,
            "type error: boom\n  --> prog.yara:2:7\n  |\n2 | y = x @ 2\n  |       ^\n"
        );
    }

    /// `add_file` must hand out disjoint, ascending ranges: the first import
    /// starts right after the entry file's last line, the next right after
    /// that, and `lookup` must resolve a line inside each range to its file.
    #[test]
    fn source_map_assigns_disjoint_ranges_and_looks_them_up() {
        let mut map = SourceMap::new("main.yara", "a\nb\nc\n"); // lines 1..=3
        let off1 = map.add_file("one.yara".into(), "x\ny\n".into()); // lines 4..=5
        let off2 = map.add_file("two.yara".into(), "z\n".into()); // line 6
        assert_eq!(off1, 3);
        assert_eq!(off2, 5);
        assert_eq!(map.lookup(2).path, "main.yara");
        assert_eq!(map.lookup(4).path, "one.yara");
        assert_eq!(map.lookup(5).path, "one.yara");
        assert_eq!(map.lookup(6).path, "two.yara");
    }

    /// An empty imported file must still reserve one line so the next file's
    /// range stays distinct.
    #[test]
    fn source_map_empty_file_reserves_a_line() {
        let mut map = SourceMap::new("main.yara", "a\n");
        let off_empty = map.add_file("empty.yara".into(), "".into());
        let off_next = map.add_file("next.yara".into(), "x\n".into());
        assert_eq!(off_empty, 1);
        assert_eq!(off_next, 2);
    }

    /// A span whose virtual line lands in an imported file's range must render
    /// with that file's path, *local* line number, and source snippet.
    #[test]
    fn render_with_map_uses_imported_file_for_its_range() {
        struct Dummy;
        impl Diagnostic for Dummy {
            fn kind(&self) -> &str {
                "type error"
            }
            fn message(&self) -> &str {
                "boom"
            }
            fn span(&self) -> Span {
                Span::new(4, 5) // virtual line 4 = helper.yara local line 2
            }
        }
        let mut map = SourceMap::new("main.yara", "a\nb\n"); // lines 1..=2
        map.add_file("helper.yara".into(), "h1\nbad line\n".into()); // 3..=4
        let rendered = render_with_map(&Dummy, &map);
        assert_eq!(
            rendered,
            "type error: boom\n  --> helper.yara:2:5\n  |\n2 | bad line\n  |     ^\n"
        );
    }

    /// With a localized `Vocabulary` (a `[messages]` override for the stage
    /// label and the frame `in`/`at` words), `render_with_map_and_vocab` must
    /// localize both the header and the call-stack-frame line; `render`/
    /// `render_with_map` (vocab: None) must stay untouched (asserted by the
    /// two tests above, unchanged).
    #[test]
    fn render_with_map_and_vocab_localizes_kind_and_frame_words() {
        struct Dummy;
        impl Diagnostic for Dummy {
            fn kind(&self) -> &str {
                "type error"
            }
            fn message(&self) -> &str {
                "boom"
            }
            fn span(&self) -> Span {
                Span::new(1, 1)
            }
            fn frames(&self) -> Vec<Frame> {
                vec![Frame {
                    name: "helper".to_string(),
                    span: Span::new(1, 1),
                }]
            }
        }
        let vocab = crate::translations::parse_vocabulary(
            "[messages]\ndiag/type-error = erro de tipo\ndiag/frame-in = em\ndiag/frame-at = as\n",
        )
        .unwrap();
        let map = SourceMap::new("main.yara", "x = 1\n");
        let rendered = render_with_map_and_vocab(&Dummy, &map, Some(&vocab));
        assert!(rendered.starts_with("erro de tipo: boom\n"));
        assert!(rendered.contains("  em `helper` as main.yara:1:1\n"));
    }
}
