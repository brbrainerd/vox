//! Regression: the interpreter must restore its scope when a function body
//! returns an `EvalError` via `?` early-return — otherwise a failed `call`
//! leaks scope state into later reuse of the same `Interpreter` (e.g. the
//! `@test` runner loops `interp.call(...)` and continues after an `Err`).
//!
//! Observability: the leak is masked externally (the next `call` overwrites
//! `interp.scope`), so we assert on `Scope::depth()` directly.

use vox_compiler::eval::Interpreter;

fn lower(src: &str) -> vox_compiler::hir::HirModule {
    let tokens = vox_compiler::lexer::lex(src);
    let module = vox_compiler::parser::parse_script(tokens).expect("parse");
    vox_compiler::hir::lower::lower_module(&module)
}

/// A function that errors inside a pushed (block) frame must leave the
/// interpreter's scope at baseline depth after the error, not at the leaked
/// inner depth.
#[test]
fn error_in_function_body_restores_scope_depth() {
    // `bad()` pushes a block frame, then `assert(false)` raises an EvalError
    // before the block frame is popped. Without scope restoration the
    // interpreter would be left at the inner (deeper) scope.
    let src = r#"
fn bad() to int {
    let x = 1
    {
        assert(false)
        return x
    }
    return x
}
fn main() to int {
    return 0
}
"#;
    let hir = lower(src);
    let mut interp = Interpreter::new(1_000_000);
    interp.run_module(&hir).expect("run_module");

    let baseline = interp.scope.depth();
    let res = interp.call("bad", vec![]);
    assert!(res.is_err(), "bad() must error (assert false)");

    assert_eq!(
        interp.scope.depth(),
        baseline,
        "scope depth must return to baseline after an EvalError, not leak the body frame"
    );
}

/// After a failed call, a subsequent call still evaluates correctly (no scope
/// corruption) — the end-to-end property the restoration protects.
#[test]
fn call_after_error_still_works() {
    let src = r#"
fn boom() to int {
    assert(false)
    return 1
}
fn good() to int {
    return 7
}
fn main() to int {
    return 0
}
"#;
    let hir = lower(src);
    let mut interp = Interpreter::new(1_000_000);
    interp.run_module(&hir).expect("run_module");

    let _ = interp.call("boom", vec![]); // errors
    let ok = interp
        .call("good", vec![])
        .expect("good() should succeed after a prior error");
    assert_eq!(ok, vox_compiler::eval::value::VoxValue::Int(7));
    assert_eq!(
        interp.scope.depth(),
        1,
        "scope is clean after the recovered call"
    );
}
