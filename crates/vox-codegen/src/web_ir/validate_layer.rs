//! GA-26 wiring: build the *surface tree* from the DOM arena and run the layered
//! layout discipline checks on real lowered `.vox` output.
//!
//! Surface-introducing nodes are the overlay-family primitives (`overlay`, `toast`,
//! `drawer`, `modal`) and any element carrying an explicit `data-vox-layer` attr.
//! Ordinary elements are *transparent*: they inherit their nearest surface ancestor
//! so `modal { row { text } }` stays legal. This transparency is why the check walks
//! the DOM arena directly rather than reusing `typeck::layer::check_tier_inversions`
//! (whose every-node model has no notion of transparent nodes).
//!
//! Codes (all errors, none advisory):
//! - `vox/layer/tier-inversion` — a stronger surface nested under a weaker one.
//! - `vox/layer/leaf-surface`   — a surface nested under a leaf surface (Toast/Popover).
//! - `vox/layer/absolute-in-partition` / `raw-z-index` / `raw-class-occlusion` (A10)
//!   — occlusion escape hatches used outside any surface parent.

use vox_compiler::hir::nodes::layer::LayerTier;

use super::{DomNode, DomNodeId, WebIrDiagnostic, WebIrModule};

/// Tags that introduce a layered surface (and thus a tier) into the view tree.
///
/// `overlay` is deliberately excluded: it is the transparent portal-host container
/// that legitimately *parents* surfaces (toast/modal/drawer), so it carries no tier
/// of its own.
fn is_surface_tag(tag: &str) -> bool {
    matches!(tag, "toast" | "drawer" | "modal")
}

pub fn validate_layer(module: &WebIrModule, out: &mut Vec<WebIrDiagnostic>) {
    for (_, root) in &module.view_roots {
        walk(module, *root, None, false, out);
    }
}

/// Walk the arena threading the nearest surface tier (`parent_surface`) and whether
/// any surface ancestor exists (`inside_surface`, for A10's escape-hatch scoping).
fn walk(
    module: &WebIrModule,
    id: DomNodeId,
    parent_surface: Option<LayerTier>,
    inside_surface: bool,
    out: &mut Vec<WebIrDiagnostic>,
) {
    let Some(node) = module.dom_nodes.get(id.0 as usize) else {
        return;
    };
    let DomNode::Element { tag, attrs, children, .. } = node else {
        return;
    };

    // Determine this node's tier: explicit data-vox-layer override, else surface tag.
    let explicit = attrs
        .iter()
        .find(|(k, _)| k == "data-vox-layer")
        .and_then(|(_, v)| LayerTier::from_str(v.trim_matches('"')));
    let this_surface: Option<LayerTier> = explicit.or_else(|| {
        if is_surface_tag(tag) {
            Some(LayerTier::default_for_primitive(tag))
        } else {
            None
        }
    });

    // If this node is itself a surface and has a surface parent, enforce the rules.
    if let Some(child_tier) = this_surface {
        if let Some(parent_tier) = parent_surface {
            if !parent_tier.may_parent_surfaces() {
                out.push(diag(
                    "vox/layer/leaf-surface",
                    format!(
                        "A `{}` ({}) is a leaf surface and cannot contain a `{}` ({}). \
                         Declare them as siblings instead — a leaf surface dismisses \
                         independently and would orphan a nested surface.",
                        parent_tier.as_str(),
                        parent_tier.as_str(),
                        tag,
                        child_tier.as_str()
                    ),
                ));
            } else if !parent_tier.allows_child(child_tier) {
                out.push(diag(
                    "vox/layer/tier-inversion",
                    format!(
                        "Tier inversion: `{}` ({}) cannot be rendered inside a `{}` surface ({}). \
                         A child surface's tier must be at most its parent's.",
                        tag,
                        child_tier.as_str(),
                        parent_tier.as_str(),
                        parent_tier.as_str()
                    ),
                ));
            }
        }
    }

    // A10: occlusion escape hatches are only legal inside a surface subtree.
    if !inside_surface && this_surface.is_none() {
        check_escape_hatches(tag, attrs, out);
    }

    let next_surface = this_surface.or(parent_surface);
    let next_inside = inside_surface || this_surface.is_some();
    for child in children {
        walk(module, *child, next_surface, next_inside, out);
    }
}

/// Flag absolute positioning, raw z-index, and occlusion-smuggling raw_class tokens
/// when they appear outside any surface parent (inside a partitioning layout).
fn check_escape_hatches(_tag: &str, attrs: &[(String, String)], out: &mut Vec<WebIrDiagnostic>) {
    for (k, v) in attrs {
        let val = v.trim_matches('"');
        match k.as_str() {
            "data-vox-pos-raw" => {
                if matches!(val, "absolute" | "fixed" | "sticky" | "inset") {
                    out.push(diag(
                        "vox/layer/absolute-in-partition",
                        format!(
                            "`position: {val}` is not allowed inside a partitioning layout — \
                             overlap requires a surface parent. Wrap this in overlay/modal/toast/drawer."
                        ),
                    ));
                }
            }
            "data-vox-z-raw" => {
                out.push(diag(
                    "vox/layer/raw-z-index",
                    format!(
                        "Raw z-index `{val}` is not allowed — z is a closed tier enum \
                         (background, content, chrome, popover, modal, toast, system_overlay)."
                    ),
                ));
            }
            "data-vox-raw-class" => {
                if let Some(bad) = val.split_whitespace().find(|t| {
                    matches!(*t, "absolute" | "fixed" | "sticky")
                        || t.starts_with("z-")
                        || t.starts_with("-m")
                }) {
                    out.push(diag(
                        "vox/layer/raw-class-occlusion",
                        format!(
                            "raw_class token `{bad}` smuggles occlusion (absolute/fixed/z-index/negative-margin) \
                             into a partitioning layout. Use a surface parent or typed tier kwargs."
                        ),
                    ));
                }
            }
            _ => {}
        }
    }
}

