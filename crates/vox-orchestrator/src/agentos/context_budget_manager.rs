//! Prune oversized structured evidence arrays for AgentOS context envelopes.

use serde_json::{Value, json};

/// Keeps the first `max_items` elements of any JSON array found under `evidence["items"]`,
/// or truncates top-level array payloads.
pub fn prune_evidence_value(mut evidence: Value, max_items: usize) -> Value {
    let cap = max_items.max(1);
    match &mut evidence {
        Value::Array(arr) => {
            if arr.len() > cap {
                arr.truncate(cap);
            }
            evidence
        }
        Value::Object(map) => {
            if let Some(Value::Array(items)) = map.get_mut("items") {
                if items.len() > cap {
                    items.truncate(cap);
                }
            }
            evidence
        }
        _ => evidence,
    }
}

/// Builds a minimal summary object when pruning non-array evidence.
pub fn summarize_evidence(evidence: &Value, max_items: usize) -> Value {
    json!({
        "pruned": true,
        "max_items": max_items,
        "kind": evidence.as_object().map(|_| "object").unwrap_or("other"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn truncates_items_array() {
        let v = json!({ "items": [1, 2, 3, 4] });
        let out = prune_evidence_value(v, 2);
        assert_eq!(out["items"].as_array().unwrap().len(), 2);
    }
}
