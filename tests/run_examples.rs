//! End-to-end integration tests over the bundled example programs.
//!
//! This is the regression net the modularization refactor leans on: it drives
//! the full public pipeline (lexer -> parser -> resolver -> typechecker ->
//! interpreter) exactly as `yara run` does, but as a library, so a regression
//! surfaces as an ordinary failed assertion instead of a CLI process exit. Only
//! reachable at all because the compiler is now a library crate (`src/lib.rs`).

use std::path::{Path, PathBuf};
use std::rc::Rc;

use yara::interpreter::Interpreter;
use yara::lexer::{self, Lexer};
use yara::parser::Parser;
use yara::translations::Vocabulary;
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
fn run_source(source: &str, path: &Path, vocab: Rc<Vocabulary>) -> Result<(), (Stage, String)> {
    let tokens = Lexer::with_vocabulary(source, vocab.clone())
        .tokenize()
        .map_err(|e| (Stage::Lex, e.message))?;
    let program = Parser::with_vocabulary(tokens, vocab.clone())
        .parse_program()
        .map_err(|e| (Stage::Parse, e.message))?;
    let mut map = yara::diagnostics::SourceMap::new(&path.display().to_string(), source);
    let program = resolver::resolve_imports(program, path, &mut map, &vocab)
        .map_err(|e| (Stage::Import, e.message))?;
    TypeChecker::with_vocabulary(vocab.clone())
        .check_program(&program)
        .map_err(|e| (Stage::Type, e.message))?;
    Interpreter::with_vocabulary(vocab.clone())
        .run_program(&program)
        .map_err(|e| (Stage::Runtime, e.message))?;
    Ok(())
}

/// Runs the file at `path` through [`run_source`], reading its source and
/// choosing its vocabulary. Mirrors `main.rs::run_file`'s stage order, minus
/// the CLI's error rendering and exit.
fn run_pipeline(path: &Path) -> Result<(), (Stage, String)> {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    // Examples written in Portuguese vocabulary (full keywords/types/builtins/
    // methods/messages, not just keywords) need the bundled `pt.vocab`; every
    // other example uses the default English vocabulary.
    let vocab =
        if path.starts_with("examples/translations") || path.ends_with("runtime_error_pt.yara") {
            let text = std::fs::read_to_string("translations/pt.vocab").unwrap();
            translations::parse_vocabulary(&text).expect("bundled pt.vocab must parse")
        } else {
            Vocabulary::english()
        };

    run_source(&source, path, Rc::new(vocab))
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
            | "class_unassigned_field"
            | "class_unknown_field"
            | "class_wrong_arg_count"
            | "class_inherited_field_unassigned"
            | "method_unknown_on_primitive" => Stage::Type,
            "array_out_of_bounds"
            | "runtime_error_stack_trace"
            | "use_after_free"
            | "double_free"
            | "nil_pointer_deref"
            | "string_to_i_invalid"
            | "runtime_error_pt" => Stage::Runtime,
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
            "alloc" => "p: Ptr<Integer> = alloc(5)\n",
            "deref" => "p: Ptr<Integer> = alloc(5)\nx: Integer = deref(p)\n",
            "set_deref" => "p: Ptr<Integer> = alloc(5)\nset_deref(p, 9)\n",
            "free" => "p: Ptr<Integer> = alloc(5)\nfree(p)\n",
            "collect" => "n: Integer = collect()\n",
            other => {
                panic!("no probe snippet for builtin `{other}` — add one so this test covers it")
            }
        }
    }

    for builtin in yara::builtins::BUILTINS {
        let path = Path::new("examples/_builtin_probe.yara");
        if let Err((stage, msg)) =
            run_source(probe(builtin.name), path, Rc::new(Vocabulary::english()))
        {
            panic!(
                "builtin `{}` is not handled at {stage:?}: {msg}",
                builtin.name
            );
        }
    }
}

