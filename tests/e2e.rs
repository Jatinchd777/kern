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

#[test]
fn runs_arithmetic_and_variables() {
    assert_run(
        r#"
        fn main() {
            let x = 10 + 20 * 3;
            let y = x - 5;
            print(y);
        }
        "#,
        "65\n",
    );
}

#[test]
fn runs_functions_and_recursion() {
    assert_run(
        r#"
        fn factorial(n: i32) -> i32 {
            if n <= 1 { return 1; }
            return n * factorial(n - 1);
        }
        fn main() { print(factorial(5)); }
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
                print(i);
                i = i + 1;
            }
            for (let mut j = 0; j < 5; j = j + 1) {
                if j == 1 { continue; }
                if j == 4 { break; }
                print(j);
            }
            let mut n = 0;
            loop {
                n = n + 1;
                if n == 2 { break; }
            }
            print(n);
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
            print(distance(origin, p));
            let status = Status::Active;
            match status {
                Status::Active => print("active"),
                Status::Inactive => print("inactive"),
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
            print(-42);
            print(255u8);
            print(3.5);
            print(true);
            print(false);
            print('K');
            print("hello");
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
            print("exiting");
            return 7;
        }
        "#,
    );
    assert_eq!(output.status.code(), Some(7), "stderr:\n{}", stderr(&output));
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
            match status { Status::Active => print("a"), }
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
        fn main() { print("built"); }
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
