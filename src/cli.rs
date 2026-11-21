use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::codegen;
use crate::compiler;
use crate::diagnostics::{Diagnostic, SourceFile};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(args: Vec<String>) -> i32 {
    let mut args = args.into_iter();
    let _program = args.next();
    let Some(command) = args.next() else {
        print_help();
        return 2;
    };
    let rest: Vec<String> = args.collect();

    match command.as_str() {
        "--version" | "-V" | "version" => {
            println!("kern {VERSION}");
            0
        }
        "--help" | "-h" | "help" => {
            print_help();
            0
        }
        "build" => command_build(rest),
        "run" => command_run(rest),
        "check" => command_check(rest),
        "emit-ir" => command_emit_ir(rest),
        other => {
            eprintln!("error: unknown command `{other}`");
            print_help();
            2
        }
    }
}

fn print_help() {
    println!(
        "kern {VERSION} - a small statically typed systems language

Usage:
  kern build <file.kr> [-o <output>] [--emit-ir <path>]
  kern run <file.kr> [-- <program args>...]
  kern check <file.kr>
  kern emit-ir <file.kr> [-o <path>]
  kern --version
  kern --help

The backend emits textual LLVM IR and invokes clang to produce a native
executable. Set KERN_CLANG to choose a different clang executable."
    );
}

fn command_build(args: Vec<String>) -> i32 {
    let mut file = None;
    let mut output = None;
    let mut emit_ir = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-o" | "--output" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("error: `-o` requires a path");
                    return 2;
                };
                output = Some(PathBuf::from(path));
            }
            "--emit-ir" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("error: `--emit-ir` requires a path");
                    return 2;
                };
                emit_ir = Some(PathBuf::from(path));
            }
            option if option.starts_with('-') => {
                eprintln!("error: unknown option `{option}`");
                return 2;
            }
            path => {
                if file.is_some() {
                    eprintln!("error: more than one input file supplied");
                    return 2;
                }
                file = Some(PathBuf::from(path));
            }
        }
        index += 1;
    }

    let Some(file) = file else {
        eprintln!("error: `build` requires an input file");
        return 2;
    };
    let output = output.unwrap_or_else(|| default_output_path(&file));

    match compile_to_executable(&file, &output, emit_ir.as_deref()) {
        Ok(()) => {
            println!("built {}", output.display());
            0
        }
        Err(code) => code,
    }
}

fn command_run(args: Vec<String>) -> i32 {
    let mut file = None;
    let mut program_args = Vec::new();
    let mut after_separator = false;

    for arg in args {
        if after_separator {
            program_args.push(arg);
            continue;
        }
        if arg == "--" {
            after_separator = true;
            continue;
        }
        if arg.starts_with('-') {
            eprintln!("error: unknown option `{arg}` for `run`");
            return 2;
        }
        if file.is_some() {
            eprintln!("error: unexpected argument `{arg}`; use `--` to pass arguments to the program");
            return 2;
        }
        file = Some(PathBuf::from(arg));
    }

    let Some(file) = file else {
        eprintln!("error: `run` requires an input file");
        return 2;
    };

    let executable = temp_path("bin");
    let build_result = compile_to_executable(&file, &executable, None);
    if let Err(code) = build_result {
        let _ = fs::remove_file(&executable);
        return code;
    }

    let status = Command::new(&executable).args(&program_args).status();
    let _ = fs::remove_file(&executable);

    match status {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("error: failed to run compiled program: {error}");
            3
        }
    }
}

fn command_check(args: Vec<String>) -> i32 {
    if args.len() != 1 {
        eprintln!("error: `check` requires exactly one input file");
        return 2;
    }
    let file = Path::new(&args[0]);
    let source = match read_source(file) {
        Ok(source) => source,
        Err(code) => return code,
    };
    match compiler::check_source(&source) {
        Ok(_) => {
            println!("checked {}: ok", file.display());
            0
        }
        Err(diagnostics) => {
            render_diagnostics(file, &source, &diagnostics);
            1
        }
    }
}