/// Every method in the `methods` registry must be wired into *both* stages: a
/// valid call to it should type-check and run without landing on "undefined
/// method". Guards against adding a `METHODS` entry (or a typecheck/eval arm)
/// without the matching arm in the other stage. A new method forces a new probe
/// snippet here, which by construction exercises both stages.
#[test]
fn every_method_is_handled_by_both_stages() {
    fn probe(receiver: yara::methods::ReceiverKind, name: &str, arity: usize) -> String {
        match (receiver, name, arity) {
            // Array methods
            (yara::methods::ReceiverKind::Array, "size", 0) => "xs: IntArray = [1]\nn: Integer = xs.size()\n".to_string(),
            (yara::methods::ReceiverKind::Array, "push", 1) => "xs: IntArray = [1]\nxs.push(2)\n".to_string(),
            (yara::methods::ReceiverKind::Array, "get", 1) => "xs: IntArray = [1]\nx: Integer = xs.get(0)\n".to_string(),
            (yara::methods::ReceiverKind::Array, "set", 2) => "xs: IntArray = [1]\nxs.set(0, 9)\n".to_string(),
            (yara::methods::ReceiverKind::Array, "pop", 0) => "xs: IntArray = [1]\ny: Integer = xs.pop()\n".to_string(),
            (yara::methods::ReceiverKind::Array, "is_empty", 0) => "xs: IntArray = [1]\nflag: Boolean = xs.is_empty()\n".to_string(),
            // String methods
            (yara::methods::ReceiverKind::String, "size", 0) => "s: Str = \"hi\"\nn: Integer = s.size()\n".to_string(),
            (yara::methods::ReceiverKind::String, "upper", 0) => "s: Str = \"hi\"\nup: Str = s.upper()\n".to_string(),
            (yara::methods::ReceiverKind::String, "lower", 0) => "s: Str = \"HI\"\nlo: Str = s.lower()\n".to_string(),
            (yara::methods::ReceiverKind::String, "trim", 0) => "s: Str = \"  hi  \"\nt: Str = s.trim()\n".to_string(),
            (yara::methods::ReceiverKind::String, "is_empty", 0) => "s: Str = \"hi\"\nflag: Boolean = s.is_empty()\n".to_string(),
            (yara::methods::ReceiverKind::String, "to_i", 0) => "s: Str = \"42\"\nn: Integer = s.to_i()\n".to_string(),
            (yara::methods::ReceiverKind::String, "to_f", 0) => "s: Str = \"3.14\"\nf: Float = s.to_f()\n".to_string(),
            (yara::methods::ReceiverKind::String, "to_s", 0) => "s: Str = \"hi\"\nresult: Str = s.to_s()\n".to_string(),
            // Integer methods
            (yara::methods::ReceiverKind::Integer, "to_s", 0) => "x: Integer = 42\ns: Str = x.to_s()\n".to_string(),
            (yara::methods::ReceiverKind::Integer, "to_f", 0) => "x: Integer = 42\nf: Float = x.to_f()\n".to_string(),
            (yara::methods::ReceiverKind::Integer, "abs", 0) => "x: Integer = -5\nresult: Integer = x.abs()\n".to_string(),
            // Float methods
            (yara::methods::ReceiverKind::Float, "to_s", 0) => "f: Float = 3.14\ns: Str = f.to_s()\n".to_string(),
            (yara::methods::ReceiverKind::Float, "to_i", 0) => "f: Float = 3.14\nx: Integer = f.to_i()\n".to_string(),
            (yara::methods::ReceiverKind::Float, "abs", 0) => "f: Float = -2.5\nresult: Float = f.abs()\n".to_string(),
            // Boolean methods
            (yara::methods::ReceiverKind::Boolean, "to_s", 0) => "flag: Boolean = true\ns: Str = flag.to_s()\n".to_string(),
            // Pointer methods
            (yara::methods::ReceiverKind::Pointer, "deref", 0) => "p: Ptr<Integer> = alloc(5)\nx: Integer = p.deref()\n".to_string(),
            (yara::methods::ReceiverKind::Pointer, "set_deref", 1) => "p: Ptr<Integer> = alloc(5)\np.set_deref(9)\n".to_string(),
            (yara::methods::ReceiverKind::Pointer, "free", 0) => "p: Ptr<Integer> = alloc(5)\np.free()\n".to_string(),
            _ => panic!(
                "no probe snippet for method `{}` on {:?} with arity {arity} — add one so this test covers it",
                name, receiver
            ),
        }
    }

    for method in yara::methods::METHODS {
        let path = Path::new("examples/_method_probe.yara");
        let source = probe(method.receiver, method.name, method.arity);
        if let Err((stage, msg)) = run_source(&source, path, Rc::new(Vocabulary::english())) {
            panic!(
                "method `{}` on {:?} is not handled at {stage:?}: {msg}",
                method.name, method.receiver
            );
        }
    }
}
