//! Binary-operator operand emission: numeric `+ - * /` must not borrow the RHS
//! (`1 + 2`, not `1 + &2`), while `String` concatenation must keep the borrow
//! (`a + &b`, the only shape that compiles for `String + &str`).
//!
//! Fast: asserts on `emit_fn` output strings — no crate compile needed.

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;

fn emit_first_fn(src: &str) -> String {
    let module = parse_script(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let f = hir.functions.first().expect("at least one function");
    emit_fn(f, Some(&hir.inferred_types), &[])
}

#[test]
fn numeric_add_has_no_spurious_borrow() {
    let out = emit_first_fn("fn add() to int { return 1 + 2 }");
    assert!(
        out.contains("1 + 2"),
        "expected a clean `1 + 2`, got:\n{out}"
    );
    assert!(
        !out.contains("1 + &2"),
        "numeric add must not borrow the RHS (`1 + &2`):\n{out}"
    );
}

#[test]
fn numeric_ops_on_locals_have_no_borrow() {
    let out = emit_first_fn("fn f(a: int, b: int) to int { return a * b - a }");
    assert!(
        !out.contains("& "),
        "numeric `* -` must not borrow operands:\n{out}"
    );
}

#[test]
fn string_concat_keeps_borrowed_rhs() {
    // `String + &str` requires the borrow — dropping it would not compile.
    let out = emit_first_fn("fn cat(a: str, b: str) to str { return a + b }");
    assert!(
        out.contains("+ &"),
        "string concatenation must keep the borrowed RHS:\n{out}"
    );
}
