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
            eprintln!("lex error: {e}");
            std::process::exit(1);
        }
    };

    let program = match Parser::new(tokens).parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("parse error: {e}");
            std::process::exit(1);
        }
    };

    let program = match resolver::resolve_imports(program, std::path::Path::new(path)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("import error: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = TypeChecker::new().check_program(&program) {
        eprintln!("type error: {e}");
        std::process::exit(1);
    }

    if let Err(e) = Interpreter::new().run_program(&program) {
        eprintln!("runtime error: {e}");
        std::process::exit(1);
    }
}
