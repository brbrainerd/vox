use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

static TAXONOMY: &str = include_str!("../../../contracts/telemetry/collection-taxonomy.v1.json");

/// A single redacted telemetry observation ready for OTLP encoding.
#[derive(Debug, Serialize)]
pub struct RedactedRecord {
    pub event_name: String,
    pub attrs: serde_json::Map<String, Value>,
}

fn allowlist() -> &'static HashMap<String, (String, HashSet<String>)> {
    static CELL: OnceLock<HashMap<String, (String, HashSet<String>)>> = OnceLock::new();
    CELL.get_or_init(|| {
        // NEVER panic on the telemetry hot path (spec §3.6). On any parse problem,
        // degrade to an EMPTY allowlist → redact_event returns None → nothing uploads.
        let mut m = HashMap::new();
        let Ok(v) = serde_json::from_str::<Value>(TAXONOMY) else {
            return m;
        };
        let Some(cats) = v["categories"].as_array() else {
            return m;
        };
        for cat in cats {
            let (Some(name), Some(ev), Some(fields)) = (
                cat["name"].as_str(),
                cat["otlp_event_name"].as_str(),
                cat["fields"].as_array(),
            ) else {
                continue;
            };
            let set = fields
                .iter()
                .filter_map(|f| f["name"].as_str().map(str::to_string))
                .collect();
            m.insert(name.to_string(), (ev.to_string(), set));
        }
        m
    })
}

/// Second-layer guard: keep ONLY taxonomy-allowlisted scalar fields for `category`.
///
/// Returns `None` for unknown categories (fail-closed). Drops object/array values
/// (they are free-form nested structures, prohibited by spec §3.2).
pub fn redact_event(
    category: &str,
    flat: &serde_json::Map<String, Value>,
) -> Option<RedactedRecord> {
    let (event_name, allowed) = allowlist().get(category)?;
    let mut attrs = serde_json::Map::new();
    for (k, val) in flat {
        if allowed.contains(k) && !val.is_object() && !val.is_array() {
            attrs.insert(k.clone(), val.clone());
        }
    }
    Some(RedactedRecord {
        event_name: event_name.clone(),
        attrs,
    })
}
