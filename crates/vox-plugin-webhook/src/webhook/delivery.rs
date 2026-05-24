//! Outbound webhook delivery with retry, backoff, and delivery receipts.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// An outbound webhook configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundWebhook {
    /// Target URL
    pub url: String,
    /// Optional HMAC secret for signing outbound payloads
    #[serde(skip_serializing)]
    pub secret: Option<String>,
    /// Maximum delivery attempts
    pub max_retries: u32,
    /// Base backoff in milliseconds
    pub backoff_ms: u64,
    /// Custom headers
    pub headers: Vec<(String, String)>,
}

impl OutboundWebhook {
    /// Default retry policy: 3 attempts, 500ms base backoff.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            secret: None,
            max_retries: 3,
            backoff_ms: 500,
            headers: Vec::new(),
        }
    }

    /// Signs outbound bodies with [`super::signing::sign_payload`].
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// Adds an extra header on every retry attempt (e.g. `Authorization`, `X-Custom-Auth`).
    ///
    /// Duplicate keys are allowed; the HTTP client sends each pair in order. Prefer this over
    /// mutating [`OutboundWebhook::headers`] after construction.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }
}

/// The result of a webhook delivery attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDeliveryResult {
    /// Target URL attempted.
    pub url: String,
    /// Whether any attempt returned HTTP 2xx.
    pub success: bool,
    /// Last observed HTTP status, if a response was received.
    pub status_code: Option<u16>,
    /// Number of attempts performed (≤ configured max).
    pub attempts: u32,
    /// Transport or HTTP error summary on failure.
    pub error: Option<String>,
}

/// Delivers payloads to outbound webhook endpoints with retry logic.
pub struct WebhookDelivery {
    client: reqwest::Client,
}

impl WebhookDelivery {
    /// Builds a delivery service using a shared `reqwest` client (10s timeout, no cookies).
    ///
    /// Reuse one instance across tasks: the client pools connections and is cheap to clone internally.
    pub fn new() -> Self {
        Self {
            client: vox_http_client::client_builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| vox_http_client::client()),
        }
    }

    /// Deliver a JSON payload to an outbound webhook with retry/backoff.
    pub async fn deliver(
        &self,
        webhook: &OutboundWebhook,
        payload: &serde_json::Value,
    ) -> WebhookDeliveryResult {
        let body = serde_json::to_string(payload).unwrap_or_default();
        let mut last_error = None;
        let mut last_status = None;

        for attempt in 1..=webhook.max_retries {
            let mut req = self
                .client
                .post(&webhook.url)
                .header("Content-Type", "application/json")
                .body(body.clone());

            // Add custom headers
            for (k, v) in &webhook.headers {
                req = req.header(k, v);
            }

            // Sign if secret configured
            if let Some(ref secret) = webhook.secret {
                let sig = super::signing::sign_payload(secret, body.as_bytes());
                req = req.header("X-Vox-Signature", sig.to_string());
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    last_status = Some(status);
                    if resp.status().is_success() {
                        info!(url = %webhook.url, attempt, "Webhook delivered");
                        return WebhookDeliveryResult {
                            url: webhook.url.clone(),
                            success: true,
                            status_code: Some(status),
                            attempts: attempt,
                            error: None,
                        };
                    }
                    last_error = Some(format!("HTTP {status}"));
                    warn!(url = %webhook.url, attempt, status, "Webhook delivery failed, retrying");
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    warn!(url = %webhook.url, attempt, "Webhook request error: {e}");
                }
            }

            // Exponential backoff
            if attempt < webhook.max_retries {
                tokio::time::sleep(std::time::Duration::from_millis(
                    webhook.backoff_ms * (2u64.pow(attempt - 1)),
                ))
                .await;
            }
        }

        WebhookDeliveryResult {
            url: webhook.url.clone(),
            success: false,
            status_code: last_status,
            attempts: webhook.max_retries,
            error: last_error,
        }
    }
}

impl Default for WebhookDelivery {
    /// Same as [`WebhookDelivery::new`].
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbound_webhook_defaults() {
        let wh = OutboundWebhook::new("https://example.com/hook");
        assert_eq!(wh.max_retries, 3);
        assert_eq!(wh.backoff_ms, 500);
        assert!(wh.secret.is_none());
    }

    #[test]
    fn outbound_webhook_builder() {
        let wh = OutboundWebhook::new("https://example.com/hook")
            .with_secret("s3cr3t")
            .with_header("X-Custom", "value");
        assert_eq!(wh.secret.as_deref(), Some("s3cr3t"));
        assert_eq!(wh.headers.len(), 1);
    }
}
