//! WebIR → TSX string preview (ADR Phase 1).
//!
//! ## Preview vs production (OP-0097)
//! This path is **diagnostic and parity-only**: deterministic JSX-shaped text for tests, diff tools, and
//! future `ReactTanStackEmitter` prototyping. **Production** apps still ship through
//! [`crate::codegen_ts::emitter::generate`]. Treat API stability as *internal* until the ADR 012 bridge
//! promotes this emitter.
//!
//! ## Deterministic preview emit (OP-S021 / OP-S022)
//! Child order follows stored [`DomNode`] edges only; [`DomNode::Element`] attributes are sorted
//! lexicographically by key before stringification so repeated emits / JSON round-trips match byte-for-byte
//! when inputs match (see `web_ir_lower_emit` preview tests).
//!
//! ## Legacy attribute rules (OP-0098, OP-0108, OP-S023)
//! Attribute names in [`crate::web_ir::DomNode::Element`] are already **React-oriented** (`className`,
//! `onClick`) — they must match the same matrix as [`crate::codegen_ts::hir_emit::map_jsx_attr_name`].
//! The preview emitter treats the lowered `(name, value)` list as an unordered map edge: **never** rely on
//! source insertion order in TSX snapshots — only on the sort step below.
//! Tag names in `DomNode::Element` are likewise pre-lowered to React-form
//! camelCase (e.g. `radialGradient`, `clipPath`) by `web_ir/lower.rs`;
//! callers must not re-apply `map_jsx_tag` here.
//!
//! ## Escape hatches (OP-0106)
//! [`crate::web_ir::DomNode::Expr`] prints raw TypeScript fragments from lowering; do not feed user
//! text here without upstream policy (validator / sanitizer).

use crate::web_ir::{DomNode, DomNodeId, WebIrModule};

/// Counts nodes visited while emitting a view (OP-0104, parity dashboards).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebIrTsxEmitStats {
    pub nodes_visited: usize,
}

/// Emit JSX for a reactive component `view:` root, if present in [`WebIrModule::view_roots`].
#[must_use]
pub fn emit_component_view_tsx(module: &WebIrModule, component_name: &str) -> Option<String> {
    emit_component_view_tsx_with_stats(module, component_name).map(|(s, _)| s)
}

/// Like [`emit_component_view_tsx`] but returns visit counts for gates / snapshots.
#[must_use]
pub fn emit_component_view_tsx_with_stats(
    module: &WebIrModule,
    component_name: &str,
) -> Option<(String, WebIrTsxEmitStats)> {
    let root_id = module
        .view_roots
        .iter()
        .find(|(n, _)| n == component_name)
        .map(|(_, id)| *id)?;
    let mut stats = WebIrTsxEmitStats::default();
    let s = emit_node(module, root_id, 0, &mut stats);
    Some((s, stats))
}

/// Shadow attrs mirrored onto the DOM arena solely for the validators (A3 palette /
/// A4 contrast / A10 occlusion-escape). They carry no runtime meaning and must be
/// stripped before emit. The runtime-meaningful `data-vox-*` attrs (`surface`,
/// `layer`, `overlay`, `z`, `pos`) are deliberately NOT listed here.
fn is_analysis_only_attr(name: &str) -> bool {
    matches!(
        name,
        "data-vox-color"
            | "data-vox-bg"
            | "data-vox-border-color"
            | "data-vox-pos-raw"
            | "data-vox-z-raw"
            | "data-vox-raw-class"
            | "data-vox-unknown-kwarg"
    )
}

