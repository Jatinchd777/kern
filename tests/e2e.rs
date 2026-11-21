use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn kern() -> &'static str {
    env!("CARGO_BIN_EXE_kern")
}

fn clang_available() -> bool {
    let clang = std::env::var("KERN_CLANG").unwrap_or_else(|_| "clang".to_string());
    Command::new(clang)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn temp_source(name: &str, source: &str) -> PathBuf {
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "kern-test-{}-{}-{}.kr",
        std::process::id(),
        counter,
        name
    ));
    fs::write(&path, source).expect("failed to write test source");
    path
}

fn run_kern(args: &[&str]) -> Output {
    Command::new(kern())
        .args(args)
        .output()
        .expect("failed to run kern")
}

fn run_source(name: &str, source: &str) -> Output {
    let path = temp_source(name, source);
    let output = Command::new(kern())
        .arg("run")
        .arg(&path)
        .output()
        .expect("failed to run kern");
    let _ = fs::remove_file(path);
    output
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_run(source: &str, expected: &str) {
    if !clang_available() {
        eprintln!("skipping native execution test: clang is not available");
        return;
    }
    let output = run_source("run", source);
    assert!(
        output.status.success(),
        "compilation/run failed:\n{}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), expected);
}

fn assert_run_with_stdin(source: &str, input: &str, expected: &str) {
    if !clang_available() {
        eprintln!("skipping native execution test: clang is not available");
        return;
    }
    let path = temp_source("cin", source);
    let mut child = Command::new(kern())
        .arg("run")
        .arg(&path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to run kern");
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let _ = fs::remove_file(path);
    assert!(
        output.status.success(),
        "compilation/run failed:\n{}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), expected);
}

#[test]
fn reads_cin_values() {
    assert_run_with_stdin(
        r#"
        fn main() {
            let mut x = 0;
            let mut y = 0;
            cin >> x >> y;
            println(x + y);
        }
        "#,
        "10 32\n",
        "42\n",
    );
}

#[test]
fn runs_arithmetic_and_variables() {
    assert_run(
        r#"
        fn main() {
            let x = 10 + 20 * 3;
            let y = x - 5;
            println(y);
        }

        "#,
        "65\n",
    );
}

#[test]
fn supports_printing_multiple_arguments_without_newline() {
    assert_run(
        r#"
        fn main() {
            let name: String = "Jatin";
            let age: i32 = 15;
            print("Hello,", name);
            println("Hello,", name);
            println("Age:", age);
            println("Result:", 10 + 20);
            println("Name:", name, "Age:", age);
            print();
            println();
        }
        "#,
        "Hello, JatinHello, Jatin\nAge: 15\nResult: 30\nName: Jatin Age: 15\n\n",
    );
}

#[test]
fn print_supports_interpolation_and_format_specs_without_newline() {
    assert_run(
        r#"
        fn main() {
            print("Hex: {:x}", 255);
            print(" | Result: {10 + 20}");
            println("!");
        }
        "#,
        "Hex: ff | Result: 30!\n",
    );
}

#[test]
fn supports_string_interpolation_and_expressions() {
    assert_run(
        r#"
        fn add(a: i32, b: i32) -> i32 { return a + b; }
        fn main() {
            let name: String = "Jatin";
            let age: i32 = 15;
            println("Hello, {name}!");
            println("You are {age} years old.");
            println("The result is {10 + 20}.");
            println("Call: {add(2, 3)}; {name} is {age}.");
            println("Escaped braces: {{name}}");
        }
        "#,
        "Hello, Jatin!\nYou are 15 years old.\nThe result is 30.\nCall: 5; Jatin is 15.\nEscaped braces: {name}\n",
    );
}

#[test]
fn supports_positional_format_specifiers() {
    assert_run(
        r#"
        fn main() {
            let number: i32 = 255;
            let big: i64 = 4294967296;
            let pi: f64 = 3.141592;
            println("Hex: {:x}", number);
            println("Hex uppercase: {:X}", number);
            println("Binary: {:b}", number);
            println("Octal: {:o}", number);
            println("Pi: {:.2}", pi);
            println("Padded: {:05}", number);
            println("Large: {}", big);
            println("Large hex: {:x}", big);
        }
        "#,
        "Hex: ff\nHex uppercase: FF\nBinary: 11111111\nOctal: 377\nPi: 3.14\nPadded: 00255\nLarge: 4294967296\nLarge hex: 100000000\n",
    );
}

#[test]
fn combines_printing_and_formatting_methods() {
    assert_run(
        r#"
        fn main() {
            let name: String = "Kern";
            let version: f64 = 0.1;
            let users: i32 = 100;
            println("Language:", name, "Version:", version);
            println("Welcome to {name}, version {version}!");
            println("Users: {} | Hex: {:x}", users, users);
            println("Prefix:", name, "| Hex: {:x}", users);
            println("The answer is {10 + 20}, and users = {}.", users);
        }
        "#,
        "Language: Kern Version: 0.1\nWelcome to Kern, version 0.1!\nUsers: 100 | Hex: 64\nPrefix: Kern | Hex: 64\nThe answer is 30, and users = 100.\n",
    );
}

#[test]
fn reports_invalid_formatting_and_interpolation() {
    for (name, source, expected) in [
        (
            "missing-format-arg",
            r#"fn main() { let name = "Kern"; println("Hello {} {}", name); }"#,
            "missing formatting argument",
        ),
        (
            "extra-format-arg",
            r#"fn main() { let name = "Kern"; let age = 1; println("Hello {}", name, age); }"#,
            "unused formatting argument",
        ),
        (
            "invalid-radix-type",
            r#"fn main() { println("{:x}", "hello"); }"#,
            "radix formatting requires an integer",
        ),
        (
            "unknown-interpolation",
            r#"fn main() { println("{invalid_variable}"); }"#,
            "undefined variable `invalid_variable`",
        ),
        (
            "unknown-format-spec",
            r#"fn main() { let number = 1; println("{:q}", number); }"#,
            "unsupported format specifier",
        ),
        (
            "precision-on-integer",
            r#"fn main() { let number = 1; println("{:.2}", number); }"#,
            "precision formatting requires a float",
        ),
        (
            "unmatched-closing-brace",
            r#"fn main() { println("value}"); }"#,
            "unmatched `}` in format string",
        ),
    ] {
        let path = temp_source(name, source);
        let output = run_kern(&["check", path.to_str().unwrap()]);
        let _ = fs::remove_file(path);
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        assert!(
            stderr(&output).contains(expected),
            "expected {expected:?} in:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn runs_functions_and_recursion() {
    assert_run(
        r#"
        fn factorial(n: i32) -> i32 {
            if n <= 1 { return 1; }
            return n * factorial(n - 1);
        }
        fn main() { println(factorial(5)); }
        "#,
        "120\n",
    );
}

#[test]
fn runs_control_flow() {
    assert_run(
        r#"
        fn main() {
            let mut i = 0;
            while i < 3 {
                println(i);
                i = i + 1;
            }
            for (let mut j = 0; j < 5; j = j + 1) {
                if j == 1 { continue; }
                if j == 4 { break; }
                println(j);
            }
            let mut n = 0;
            loop {
                n = n + 1;
                if n == 2 { break; }
            }
            println(n);
        }
        "#,
        "0\n1\n2\n0\n2\n3\n2\n",
    );
}

#[test]
fn runs_structs_and_enums() {
    assert_run(
        r#"
        struct Point { x: i32, y: i32 }
        enum Status { Active, Inactive }

        fn distance(a: Point, b: Point) -> i32 {
            let dx = a.x - b.x;
            let dy = a.y - b.y;
            return dx * dx + dy * dy;
        }

        fn main() {
            let origin = Point { x: 0, y: 0 };
            let p = Point { x: 3, y: 4 };
            println(distance(origin, p));
            let status = Status::Active;
            match status {
                Status::Active => println("active"),
                Status::Inactive => println("inactive"),
            }
        }
        "#,
        "25\nactive\n",
    );
}

#[test]
fn runs_scalar_printing() {
    assert_run(
        r#"
        fn main() {
            println(-42);
            println(255u8);
            println(3.5);
            println(true);
            println(false);
            println('K');
            println("hello");
        }
        "#,
        "-42\n255\n3.5\ntrue\nfalse\nK\nhello\n",
    );
}

#[test]
fn main_return_value_becomes_exit_code() {
    if !clang_available() {
        eprintln!("skipping native execution test: clang is not available");
        return;
    }
    let output = run_source(
        "exit",
        r#"
        fn main() -> i32 {
            println("exiting");
            return 7;
        }
        "#,
    );
    assert_eq!(
        output.status.code(),
        Some(7),
        "stderr:\n{}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), "exiting\n");
}

#[test]
fn reports_immutable_and_type_errors() {
    let path = temp_source(
        "errors",
        r#"
        fn main() {
            let x: i32 = 1;
            x = 2;
            let y: bool = 3;
            print(missing);
        }
        "#,
    );
    let output = run_kern(&["check", path.to_str().unwrap()]);
    let _ = fs::remove_file(path);
    assert_eq!(output.status.code(), Some(1));
    let text = stderr(&output);
    assert!(text.contains("immutable variable"), "{text}");
    assert!(text.contains("type mismatch"), "{text}");
    assert!(text.contains("undefined variable"), "{text}");
}

#[test]
fn reports_missing_return_and_non_exhaustive_match() {
    let path = temp_source(
        "returns",
        r#"
        enum Status { Active, Inactive }
        fn choose(x: bool) -> i32 {
            if x { return 1; }
        }
        fn main() {
            let status = Status::Active;
            match status { Status::Active => println("a"), }
        }
        "#,
    );
    let output = run_kern(&["check", path.to_str().unwrap()]);
    let _ = fs::remove_file(path);
    assert_eq!(output.status.code(), Some(1));
    let text = stderr(&output);
    assert!(text.contains("may reach the end"), "{text}");
    assert!(text.contains("non-exhaustive match"), "{text}");
}

#[test]
fn reports_syntax_errors_without_panicking() {
    let path = temp_source("syntax", "fn main() { let x = 1 }");
    let output = run_kern(&["check", path.to_str().unwrap()]);
    let _ = fs::remove_file(path);
    assert_eq!(output.status.code(), Some(1));
    let text = stderr(&output);
    assert!(text.contains("error:"), "{text}");
    assert!(text.contains("expected"), "{text}");
    assert!(!text.contains("panicked"), "{text}");
}

#[test]
fn reports_unsupported_imports_explicitly() {
    let path = temp_source(
        "import",
        r#"
        use std::io;
        fn main() { print("hi"); }
        "#,
    );
    let output = run_kern(&["check", path.to_str().unwrap()]);
    let _ = fs::remove_file(path);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("module imports are not implemented yet"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn emits_llvm_ir() {
    let path = temp_source(
        "ir",
        r#"
        fn add(a: i32, b: i32) -> i32 { return a + b; }
        fn main() { print(add(1, 2)); }
        "#,
    );
    let output = run_kern(&["emit-ir", path.to_str().unwrap()]);
    let _ = fs::remove_file(path);
    assert!(output.status.success(), "{}", stderr(&output));
    let ir = stdout(&output);
    assert!(ir.contains("define i32 @main()"), "{ir}");
    assert!(ir.contains("define i32 @kern.add"), "{ir}");
    assert!(ir.contains("@printf"), "{ir}");
    assert!(ir.contains("add i32"), "{ir}");
}

#[test]
fn check_succeeds_for_valid_program() {
    let path = temp_source(
        "valid",
        r#"
        fn main() {
            let mut i = 0;
            while i < 2 { i = i + 1; }
            print(i);
        }
        "#,
    );
    let output = run_kern(&["check", path.to_str().unwrap()]);
    let _ = fs::remove_file(path);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("ok"));
}

#[test]
fn build_creates_native_executable() {
    if !clang_available() {
        eprintln!("skipping native build test: clang is not available");
        return;
    }
    let source = temp_source(
        "build",
        r#"
        fn main() { println("built"); }
        "#,
    );
    let executable = std::env::temp_dir().join(format!(
        "kern-test-build-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let output = run_kern(&[
        "build",
        source.to_str().unwrap(),
        "-o",
        executable.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(executable.exists());

    let run = Command::new(&executable)
        .output()
        .expect("failed to run built executable");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "built\n");

    let _ = fs::remove_file(source);
    let _ = fs::remove_file(executable);
}

#[test]
fn version_and_help_work() {
    let version = run_kern(&["--version"]);
    assert!(version.status.success());
    assert!(stdout(&version).starts_with("kern "));

    let help = run_kern(&["--help"]);
    assert!(help.status.success());
    let text = stdout(&help);
    assert!(text.contains("kern build"));
    assert!(text.contains("kern run"));
    assert!(text.contains("kern emit-ir"));
}

#[test]
fn unknown_command_is_usage_error() {
    let output = run_kern(&["frobnicate"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("unknown command"));
}
