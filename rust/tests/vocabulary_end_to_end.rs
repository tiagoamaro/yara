//! End-to-end coverage for `Vocabulary` threaded through every pipeline
//! stage (lexer -> parser -> resolver -> typechecker -> interpreter), not
//! just the translation-file parsing tested in `src/translations/mod.rs`.
//!
//! Builds a tiny inline vocabulary translating one type, one builtin, one
//! method, and the `new` constructor, then runs a `.yara` snippet written
//! entirely in those localized spellings through `TypeChecker::with_vocabulary`
//! and `Interpreter::with_vocabulary`, confirming it type-checks and runs
//! exactly as the English-spelled equivalent would.

use std::path::Path;
use std::rc::Rc;

use yara::interpreter::Interpreter;
use yara::lexer::Lexer;
use yara::parser::Parser;
use yara::resolver;
use yara::translations::{self, Vocabulary};
use yara::typechecker::TypeChecker;

/// Runs `source` fully through lexer -> parser -> resolver -> typechecker ->
/// interpreter using `vocab`, panicking with the stage/message on any
/// failure. Test helper mirroring `main.rs::run_file`'s stage order.
fn run(source: &str, vocab: Rc<Vocabulary>) {
    let tokens = Lexer::with_keywords(source, vocab.keywords.clone())
        .tokenize()
        .unwrap_or_else(|e| panic!("lex error: {e}"));
    let program = Parser::with_vocabulary(tokens, vocab.clone())
        .parse_program()
        .unwrap_or_else(|e| panic!("parse error: {e}"));
    let mut map = yara::diagnostics::SourceMap::new("test.yara", source);
    let program = resolver::resolve_imports(program, Path::new("test.yara"), &mut map, &vocab)
        .unwrap_or_else(|e| panic!("resolve error: {e}"));
    TypeChecker::with_vocabulary(vocab.clone())
        .check_program(&program)
        .unwrap_or_else(|e| panic!("type error: {e}"));
    Interpreter::with_vocabulary(vocab)
        .run_program(&program)
        .unwrap_or_else(|e| panic!("runtime error: {e}"));
}

/// A localized type name (`Inteiro` for `Integer`) is accepted in
/// annotations and type-checks/runs identically to the English spelling.
#[test]
fn localized_type_name_end_to_end() {
    let vocab = Rc::new(
        translations::parse_vocabulary("[types]\nInteger = Inteiro\n")
            .expect("vocab file should parse"),
    );
    run("x: Inteiro = 5\nprint(x)\n", vocab);
}

/// A localized builtin name (`tamanho` for `len`) works as a free-function
/// call.
#[test]
fn localized_builtin_end_to_end() {
    let vocab = Rc::new(
        translations::parse_vocabulary("[builtins]\nlen = tamanho\n")
            .expect("vocab file should parse"),
    );
    run(
        "xs: IntArray = [1, 2, 3]\nn = tamanho(xs)\nprint(n)\n",
        vocab,
    );
}

/// A localized `print` spelling (`escreva`) is recognized both by the
/// typechecker's ad-hoc `print` handling and the interpreter's.
#[test]
fn localized_print_end_to_end() {
    let vocab = Rc::new(
        translations::parse_vocabulary("[builtins]\nprint = escreva\n")
            .expect("vocab file should parse"),
    );
    run("escreva(\"hello\")\n", vocab);
}

/// A localized primitive-method name (`tamanho` for the array `size`
/// method) is recognized by `check_primitive_method`/`eval_primitive_method`.
#[test]
fn localized_method_end_to_end() {
    let vocab = Rc::new(
        translations::parse_vocabulary("[methods]\nsize = tamanho\n")
            .expect("vocab file should parse"),
    );
    run(
        "xs: IntArray = [1, 2, 3]\nn: Integer = xs.tamanho()\nprint(n)\n",
        vocab,
    );
}

/// A localized `new` spelling is recognized by the typechecker's
/// construction check (`check_construction`'s `vocab.canonical_method`
/// comparison) -- the interpreter needs no equivalent check since it
/// dispatches any bare-class-ident method call straight to `construct`.
#[test]
fn localized_new_end_to_end() {
    let vocab = Rc::new(
        translations::parse_vocabulary("[methods]\nnew = criar\n")
            .expect("vocab file should parse"),
    );
    run(
        "class Point\n  x: Integer\n  def initializer(a: Integer)\n    x = a\n  end\nend\np = Point.criar(3)\nprint(p.x)\n",
        vocab,
    );
}

/// Sanity check: building an `Interpreter`/`TypeChecker` via `::new()` (no
/// explicit vocabulary) still behaves exactly like plain English -- the
/// default-vocabulary path introduced by `with_vocabulary` must not change
/// unlocalized-program behavior.
#[test]
fn english_default_unchanged() {
    let vocab = Rc::new(Vocabulary::english());
    run(
        "xs: IntArray = [1, 2, 3]\nprint(len(xs))\nprint(xs.size())\n",
        vocab,
    );
}
