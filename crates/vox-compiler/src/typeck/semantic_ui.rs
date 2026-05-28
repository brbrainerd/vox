//! GA-19 typecheck: a11y rigor for semantic UI primitives.
//!
//! Catches missing-label on `Dialog`, `Menu`, `Listbox`, `Combobox`, `Tabs`.
//! These are P0 levers — a screen-reader user encounters a label-less dialog
//! as "interactive region of unknown purpose" and cannot orient.

use crate::ast::span::Span;
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

/// A semantic UI primitive callsite the lint walks.
#[derive(Debug, Clone)]
pub struct SemanticUiCallSite {
    /// Primitive name: `"Dialog"`, `"Menu"`, `"Listbox"`, `"Combobox"`, `"Tabs"`.
    pub primitive: String,
    /// Whether the callsite supplies a `label` prop.
    pub has_label: bool,
    pub span: Span,
}

/// Refuse compile when a semantic UI primitive that requires a label omits it.
///
/// Diagnostic ids:
/// - `vox/a11y/dialog-missing-label`
/// - `vox/a11y/menu-missing-label`
/// - `vox/a11y/listbox-missing-label`
/// - `vox/a11y/combobox-missing-label`
/// - `vox/a11y/tabs-missing-label`
pub fn check_semantic_ui(callsites: &[SemanticUiCallSite]) -> Vec<Diagnostic> {
    callsites
        .iter()
        .filter(|c| !c.has_label && requires_label(&c.primitive))
        .map(|c| {
            let diag_code = format!("vox/a11y/{}-missing-label", c.primitive.to_lowercase());
            Diagnostic {
                severity: TypeckSeverity::Error,
                message: format!(
                    "<{}> requires a `label` prop for accessibility (screen-reader orientation, ARIA labelling).",
                    c.primitive
                ),
                span: c.span,
                code: Some(diag_code),
                category: DiagnosticCategory::Typecheck,
                suggestions: vec![format!(
                    "Add `label=\"…\"` to the <{}> callsite.",
                    c.primitive
                )],
                fixes: vec![],
                line_col: None,
                missing_cases: vec![],
                expected_type: Some("label prop".into()),
                found_type: Some("missing".into()),
                context: None,
                ast_node_kind: None,
            }
        })
        .collect()
}

fn requires_label(primitive: &str) -> bool {
    matches!(
        primitive,
        "Dialog" | "Menu" | "Listbox" | "Combobox" | "Tabs"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    #[test]
    fn dialog_without_label_rejected() {
        let cs = SemanticUiCallSite {
            primitive: "Dialog".into(),
            has_label: false,
            span: span(),
        };
        let diags = check_semantic_ui(&[cs]);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code.as_deref(),
            Some("vox/a11y/dialog-missing-label")
        );
    }

    #[test]
    fn dialog_with_label_passes() {
        let cs = SemanticUiCallSite {
            primitive: "Dialog".into(),
            has_label: true,
            span: span(),
        };
        assert!(check_semantic_ui(&[cs]).is_empty());
    }

    #[test]
    fn menu_listbox_combobox_tabs_all_require_label() {
        for p in ["Menu", "Listbox", "Combobox", "Tabs"] {
            let cs = SemanticUiCallSite {
                primitive: p.into(),
                has_label: false,
                span: span(),
            };
            assert_eq!(
                check_semantic_ui(&[cs]).len(),
                1,
                "{p} should require label"
            );
        }
    }

    #[test]
    fn unknown_primitives_dont_get_a_label_check() {
        let cs = SemanticUiCallSite {
            primitive: "MyCustomThing".into(),
            has_label: false,
            span: span(),
        };
        assert!(check_semantic_ui(&[cs]).is_empty());
    }
}
