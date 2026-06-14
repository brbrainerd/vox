//! PopuliMeshPlugin — composite plugin's code side.
//!
//! Implements [`MeshDriver`] by delegating to the ported transport layer
//! (`crate::transport`). The HTTP control plane is hosted in a background
//! tokio runtime started by [`MeshDriver::start_transport`].

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use abi_stable::{erased_types::TD_Opaque, std_types::*};
use vox_plugin_api::abi::VoxPlugin;
use vox_plugin_api::extensions::mesh_driver::{MeshDriver, MeshDriver_TO};

use crate::transport::PopuliTransportState;

/// Configuration JSON for `start_transport`.
///
/// Example: `{"addr":"127.0.0.1:9847"}`
#[derive(serde::Deserialize)]
struct MeshStartConfig {
    /// Socket address to bind (e.g. `"127.0.0.1:9847"`).
    #[serde(default = "default_addr")]
    addr: String,
}

fn default_addr() -> String {
    "127.0.0.1:9847".to_string()
}

/// Shared interior state for the running transport server.
struct PluginState {
    /// Handle to the tokio runtime hosting the control plane.
    runtime: tokio::runtime::Runtime,
    /// Shutdown sender — drop or send to initiate graceful stop.
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Client base URL for dispatch / join / list operations.
    client_base: Option<String>,
}

/// The plugin object (cdylib entry point).
///
/// `PopuliMeshPlugin` is `Clone` (required by abi_stable `TD_Opaque` wrapping)
/// and internally shares state via `Arc<Mutex<>>` so all clones see the same
/// running server.
#[derive(Clone)]
pub struct PopuliMeshPlugin {
    state: Arc<Mutex<Option<PluginState>>>,
}

impl PopuliMeshPlugin {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
        }
    }
}

impl VoxPlugin for PopuliMeshPlugin {
    fn id(&self) -> RString {
        RString::from("populi-mesh")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        // Trigger stop_transport on plugin shutdown.
        self.stop_transport()
    }

    fn as_mesh_driver(&self) -> ROption<MeshDriver_TO<'static, RBox<()>>> {
        ROption::RSome(MeshDriver_TO::from_value(self.clone(), TD_Opaque))
    }
}

