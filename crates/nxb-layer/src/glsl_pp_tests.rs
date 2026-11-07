#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::preprocess;

#[test]
fn defines_and_conditionals() {
    let src = "#version 310 es\n#define N 2\n#if N > 1 && defined(N)\nint a = N;\n#else\nint b;\n#endif\n#ifdef M\nint c;\n#endif\n";
    let out = preprocess(src).unwrap();
    assert!(out.contains("int a = 2;"), "{out}");
    assert!(!out.contains("int b"), "{out}");
    assert!(!out.contains("int c"), "{out}");
}

#[test]
fn function_like_macros() {
    let out =
        preprocess("#version 310 es\n#define SQ(x) ((x) * (x))\nfloat f = SQ(a + 1.0);\n").unwrap();
    assert!(out.contains("((a + 1.0) * (a + 1.0))"), "{out}");
}

#[test]
fn comments_are_removed() {
    let out = preprocess("#version 310 es\nint a; // x\n/* y\n z */ int b;\n").unwrap();
    assert!(!out.contains('x') && !out.contains('y'), "{out}");
    assert!(out.contains("int b;"), "{out}");
}

#[test]
fn unterminated_conditional_is_an_error() {
    assert!(preprocess("#version 310 es\n#if 1\nint a;\n").is_err());
}
