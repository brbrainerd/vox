//! Inbound webhook handler — parses, verifies and routes incoming webhook events.

use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::WebhookError;

/// A normalized inbound webhook payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundPayload {
    /// Source identifier (e.g. "github", "gitlab", "custom")
    pub source: Arc<str>,
    /// Event type string (e.g. "push", "pull_request")
    pub event_type: Arc<str>,
    /// Raw JSON body of the event
    pub body: serde_json::Value,
    /// HMAC signature header value, if provided
    pub signature: Option<String>,
    /// Delivery timestamp (unix seconds)
    pub timestamp: u64,
}

/// A parsed webhook event ready for dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub id: Arc<str>,
    pub source: Arc<str>,
    pub event_type: Arc<str>,
    pub payload: serde_json::Value,
    pub received_at: u64,
}

impl WebhookEvent {
    pub fn new(payload: &InboundPayload) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        Self {
            id: Arc::from(generate_id().as_str()),
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
    pub secret: Option<String>,
    pub allowed_sources: Vec<Arc<str>>,
}

impl WebhookHandler {
    pub fn new() -> Self {
        Self {
            secret: None,
            allowed_sources: Vec::new(),
        }
    }

    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    pub fn allow_source(mut self, source: impl Into<String>) -> Self {
        let s: String = source.into();
        self.allowed_sources.push(Arc::from(s.as_str()));
        self
    }

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
                Some(sig) => crate::signing::verify_payload(secret, raw_body.as_bytes(), sig)?,
                None => return Err(WebhookError::InvalidSignature),
            }
        }

        Ok(WebhookEvent::new(payload))
    }
}

impl Default for WebhookHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_payload(source: &str, event_type: &str) -> InboundPayload {
        InboundPayload {
            source: Arc::from(source),
            event_type: Arc::from(event_type),
            body: serde_json::json!({"ref": "refs/heads/main"}),
            signature: None,
            timestamp: 0,
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
        let secret = "test-secret";
        let body = serde_json::json!({"ref": "refs/heads/main"});
        let body_str = serde_json::to_string(&body).unwrap();
        let sig = crate::signing::sign_payload(secret, body_str.as_bytes());

        let h = WebhookHandler::new().with_secret(secret);
        let p = InboundPayload {
            source: Arc::from("github"),
            event_type: Arc::from("push"),
            body,
            signature: Some(sig.to_string()),
            timestamp: 0,
        };
        assert!(h.handle(&p).is_ok());
    }
}
