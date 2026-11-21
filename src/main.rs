mod lexer;
use std::process::Command;

fn main() {
    let source = std::fs::read_to_string("main.kr").expect("unable to read source file!");
    let source = source.trim();

    let operator = ['+', '-', '*', '/']
        .iter()
        .find(|op| source.contains(**op))
        .expect("unsupported operator");

    let (left, right) = source
        .trim()
        .split_once(*operator)
        .expect("invalid expression");

    let left: i32 = left.trim().parse().expect("invalid integer");
    let right: i32 = right.trim().parse().expect("invalid integers");

    let instruction = match operator {
        '+' => "add",
        '-' => "sub",
        '*' => "mul",
        '/' => "sdiv",
        _ => unreachable!(),
    };

    let llvm_ir = format!(
        r#"
@.fmt = private unnamed_addr constant [4 x i8] c"%d\0A\00"

declare i32 @printf(ptr, ...)

define i32 @main() {{
entry:
    %result = {instruction} i32 {left}, {right}
    %format = getelementptr inbounds [4 x i8], ptr @.fmt, i64 0, i64 0
    call i32 (ptr, ...) @printf(ptr %format, i32 %result)
    ret i32 0
}}
"#
    );
    std::fs::write("output.ll", &llvm_ir).expect("failed to write LLVM ir");
    let status = Command::new("clang")
        .args(["output.ll", "-o", "program"])
        .status()
        .expect("failed to execute clang");

    if !status.success() {
        panic!("LLVM compilation failed");
    } else {
        println!("compiled successfully");
    }
}
