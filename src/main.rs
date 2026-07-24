use yara::diagnostics::{self, Diagnostic};
use yara::interpreter::Interpreter;
use yara::lexer::{self, Lexer};
use yara::parser::Parser;
use yara::typechecker::TypeChecker;
use yara::{resolver, translations};

/// CLI entry point. Parses `std::env::args()` and dispatches on the
/// subcommand: only `yara run <file> [--keywords <path>]` is supported,
/// which hands `<file>` (and the optional keyword-translation file) off to
/// [`run_file`]. Any other invocation (missing subcommand, unknown
/// subcommand, or `run` with no file argument) prints a usage message to
/// stderr and exits with status 1.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: yara run <file> [--keywords <path>]");
                std::process::exit(1);
            };
            let keywords_path = parse_keywords_flag(&args[3..]);
            run_file(path, keywords_path.as_deref());
        }
        _ => {
            eprintln!("usage: yara run <file> [--keywords <path>]");
            std::process::exit(1);
        }
    }
}

/// Scans the arguments following `<file>` for `--keywords <path>`, Yara's
/// only optional flag. Hand-rolled rather than pulling in an args-parsing
/// crate, matching the rest of the project's zero-dependency stance.
fn parse_keywords_flag(rest: &[String]) -> Option<String> {
    let pos = rest.iter().position(|a| a == "--keywords")?;
    rest.get(pos + 1).cloned()
}

/// Wraps a stage's `Result`: returns the success value, or renders the error
/// rustc-style to stderr (via [`diagnostics::render`]) and exits with status 1.
/// Every stage flows through this one helper, so [`run_file`] reads as a linear
/// pipeline instead of a stack of near-identical match-print-exit branches, and
/// error rendering lives in exactly one place. `path`/`source` are the file
/// positions render against.
fn stage<T, E: Diagnostic>(result: Result<T, E>, path: &str, source: &str) -> T {
    match result {
        Ok(value) => value,
        Err(err) => {
            eprint!("{}", diagnostics::render(&err, path, source));
            std::process::exit(1);
        }
    }
}

/// Runs the full pipeline on the file at `path`: read the source, then in
/// order [`Lexer`] -> [`Parser`] -> [`resolver::resolve_imports`] ->
/// [`TypeChecker`] -> [`Interpreter`]. Each fallible stage is wrapped in
/// [`stage`], which renders the error and exits at the *first* stage that fails
/// — later stages never run against a program that didn't pass the earlier ones.
///
/// `keywords_path`, if present, names a translation file (see
/// `translations::parse_keyword_file`) whose keyword table the lexer uses
/// instead of the English default — read and parsed *before* the source file
/// itself, against its own path/text, since a bad translation file should fail
/// independently of whatever program it would have been applied to.
fn run_file(path: &str, keywords_path: Option<&str>) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{path}`: {e}");
            std::process::exit(1);
        }
    };

    let keywords = match keywords_path {
        Some(kw_path) => {
            let kw_text = match std::fs::read_to_string(kw_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: cannot read `{kw_path}`: {e}");
                    std::process::exit(1);
                }
            };
            // A translation error renders against the keyword file itself, not
            // the program that would have used it.
            stage(
                translations::parse_keyword_file(&kw_text),
                kw_path,
                &kw_text,
            )
        }
        None => lexer::default_keywords(),
    };

    let tokens = stage(
        Lexer::with_keywords(&source, keywords).tokenize(),
        path,
        &source,
    );
    let program = stage(Parser::new(tokens).parse_program(), path, &source);
    let program = stage(
        resolver::resolve_imports(program, std::path::Path::new(path)),
        path,
        &source,
    );
    stage(TypeChecker::new().check_program(&program), path, &source);
    stage(Interpreter::new().run_program(&program), path, &source);
}
