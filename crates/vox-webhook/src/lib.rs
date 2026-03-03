//! # vox-webhook — HTTP Webhook Gateway
#![allow(missing_docs)]
#![allow(unused)]

//! Provides an inbound webhook receiver, outbound delivery with retry/signing,
//! and a `Channel` abstraction for Discord/Slack/WebSocket integrations.

pub mod channel;
pub mod delivery;
pub mod handler;
pub mod router;
pub mod signing;

pub use channel::{Channel, ChannelEvent, ChannelKind, ChannelManager};
pub use delivery::{OutboundWebhook, WebhookDelivery, WebhookDeliveryResult};
pub use handler::{InboundPayload, WebhookEvent, WebhookHandler};
pub use router::{build_router, serve, WebhookState};
pub use signing::{sign_payload, verify_payload, WebhookSignature};

/// Errors from the webhook system.
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("Signature verification failed")]
    InvalidSignature,
    #[error("Unknown event type: {0}")]
    UnknownEvent(String),
    #[error("Delivery failed: {0}")]
    DeliveryFailed(String),
    #[error("Channel error: {0}")]
    Channel(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HTTP error: {0}")]
    Http(String),
}
