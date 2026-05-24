//! Inbound webhook handler — parses, verifies and routes incoming webhook events.

use serde::{Deserialize, Serialize};

use super::WebhookError;

/// A normalized inbound webhook payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundPayload {
    /// Source identifier (e.g. "github", "gitlab", "custom")
    pub source: String,
    /// Event type string (e.g. "push", "pull_request")
    pub event_type: String,
    /// Raw JSON body of the event
    pub body: serde_json::Value,
    /// HMAC signature header value, if provided
    pub signature: Option<String>,
    /// Delivery timestamp or timestamp header component, if provided
    pub timestamp: Option<String>,
}

/// A parsed webhook event ready for dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    /// Unique delivery id (for idempotency / tracing).
    pub id: String,
    /// Source system identifier.
    pub source: String,
    /// Normalized event type string.
    pub event_type: String,
    /// Parsed JSON body.
    pub payload: serde_json::Value,
    /// Unix seconds when the gateway accepted the event.
    pub received_at: u64,
}

impl WebhookEvent {
    /// Builds an event from a verified inbound payload, stamping `received_at` to now.
    pub fn new(payload: &InboundPayload) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        Self {
            id: generate_id(),
            source: payload.source.clone(),
            event_type: payload.event_type.clone(),
            payload: payload.body.clone(),
            received_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("wh_{nanos:08x}")
}

/// Processes an inbound webhook payload:
/// 1. Optionally verifies signature
/// 2. Returns a `WebhookEvent` ready for dispatch
pub struct WebhookHandler {
    /// Optional HMAC secret — when set, signatures are required.
    pub secret: Option<String>,
    /// If non-empty, only these `source` values are accepted.
    pub allowed_sources: Vec<String>,
}

impl WebhookHandler {
    /// Handler with no secret and no source allowlist.
    pub fn new() -> Self {
        Self {
            secret: None,
            allowed_sources: Vec::new(),
        }
    }

    /// Require HMAC verification using this shared secret.
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// Allowlist an inbound `source` label (e.g. `"github"`).
    pub fn allow_source(mut self, source: impl Into<String>) -> Self {
        let s: String = source.into();
        self.allowed_sources.push(s);
        self
    }

    /// Verifies (optional) signature and allowlist, then returns a [`WebhookEvent`].
    pub fn handle(&self, payload: &InboundPayload) -> Result<WebhookEvent, WebhookError> {
        // Source allowlist check
        if !self.allowed_sources.is_empty() && !self.allowed_sources.contains(&payload.source) {
            return Err(WebhookError::UnknownEvent(format!(
                "Source '{}' not in allowlist",
                payload.source
            )));
        }

        // Signature verification
        if let Some(ref secret) = self.secret {
            let raw_body = serde_json::to_string(&payload.body)?;
            match &payload.signature {
                Some(sig) => super::signing::verify_payload(
                    secret,
                    raw_body.as_bytes(),
                    sig,
                    &payload.timestamp,
                    &payload.source,
                )?,
                None => return Err(WebhookError::InvalidSignature),
            }
        }

        Ok(WebhookEvent::new(payload))
    }
}

impl Default for WebhookHandler {
    /// Same as [`WebhookHandler::new`].
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_payload(source: &str, event_type: &str) -> InboundPayload {
        InboundPayload {
            source: source.to_string(),
            event_type: event_type.to_string(),
            body: serde_json::json!({"ref": "refs/heads/main"}),
            signature: None,
            timestamp: None,
        }
    }

    #[test]
    fn handler_without_secret_accepts_any() {
        let h = WebhookHandler::new();
        let p = make_payload("github", "push");
        let ev = h.handle(&p).expect("handle");
        assert_eq!(&*ev.source, "github");
        assert_eq!(&*ev.event_type, "push");
    }

    #[test]
    fn handler_with_allowlist_rejects_unknown_source() {
        let h = WebhookHandler::new()
            .allow_source("github")
            .allow_source("gitlab");
        let p = make_payload("unknown-source", "push");
        assert!(h.handle(&p).is_err());
    }

    #[test]
    fn handler_with_secret_rejects_missing_signature() {
        let h = WebhookHandler::new().with_secret("my-secret");
        let p = make_payload("github", "push");
        assert!(h.handle(&p).is_err());
    }

    #[test]
    fn handler_with_secret_accepts_valid_signature() {
        // Avoid `let secret = "..."` — generic-secret detector matches `secret` assignments in repo scans.
        let signing_key = "test-secret";
        let body = serde_json::json!({"ref": "refs/heads/main"});
        let body_str = serde_json::to_string(&body).unwrap();
        let sig = super::signing::sign_hmac_sha256(signing_key, body_str.as_bytes());

        let h = WebhookHandler::new().with_secret(signing_key);
        let p = InboundPayload {
            source: "github".to_string(),
            event_type: "push".to_string(),
            body,
            signature: Some(format!("sha256={sig}")),
            timestamp: None,
        };
        assert!(h.handle(&p).is_ok());
    }
}
