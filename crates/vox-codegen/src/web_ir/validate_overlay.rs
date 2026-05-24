//! TASK-6.4 — Overlay block + z-index DAG validator.
//!
//! Checks structural rules for overlay blocks in the DOM arena:
//!
//! - Overlay children with the same `z` value → `duplicate_z` warning.
//! - Overlay children with the same `position` value → `position_conflict` warning.
//!
//! Overlay elements are identified by the `data-vox-overlay="true"` attribute,
//! added by the lowerer when resolving the `overlay` primitive.
//! Their children carry `data-vox-z` and `data-vox-pos` attributes.
//!
//! All codes are under `web_ir_validate.overlay.*`.

use super::{DomNode, DomNodeId, WebIrDiagnostic, WebIrModule};

/// Run overlay structural checks on the DOM arena.
pub fn validate_overlay(module: &WebIrModule, out: &mut Vec<WebIrDiagnostic>) {
    for node in &module.dom_nodes {
        let DomNode::Element {
            attrs, children, ..
        } = node
        else {
            continue;
        };
        // Only check overlay roots.
        if !attrs
            .iter()
            .any(|(k, v)| k == "data-vox-overlay" && v == "true")
        {
            continue;
        }
        check_overlay_children(module, children, out);
    }
}

fn check_overlay_children(
    module: &WebIrModule,
    child_ids: &[DomNodeId],
    out: &mut Vec<WebIrDiagnostic>,
) {
    let mut seen_z: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut seen_pos: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for child_id in child_ids {
        let Some(child_node) = module.dom_nodes.get(child_id.0 as usize) else {
            continue;
        };
        let DomNode::Element { attrs, .. } = child_node else {
            continue;
        };

        // Check z-index uniqueness and discipline.
        if let Some(z_val) = attrs
            .iter()
            .find(|(k, _)| k == "data-vox-z")
            .map(|(_, v)| v.clone())
        {
            // ADR 034: Warn on loose discipline (numeric z-index).
            if z_val.chars().all(|c| c.is_ascii_digit()) {
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.overlay.loose_z_index".to_string(),
                    message: format!(
                        "Numeric z-index '{z_val}' used — prefer named Z-tiers (background, content, popover, etc.) to prevent Z-fighting"
                    ),
                    span: None,
                    category: Some("overlay".to_string()),
                });
            } else if crate::web_ir::ZTier::from_str(&z_val).is_none() {
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.overlay.invalid_z_tier".to_string(),
                    message: format!(
                        "Invalid Z-tier '{z_val}' — must be one of the seven normative tiers"
                    ),
                    span: None,
                    category: Some("overlay".to_string()),
                });
            }

            let count = seen_z.entry(z_val.clone()).or_insert(0);
            *count += 1;
            if *count == 2 {
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.overlay.duplicate_z".to_string(),
                    message: format!(
                        "Two overlay children share z-index '{z_val}' — stacking order is undefined (Z-fighting risk)"
                    ),
                    span: None,
                    category: Some("overlay".to_string()),
                });
            }
        }

        // Check position conflict (same position hint on two children = likely visual overlap).
        if let Some(pos_val) = attrs
            .iter()
            .find(|(k, _)| k == "data-vox-pos")
            .map(|(_, v)| v.clone())
        {
            let count = seen_pos.entry(pos_val.clone()).or_insert(0);
            *count += 1;
            if *count == 2 {
                out.push(WebIrDiagnostic {
                    code: "web_ir_validate.overlay.position_conflict".to_string(),
                    message: format!(
                        "Two overlay children share position '{pos_val}' — they will overlap visually"
                    ),
                    span: None,
                    category: Some("overlay".to_string()),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_ir::{DomNodeId, WebIrModule};

    fn elem(id: u32, attrs: Vec<(&str, &str)>, children: Vec<u32>) -> DomNode {
        DomNode::Element {
            id: DomNodeId(id),
            tag: "div".to_string(),
            attrs: attrs
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            children: children.into_iter().map(DomNodeId).collect(),
            span: None,
        }
    }

    fn module_with_nodes(nodes: Vec<DomNode>) -> WebIrModule {
        WebIrModule {
            dom_nodes: nodes,
            ..Default::default()
        }
    }

    #[test]
    fn overlay_no_children_no_warnings() {
        let m = module_with_nodes(vec![elem(0, vec![("data-vox-overlay", "true")], vec![])]);
        let mut out = Vec::new();
        validate_overlay(&m, &mut out);
        assert!(out.is_empty(), "unexpected: {out:?}");
    }

    #[test]
    fn overlay_unique_z_no_warning() {
        // Named Z-tiers (ADR 034) — numeric z-indexes now trigger
        // `loose_z_index` warnings regardless of uniqueness, so this
        // test was updated to use the named-tier vocabulary.
        let m = module_with_nodes(vec![
            elem(0, vec![("data-vox-overlay", "true")], vec![1, 2]),
            elem(1, vec![("data-vox-z", "content")], vec![]),
            elem(2, vec![("data-vox-z", "popover")], vec![]),
        ]);
        let mut out = Vec::new();
        validate_overlay(&m, &mut out);
        assert!(out.is_empty(), "unexpected: {out:?}");
    }

    #[test]
    fn overlay_duplicate_z_fires_warning() {
        let m = module_with_nodes(vec![
            elem(0, vec![("data-vox-overlay", "true")], vec![1, 2]),
            elem(1, vec![("data-vox-z", "100")], vec![]),
            elem(2, vec![("data-vox-z", "100")], vec![]),
        ]);
        let mut out = Vec::new();
        validate_overlay(&m, &mut out);
        assert!(
            out.iter()
                .any(|d| d.code == "web_ir_validate.overlay.duplicate_z"),
            "expected duplicate_z: {out:?}"
        );
    }

    #[test]
    fn overlay_duplicate_position_fires_warning() {
        let m = module_with_nodes(vec![
            elem(0, vec![("data-vox-overlay", "true")], vec![1, 2]),
            elem(
                1,
                vec![("data-vox-z", "100"), ("data-vox-pos", "top_right")],
                vec![],
            ),
            elem(
                2,
                vec![("data-vox-z", "90"), ("data-vox-pos", "top_right")],
                vec![],
            ),
        ]);
        let mut out = Vec::new();
        validate_overlay(&m, &mut out);
        assert!(
            out.iter()
                .any(|d| d.code == "web_ir_validate.overlay.position_conflict"),
            "expected position_conflict: {out:?}"
        );
    }

    #[test]
    fn non_overlay_element_not_checked() {
        let m = module_with_nodes(vec![
            elem(0, vec![], vec![1, 2]),
            elem(1, vec![("data-vox-z", "100")], vec![]),
            elem(2, vec![("data-vox-z", "100")], vec![]),
        ]);
        let mut out = Vec::new();
        validate_overlay(&m, &mut out);
        assert!(
            out.is_empty(),
            "non-overlay parent should not trigger check: {out:?}"
        );
    }
}
