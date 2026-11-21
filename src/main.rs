mod cli;
mod codegen;
mod compiler;
mod diagnostics;
mod hir;
mod lexer;
mod parser;
mod semantic;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    std::process::exit(cli::run(args));
}
