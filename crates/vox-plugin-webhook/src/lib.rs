//! # vox-plugin-webhook
//!
//! Plugin entry point for the Vox webhook HTTP listener gateway.
//!
//! On `init()` the plugin spawns a Tokio task that runs the Axum webhook
//! server on the address configured by the `VOX_WEBHOOK_ADDR` environment
//! variable (default: `0.0.0.0:9080`).
//!
//! ## Event routing
//!
//! The plugin uses a no-op `WebhookEventSink` by default. For production use,
//! the host should wire an `Arc<dyn WebhookEventSink>` backed by the Orchestrator
//! (see `WebhookOrchestratorBridge` in `webhook::bridge`). The orchestrator-side
//! wiring is deferred — tracked as Step 8 of the extraction plan.
//!
//! ## Plugin trait
//!
//! Implements `VoxPlugin` (id + shutdown). The HTTP server is a long-running
//! background tokio task started from `init()`. There is no dedicated
//! "start-service" lifecycle hook in ABI v11 — this matches the pattern used
//! by other long-running plugins (e.g. vox-plugin-cloud).

// Public types are designed for orchestrator wiring (Step 8). Suppress dead-code
// lint until the bridge is wired — these are real implementations, not stubs.
#![allow(dead_code, unused_imports)]

mod webhook;

use abi_stable::{
    erased_types::TD_Opaque, export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn,
    std_types::*,
};
use anyhow::Result;
use async_trait::async_trait;
use tracing::{info, warn};
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
use vox_plugin_api::host::VoxHost_TO;
use webhook::{
    WebhookEvent, WebhookEventSink, WebhookHandler,
    router::{WebhookState, serve},
};

// ---------------------------------------------------------------------------
// ABI root module
// ---------------------------------------------------------------------------

#[export_root_module]
fn root_module() -> VoxPluginRootRef {
    VoxPluginRoot {
        abi_version: VOX_PLUGIN_ABI_VERSION,
        manifest_json,
        init,
    }
    .leak_into_prefix()
}

#[sabi_extern_fn]
fn manifest_json() -> RString {
    RString::from(r#"{"id":"webhook","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    // Start the HTTP listener on a background tokio task.
    //
    // NOTE: this relies on a tokio runtime already being active in the host
    // process, which is guaranteed by the vox-plugin-host bootstrap.
    let addr = std::env::var("VOX_WEBHOOK_ADDR").unwrap_or_else(|_| "0.0.0.0:9080".to_string());
    let ingress_token = std::env::var("VOX_WEBHOOK_INGRESS_TOKEN").ok();

    let mut state = WebhookState::new(WebhookHandler::new());
    if let Some(token) = ingress_token {
        state = state.with_ingress_token(token);
    } else {
        warn!(
            "vox-plugin-webhook: VOX_WEBHOOK_INGRESS_TOKEN not set — running in degraded (no-auth) mode"
        );
    }

    // Spawn the HTTP server. The broadcast channel inside WebhookState will
    // accumulate events; wire WebhookOrchestratorBridge to consume them.
    let addr_clone = addr.clone();
    tokio::spawn(async move {
        info!(addr = %addr_clone, "vox-plugin-webhook: starting HTTP listener");
        if let Err(e) = serve(state, &addr_clone).await {
            tracing::error!("vox-plugin-webhook: server error: {e}");
        }
    });

    let plugin = WebhookPlugin;
    let to = VoxPlugin_TO::from_value(plugin, TD_Opaque);
    RResult::ROk(to)
}

// ---------------------------------------------------------------------------
// Plugin impl
// ---------------------------------------------------------------------------

struct WebhookPlugin;

impl VoxPlugin for WebhookPlugin {
    fn id(&self) -> RString {
        RString::from("webhook")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        // The tokio task will be dropped when the runtime shuts down.
        // No explicit handle is stored (acceptable for the current ABI surface).
        RResult::ROk(())
    }
}

// ---------------------------------------------------------------------------
// No-op sink (placeholder until orchestrator wiring is complete)
// ---------------------------------------------------------------------------

/// A no-op `WebhookEventSink` that logs received events and discards them.
///
/// Replace with an `OrchestratorWebhookSink` impl in vox-orchestrator once
/// Step 8 of the extraction plan is implemented.
pub struct LoggingWebhookSink;

#[async_trait]
impl WebhookEventSink for LoggingWebhookSink {
    async fn dispatch(&self, event: WebhookEvent) -> Result<()> {
        tracing::info!(
            source = %event.source,
            event_type = %event.event_type,
            id = %event.id,
            "WebhookEvent received (no-op sink — wire an OrchestratorWebhookSink for production)"
        );
        Ok(())
    }
}
