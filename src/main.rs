#![allow(dead_code)]

mod ast;
mod interpreter;
mod lexer;
mod parser;
mod resolver;
mod typechecker;

use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;
use typechecker::TypeChecker;

/// CLI entry point. Parses `std::env::args()` and dispatches on the
/// subcommand: only `yara run <file>` is supported, which hands `<file>` off
/// to [`run_file`]. Any other invocation (missing subcommand, unknown
/// subcommand, or `run` with no file argument) prints a usage message to
/// stderr and exits with status 1.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: yara run <file>");
                std::process::exit(1);
            };
            run_file(path);
        }
        _ => {
            eprintln!("usage: yara run <file>");
            std::process::exit(1);
        }
    }
}

/// Runs the full pipeline on the file at `path`: read the source, then in
/// order [`Lexer`] -> [`Parser`] -> [`resolver::resolve_imports`] ->
/// [`TypeChecker`] -> [`Interpreter`]. Each stage's error is rendered with
/// [`print_error`] and the process exits with status 1 at the *first* stage
/// that fails — later stages never run against a program that didn't pass
/// the earlier ones.
fn run_file(path: &str) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{path}`: {e}");
            std::process::exit(1);
        }
    };

    let tokens = match Lexer::new(&source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            print_error(
                path,
                &source,
                "lex error",
                &e.message,
                e.line,
                e.column,
                &[],
            );
            std::process::exit(1);
        }
    };

    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => {
            print_error(
                path,
                &source,
                "parse error",
                &e.message,
                e.line,
                e.column,
                &[],
            );
            std::process::exit(1);
        }
    };

    let program = match resolver::resolve_imports(program, std::path::Path::new(path)) {
        Ok(p) => p,
        Err(e) => {
            print_error(
                path,
                &source,
                "import error",
                &e.message,
                e.line,
                e.column,
                &[],
            );
            std::process::exit(1);
        }
    };

    if let Err(e) = TypeChecker::new().check_program(&program) {
        print_error(
            path,
            &source,
            "type error",
            &e.message,
            e.line,
            e.column,
            &[],
        );
        std::process::exit(1);
    }

    if let Err(e) = Interpreter::new().run_program(&program) {
        let frames: Vec<(String, usize, usize)> = e
            .call_stack
            .iter()
            .rev()
            .map(|f| (f.function_name.clone(), f.line, f.column))
            .collect();
        print_error(
            path,
            &source,
            "runtime error",
            &e.message,
            e.line,
            e.column,
            &frames,
        );
        std::process::exit(1);
    }
}

/// Renders a rustc-style error: a header naming the file:line:column, the
/// offending source line with a `^` caret under the exact column, and (for
/// runtime errors) the same treatment for every call-stack frame, innermost
/// first — so the *source*, not just a bare line number, is always visible.
///
/// Note: line/column are only meaningful against `source` when they belong to
/// `path` itself. Imports are spliced in before typechecking/interpretation
/// (see `resolver`), so an error whose position actually originated in an
/// imported file will render the wrong snippet — a known gap until errors
/// carry their own file path.
fn print_error(
    path: &str,
    source: &str,
    kind: &str,
    message: &str,
    line: usize,
    column: usize,
    frames: &[(String, usize, usize)],
) {
    eprintln!("{kind}: {message}");
    eprintln!("  --> {path}:{line}:{column}");
    eprint!("{}", render_snippet(source, line, column));
    for (name, fline, fcolumn) in frames {
        eprintln!("  in `{name}` at {path}:{fline}:{fcolumn}");
        eprint!("{}", render_snippet(source, *fline, *fcolumn));
    }
}

fn render_snippet(source: &str, line: usize, column: usize) -> String {
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
}
