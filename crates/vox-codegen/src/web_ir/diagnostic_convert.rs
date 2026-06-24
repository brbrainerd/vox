//! [`From`] impl bridging [`WebIrDiagnostic`] into the unified [`Diagnostic`] envelope.
//!
//! This wires web-IR validation errors into the `vox check --for-llm` pipeline so
//! that frontend (DOM/style/a11y) failures appear alongside typeck diagnostics.

use vox_compiler::ast::span::Span;
use vox_compiler::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

use super::{WebIrDiagnostic, WebIrDiagnosticSeverity};

impl From<WebIrDiagnostic> for Diagnostic {
    fn from(w: WebIrDiagnostic) -> Self {
        let severity = match w.severity() {
            WebIrDiagnosticSeverity::Error => TypeckSeverity::Error,
            // Collapse Info → Warning: TypeckSeverity has no Info variant.
            WebIrDiagnosticSeverity::Warning | WebIrDiagnosticSeverity::Info => {
                TypeckSeverity::Warning
            }
        };

        // Map the optional web-IR category string to the closest DiagnosticCategory.
        let category = match w.category.as_deref() {
            Some("a11y") | Some("dom") | Some("style") | Some("lower") => {
                DiagnosticCategory::Lowering
            }
            Some("lint") => DiagnosticCategory::Lint,
            _ => DiagnosticCategory::Lowering,
        };

        Diagnostic {
            severity,
            message: w.message,
            // TODO(span): resolve SourceSpanId to byte-offset Span when the
            // SourceSpanTable is threaded through the conversion boundary.
            span: Span::new(0, 0),
            expected_type: None,
            found_type: None,
            context: None,
            suggestions: vec![],
            category,
            code: Some(w.code),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::typeck::diagnostics::TypeckSeverity;

    #[test]
    fn webir_diag_converts_severity_and_code() {
        let w = WebIrDiagnostic {
            code: "vox/webir/overlay-occlusion".into(),
            message: "overlay occludes interactive element".into(),
            span: None,
            category: Some("a11y".into()),
        };
        let d: Diagnostic = w.into();
        assert_eq!(d.code.as_deref(), Some("vox/webir/overlay-occlusion"));
        assert!(matches!(
            d.severity,
            TypeckSeverity::Error | TypeckSeverity::Warning
        ));
    }

    #[test]
    fn webir_warning_code_maps_to_typeck_warning() {
        // "web_ir_validate.a11y.anchor_missing_href" is in the advisory-only list
        let w = WebIrDiagnostic {
            code: "web_ir_validate.a11y.anchor_missing_href".into(),
            message: "anchor missing href".into(),
            span: None,
            category: Some("a11y".into()),
        };
        let d: Diagnostic = w.into();
        assert_eq!(d.severity, TypeckSeverity::Warning);
    }

    #[test]
    fn webir_error_code_maps_to_typeck_error() {
        let w = WebIrDiagnostic {
            code: "web_ir_validate.dom.invalid_root".into(),
            message: "invalid DOM root".into(),
            span: None,
            category: None,
        };
        let d: Diagnostic = w.into();
        assert_eq!(d.severity, TypeckSeverity::Error);
    }
}
