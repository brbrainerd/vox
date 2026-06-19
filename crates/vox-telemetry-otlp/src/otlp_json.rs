use crate::redact::RedactedRecord;
use serde_json::{Value, json};

/// Encode a `RedactedRecord` into an OTLP/HTTP logs JSON envelope.
///
/// The envelope is a minimal `resourceLogs` wrapper compatible with the OTLP/HTTP
/// logs JSON framing (`Content-Type: application/json`). No `opentelemetry` SDK
/// is used — we hand-encode the stable format with `serde_json` to avoid pulling
/// the `logs` feature into the workspace-wide 0.29 otel pin.
///
/// `install_id` is added to `resource.attributes` so the server can group events
/// by installation (k-anonymity enforced server-side; the install_id itself is
/// opaque and not user-identifiable).
pub fn to_otlp_log(rec: &RedactedRecord, install_id: &str) -> Value {
    // Build attribute key-value pairs from the redacted record.
    let attrs: Vec<Value> = rec
        .attrs
        .iter()
        .map(|(k, v)| {
            json!({
                "key": k,
                "value": scalar_to_any_value(v),
            })
        })
        .collect();

    json!({
        "resourceLogs": [{
            "resource": {
                "attributes": [
                    { "key": "vox.install_id", "value": { "stringValue": install_id } }
                ]
            },
            "scopeLogs": [{
                "scope": { "name": "vox-telemetry-otlp" },
                "logRecords": [{
                    "body": { "stringValue": rec.event_name },
                    "attributes": attrs,
                    "severityText": "INFO",
                    "severityNumber": 9
                }]
            }]
        }]
    })
}

fn scalar_to_any_value(v: &Value) -> Value {
    match v {
        Value::String(s) => json!({ "stringValue": s }),
        Value::Bool(b) => json!({ "boolValue": b }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                json!({ "intValue": i.to_string() })
            } else if let Some(f) = n.as_f64() {
                json!({ "doubleValue": f })
            } else {
                json!({ "stringValue": n.to_string() })
            }
        }
        // Arrays and objects must not reach here (filtered by redact_event).
        _ => json!({ "stringValue": v.to_string() }),
    }
}