fn command_emit_ir(args: Vec<String>) -> i32 {
    let mut file = None;
    let mut output = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-o" | "--output" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("error: `-o` requires a path");
                    return 2;
                };
                output = Some(PathBuf::from(path));
            }
            option if option.starts_with('-') => {
                eprintln!("error: unknown option `{option}`");
                return 2;
            }
            path => {
                if file.is_some() {
                    eprintln!("error: more than one input file supplied");
                    return 2;
                }
                file = Some(PathBuf::from(path));
            }
        }
        index += 1;
    }

    let Some(file) = file else {
        eprintln!("error: `emit-ir` requires an input file");
        return 2;
    };
    let source = match read_source(&file) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let checked = match compiler::check_source(&source) {
        Ok(checked) => checked,
        Err(diagnostics) => {
            render_diagnostics(&file, &source, &diagnostics);
            return 1;
        }
    };
    let ir = match codegen::generate(&checked.hir) {
        Ok(ir) => ir,
        Err(error) => {
            eprintln!("error: code generation failed: {error}");
            return 3;
        }
    };

    match output {
        Some(path) => {
            if let Err(error) = fs::write(&path, ir) {
                eprintln!("error: failed to write `{}`: {error}", path.display());
                return 2;
            }
            println!("wrote {}", path.display());
        }
        None => {
            print!("{ir}");
        }
    }
    0
}

fn compile_to_executable(
    file: &Path,
    output: &Path,
    emit_ir_path: Option<&Path>,
) -> Result<(), i32> {
    let source = read_source(file)?;
    let checked = match compiler::check_source(&source) {
        Ok(checked) => checked,
        Err(diagnostics) => {
            render_diagnostics(file, &source, &diagnostics);
            return Err(1);
        }
    };
    let ir = match codegen::generate(&checked.hir) {
        Ok(ir) => ir,
        Err(error) => {
            eprintln!("error: code generation failed: {error}");
            return Err(3);
        }
    };

    if let Some(path) = emit_ir_path {
        if let Err(error) = fs::write(path, &ir) {
            eprintln!("error: failed to write `{}`: {error}", path.display());
            return Err(2);
        }
    }

    let ir_path = temp_path("ll");
    if let Err(error) = fs::write(&ir_path, ir) {
        eprintln!("error: failed to write temporary IR file: {error}");
        return Err(2);
    }

    let clang = std::env::var("KERN_CLANG").unwrap_or_else(|_| "clang".to_string());
    let result = Command::new(&clang)
        .arg("-O2")
        .arg("-Wno-override-module")
        .arg("-x")
        .arg("ir")
        .arg(&ir_path)
        .arg("-o")
        .arg(output)
        .output();

    let _ = fs::remove_file(&ir_path);

    match result {
        Ok(result) if result.status.success() => Ok(()),
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            if !stderr.trim().is_empty() {
                eprint!("{stderr}");
                if !stderr.ends_with('\n') {
                    eprintln!();
                }
            }
            eprintln!(
                "error: clang failed with status {} while compiling `{}`",
                result.status,
                file.display()
            );
            Err(3)
        }
        Err(error) => {
            eprintln!(
                "error: failed to invoke clang (`{clang}`): {error}; set KERN_CLANG to a clang executable"
            );
            Err(3)
        }
    }
}

fn read_source(path: &Path) -> Result<String, i32> {
    match fs::read_to_string(path) {
        Ok(source) => Ok(source),
        Err(error) => {
            eprintln!("error: could not read `{}`: {error}", path.display());
            Err(2)
        }
    }
}

fn render_diagnostics(file: &Path, source: &str, diagnostics: &[Diagnostic]) {
    let source_file = SourceFile::new(file.display().to_string(), source);
    eprint!("{}", source_file.render(diagnostics));
}

fn default_output_path(file: &Path) -> PathBuf {
    match file.file_stem() {
        Some(stem) => PathBuf::from(stem),
        None => PathBuf::from("a.out"),
    }
}

fn temp_path(extension: &str) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "kern-{}-{}-{}.{}",
        std::process::id(),
        nanos,
        counter,
        extension
    ))
}