fn diag(code: &str, message: String) -> WebIrDiagnostic {
    WebIrDiagnostic {
        code: code.to_string(),
        message,
        span: None,
        category: Some("layer".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_ir::{DomNode, DomNodeId, WebIrModule};

    /// `nested(&["a","b","c"])` builds a single spine a > b > c.
    fn nested(tags: &[&str]) -> WebIrModule {
        let mut m = WebIrModule::default();
        for (i, tag) in tags.iter().enumerate() {
            let children = if i + 1 < tags.len() {
                vec![DomNodeId(i as u32 + 1)]
            } else {
                vec![]
            };
            m.dom_nodes.push(DomNode::Element {
                id: DomNodeId(i as u32),
                tag: tag.to_string(),
                attrs: vec![],
                children,
                span: None,
            });
        }
        m.view_roots.push(("V".to_string(), DomNodeId(0)));
        m
    }

    fn codes(m: &WebIrModule) -> Vec<String> {
        let mut out = vec![];
        validate_layer(m, &mut out);
        out.into_iter().map(|d| d.code).collect()
    }

    /// Build a single spine where node at `layer_idx` carries an explicit
    /// `data-vox-layer` tier attr.
    fn nested_with_layer(tags: &[&str], layer_idx: usize, tier: &str) -> WebIrModule {
        let mut m = nested(tags);
        if let DomNode::Element { attrs, .. } = &mut m.dom_nodes[layer_idx] {
            attrs.push(("data-vox-layer".to_string(), format!("\"{tier}\"")));
        }
        m
    }

    #[test]
    fn modal_inside_a_chrome_surface_is_tier_inversion() {
        // A chrome-tier surface (app shell) may parent surfaces, but a Modal(4) is
        // stronger than Chrome(2) → must portal up, not nest.
        let c = codes(&nested_with_layer(&["column", "section", "modal"], 1, "chrome"));
        assert!(c.iter().any(|c| c == "vox/layer/tier-inversion"), "{c:?}");
    }

    #[test]
    fn modal_inside_overlay_host_is_valid() {
        // overlay is the transparent portal host — hosting a modal is its whole job.
        let c = codes(&nested(&["column", "overlay", "modal"]));
        assert!(c.is_empty(), "{c:?}");
    }

    #[test]
    fn modal_inside_toast_is_rejected_as_leaf_surface_violation() {
        let c = codes(&nested(&["column", "toast", "modal"]));
        assert!(c.iter().any(|c| c == "vox/layer/leaf-surface"), "{c:?}");
    }

    #[test]
    fn modal_with_ordinary_content_is_fine() {
        let c = codes(&nested(&["column", "modal", "row", "text"]));
        assert!(c.is_empty(), "{c:?}");
    }

    #[test]
    fn toast_inside_modal_is_a_tier_inversion() {
        // toast(5) under modal(4): 5 > 4 → inversion (modal may parent surfaces).
        let c = codes(&nested(&["modal", "toast"]));
        assert!(c.iter().any(|c| c == "vox/layer/tier-inversion"), "{c:?}");
    }

    #[test]
    fn plain_partition_tree_is_clean() {
        let c = codes(&nested(&["column", "row", "panel", "text"]));
        assert!(c.is_empty(), "{c:?}");
    }

    // ── A10: escape hatches ─────────────────────────────────────────────────

    fn with_attr(tags: &[&str], leaf_attr: (&str, &str)) -> WebIrModule {
        let mut m = nested(tags);
        let last = (tags.len() - 1) as u32;
        if let DomNode::Element { attrs, .. } = &mut m.dom_nodes[last as usize] {
            attrs.push((leaf_attr.0.to_string(), format!("\"{}\"", leaf_attr.1)));
        }
        m
    }

    #[test]
    fn absolute_position_inside_partition_is_rejected() {
        let c = codes(&with_attr(&["row", "panel"], ("data-vox-pos-raw", "absolute")));
        assert!(c.iter().any(|c| c == "vox/layer/absolute-in-partition"), "{c:?}");
    }

    #[test]
    fn raw_z_index_outside_surface_is_rejected() {
        let c = codes(&with_attr(&["column", "panel"], ("data-vox-z-raw", "999")));
        assert!(c.iter().any(|c| c == "vox/layer/raw-z-index"), "{c:?}");
    }

    #[test]
    fn raw_class_smuggling_absolute_is_rejected() {
        let c = codes(&with_attr(
            &["row", "panel"],
            ("data-vox-raw-class", "shrink-0 absolute z-[9999]"),
        ));
        assert!(c.iter().any(|c| c == "vox/layer/raw-class-occlusion"), "{c:?}");
    }

    #[test]
    fn absolute_inside_a_surface_is_allowed() {
        // Inside a modal subtree, absolute positioning is the surface's own business.
        let c = codes(&with_attr(&["modal", "panel"], ("data-vox-pos-raw", "absolute")));
        assert!(!c.iter().any(|c| c == "vox/layer/absolute-in-partition"), "{c:?}");
    }
}
