//! Task 8 — empirical parity smoke tests.
//!
//! Drives real HIR nodes through the live emitters (interpreter, Rust codegen,
//! TS codegen) and verifies that parity-matrix cells produce the stable
//! diagnostic codes declared in `feature_matrix.rs`.
//!
//! Each test is a "does the *right channel* carry the *right code*?" assertion.
//! No golden snapshots: these tests deliberately ignore incidental message text
//! and only pin the stable codes from `codes::PARITY_*`.

use vox_compiler::ast::span::Span;
use vox_compiler::eval::expr::eval_expr;
use vox_compiler::eval::value::VoxValue;
use vox_compiler::eval::{EvalError, Interpreter};
use vox_compiler::hir::{HirExpr, HirJsxElement, HirWorkflowVersion};
use vox_compiler::typeck::diagnostics::codes;

// ── helpers ─────────────────────────────────────────────────────────────────

fn zero() -> Span {
    Span::new(0, 0)
}

fn assert_assertion_failed_contains(result: Result<VoxValue, EvalError>, code: &str) {
    match result {
        Err(EvalError::AssertionFailed(msg)) => {
            assert!(
                msg.contains(code),
                "expected parity code {code:?} in AssertionFailed message, got: {msg:?}"
            );
        }
        other => panic!("expected EvalError::AssertionFailed containing {code:?}, got: {other:?}"),
    }
}

// ── Test 1: JSX → interpreter returns PARITY_FRONTEND_ONLY ──────────────────

#[test]
fn jsx_interp_errors_with_frontend_only_code() {
    let jsx = HirExpr::Jsx(HirJsxElement {
        tag: "div".to_string(),
        attributes: vec![],
        children: vec![],
        span: zero(),
    });
    let mut interp = Interpreter::new(1_000_000);
    let result = eval_expr(&mut interp, &jsx);
    assert_assertion_failed_contains(result, codes::PARITY_FRONTEND_ONLY);
}

// ── Test 2: Spawn → interpreter returns PARITY_BACKEND_ONLY ─────────────────

#[test]
fn spawn_interp_errors_with_backend_only_code() {
    // Spawn(Box<HirExpr>, Span) — the inner expr is never evaluated because
    // the parity gate fires before any sub-expression walk.
    let spawn = HirExpr::Spawn(
        Box::new(HirExpr::StringLit("actor".to_string(), zero())),
        zero(),
    );
    let mut interp = Interpreter::new(1_000_000);
    let result = eval_expr(&mut interp, &spawn);
    assert_assertion_failed_contains(result, codes::PARITY_BACKEND_ONLY);
}

// ── Test 3: With → interpreter returns PARITY_UNIMPLEMENTED ─────────────────

#[test]
fn with_interp_errors_with_unimplemented_code() {
    let lit = || Box::new(HirExpr::StringLit("x".to_string(), zero()));
    let with_expr = HirExpr::With(lit(), lit(), zero());
    let mut interp = Interpreter::new(1_000_000);
    let result = eval_expr(&mut interp, &with_expr);
    assert_assertion_failed_contains(result, codes::PARITY_UNIMPLEMENTED);
}

// ── Test 4: WorkflowVersion → Rust codegen emits compile_error! ─────────────

#[test]
fn workflow_version_rust_emitter_produces_parity_code() {
    use vox_codegen::codegen_rust::emit::emit_expr;

    let wv = HirExpr::WorkflowVersion(HirWorkflowVersion {
        change_id: "v1".to_string(),
        min: 1,
        max: 2,
        span: zero(),
    });
    let output = emit_expr(&wv);
    assert!(
        output.contains("compile_error!"),
        "Rust emitter must emit compile_error! for WorkflowVersion; got: {output:?}"
    );
    assert!(
        output.contains(codes::PARITY_UNIMPLEMENTED),
        "compile_error! must carry parity code {:?}; got: {output:?}",
        codes::PARITY_UNIMPLEMENTED
    );
}

// ── Test 5: WorkflowVersion → TS codegen emits `satisfies never` type error ──

#[test]
fn workflow_version_ts_emitter_produces_parity_code() {
    use std::collections::HashSet;
    use vox_codegen_ts::codegen_ts::hir_emit::{EmitCtx, emit_hir_expr};

    let wv = HirExpr::WorkflowVersion(HirWorkflowVersion {
        change_id: "v1".to_string(),
        min: 1,
        max: 2,
        span: zero(),
    });
    let state_names = HashSet::new();
    let ctx = EmitCtx::new(&state_names);
    let output = emit_hir_expr(&wv, &ctx);
    // Must use `satisfies never` (a real TS compile error, TS2735) not `as never`.
    assert!(
        output.contains("satisfies never"),
        "TS emitter must emit 'satisfies never' type error for WorkflowVersion; got: {output:?}"
    );
    assert!(
        output.contains(codes::PARITY_UNIMPLEMENTED),
        "TS emitter must carry parity code {:?}; got: {output:?}",
        codes::PARITY_UNIMPLEMENTED
    );
}
