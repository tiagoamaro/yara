mod ast;
mod interpreter;
mod lexer;
mod parser;
mod typechecker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: yara run <file>");
                std::process::exit(1);
            };
            eprintln!("not yet implemented: cannot run {path}");
            std::process::exit(1);
        }
        _ => {
            eprintln!("usage: yara run <file>");
            std::process::exit(1);
        }
    }
}
