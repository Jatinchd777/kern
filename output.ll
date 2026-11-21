
@.fmt = private unnamed_addr constant [4 x i8] c"%d\0A\00"

declare i32 @printf(ptr, ...)

define i32 @main() {
entry:
    %result = sub i32 10, 5
    %format = getelementptr inbounds [4 x i8], ptr @.fmt, i64 0, i64 0
    call i32 (ptr, ...) @printf(ptr %format, i32 %result)
    ret i32 0
}
