//! Binary-operator operand emission: numeric `+ - * /` emit plain infix
//! (`1 + 2`, no borrow), while string concatenation emits `format!("{}{}", …)`
//! — which Displays both operands, so `str + str` AND `str + <numeric>` (`s + 5`)
//! both compile, matching the interpreter's auto-stringify semantics.
//!
//! Fast: asserts on `emit_fn` output strings — no crate compile needed.

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::typeck::typecheck_hir_module;

fn emit_first_fn(src: &str) -> String {
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    // Typeck populates `inferred_types` (which codegen reads for the binary's
    // result type); the real pipeline always runs it before emit.
    let _ = typecheck_hir_module(src, &mut hir);
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
fn string_concat_uses_format() {
    // `str + str` → `format!("{}{}", a, b)` (Displays both; no `String + String`).
    let out = emit_first_fn("fn cat(a: str, b: str) to str { return a + b }");
    assert!(
        out.contains("format!(\"{}{}\""),
        "string concatenation must use format!, got:\n{out}"
    );
}

#[test]
fn str_plus_numeric_uses_format_not_plus() {
    // `s + 5` type-checks as `str` (auto-stringify); codegen must `format!` it,
    // NOT emit `String + i64` (which has no Add impl). Regression for the
    // pre-existing str+numeric miscompile.
    let out = emit_first_fn("fn f(s: str) to str { return s + 5 }");
    assert!(
        out.contains("format!(\"{}{}\""),
        "str + numeric must use format!, got:\n{out}"
    );
    assert!(
        !out.contains("s + 5") && !out.contains("s + &5"),
        "str + numeric must NOT emit `String + i64`:\n{out}"
    );
}
