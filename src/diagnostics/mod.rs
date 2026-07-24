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

use std::path::PathBuf;

/// A source position for a diagnostic: 1-indexed line/column, plus an optional
/// originating file.
///
/// `file` is `None` everywhere today — positions are still resolved against the
/// single entry-file the CLI passes to the renderer. It exists as the hook for
/// the known "imported-file snippet" gap (an error whose position belongs to an
/// `import`ed file currently renders against the wrong source): once errors
/// start carrying their own file here, the renderer can prefer it with no
/// further plumbing change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// 1-indexed source line.
    pub line: usize,
    /// 1-indexed source column.
    pub column: usize,
    /// The file this position belongs to, if known. `None` means "the file the
    /// renderer was handed" (see the type-level note on the imported-file gap).
    pub file: Option<PathBuf>,
}

impl Span {
    /// A file-less span at `line`/`column` — the common case, since positions
    /// are resolved against the renderer's entry file today.
    pub fn new(line: usize, column: usize) -> Self {
        Span {
            line,
            column,
            file: None,
        }
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
/// `path`/`source` are the file the caller wants positions resolved against.
/// When [`Span::file`] eventually becomes non-`None`, this is where a
/// per-error file would override them.
pub fn render(diag: &dyn Diagnostic, path: &str, source: &str) -> String {
    let span = diag.span();
    let mut out = String::new();
    out.push_str(&format!("{}: {}\n", diag.kind(), diag.message()));
    out.push_str(&format!("  --> {}:{}:{}\n", path, span.line, span.column));
    out.push_str(&render_snippet(source, span.line, span.column));
    for frame in diag.frames() {
        out.push_str(&format!(
            "  in `{}` at {}:{}:{}\n",
            frame.name, path, frame.span.line, frame.span.column
        ));
        out.push_str(&render_snippet(source, frame.span.line, frame.span.column));
    }
    out
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
}
