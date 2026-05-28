//! Validator: every `Loop` node must carry a `key` attribute.
//!
//! Fires `validate.list_key.required` when a [`super::DomNode::Loop`] has no `key` clause.
//! This ensures React list renders always have a stable key prop, preventing silent
//! reorder/insert/delete corruption.

use super::{DomNode, WebIrDiagnostic, WebIrModule};

/// Check all [`DomNode::Loop`] nodes in the module for a missing `key` attribute.
///
/// Returns one diagnostic per keyless loop found.
pub fn validate_keys(module: &WebIrModule) -> Vec<WebIrDiagnostic> {
    let mut out = Vec::new();
    for node in &module.dom_nodes {
        if let DomNode::Loop { iterator, key, .. } = node
            && key.is_none()
        {
            out.push(WebIrDiagnostic {
                code: "validate.list_key.required".to_string(),
                message: format!(
                    "list render `for … in {iterator} {{ … }}` is missing a `key` clause. \
                         Add `for x in {iterator} key=x.id {{ … }}` for stable React identity."
                ),
                span: None,
                category: Some("dom".to_string()),
            });
        }
    }
    out
}
