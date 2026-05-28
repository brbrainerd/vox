//! # webhook — HTTP Webhook Gateway (inlined from vox-webhook)
//!
//! Provides an inbound webhook receiver, outbound delivery with retry/signing,
//! and a `Channel` abstraction for Discord/Slack/WebSocket integrations.

/// Bridge: routes broadcast webhook events into a WebhookEventSink.
pub mod bridge;
/// Channel adapters (Discord, Slack, …).
pub mod channel;
/// Outbound webhook delivery and retries.
pub mod delivery;
/// Inbound payload types and handler trait.
pub mod handler;
/// Axum router wiring and server entry.
pub mod router;
/// HMAC / digest signing helpers.
pub mod signing;
/// Abstract event sink trait — decouples the library from any concrete consumer.
pub mod sink;

pub use bridge::{InboxItemKind, OrchestratorInboxItem, WebhookOrchestratorBridge};
pub use sink::WebhookEventSink;

pub use channel::{Channel, ChannelEvent, ChannelKind, ChannelManager};
pub use delivery::{OutboundWebhook, WebhookDelivery, WebhookDeliveryResult};
pub use handler::{InboundPayload, WebhookEvent, WebhookHandler};
pub use router::{WebhookState, build_router, serve};
pub use signing::{WebhookSignature, sign_payload, verify_payload};

/// Errors from the webhook system.
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    /// HMAC or signature header mismatch.
    #[error("Signature verification failed")]
    InvalidSignature,
    /// Timestamp missing on a source that requires it (e.g. Slack, Discord).
    #[error("Missing or empty timestamp header")]
    MissingTimestamp,
    /// Timestamp is outside the configured replay window.
    #[error("Timestamp {0} is outside the allowed replay window")]
    TimestampOutOfWindow(String),
    /// Source or event rejected by policy.
    #[error("Unknown event type: {0}")]
    UnknownEvent(String),
    /// Outbound POST failed after retries.
    #[error("Delivery failed: {0}")]
    DeliveryFailed(String),
    /// Channel registry / adapter failure.
    #[error("Channel error: {0}")]
    Channel(String),
    /// Local I/O (bind, disk, etc.).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON encode/decode failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// HTTP client / status error string.
    #[error("HTTP error: {0}")]
    Http(String),
}
