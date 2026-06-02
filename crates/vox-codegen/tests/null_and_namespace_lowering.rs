//! Codegen-parity acceptance: statement-position namespace calls and the
//! typed `null` (Option None) literal must lower to valid Rust.
//!
//! Background: `null` is Vox's polymorphic None — typeck binds it as a
//! `Constructor` of type `Option[T]` (see `vox-compiler` typeck builtins), and
//! the interpreter treats it as `VoxValue::Null`. Only the Rust emit layer was
//! missing the lowering, so generated crates referenced an undefined `null`
//! identifier. Likewise `process.exit(..)` / `process.run(..)` parse as
//! `MethodCall` and bypassed the FieldAccess namespace lowering, emitting a
//! method call on an undefined `process` value. These tests pin the fixes.

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

/// Lower a single top-level function `f` from `src` to Rust.
fn emit_first_fn(src: &str) -> String {
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let func = hir
        .functions
        .iter()
        .find(|f| f.name == "f")
        .expect("function `f` present");
    emit_fn(func, Some(&hir.inferred_types), &[])
}

#[test]
fn is_null_lowers_to_is_none() {
    let rust = emit_first_fn("fn f(opt: Option[str]) to bool { return opt is null }");
    assert!(
        rust.contains(".is_none()"),
        "`opt is null` MUST lower to `.is_none()`; got:\n{rust}"
    );
    assert!(
        !rust.contains("== null") && !rust.contains("clone() == None"),
        "`is null` must not lower to a `== null`/`== None` comparison; got:\n{rust}"
    );
}

#[test]
fn isnt_null_lowers_to_is_some() {
    let rust = emit_first_fn("fn f(opt: Option[str]) to bool { return opt isnt null }");
    assert!(
        rust.contains(".is_some()"),
        "`opt isnt null` MUST lower to `.is_some()`; got:\n{rust}"
    );
}

#[test]
fn bare_null_lowers_to_none() {
    let rust = emit_first_fn("fn f() to Option[int] { return null }");
    assert!(
        rust.contains("None"),
        "bare `null` in value position MUST lower to `None`; got:\n{rust}"
    );
    // The undefined-identifier footgun: a literal `null` token must never reach
    // the generated Rust.
    assert!(
        !rust.contains(" null") && !rust.contains("null;") && !rust.contains("null\n"),
        "generated Rust must not reference a bare `null` identifier; got:\n{rust}"
    );
}

#[test]
fn process_run_error_arm_is_err_not_error() {
    // Regression: the namespace runtime-call templates emitted the undefined
    // Rust constructor `Error(m)` instead of `Err(m)` for the error arm.
    let rust = emit_first_fn("fn f() to unit { let r = process.run(\"cargo\", [\"build\"]) }");
    assert!(
        rust.contains("vox_process_run"),
        "`process.run` MUST lower to the runtime builtin; got:\n{rust}"
    );
    assert!(
        rust.contains("Err(m)") && !rust.contains("Error(m)"),
        "the error arm MUST be `Err(m)`, never the undefined `Error(m)`; got:\n{rust}"
    );
}

#[test]
fn process_exit_lowers_to_std_process_exit() {
    let rust = emit_first_fn("fn f() to unit { process.exit(1) }");
    assert!(
        rust.contains("std::process::exit"),
        "`process.exit(1)` MUST lower to `std::process::exit`; got:\n{rust}"
    );
    assert!(
        !rust.contains("process.clone()") && !rust.contains("process.exit"),
        "`process.exit` must not emit a method call on an undefined `process`; got:\n{rust}"
    );
}
