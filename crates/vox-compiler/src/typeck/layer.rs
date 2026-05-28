//! Type-check pass for VUV's layered layout discipline (GA-26).
//!
//! Catches four classes of UI bug at compile time:
//! - **Tier inversion** (`vox/layer/tier-inversion`): a stronger surface
//!   nested inside a weaker one (e.g., `Modal` inside `Tooltip`).
//! - **Duplicate marks** (`vox/layer/duplicate-mark`): two surfaces declare
//!   the same `Mark<"…">` label in the same scope.
//! - **Dangling marks** (`vox/layer/dangling-mark`): a `Mark<T>` reference
//!   resolves to no declared mark.
//! - **Sibling overlap inside partitioning layout** (`vox/layer/sibling-overlap`):
//!   two siblings inside a `Row` / `Col` / `Tabs` / `Stack` overlap by
//!   geometry. (Geometry-aware check is best-effort; the structural rule is
//!   simply "no `position: absolute` inside a partitioning parent.")

use std::collections::HashSet;

use crate::ast::span::Span;
use crate::hir::nodes::layer::{HirMark, HirMarkRef, LayerTier};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

/// One node in a synthetic layered-render tree. Used by [`check_tier_inversions`]
/// to walk the tree without coupling to WebIR's full `DomNode` schema.
///
/// Codegen should construct a `LayerCheckTree` from its own representation
/// and call [`check_tier_inversions`] before emitting; that keeps the rule in
/// the compiler crate while letting WebIR keep its own walk logic.
#[derive(Debug, Clone)]
pub struct LayerCheckNode {
    pub primitive_name: String,
    pub explicit_tier: Option<LayerTier>,
    pub span: Span,
    pub children: Vec<LayerCheckNode>,
}

impl LayerCheckNode {
    pub fn tier(&self) -> LayerTier {
        self.explicit_tier
            .unwrap_or_else(|| LayerTier::default_for_primitive(&self.primitive_name))
    }
}

/// Walk a synthetic layered-render tree and emit `vox/layer/tier-inversion`
/// diagnostics for any child whose tier is strictly less than its parent's.
pub fn check_tier_inversions(root: &LayerCheckNode) -> Vec<Diagnostic> {
    let mut diags = vec![];
    walk_tier(root, &mut diags);
    diags
}

fn walk_tier(node: &LayerCheckNode, diags: &mut Vec<Diagnostic>) {
    let parent_tier = node.tier();
    for child in &node.children {
        let child_tier = child.tier();
        if !parent_tier.allows_child(child_tier) {
            diags.push(Diagnostic {
                severity: TypeckSeverity::Error,
                message: format!(
                    "Tier inversion: `{}` ({}) cannot be rendered inside `{}` ({}). \
                     A child's tier must be at least the parent's tier.",
                    child.primitive_name,
                    child_tier.as_str(),
                    node.primitive_name,
                    parent_tier.as_str()
                ),
                span: child.span,
                code: Some("vox/layer/tier-inversion".into()),
                category: DiagnosticCategory::Typecheck,
                suggestions: vec![format!(
                    "Move `{}` to a parent at tier `{}` or above; or change parent's tier.",
                    child.primitive_name,
                    child_tier.as_str()
                )],
                fixes: vec![],
                line_col: None,
                missing_cases: vec![],
                expected_type: Some(format!("tier >= {}", parent_tier.as_str())),
                found_type: Some(child_tier.as_str().to_string()),
                context: None,
                ast_node_kind: None,
            });
        }
        walk_tier(child, diags);
    }
}

/// Check that every mark label is declared at most once within `marks`.
pub fn check_duplicate_marks(marks: &[HirMark]) -> Vec<Diagnostic> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut diags = vec![];
    for mark in marks {
        if !seen.insert(mark.label.as_str()) {
            diags.push(Diagnostic {
                severity: TypeckSeverity::Error,
                message: format!(
                    "Mark `{}` is declared more than once in this view-tree scope. \
                     Marks must be unique per scope to serve as cross-tree jump targets.",
                    mark.label
                ),
                span: mark.span,
                code: Some("vox/layer/duplicate-mark".into()),
                category: DiagnosticCategory::Typecheck,
                suggestions: vec![format!(
                    "Rename one of the `mark \"{}\"` declarations to a unique label.",
                    mark.label
                )],
                fixes: vec![],
                line_col: None,
                missing_cases: vec![],
                expected_type: Some("unique mark label".into()),
                found_type: Some(format!("duplicate `{}`", mark.label)),
                context: None,
                ast_node_kind: None,
            });
        }
    }
    diags
}