fn emit_node(
    module: &WebIrModule,
    id: DomNodeId,
    indent: usize,
    stats: &mut WebIrTsxEmitStats,
) -> String {
    stats.nodes_visited += 1;
    let Some(node) = module.dom_nodes.get(id.0 as usize) else {
        return String::new();
    };
    let pad = "  ".repeat(indent);
    match node {
        DomNode::Element {
            tag,
            attrs,
            children,
            ..
        } => {
            // Deterministic ordering for snapshot parity (OP-0102, OP-S023): `attrs` is semantic map, not ordered list.
            let mut sorted = attrs.to_vec();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            // Analysis-only shadow attrs are mirrored onto the DOM arena purely so the
            // validators (palette/contrast, layer/occlusion) can see the author's
            // intent. They must NOT ship to the rendered DOM. Runtime-meaningful
            // `data-vox-*` attrs (surface vars, layer/overlay portal hooks) are kept.
            let attr_str = sorted
                .iter()
                .filter(|(k, _)| !is_analysis_only_attr(k))
                .map(|(k, v)| format!("{k}={{{v}}}"))
                .collect::<Vec<_>>()
                .join(" ");
            let open = if attr_str.is_empty() {
                format!("<{tag}")
            } else {
                format!("<{tag} {attr_str}")
            };
            // A11 part 2: an element declaring a Z-tier (`data-vox-layer`) renders
            // through its portal root so it escapes any transformed/filtered ancestor
            // stacking context (the bug the CSS ladder alone cannot fix). The guard
            // makes it SSR-safe: `voxResolveLayerRoot` returns null on the server, so
            // the `&&` short-circuits to nothing and the overlay hydrates client-side.
            let portal_tier = sorted
                .iter()
                .find(|(k, _)| k == "data-vox-layer")
                .map(|(_, v)| v.clone());

            let element = if children.is_empty() {
                format!("{pad}{open} />\n")
            } else {
                let mut inner = String::new();
                for c in children {
                    inner.push_str(&emit_node(module, *c, indent + 1, stats));
                }
                format!("{pad}{open}>\n{inner}{pad}</{tag}>\n")
            };

            match portal_tier {
                Some(tier) => format!(
                    "{pad}{{voxResolveLayerRoot({tier}) && createPortal(\n{element}{pad}, voxResolveLayerRoot({tier})!)}}\n"
                ),
                None => element,
            }
        }
        DomNode::Text { content, .. } => {
            let lit = serde_json::to_string(content).unwrap_or_else(|_| "\"\"".into());
            format!("{pad}{{{lit}}}\n")
        }
        DomNode::Fragment { children, .. } => {
            let mut inner = String::new();
            for c in children {
                inner.push_str(&emit_node(module, *c, indent, stats));
            }
            format!("{pad}<>\n{inner}{pad}</>\n")
        }
        DomNode::Slot { .. } => format!("{pad}{{/* slot */ null}}\n"),
        DomNode::Conditional {
            predicate,
            then_children,
            else_children,
            ..
        } => {
            let then_s: String = then_children
                .iter()
                .map(|c| emit_node(module, *c, indent + 1, stats))
                .collect();
            let else_s: String = else_children
                .iter()
                .map(|c| emit_node(module, *c, indent + 1, stats))
                .collect();
            format!("{pad}{{({predicate}) ? (\n{then_s}{pad}) : (\n{else_s}{pad})}}\n")
        }
        DomNode::Loop {
            iterator,
            key,
            body,
            ..
        } => {
            let body_s: String = body
                .iter()
                .map(|c| emit_node(module, *c, indent + 1, stats))
                .collect();
            // Inject the key attribute into the first JSX element in body if present.
            let body_with_key = if let Some(k) = key {
                let key_attr = format!(" key={{{k}}}");
                inject_key_into_jsx(body_s, &key_attr)
            } else {
                body_s
            };
            format!("{pad}{{{iterator}.map(() => (\n{body_with_key}{pad}))}}\n")
        }
        DomNode::Expr { ts, .. } => format!("{pad}{{{ts}}}\n"),
    }
}

/// Inject a `key` attribute string into the first JSX element opening tag.
///
/// Looks for the first `<` and inserts `key_attr` before the first `>` or `/>`
/// of that tag. Falls back to returning the original string if no suitable
/// insertion point is found.
fn inject_key_into_jsx(jsx: String, key_attr: &str) -> String {
    if let Some(lt_pos) = jsx.find('<') {
        let after_lt = &jsx[lt_pos..];
        if let Some(rel_end) = after_lt.find(['>', '/']) {
            let insert_at = lt_pos + rel_end;
            return format!("{}{}{}", &jsx[..insert_at], key_attr, &jsx[insert_at..]);
        }
    }
    jsx
}

#[cfg(test)]
mod a11p2_portal_tests {
    use crate::web_ir::{DomNode, DomNodeId, WebIrModule};

    fn module_with_layer_element() -> WebIrModule {
        let mut m = WebIrModule::default();
        // child text
        m.dom_nodes.push(DomNode::Element {
            id: DomNodeId(0),
            tag: "div".to_string(),
            attrs: vec![
                ("data-vox-layer".to_string(), "\"modal\"".to_string()),
                ("data-vox-bg".to_string(), "\"white\"".to_string()),
                ("className".to_string(), "\"fixed inset-0\"".to_string()),
            ],
            children: vec![],
            span: None,
        });
        m.view_roots.push(("Dialog".to_string(), DomNodeId(0)));
        m
    }

    #[test]
    fn layer_element_renders_through_create_portal() {
        let m = module_with_layer_element();
        let tsx = super::emit_component_view_tsx(&m, "Dialog").expect("emits");
        assert!(tsx.contains("createPortal("), "expected createPortal; got:\n{tsx}");
        assert!(tsx.contains("voxResolveLayerRoot(\"modal\")"), "expected resolver; got:\n{tsx}");
        // data-vox-layer is kept (CSS hook); the analysis-only data-vox-bg is stripped.
        assert!(tsx.contains("data-vox-layer"), "data-vox-layer kept; got:\n{tsx}");
        assert!(!tsx.contains("data-vox-bg"), "analysis attr stripped; got:\n{tsx}");
    }
}
