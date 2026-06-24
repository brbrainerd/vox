//! ICE-guard integration tests (Task 3.3).
//!
//! Verifies that `Diagnostic::ice` produces a well-formed ICE diagnostic
//! with the canonical `vox/internal/ice` code, and that it integrates
//! correctly with the `VoxCompilerDiagnosticPayload` serialization path.
//!
//! Prior to Task 3.3, several compiler invariant sites used bare
//! `panic!` / `unreachable!`.  These tests guard the post-fix
//! behaviour — the compiler must emit a recoverable ICE diagnostic
//! instead of aborting the process.

use vox_compiler::ast::span::Span;
use vox_compiler::typeck::diagnostics::{
    Diagnostic, DiagnosticCategory, TypeckSeverity, VoxCompilerDiagnosticPayload,
};

// ── 1. Constructor contracts ──────────────────────────────────────────────────

#[test]
fn ice_code_is_canonical() {
    let d = Diagnostic::ice("some invariant", Span { start: 0, end: 0 }, "");
    assert_eq!(
        d.code.as_deref(),
        Some("vox/internal/ice"),
        "ICE diagnostics must use the canonical vox/internal/ice code"
    );
}

#[test]
fn ice_category_is_hir_invariant() {
    let d = Diagnostic::ice("some invariant", Span { start: 0, end: 0 }, "");
    assert!(
        matches!(d.category, DiagnosticCategory::HirInvariant),
        "ICE diagnostics must be categorised as HirInvariant, got {:?}",
        d.category
    );
}

#[test]
fn ice_severity_is_error() {
    let d = Diagnostic::ice("some invariant", Span { start: 0, end: 0 }, "");
    assert!(
        matches!(d.severity, TypeckSeverity::Error),
        "ICE diagnostics must be Error severity"
    );
}

#[test]
fn ice_message_contains_report_hint() {
    let d = Diagnostic::ice("null type in emit", Span { start: 0, end: 0 }, "");
    assert!(
        d.message.contains("internal compiler error"),
        "ICE message must contain 'internal compiler error', got: {}",
        d.message
    );
    assert!(
        d.message.contains("null type in emit"),
        "ICE message must embed the caller's description, got: {}",
        d.message
    );
}

// ── 2. Payload serialisation ──────────────────────────────────────────────────

#[test]
fn ice_payload_has_explain_url() {
    let d = Diagnostic::ice("test", Span { start: 0, end: 0 }, "");
    let payload = VoxCompilerDiagnosticPayload::from_diagnostic(&d, "test.vox", "");
    // vox/ prefix ⇒ explain URL is produced
    assert_eq!(
        payload.explain_url.as_deref(),
        Some("https://vox-lang.org/diag/vox/internal/ice"),
        "ICE payload must have the canonical explain URL"
    );
}

#[test]
fn ice_payload_error_code_is_canonical() {
    let d = Diagnostic::ice("test", Span { start: 0, end: 0 }, "");
    let payload = VoxCompilerDiagnosticPayload::from_diagnostic(&d, "test.vox", "");
    assert_eq!(
        payload.error_code, "vox/internal/ice",
        "payload.error_code must be the canonical ICE code"
    );
}

// ── 3. Source-span context is captured ───────────────────────────────────────

#[test]
fn ice_captures_source_context() {
    let src = "fn foo() { }";
    let span = Span { start: 0, end: 2 };
    let d = Diagnostic::ice("test with source", span, src);
    // context is populated from the source snippet
    assert!(
        d.context.is_some(),
        "ICE must capture source context when source is non-empty"
    );
}

// ── 4. Determinism-lint ICE guard (Task 3.3 conversion) ──────────────────────
//
// The `walk_expr` function in `typeck/determinism_lint.rs` previously had an
// `unreachable!()` for the branch where `callee_path()` returns `Some` but
// the expression is neither `Call` nor `MethodCall` (a compiler invariant
// violation).  After Task 3.3 it pushes a `Diagnostic::ice` instead.
//
// We can't cheaply construct a synthetic HIR that triggers the guarded path
// from outside the crate; the integration contract is covered by the unit
// test `ice_has_correct_code_and_category` inside `diagnostics.rs` which
// compiles alongside the conversion.  The test below verifies that the
// lint pipeline itself still rejects non-deterministic calls correctly
// (i.e. the guard did not break the happy path).

use vox_compiler::{lexer::cursor::lex, parser::parse, typeck::typecheck_ast_module};

#[test]
fn determinism_lint_still_fires_after_ice_guard_conversion() {
    // This is a workflow that calls std.time.now_ms() — must remain flagged.
    let src = r#"
workflow timing_check() to int {
    let t = std.time.now_ms()
    return t
}
"#;
    let m = parse(lex(src)).expect("parse");
    let diags = typecheck_ast_module(src, &m);
    let has_det_diag = diags
        .iter()
        .any(|d| d.code.as_deref() == Some("lint.workflow.non_deterministic"));
    assert!(
        has_det_diag,
        "determinism lint must still fire after the ICE-guard conversion; got: {:?}",
        diags
            .iter()
            .map(|d| d.code.as_deref().unwrap_or("<none>"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn determinism_lint_produces_no_ice_for_valid_workflow() {
    // A deterministic workflow — must not produce any ICE diagnostic.
    let src = r#"
workflow pure_add(x: int, y: int) to int {
    return x + y
}
"#;
    let m = parse(lex(src)).expect("parse");
    let diags = typecheck_ast_module(src, &m);
    let ice_diag = diags
        .iter()
        .find(|d| d.code.as_deref() == Some("vox/internal/ice"));
    assert!(
        ice_diag.is_none(),
        "a deterministic workflow must not produce an ICE diagnostic; got: {:?}",
        ice_diag
    );
}
