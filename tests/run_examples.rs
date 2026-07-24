//! End-to-end integration tests over the bundled example programs.
//!
//! This is the regression net the modularization refactor leans on: it drives
//! the full public pipeline (lexer -> parser -> resolver -> typechecker ->
//! interpreter) exactly as `yara run` does, but as a library, so a regression
//! surfaces as an ordinary failed assertion instead of a CLI process exit. Only
//! reachable at all because the compiler is now a library crate (`src/lib.rs`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use yara::interpreter::Interpreter;
use yara::lexer::{self, KeywordToken, Lexer};
use yara::parser::Parser;
use yara::typechecker::TypeChecker;
use yara::{resolver, translations};

/// Which pipeline stage a run reached before failing.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Stage {
    Lex,
    Parse,
    Import,
    Type,
    Runtime,
}

/// Runs `source` through the full pipeline (lexer -> parser -> resolver ->
/// typechecker -> interpreter), returning the stage that failed with its
/// message or `Ok(())`. `path` is used only for relative `import` resolution.
fn run_source(
    source: &str,
    path: &Path,
    keywords: HashMap<String, KeywordToken>,
) -> Result<(), (Stage, String)> {
    let tokens = Lexer::with_keywords(source, keywords)
        .tokenize()
        .map_err(|e| (Stage::Lex, e.message))?;
    let program = Parser::new(tokens)
        .parse_program()
        .map_err(|e| (Stage::Parse, e.message))?;
    let mut map = yara::diagnostics::SourceMap::new(&path.display().to_string(), source);
    let program = resolver::resolve_imports(program, path, &mut map)
        .map_err(|e| (Stage::Import, e.message))?;
    TypeChecker::new()
        .check_program(&program)
        .map_err(|e| (Stage::Type, e.message))?;
    Interpreter::new()
        .run_program(&program)
        .map_err(|e| (Stage::Runtime, e.message))?;
    Ok(())
}

/// Runs the file at `path` through [`run_source`], reading its source and
/// choosing its keyword table. Mirrors `main.rs::run_file`'s stage order, minus
/// the CLI's error rendering and exit.
fn run_pipeline(path: &Path) -> Result<(), (Stage, String)> {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    // The translated example needs its Portuguese keyword table; every other
    // example uses the default English keywords.
    let keywords = if path.starts_with("examples/translations") {
        let kw = std::fs::read_to_string("translations/pt.keywords").unwrap();
        translations::parse_keyword_file(&kw).expect("bundled pt.keywords must parse")
    } else {
        lexer::default_keywords()
    };

    run_source(&source, path, keywords)
}

/// Recursively collects every `.yara` file under `dir`.
fn collect_yara(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_yara(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("yara") {
            out.push(path);
        }
    }
}

/// Every example that is *meant* to work must run cleanly end to end (all of
/// `examples/` except the deliberately-broken `examples/errors/`).
#[test]
fn all_non_error_examples_run_clean() {
    let mut files = Vec::new();
    collect_yara(Path::new("examples"), &mut files);
    files.sort();

    let mut checked = 0;
    for path in &files {
        if path.starts_with("examples/errors") {
            continue;
        }
        checked += 1;
        if let Err((stage, msg)) = run_pipeline(path) {
            panic!(
                "{} should run clean but failed at {stage:?}: {msg}",
                path.display()
            );
        }
    }
    assert!(
        checked >= 10,
        "expected to exercise the example programs, only ran {checked}"
    );
}

/// Every deliberately-broken example must fail, and fail at the *specific* stage
/// it is meant to — so a lexer error can't silently mutate into a parse error
/// (etc.) unnoticed across a refactor.
#[test]
fn every_error_example_fails_at_expected_stage() {
    fn expected(name: &str) -> Stage {
        match name {
            "lex_error" => Stage::Lex,
            "parse_error" => Stage::Parse,
            "type_error"
            | "import_type_error"
            | "import_type_error_helper"
            | "undefined_variable"
            | "class_field_type_mismatch"
            | "class_unknown_field"
            | "class_wrong_arg_count" => Stage::Type,
            "array_out_of_bounds" | "runtime_error_stack_trace" => Stage::Runtime,
            other => panic!("no expected stage recorded for error example `{other}` — add one"),
        }
    }

    let mut files = Vec::new();
    collect_yara(Path::new("examples/errors"), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no error examples found");

    for path in &files {
        let name = path.file_stem().unwrap().to_str().unwrap();
        match run_pipeline(path) {
            Ok(()) => panic!("{} should have failed but ran clean", path.display()),
            Err((stage, _)) => assert_eq!(
                stage,
                expected(name),
                "wrong failing stage for {}",
                path.display()
            ),
        }
    }
}

/// Every builtin in the `builtins` registry must be wired into *both* stages: a
/// valid call to it should type-check and run without landing on "undefined
/// function". Guards against adding a `BUILTINS` entry (or a typecheck/execute
/// arm) without the matching arm in the other stage. A new builtin forces a new
/// probe snippet here, which by construction exercises both stages.
#[test]
fn every_builtin_is_handled_by_both_stages() {
    fn probe(name: &str) -> &'static str {
        match name {
            "len" => "xs: IntArray = [1]\nn: Integer = len(xs)\n",
            "push" => "xs: IntArray = [1]\npush(xs, 2)\n",
            "get" => "xs: IntArray = [1]\nx: Integer = get(xs, 0)\n",
            "set" => "xs: IntArray = [1]\nset(xs, 0, 9)\n",
            "pop" => "xs: IntArray = [1]\ny: Integer = pop(xs)\n",
            other => {
                panic!("no probe snippet for builtin `{other}` — add one so this test covers it")
            }
        }
    }

    for builtin in yara::builtins::BUILTINS {
        let path = Path::new("examples/_builtin_probe.yara");
        if let Err((stage, msg)) = run_source(probe(builtin.name), path, lexer::default_keywords())
        {
            panic!(
                "builtin `{}` is not handled at {stage:?}: {msg}",
                builtin.name
            );
        }
    }
}
