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
            let attr_str = sorted
                .iter()
                .map(|(k, v)| format!("{k}={{{v}}}"))
                .collect::<Vec<_>>()
                .join(" ");
            let open = if attr_str.is_empty() {
                format!("<{tag}")
            } else {
                format!("<{tag} {attr_str}")
            };
            if children.is_empty() {
                return format!("{pad}{open} />\n");
            }
            let mut inner = String::new();
            for c in children {
                inner.push_str(&emit_node(module, *c, indent + 1, stats));
            }
            format!("{pad}{open}>\n{inner}{pad}</{tag}>\n")
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
