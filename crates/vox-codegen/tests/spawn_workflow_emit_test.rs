/// Task 8D: Spawn + WorkflowVersion Rust emission (replaces compile_error! stubs).
use vox_codegen::codegen_rust::emit::emit_expr;
use vox_compiler::ast::span::Span;
use vox_compiler::hir::{HirExpr, HirWorkflowVersion};

fn span() -> Span {
    Span { start: 0, end: 0 }
}

#[test]
fn spawn_emits_tokio_spawn() {
    let inner = HirExpr::IntLit(42, span());
    let expr = HirExpr::Spawn(Box::new(inner), span());
    let result = emit_expr(&expr);
    assert!(
        result.contains("tokio::spawn"),
        "Spawn must emit tokio::spawn, got: {result}"
    );
    assert!(
        result.contains("async move"),
        "Spawn must wrap in async move, got: {result}"
    );
    assert!(
        result.contains("42"),
        "Spawn must include inner expression, got: {result}"
    );
}

#[test]
fn workflow_version_emits_coded_error() {
    let expr = HirExpr::WorkflowVersion(HirWorkflowVersion {
        change_id: "add-discount-field".to_string(),
        min: 1,
        max: 3,
        span: span(),
    });
    let result = emit_expr(&expr);
    assert!(
        result.contains("compile_error!"),
        "WorkflowVersion must emit compile_error! (no Rust runtime backing yet), got: {result}"
    );
    assert!(
        result.contains("add-discount-field"),
        "compile_error! message must include change_id, got: {result}"
    );
    assert!(
        result.contains("vox.codegen_rust.workflow_version_unimplemented"),
        "compile_error! must include diagnostic code, got: {result}"
    );
}
