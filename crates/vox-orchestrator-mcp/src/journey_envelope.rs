//! Versioned journey metadata for structured transcripts (`conversation_messages` payload).
//!
//! SSOT: `contracts/orchestration/journey-envelope.v1.schema.json`.

use serde_json::{Value, json};

pub const JOURNEY_ENVELOPE_VERSION: u32 = 1;

/// Build a JSON object matching [`JOURNEY_ENVELOPE_VERSION`] schema (additional fields allowed by contract).
#[must_use]
pub fn build_journey_envelope_v1(
    journey_id: impl AsRef<str>,
    session_id: impl AsRef<str>,
    thread_id: Option<&str>,
    trace_id: Option<&str>,
    correlation_id: Option<&str>,
    repository_id: impl AsRef<str>,
    origin_surface: &str,
    cognitive_profile: Option<&str>,
) -> Value {
    let mut v = json!({
        "envelope_version": JOURNEY_ENVELOPE_VERSION,
        "journey_id": journey_id.as_ref(),
        "session_id": session_id.as_ref(),
        "thread_id": thread_id,
        "trace_id": trace_id,
        "correlation_id": correlation_id,
        "repository_id": repository_id.as_ref(),
        "origin_surface": origin_surface,
    });
    if let Some(o) = v.as_object_mut() {
        if let Some(cp) = cognitive_profile.map(str::trim).filter(|s| !s.is_empty()) {
            o.insert("cognitive_profile".into(), json!(cp));
        }
    }
    v
}
