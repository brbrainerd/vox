//! Abstract event sink trait — decouples the webhook library from any concrete consumer.
//!
//! The orchestrator (or any other runtime) implements [`WebhookEventSink`] and
//! passes an `Arc<dyn WebhookEventSink>` to the bridge, avoiding a hard
//! dependency on `vox-orchestrator`.

use anyhow::Result;
use async_trait::async_trait;

use super::handler::WebhookEvent;

/// Abstract surface that a webhook bridge dispatches validated events to.
///
/// Implement this trait on your consumer (e.g. `Orchestrator`) and pass an
/// `Arc<dyn WebhookEventSink>` to the bridge constructor in `super::bridge`.
#[async_trait]
pub trait WebhookEventSink: Send + Sync {
    /// Dispatch a validated webhook event.
    ///
    /// The implementation is responsible for further routing (queueing,
    /// persistence, task submission, etc.).
    async fn dispatch(&self, event: WebhookEvent) -> Result<()>;
}
