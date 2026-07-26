//! Golden tests over the rendered error output of every `examples/errors/*`
//! program.
//!
//! `tests/run_examples.rs` asserts each error example fails at the right
//! *stage*; this test locks down the exact *rendered text* — header, path,
//! line:column, snippet, caret, stack trace — byte for byte against a golden
//! file in `tests/golden/<name>.stderr`. The project convention that error
//! output stays byte-identical across refactors was previously verified by
//! hand; this automates it.
//!
//! To regenerate a golden after an *intentional* format change:
//! `cargo run -- run examples/errors/<name>.yara 2> tests/golden/<name>.stderr`

use std::path::Path;

use yara::diagnostics::{self, Diagnostic, SourceMap};
use yara::interpreter::Interpreter;
use yara::lexer::Lexer;
use yara::parser::Parser;
use yara::translations::Vocabulary;
use yara::typechecker::TypeChecker;
use yara::{resolver, translations};

/// Runs the file at `path` through the full pipeline exactly as
/// `main.rs::run_file` does, returning the rendered diagnostic of the first
/// stage that fails (the text the CLI would print to stderr), or `None` if
/// the program runs clean. Mirrors the CLI's rendering choices: plain
/// `render` against the entry file for lex/parse errors, `render_with_map`
/// for everything at or after import resolution.
///
/// `runtime_error_pt.yara` is written in Portuguese vocabulary (see
/// `translations/pt.vocab`), same as `run_pipeline` in
/// `tests/run_examples.rs` — this is the one example in `examples/errors/`
/// that needs a non-English `Vocabulary` to reach the error it's meant to
/// demonstrate (a Portuguese runtime message) rather than failing earlier at
/// typecheck on an unrecognized type name.
fn rendered_error(path: &Path) -> Option<String> {
    let path_str = path.to_str().unwrap();
    let source = std::fs::read_to_string(path).unwrap();

    let render = |err: &dyn Diagnostic| diagnostics::render(err, path_str, &source);

    let vocab = if path.ends_with("runtime_error_pt.yara") {
        let text = std::fs::read_to_string("translations/pt.vocab").unwrap();
        std::rc::Rc::new(translations::parse_vocabulary(&text).expect("bundled pt.vocab parses"))
    } else {
        std::rc::Rc::new(Vocabulary::english())
    };

    let tokens = match Lexer::with_vocabulary(&source, vocab.clone()).tokenize() {
        Ok(t) => t,
        Err(e) => return Some(render(&e)),
    };
    let program = match Parser::with_vocabulary(tokens, vocab.clone()).parse_program() {
        Ok(p) => p,
        Err(e) => return Some(render(&e)),
    };
    let mut map = SourceMap::new(path_str, &source);
    let program = match resolver::resolve_imports(program, path, &mut map, &vocab) {
        Ok(p) => p,
        Err(e) => return Some(diagnostics::render_with_map(&e, &map)),
    };
    if let Err(e) = TypeChecker::with_vocabulary(vocab.clone()).check_program(&program) {
        return Some(diagnostics::render_with_map(&e, &map));
    }
    if let Err(e) = Interpreter::with_vocabulary(vocab.clone()).run_program(&program) {
        return Some(diagnostics::render_with_map(&e, &map));
    }
    None
}

/// Every error example's rendered output must match its golden file byte for
/// byte, and every golden file must correspond to an existing example (no
/// orphans as examples are renamed/removed).
#[test]
fn error_examples_render_byte_identical_to_golden() {
    let mut names: Vec<String> = std::fs::read_dir("examples/errors")
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            (p.extension().and_then(|x| x.to_str()) == Some("yara"))
                .then(|| p.file_stem().unwrap().to_str().unwrap().to_string())
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no error examples found");

    for name in &names {
        let example = format!("examples/errors/{name}.yara");
        let golden_path = format!("tests/golden/{name}.stderr");
        let golden = std::fs::read_to_string(&golden_path)
            .unwrap_or_else(|_| panic!("missing golden file {golden_path} — see module docs"));
        let rendered = rendered_error(Path::new(&example))
            .unwrap_or_else(|| panic!("{example} ran clean but should fail"));
        assert_eq!(
            rendered, golden,
            "rendered error for {example} diverged from {golden_path}"
        );
    }

    for entry in std::fs::read_dir("tests/golden").unwrap() {
        let p = entry.unwrap().path();
        let stem = p.file_stem().unwrap().to_str().unwrap().to_string();
        assert!(
            names.contains(&stem),
            "orphan golden file {} has no matching error example",
            p.display()
        );
    }
}
