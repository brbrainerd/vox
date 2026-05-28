//! Exhaustiveness check for `Async[T]` view arms.
//!
//! Per GA-01: every `when` block that discriminates an `Async[T]` value must
//! supply all four structural arms (`fetching`, `empty`, `error`, `ok`).
//! Missing any arm is a `vox/async/missing-arm` compile error.

use crate::hir::nodes::async_view::HirAsyncView;
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

/// Check a single `HirAsyncView` for arm exhaustiveness.
///
/// Returns at most one `vox/async/missing-arm` diagnostic listing all absent arms.
pub fn check_async_view(view: &HirAsyncView) -> Option<Diagnostic> {
    let missing = view.missing_arms();
    if missing.is_empty() {
        return None;
    }
    let arm_list = missing.join("`, `");
    Some(Diagnostic {
        severity: TypeckSeverity::Error,
        message: format!(
            "`Async[T]` view is not exhaustive: missing arm(s) `{arm_list}`. \
             All four arms (`fetching`, `empty`, `error`, `ok`) are required."
        ),
        span: view.span,
        code: Some("vox/async/missing-arm".into()),
        category: DiagnosticCategory::Typecheck,
        suggestions: missing
            .iter()
            .map(|arm| format!("Add: `when {arm} => {{ … }}`"))
            .collect(),
        fixes: vec![],
        line_col: None,
        missing_cases: missing.iter().map(|s| s.to_string()).collect(),
        expected_type: Some("exhaustive async arms".into()),
        found_type: Some(format!("missing: {}", missing.join(", "))),
        context: None,
        ast_node_kind: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::span::Span;
    use crate::hir::HirExpr;
    use crate::hir::nodes::async_view::HirAsyncView;

    fn dummy_span() -> Span {
        Span { start: 0, end: 0 }
    }

    fn dummy_expr() -> Box<HirExpr> {
        Box::new(HirExpr::BoolLit(true, dummy_span()))
    }

    fn source_expr() -> Box<HirExpr> {
        Box::new(HirExpr::Ident("myData".into(), dummy_span()))
    }

    #[test]
    fn exhaustive_view_passes() {
        let view = HirAsyncView {
            source: source_expr(),
            fetching_arm: Some(dummy_expr()),
            empty_arm: Some(dummy_expr()),
            error_arm: Some(dummy_expr()),
            error_binding: Some("e".into()),
            ok_arm: Some(dummy_expr()),
            ok_binding: Some("data".into()),
            span: dummy_span(),
        };
        assert!(check_async_view(&view).is_none());
    }

    #[test]
    fn missing_single_arm_fails() {
        let view = HirAsyncView {
            source: source_expr(),
            fetching_arm: Some(dummy_expr()),
            empty_arm: Some(dummy_expr()),
            error_arm: None,
            error_binding: None,
            ok_arm: Some(dummy_expr()),
            ok_binding: Some("data".into()),
            span: dummy_span(),
        };
        let diag = check_async_view(&view).expect("should have diagnostic");
        assert_eq!(diag.code.as_deref(), Some("vox/async/missing-arm"));
        assert!(diag.missing_cases.contains(&"error".to_string()));
    }

    #[test]
    fn missing_all_arms_reports_all() {
        let view = HirAsyncView {
            source: source_expr(),
            fetching_arm: None,
            empty_arm: None,
            error_arm: None,
            error_binding: None,
            ok_arm: None,
            ok_binding: None,
            span: dummy_span(),
        };
        let diag = check_async_view(&view).expect("should have diagnostic");
        assert_eq!(diag.missing_cases.len(), 4);
    }

    #[test]
    fn missing_arm_code_is_stable() {
        let view = HirAsyncView {
            source: source_expr(),
            fetching_arm: None,
            empty_arm: Some(dummy_expr()),
            error_arm: Some(dummy_expr()),
            error_binding: None,
            ok_arm: Some(dummy_expr()),
            ok_binding: None,
            span: dummy_span(),
        };
        let diag = check_async_view(&view).unwrap();
        // diagnostic ID must be stable per Phase 1 SSOT Collapse append-only policy
        assert_eq!(diag.code.as_deref(), Some("vox/async/missing-arm"));
    }
}
