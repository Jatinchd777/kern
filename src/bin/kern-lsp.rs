fn main() {
    if let Err(error) = kern::lsp::run() {
        eprintln!("kern-lsp: {error}");
        std::process::exit(1);
    }
}
