use std::rc::Rc;
use yara::diagnostics::{self, Diagnostic};
use yara::interpreter::Interpreter;
use yara::lexer::Lexer;
use yara::parser::Parser;
use yara::translations::Vocabulary;
use yara::typechecker::TypeChecker;
use yara::{resolver, translations};

/// CLI entry point. Parses `std::env::args()` and dispatches on the
/// subcommand: only `yara run <file> [--vocabulary <path>]` is supported
/// (`--keywords <path>` is kept working as an alias for backward
/// compatibility), which hands `<file>` (and the optional vocabulary file)
/// off to [`run_file`]. Any other invocation (missing subcommand, unknown
/// subcommand, or `run` with no file argument) prints a usage message to
/// stderr and exits with status 1.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: yara run <file> [--vocabulary <path>]");
                std::process::exit(1);
            };
            let vocabulary_path = parse_vocabulary_flag(&args[3..]);
            run_file(path, vocabulary_path.as_deref());
        }
        _ => {
            eprintln!("usage: yara run <file> [--vocabulary <path>]");
            std::process::exit(1);
        }
    }
}

/// Scans the arguments following `<file>` for `--vocabulary <path>` (primary
/// flag) or `--keywords <path>` (older alias, kept working for
/// backward-compatible translation files that only translate keywords).
/// Hand-rolled rather than pulling in an args-parsing crate, matching the
/// rest of the project's zero-dependency stance.
fn parse_vocabulary_flag(rest: &[String]) -> Option<String> {
    let pos = rest
        .iter()
        .position(|a| a == "--vocabulary" || a == "--keywords")?;
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

/// [`stage`] for the pipeline stages that run *after* imports are spliced in
/// (resolving itself, typechecking, interpretation): renders through the
/// resolver-built [`diagnostics::SourceMap`] so a position belonging to an
/// imported file gets that file's path/line/snippet, not the entry file's.
/// `vocab` localizes the stage label and call-stack-frame words when the run
/// used a translated vocabulary; pass `None` to keep the untranslated English
/// rendering.
fn stage_mapped<T, E: Diagnostic>(
    result: Result<T, E>,
    map: &diagnostics::SourceMap,
    vocab: Option<&Vocabulary>,
) -> T {
    match result {
        Ok(value) => value,
        Err(err) => {
            eprint!(
                "{}",
                diagnostics::render_with_map_and_vocab(&err, map, vocab)
            );
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
/// `vocabulary_path`, if present, names a vocabulary-translation file (see
/// `translations::parse_vocabulary`) whose `Vocabulary` (keywords, types,
/// builtins, methods, messages) every stage uses instead of the English
/// default — read and parsed *before* the source file itself, against its own
/// path/text, since a bad vocabulary file should fail independently of
/// whatever program it would have been applied to.
fn run_file(path: &str, vocabulary_path: Option<&str>) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{path}`: {e}");
            std::process::exit(1);
        }
    };

    let vocab = match vocabulary_path {
        Some(vocab_path) => {
            let vocab_text = match std::fs::read_to_string(vocab_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: cannot read `{vocab_path}`: {e}");
                    std::process::exit(1);
                }
            };
            // A translation error renders against the vocabulary file itself,
            // not the program that would have used it.
            stage(
                translations::parse_vocabulary(&vocab_text),
                vocab_path,
                &vocab_text,
            )
        }
        None => Vocabulary::english(),
    };
    let vocab = Rc::new(vocab);

    let tokens = stage(
        Lexer::with_vocabulary(&source, vocab.clone()).tokenize(),
        path,
        &source,
    );
    let program = stage(
        Parser::with_vocabulary(tokens, vocab.clone()).parse_program(),
        path,
        &source,
    );
    let mut map = diagnostics::SourceMap::new(path, &source);
    let program = stage_mapped(
        resolver::resolve_imports(program, std::path::Path::new(path), &mut map, &vocab),
        &map,
        Some(&vocab),
    );
    stage_mapped(
        TypeChecker::with_vocabulary(vocab.clone()).check_program(&program),
        &map,
        Some(&vocab),
    );
    stage_mapped(
        Interpreter::with_vocabulary(vocab.clone()).run_program(&program),
        &map,
        Some(&vocab),
    );
}
