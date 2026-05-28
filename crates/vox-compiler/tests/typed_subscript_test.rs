//! Regression test for the typed subscript decision (2026-05-23).
//!
//! `list[T] [i]` returns `Option[T]`. Out-of-bounds and wrong-receiver-type
//! both yield `None`. Negative indices yield `None` (no Python wraparound).
//! Honors Vox's no-silent-failure policy: callers MUST handle the Option.

use vox_compiler::eval::Interpreter;
use vox_compiler::eval::value::VoxValue;
use vox_compiler::hir::lower::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;

fn run(src: &str) -> VoxValue {
    let tokens = lex(src);
    let module = parse_script(tokens).expect("parse");
    let lowered = lower_module(&module);
    let mut interp = Interpreter::new(10_000_000);
    interp.run_module(&lowered).expect("module");
    interp.call("main", vec![]).expect("main")
}

#[test]
fn list_subscript_in_bounds_returns_some() {
    let v = run(r#"fn main() to int {
            let xs = [10, 20, 30]
            return xs[1].unwrap_or(-1)
        }"#);
    assert_eq!(v, VoxValue::Int(20));
}

#[test]
fn list_subscript_out_of_bounds_returns_none() {
    let v = run(r#"fn main() to int {
            let xs = [10, 20, 30]
            return xs[99].unwrap_or(-1)
        }"#);
    assert_eq!(v, VoxValue::Int(-1));
}

#[test]
fn list_subscript_negative_returns_none() {
    let v = run(r#"fn main() to int {
            let xs = [10, 20, 30]
            return xs[-1].unwrap_or(-1)
        }"#);
    assert_eq!(v, VoxValue::Int(-1));
}

#[test]
fn string_subscript_returns_char_string() {
    let v = run(r#"fn main() to str {
            let s = "hello"
            return s[1].unwrap_or("?")
        }"#);
    assert_eq!(v, VoxValue::Str("e".to_string()));
}