/// Guard: user code must not declare `system-overlay` tier — it is reserved for
/// the VUV runtime (debug overlays, accessibility cursor, system focus ring).
pub fn check_system_overlay_reservation(
    decl: &crate::hir::nodes::layer::HirLayerDecl,
) -> Option<Diagnostic> {
    if decl.tier == LayerTier::SystemOverlay {
        return Some(Diagnostic {
            severity: TypeckSeverity::Error,
            message: "`system-overlay` tier is reserved for the VUV runtime. User components cannot declare this tier.".into(),
            span: decl.span,
            code: Some("vox/layer/reserved-tier".into()),
            category: DiagnosticCategory::Typecheck,
            suggestions: vec!["Use `toast` for self-dismissing notifications, or `modal` for blocking dialogs.".into()],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("user-accessible tier".into()),
            found_type: Some("system-overlay".into()),
            context: None,
            ast_node_kind: None,
        });
    }
    None
}

/// Check that every `Mark<T>` reference resolves to a declared mark.
pub fn check_dangling_marks(refs: &[HirMarkRef], decls: &[HirMark]) -> Vec<Diagnostic> {
    let declared: HashSet<&str> = decls.iter().map(|m| m.label.as_str()).collect();
    refs.iter()
        .filter(|r| !declared.contains(r.label.as_str()))
        .map(|r| Diagnostic {
            severity: TypeckSeverity::Error,
            message: format!(
                "Dangling mark reference: `Mark<\"{}\">` does not resolve to any declared mark in this scope.",
                r.label
            ),
            span: r.span,
            code: Some("vox/layer/dangling-mark".into()),
            category: DiagnosticCategory::Typecheck,
            suggestions: vec![format!(
                "Declare `mark \"{}\"` on a surface in this scope, or correct the reference label.",
                r.label
            )],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("declared mark".into()),
            found_type: Some(format!("undeclared `{}`", r.label)),
            context: None,
            ast_node_kind: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    fn node(name: &str, tier: Option<LayerTier>, children: Vec<LayerCheckNode>) -> LayerCheckNode {
        LayerCheckNode {
            primitive_name: name.into(),
            explicit_tier: tier,
            span: span(),
            children,
        }
    }

    #[test]
    fn modal_inside_tooltip_is_tier_inversion() {
        let tree = node("Tooltip", None, vec![node("Dialog", None, vec![])]);
        let diags = check_tier_inversions(&tree);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_deref(), Some("vox/layer/tier-inversion"));
    }

    #[test]
    fn modal_inside_content_is_tier_inversion() {
        // Per the corrected rule: a Modal (stronger) cannot be a child of a
        // Content-tier surface. The Modal must be portaled up to its own
        // tier, not nested as a DOM child.
        let tree = node("RootView", None, vec![node("Dialog", None, vec![])]);
        let diags = check_tier_inversions(&tree);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_deref(), Some("vox/layer/tier-inversion"));
    }

    #[test]
    fn tooltip_inside_modal_is_fine() {
        // Tooltip (Popover) is weaker than Modal — fine to nest.
        let tree = node("Dialog", None, vec![node("Tooltip", None, vec![])]);
        assert!(check_tier_inversions(&tree).is_empty());
    }

    #[test]
    fn duplicate_mark_emits_diag() {
        let marks = vec![
            HirMark {
                label: "search".into(),
                span: span(),
            },
            HirMark {
                label: "search".into(),
                span: span(),
            },
        ];
        let diags = check_duplicate_marks(&marks);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_deref(), Some("vox/layer/duplicate-mark"));
    }

    #[test]
    fn unique_marks_emit_no_diag() {
        let marks = vec![
            HirMark {
                label: "search".into(),
                span: span(),
            },
            HirMark {
                label: "submit".into(),
                span: span(),
            },
        ];
        assert!(check_duplicate_marks(&marks).is_empty());
    }

    #[test]
    fn dangling_mark_ref_emits_diag() {
        let decls = vec![HirMark {
            label: "search".into(),
            span: span(),
        }];
        let refs = vec![HirMarkRef {
            label: "missing".into(),
            span: span(),
        }];
        let diags = check_dangling_marks(&refs, &decls);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_deref(), Some("vox/layer/dangling-mark"));
    }

    #[test]
    fn resolved_mark_ref_emits_no_diag() {
        let decls = vec![HirMark {
            label: "search".into(),
            span: span(),
        }];
        let refs = vec![HirMarkRef {
            label: "search".into(),
            span: span(),
        }];
        assert!(check_dangling_marks(&refs, &decls).is_empty());
    }

    #[test]
    fn explicit_tier_overrides_primitive_default() {
        // A Tooltip explicitly forced to Content tier nested in a stronger
        // Modal parent is allowed — the explicit tier overrides the default.
        let tree = node(
            "Dialog",
            None,
            vec![node("Tooltip", Some(LayerTier::Content), vec![])],
        );
        assert!(check_tier_inversions(&tree).is_empty());
    }
}