impl MeshDriver for PopuliMeshPlugin {
    /// Boot the HTTP control plane.
    ///
    /// Accepts JSON: `{"addr":"<host:port>"}` (optional; defaults to `127.0.0.1:9847`).
    /// Idempotent — if transport is already running, returns `ROk` immediately.
    fn start_transport(&self, config_json: RStr<'_>) -> RResult<(), RBoxError> {
        let cfg: MeshStartConfig =
            if config_json.as_str().trim().is_empty() || config_json.as_str().trim() == "{}" {
                MeshStartConfig {
                    addr: default_addr(),
                }
            } else {
                match serde_json::from_str(config_json.as_str()) {
                    Ok(c) => c,
                    Err(e) => {
                        return RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                            "invalid MeshStartConfig JSON: {e}"
                        ))));
                    }
                }
            };

        let mut guard = match self.state.lock() {
            Ok(g) => g,
            Err(_) => {
                return RResult::RErr(RBoxError::new(std::io::Error::other(
                    "state mutex poisoned",
                )));
            }
        };

        // Idempotent: already running.
        if guard.is_some() {
            return RResult::ROk(());
        }

        let addr: SocketAddr = match cfg.addr.parse() {
            Ok(a) => a,
            Err(e) => {
                return RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                    "invalid addr `{}`: {e}",
                    cfg.addr
                ))));
            }
        };

        let rt = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("populi-mesh")
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                return RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                    "failed to create tokio runtime: {e}"
                ))));
            }
        };

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Build transport state using environment config.
        let state = PopuliTransportState::new_for_serve();

        // Spawn the server on the background runtime.
        let bound_addr_cell: Arc<std::sync::OnceLock<SocketAddr>> =
            Arc::new(std::sync::OnceLock::new());
        let cell_clone = Arc::clone(&bound_addr_cell);

        rt.spawn(async move {
            // Bind listener here so we can capture the actual bound port.
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!(error = %e, "populi-mesh: failed to bind TCP listener");
                    return;
                }
            };
            let bound = listener.local_addr().unwrap_or(addr);
            let _ = cell_clone.set(bound);
            tracing::info!(%bound, "populi-mesh transport listening");

            state.start_federation_gossip();

            let app = crate::transport::populi_http_app(state);

            // Graceful shutdown: race serve against shutdown signal.
            let serve_fut = axum::serve(listener, app);
            tokio::select! {
                res = serve_fut => {
                    if let Err(e) = res {
                        tracing::error!(error = %e, "populi-mesh transport exited with error");
                    }
                }
                _ = shutdown_rx => {
                    tracing::info!("populi-mesh transport shutting down (signal received)");
                }
            }
        });

        // Brief wait so the OnceLock is populated before we read it.
        std::thread::sleep(vox_config::timeouts::D_50MS);
        let bound = bound_addr_cell.get().copied().unwrap_or(addr);
        let client_base = format!("http://{bound}");

        *guard = Some(PluginState {
            runtime: rt,
            shutdown_tx: Some(shutdown_tx),
            client_base: Some(client_base),
        });

        RResult::ROk(())
    }

    /// Gracefully shut down the HTTP control plane.
    fn stop_transport(&self) -> RResult<(), RBoxError> {
        let mut guard = match self.state.lock() {
            Ok(g) => g,
            Err(_) => {
                return RResult::RErr(RBoxError::new(std::io::Error::other(
                    "state mutex poisoned",
                )));
            }
        };
        if let Some(ps) = guard.take() {
            // Signal shutdown.
            if let Some(tx) = ps.shutdown_tx {
                let _ = tx.send(());
            }
            // Shut down the runtime (waits for tasks to complete or forcibly stops them).
            ps.runtime.shutdown_timeout(vox_config::timeouts::D_5S);
        }
        RResult::ROk(())
    }

    /// Dispatch a JSON-encoded [`DispatchRequest`] to the hosted control plane.
    ///
    /// The request JSON is forwarded to `POST /v1/populi/dispatch` via the
    /// in-process HTTP client. Returns the [`DispatchResponse`] as JSON.
    fn dispatch(&self, request_json: RStr<'_>) -> RResult<RString, RBoxError> {
        let req: crate::transport::DispatchRequest =
            match serde_json::from_str(request_json.as_str()) {
                Ok(r) => r,
                Err(e) => {
                    return RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                        "invalid DispatchRequest JSON: {e}"
                    ))));
                }
            };

        let base = match self.client_base() {
            Some(b) => b,
            None => {
                return RResult::RErr(RBoxError::new(std::io::Error::other(
                    "transport not started; call start_transport first",
                )));
            }
        };

        let result = self.block_on(async move {
            let client = crate::http_client::PopuliHttpClient::new(&base).with_env_token();
            client.dispatch(&req).await
        });

        match result {
            Ok(resp) => match serde_json::to_string(&resp) {
                Ok(s) => RResult::ROk(RString::from(s)),
                Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                    "serialize DispatchResponse: {e}"
                )))),
            },
            Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                "dispatch failed: {e}"
            )))),
        }
    }

    /// Register a node by forwarding a JSON-encoded [`NodeRecord`] to `POST /v1/populi/join`.
    fn node_join(&self, node_record_json: RStr<'_>) -> RResult<(), RBoxError> {
        let node: vox_populi::NodeRecord = match serde_json::from_str(node_record_json.as_str()) {
            Ok(n) => n,
            Err(e) => {
                return RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                    "invalid NodeRecord JSON: {e}"
                ))));
            }
        };

        let base = match self.client_base() {
            Some(b) => b,
            None => {
                return RResult::RErr(RBoxError::new(std::io::Error::other(
                    "transport not started; call start_transport first",
                )));
            }
        };

        let result = self.block_on(async move {
            let client = crate::http_client::PopuliHttpClient::new(&base).with_env_token();
            client.join(&node).await
        });

        match result {
            Ok(_) => RResult::ROk(()),
            Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                "node_join failed: {e}"
            )))),
        }
    }

    /// Relay an A2A message directly to a specific peer node URL.
    ///
    /// `target_url` is the base URL of the remote node (e.g. `http://10.0.0.5:9847`).
    /// `request_json` is a JSON-serialised [`vox_mesh_types::A2ADeliverRequest`].
    fn relay_a2a(
        &self,
        target_url: RStr<'_>,
        request_json: RStr<'_>,
    ) -> RResult<RString, RBoxError> {
        let url = target_url.to_string();
        let payload = request_json.to_string();

        let result: Result<String, String> = (|| {
            let request: vox_mesh_types::A2ADeliverRequest = serde_json::from_str(&payload)
                .map_err(|e| format!("invalid A2ADeliverRequest JSON: {e}"))?;
            let response = self.block_on(async move {
                let client = crate::http_client::PopuliHttpClient::new(&url).with_env_token();
                client.relay_a2a(&request).await
            });
            response
                .map(|_| r#"{"ok":true}"#.to_string())
                .map_err(|e| format!("relay_a2a: {e}"))
        })();

        match result {
            Ok(s) => RResult::ROk(RString::from(s)),
            Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(e))),
        }
    }

    /// Return the current node list from `GET /v1/populi/nodes` as a JSON array.
    ///
    /// Falls back to `[]` when transport is not started (caller degrades gracefully).
    fn list_nodes(&self) -> RResult<RString, RBoxError> {
        let base = match self.client_base() {
            Some(b) => b,
            None => return RResult::ROk(RString::from("[]")),
        };

        let result = self.block_on(async move {
            let client = crate::http_client::PopuliHttpClient::new(&base).with_env_token();
            client.list_nodes().await
        });

        match result {
            Ok(reg) => match serde_json::to_string(&reg.nodes) {
                Ok(s) => RResult::ROk(RString::from(s)),
                Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                    "serialize nodes: {e}"
                )))),
            },
            Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(format!(
                "list_nodes failed: {e}"
            )))),
        }
    }
}

