# Kern

Kern is a small, statically typed systems programming language implemented in Rust. It includes a lexer, parser, semantic analyzer, HIR, LLVM IR code generator, native compiler CLI, language server, Tree-sitter grammar, and standard-library declarations.

Source files use the `.kr` extension. Kern lowers programs to LLVM IR and uses Clang to produce native executables.

## Features

- Static type checking with primitive, struct, and enum types
- Functions, mutable variables, control flow, and C-style for loops
- `if`/`else`, `while`, `loop`, `break`, `continue`, `return`, and `match`
- Struct construction and field access
- Enum variants and pattern matching
- Console input and output, including string interpolation and format specifiers
- Native builds and direct execution through the `kern` CLI
- LLVM IR emission for inspection
- Diagnostics for compiler errors
- Language-server support and Tree-sitter syntax tooling

## Requirements

- Rust and Cargo
- Clang
- LLVM IR support provided by the installed Clang toolchain

Verify the required tools:

```sh
rustc --version
cargo --version
clang --version
```

On Debian/Ubuntu:

```sh
sudo apt install build-essential clang rustc cargo
```

On Arch Linux:

```sh
sudo pacman -S --needed base-devel clang rustup
rustup default stable
```

## Build and Install

From the repository root:

```sh
cargo build --release
cargo install --path . --bins
```

This installs both executables:

- `kern` for checking, building, running, and emitting IR
- `kern-lsp` for language-server integration

## Quick Start

Create `hello.kr`:

```kern
fn main() {
    let name: String = "Kern";
    println("Hello, {name}!");
}
```

Run it directly:

```sh
kern run hello.kr
```

Build a native executable:

```sh
kern build hello.kr -o hello
./hello
```

Other commands:

```sh
kern check hello.kr
kern emit-ir hello.kr -o hello.ll
kern build hello.kr --emit-ir hello.ll -o hello
kern --version
```

`kern run` compiles to a temporary executable and removes it after execution. Arguments for the Kern program can be passed after `--`:

```sh
kern run program.kr -- arg1 arg2
```

Set `KERN_CLANG` to use a specific Clang executable:

```sh
KERN_CLANG=clang-18 kern build hello.kr
```

## Language Overview

### Variables

Variables are immutable by default. Use `mut` for values that need reassignment.

```kern
let name: String = "Kern";
let mut count: i32 = 1;
count = count + 1;
```

### Types

Supported primitive types include:

| Type | Description |
| --- | --- |
| `bool` | Boolean value |
| `char` | Character |
| `i8`, `i16`, `i32`, `i64`, `i128` | Signed integers |
| `u8`, `u16`, `u32`, `u64`, `u128` | Unsigned integers |
| `isize`, `usize` | Pointer-sized integers |
| `f32`, `f64` | Floating-point numbers |
| `String` | String text |

Types are checked during semantic analysis. Local variables can often infer their type from the initializer.

### Functions

```kern
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

fn main() {
    println("Result: {add(20, 22)}");
}
```

### Control Flow

```kern
fn main() {
    let mut i: i32 = 0;

    while i < 3 {
        println("{i}");
        i = i + 1;
    }

    for (let mut j: i32 = 0; j < 3; j = j + 1) {
        if j == 1 {
            continue;
        }
        println("{j}");
    }

    loop {
        break;
    }
}
```

### Structs

```kern
struct Point {
    x: i32,
    y: i32,
}

fn main() {
    let point = Point { x: 3, y: 4 };
    println("({point.x}, {point.y})");
}
```

### Enums and Match

```kern
enum Status {
    Active,
    Inactive,
    Pending,
}

fn main() {
    let status = Status::Active;

    match status {
        Status::Active => println("Active"),
        Status::Inactive => println("Inactive"),
        Status::Pending => println("Pending"),
        _ => println("Unknown"),
    }
}
```

### Input and Output

`print` writes values without a trailing newline. `println` adds one newline. Arguments are separated by spaces.

```kern
fn main() {
    print("Name: ");
    let name: String = "Kern";
    println(name);

    let mut value: i32 = 0;
    cin >> value;
    println("Read: {value}");
}
```

Strings support expression interpolation and positional format arguments:

```kern
let count: i32 = 255;
let pi: f64 = 3.141592;

println("Count: {count}");
println("Count: {:x}, pi: {:.2}", count, pi);
println("Escaped: {{count}}");
```

Supported format specifiers include:

| Syntax | Meaning |
| --- | --- |
| `{}` | Default formatting |
| `{:x}` / `{:X}` | Lowercase or uppercase hexadecimal |
| `{:b}` | Binary |
| `{:o}` | Octal |
| `{:.N}` | Fixed-point precision |
| `{:N}` | Minimum field width |
| `{:0N}` | Zero-padded field width |

## Examples

The `examples/` directory contains complete programs demonstrating language features:

- `hello.kr` - basic output
- `values.kr` - primitive types and operators
- `functions.kr` - functions, recursion, and loops
- `control_flow.kr` - conditionals and loop control
- `structs.kr` - struct declarations and field access
- `enums.kr` - enums and match expressions
- `formatting.kr` - interpolation and format specifiers

Run any example with:

```sh
kern run examples/functions.kr
```

## Editor Support

The `tree-sitter-kern/` directory contains a Tree-sitter grammar and queries for syntax highlighting, indentation, locals, and folding.

```sh
cd tree-sitter-kern
npm install
npm run generate
tree-sitter test
```

The generated parser is checked in, so normal editor setup does not require regeneration.

`kern-lsp` provides:

- Compiler diagnostics
- Completions
- Hover information
- Go to definition
- Find references
- Rename
- Document symbols
- Missing-semicolon quick fixes

The server communicates over stdin/stdout:

```sh
KERN_LSP_LOG=1 kern-lsp
```

Set `KERN_LSP_LOG=1` to write server events to stderr.

## Project Layout

```text
src/
  cli.rs                 Compiler command-line interface
  compiler.rs            Front-end pipeline
  lexer/                 Tokenization
  parser/                Parser and AST
  semantic/              Type checking and symbol analysis
  hir/                   High-level intermediate representation
  codegen/               LLVM IR generation
  diagnostics.rs         Rendered compiler diagnostics
  lsp/                   Language server
examples/                Kern example programs
std/                     Standard-library source declarations
tests/                   End-to-end compiler tests
tree-sitter-kern/        Tree-sitter grammar and editor queries
```

## Development

Run the test suite:

```sh
cargo test
```

Native execution tests require Clang. When Clang is unavailable or `KERN_CLANG` does not point to a valid executable, those tests report that native execution is skipped.

Format and lint:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Current Limitations

- Module and import resolution is not implemented.
- Enum payloads and arrays are not fully supported by the compiler pipeline.
- Generic type syntax is only partially represented.
- Language-server navigation is currently limited to the open document.
- Kern is a work-in-progress language and the syntax or behavior may change.
