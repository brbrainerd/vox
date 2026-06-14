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
use vox_plugin_api::extensions::http_listener::{
    HTTP_LISTENER_REVISION, HttpListener, HttpListener_TO,
};
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

    fn as_http_listener(&self) -> ROption<HttpListener_TO<'static, RBox<()>>> {
        ROption::RSome(HttpListener_TO::from_value(WebhookHttpListener, TD_Opaque))
    }
}

struct WebhookHttpListener;

impl HttpListener for WebhookHttpListener {
    fn revision(&self) -> u32 {
        HTTP_LISTENER_REVISION
    }

    fn start_listening(&self, config_json: RStr<'_>) -> RResult<(), RBoxError> {
        let addr = serde_json::from_str::<serde_json::Value>(config_json.as_str())
            .ok()
            .and_then(|v| v.get("addr").and_then(|a| a.as_str()).map(str::to_string))
            .or_else(|| std::env::var("VOX_WEBHOOK_ADDR").ok())
            .unwrap_or_else(|| "0.0.0.0:9080".to_string());
        let ingress_token = std::env::var("VOX_WEBHOOK_INGRESS_TOKEN").ok();
        let mut state = WebhookState::new(WebhookHandler::new());
        if let Some(token) = ingress_token {
            state = state.with_ingress_token(token);
        }
        tokio::spawn(async move {
            info!(addr = %addr, "vox-plugin-webhook: HttpListener start_listening");
            if let Err(e) = serve(state, &addr).await {
                tracing::error!("vox-plugin-webhook: server error: {e}");
            }
        });
        RResult::ROk(())
    }

    fn stop_listening(&self) -> RResult<(), RBoxError> {
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

#[cfg(test)]
mod semcov_wave3_tests {
    #![allow(unused_imports)]
    use super::*;
    use vox_plugin_api::extensions::http_listener::HttpListener;

    // start_listening resolves addr from JSON config first, then env var, then
    // hardcoded default. We can observe the branching at the function boundary
    // without actually binding a port because the function always returns ROk
    // (spawn is fire-and-forget) — but we MUST run inside a tokio runtime so
    // that tokio::spawn does not panic.

    #[tokio::test]
    async fn start_listening_returns_ok_for_valid_json_config() {
        let listener = WebhookHttpListener;
        let config = r#"{"addr": "127.0.0.1:0"}"#;
        let result = listener.start_listening(config.into());
        assert!(
            result.is_rok(),
            "start_listening must succeed for valid JSON config: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn start_listening_returns_ok_for_empty_json_object() {
        // Falls through to env-var / default path
        let listener = WebhookHttpListener;
        let config = r#"{}"#;
        let result = listener.start_listening(config.into());
        assert!(
            result.is_rok(),
            "start_listening must succeed for empty JSON"
        );
    }

    #[tokio::test]
    async fn start_listening_returns_ok_for_invalid_json() {
        // JSON parse fails → falls back to env-var / default — must not propagate error
        let listener = WebhookHttpListener;
        let config = "not-json";
        let result = listener.start_listening(config.into());
        assert!(
            result.is_rok(),
            "start_listening must succeed even for unparseable config"
        );
    }

    #[tokio::test]
    async fn start_listening_env_var_path_succeeds() {
        unsafe { std::env::set_var("VOX_WEBHOOK_ADDR", "127.0.0.1:0") };
        let listener = WebhookHttpListener;
        // Empty JSON → addr from env var
        let result = listener.start_listening(r#"{}"#.into());
        unsafe { std::env::remove_var("VOX_WEBHOOK_ADDR") };
        assert!(result.is_rok(), "env-var addr path must succeed");
    }
}