impl PopuliMeshPlugin {
    /// Return the client base URL when the transport is running.
    fn client_base(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|g| g.as_ref()?.client_base.clone())
    }

    /// Block on an async future using the plugin's runtime, or create a temporary one.
    fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        // Try to use the already-running runtime's handle to run the future synchronously.
        // When the transport is started, the runtime lives inside PluginState; we can't call
        // `block_on` on it while it's borrowed, so we create a small secondary runtime
        // for client calls (cheap — 1 worker thread, short-lived).
        let mini_rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create mini runtime for populi client call");
        mini_rt.block_on(fut)
    }
}

#[cfg(test)]
mod semcov_wave5_tests {
    use super::*;
    use abi_stable::std_types::RStr;

    // --- stop_transport ---

    #[test]
    fn stop_transport_on_idle_plugin_returns_ok() {
        let plugin = PopuliMeshPlugin::new();
        let result = plugin.stop_transport();
        assert!(
            result.is_rok(),
            "stop_transport on idle plugin must return ROk"
        );
    }

    #[test]
    fn stop_transport_is_idempotent_when_called_twice_without_start() {
        let plugin = PopuliMeshPlugin::new();
        let r1 = plugin.stop_transport();
        let r2 = plugin.stop_transport();
        assert!(r1.is_rok());
        assert!(r2.is_rok());
    }

    // --- dispatch ---

    #[test]
    fn dispatch_invalid_json_returns_err() {
        let plugin = PopuliMeshPlugin::new();
        let result = plugin.dispatch(RStr::from("{not valid json}"));
        match result {
            RResult::RErr(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("invalid DispatchRequest JSON"),
                    "unexpected error: {msg}"
                );
            }
            RResult::ROk(_) => panic!("expected RErr for invalid JSON"),
        }
    }

    #[test]
    fn dispatch_without_start_returns_err() {
        let plugin = PopuliMeshPlugin::new();
        let req_json = r#"{"task":"echo","input":{}}"#;
        let result = plugin.dispatch(RStr::from(req_json));
        assert!(
            result.is_rerr(),
            "dispatch without transport must return RErr"
        );
    }

    // --- node_join ---

    #[test]
    fn node_join_invalid_json_returns_err() {
        let plugin = PopuliMeshPlugin::new();
        let result = plugin.node_join(RStr::from("not-json"));
        match result {
            RResult::RErr(e) => {
                let msg = e.to_string();
                assert!(msg.contains("invalid NodeRecord JSON"), "unexpected: {msg}");
            }
            RResult::ROk(_) => panic!("expected RErr for invalid JSON"),
        }
    }

    #[test]
    fn node_join_without_start_transport_returns_err() {
        let plugin = PopuliMeshPlugin::new();
        let node_json = r#"{"id":"node-1","addr":"127.0.0.1:9000"}"#;
        let result = plugin.node_join(RStr::from(node_json));
        assert!(
            result.is_rerr(),
            "node_join before start_transport must fail"
        );
    }

    // --- relay_a2a ---

    #[test]
    fn relay_a2a_invalid_json_returns_err() {
        let plugin = PopuliMeshPlugin::new();
        let result = plugin.relay_a2a(
            RStr::from("http://127.0.0.1:9999"),
            RStr::from("{bad json}"),
        );
        match result {
            RResult::RErr(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("invalid A2ADeliverRequest JSON"),
                    "unexpected error: {msg}"
                );
            }
            RResult::ROk(_) => panic!("expected RErr for malformed JSON"),
        }
    }

    // --- list_nodes ---

    #[test]
    fn list_nodes_returns_empty_array_when_transport_not_started() {
        let plugin = PopuliMeshPlugin::new();
        let result = plugin.list_nodes();
        match result {
            RResult::ROk(s) => {
                assert_eq!(s.as_str(), "[]", "expected empty JSON array");
            }
            RResult::RErr(e) => panic!("expected ROk, got RErr: {e}"),
        }
    }
}
