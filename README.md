# Kern

Kern is a small statically typed language compiled to native executables through
LLVM IR and Clang.

## Printing

Use `print(...)` to write without a newline and `println(...)` to write with
exactly one newline. Both accept zero or more comma-separated expressions:

```kern
print("Hello");
print(" ");
println("World", 42); // writes: World 42\n
println();             // writes one newline
```

Arguments are converted to printable text and separated by one space. Text
inside a single string argument is emitted as-is; interpolation inside that
string does not add spaces.

Braced expressions are interpolated in strings. Kern parses each expression
using its normal expression grammar, so variables, arithmetic, and supported
function calls can be used. Double braces escape a literal brace:

```kern
let name: String = "Kern";
let count: i32 = 3;
println("Hello, {name}! {count + 1} users; {{name}} is literal.");
```

Positional placeholders use the following initial format specifiers:

| Syntax | Meaning |
| --- | --- |
| `{}` | Default formatting |
| `{:x}` / `{:X}` | Lowercase / uppercase hexadecimal (integers) |
| `{:b}` | Binary (integers) |
| `{:o}` | Octal (integers) |
| `{:.N}` | Fixed-point float precision |
| `{:N}` | Minimum field width |
| `{:0N}` | Zero-padded minimum field width |

Placeholders consume the non-template expressions passed after the format
string, in order. When a format string contains placeholders, the number of
arguments must match exactly. For example:

```kern
let users: i32 = 255;
let pi: f64 = 3.141592;
println("Users: {} (hex {:x}), pi {:.2}", users, users, pi);
```

Default floats use compact general formatting. Integers up to 64 bits, floats,
booleans, characters, and strings are printable. Unsupported types and
incompatible format specifiers are reported during compilation.

Run an example with `kern run examples/formatting.kr`.

## Editor tooling

The repository includes a Tree-sitter grammar in `tree-sitter-kern/` and a
language server that reuses Kern's lexer, parser, and semantic analyzer.
Install both compiler executables from the repository root:

```sh
cargo install --path . --bins
```

On Arch Linux, install the build and editor dependencies first:

```sh
sudo pacman -S --needed base-devel clang llvm rustup nodejs npm neovim tree-sitter-cli
rustup default stable
```

Generate and test the parser:

```sh
cd tree-sitter-kern
tree-sitter generate
tree-sitter test
```

The LazyVim configuration is in `~/.config/nvim/lua/plugins/kern.lua`. It
registers `.kr` as `kern`, adds the local parser without replacing other
Tree-sitter parsers, and configures `kern-lsp` through LazyVim's native LSP
configuration. If the repository is not at
`~/Documents/projects/Rust/kern`, set `KERN_TREE_SITTER_PATH` to the
`tree-sitter-kern` directory before launching Neovim. Then run `:TSInstall kern`
and open a `.kr` file. The LSP command is discovered from `PATH`, with a
fallback to `~/.cargo/bin/kern-lsp`.

The server provides compiler diagnostics, completions from the current syntax
tree, symbol hover, go-to-definition, references, rename, document symbols,
and a missing-semicolon quick fix. Diagnostics run asynchronously against
unsaved buffer text and use UTF-16 LSP positions. The compiler does not yet
resolve imports or modules, so navigation and references are limited to the
currently open document; `use` statements remain unsupported by semantic
analysis.

Troubleshoot with `:LspInfo`, `:checkhealth vim.lsp`, `:TSInstallInfo`, and
`:messages`. Verify the binaries independently with `kern --version` and
`kern-lsp` (the server speaks LSP over stdin/stdout; set `KERN_LSP_LOG=1` to
log server events to stderr).
