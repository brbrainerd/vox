/// Task 8C: AsyncView real TS emission.
///
/// `HirExpr::AsyncView` must emit the four-branch IIFE produced by
/// `emit_async_view_tsx`, not the dead `{src} /* async view */` stub.
use std::collections::HashSet;
use vox_codegen_ts::hir_emit::emit_hir_expr;
use vox_codegen_ts::hir_emit::EmitCtx;
use vox_compiler::hir::{HirAsyncView, HirExpr};
use vox_compiler::ast::span::Span;

fn s() -> Span {
    Span::new(0, 0)
}

fn make_async_view(
    source: HirExpr,
    fetching_arm: Option<HirExpr>,
    empty_arm: Option<HirExpr>,
    error_binding: Option<&str>,
    error_arm: Option<HirExpr>,
    ok_binding: Option<&str>,
    ok_arm: Option<HirExpr>,
) -> HirExpr {
    HirExpr::AsyncView(Box::new(HirAsyncView {
        source: Box::new(source),
        fetching_arm: fetching_arm.map(Box::new),
        empty_arm: empty_arm.map(Box::new),
        error_arm: error_arm.map(Box::new),
        error_binding: error_binding.map(str::to_string),
        ok_arm: ok_arm.map(Box::new),
        ok_binding: ok_binding.map(str::to_string),
        span: s(),
    }))
}

#[test]
fn async_view_emits_four_branch_iife() {
    let expr = make_async_view(
        HirExpr::Ident("userData".to_string(), s()),
        Some(HirExpr::StringLit("Loading...".to_string(), s())),
        Some(HirExpr::StringLit("No data".to_string(), s())),
        Some("e"),
        Some(HirExpr::StringLit("Error".to_string(), s())),
        Some("data"),
        Some(HirExpr::Ident("data".to_string(), s())),
    );

    let state_names = HashSet::new();
    let ctx = EmitCtx::new(&state_names);
    let out = emit_hir_expr(&expr, &ctx);

    assert!(
        out.contains("fetching"),
        "must contain 'fetching' branch dispatch\ngot: {out}"
    );
    assert!(
        out.contains("empty"),
        "must contain 'empty' branch dispatch\ngot: {out}"
    );
    assert!(
        out.contains("error"),
        "must contain 'error' branch dispatch\ngot: {out}"
    );
    assert!(
        !out.contains("/* async view */"),
        "must not contain the old compat stub comment\ngot: {out}"
    );
}

#[test]
fn async_view_binds_ok_and_error_identifiers() {
    let expr = make_async_view(
        HirExpr::Ident("posts".to_string(), s()),
        None,
        None,
        Some("err"),
        Some(HirExpr::Ident("err".to_string(), s())),
        Some("post"),
        Some(HirExpr::Ident("post".to_string(), s())),
    );

    let state_names = HashSet::new();
    let ctx = EmitCtx::new(&state_names);
    let out = emit_hir_expr(&expr, &ctx);

    assert!(
        out.contains("err"),
        "error binding 'err' must appear in emission\ngot: {out}"
    );
    assert!(
        out.contains("post"),
        "ok binding 'post' must appear in emission\ngot: {out}"
    );
}
