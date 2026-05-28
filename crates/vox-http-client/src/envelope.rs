//! JSON error bodies for [Wire Format v1 SSOT](./../../../../docs/src/architecture/wire-format-v1-ssot.md) §6.
//!
//! Generated Axum apps depend on this crate so error shapes stay consistent across handlers.

use serde::Serialize;
use serde_json::Value;

/// Wire-format v1 error payload (`ok` is always `false`).
#[derive(Debug, Clone, Serialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

/// Build the §6 JSON object for `(StatusCode, Json<Value>)` Axum responses.
#[must_use]
pub fn error_json(
    code: impl Into<String>,
    message: impl Into<String>,
    request_id: Option<String>,
    details: Option<Value>,
) -> Value {
    serde_json::to_value(ErrorEnvelope {
        ok: false,
        code: code.into(),
        message: message.into(),
        request_id,
        details,
    })
    .expect("error envelope always serializes to JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_serializes_ok_false_and_code() {
        let v = error_json("BAD_REQUEST", "nope", Some("rid".into()), None);
        assert_eq!(v["ok"], false);
        assert_eq!(v["code"], "BAD_REQUEST");
        assert_eq!(v["message"], "nope");
        assert_eq!(v["request_id"], "rid");
    }
}
