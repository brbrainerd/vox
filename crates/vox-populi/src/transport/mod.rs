//! Minimal HTTP control plane for populi join / list / heartbeat / leave (loopback-first).
#![allow(missing_docs)]
//!
//! When **any** mesh-class secret resolves via Clavis (`VOX_MESH_TOKEN` and/or worker/submitter/admin
//! role tokens, or optional **`VOX_MESH_JWT_HMAC_SECRET`** for HS256 JWTs), all routes except
//! **`GET /health`** and **`POST /v1/populi/bootstrap/exchange`**
//! require `Authorization: Bearer <token>` matching a configured role or valid JWT (`role` / `jti` / `exp`;
//! values are never logged).
//! Opaque bearer comparison uses [`subtle::ConstantTimeEq`] on UTF-8 bytes when lengths match.
//!
//! When **`VOX_MESH_SCOPE_ID`** is set on the server process, **`POST /v1/populi/join`** and
//! **`POST /v1/populi/heartbeat`** require the JSON [`crate::NodeRecord::scope_id`] to match.
//!
//! **Optional HTTP rate limiting:** **`VOX_MESH_HTTP_RATE_LIMIT`** = `1` / `true` / `on` / `yes`, plus
//! **`VOX_MESH_HTTP_RATE_LIMIT_PER_SEC`** / **`VOX_MESH_HTTP_RATE_LIMIT_BURST`**.

mod auth;
pub mod auth_ed25519;
pub mod envelope;
mod handlers;
mod mesh_replay;
mod result_attestation;
mod router;
pub mod store;

pub use auth::{AuthScheme, PopuliAuthContext, PopuliBearerRole, PopuliMeshAuthRuntime};
pub use router::{
    PopuliHttpAuth, populi_http_app, populi_http_app_with_auth, router, serve, serve_with_listener,
};

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{PopuliRegistryError, PopuliRegistryFile};

/// Well-known A2A [`A2ADeliverRequest::message_type`] tokens for mesh job flows (Horde-style hooks).
/// Submit work to a receiver inbox (payload carries job definition).
pub const A2A_MESSAGE_JOB_SUBMIT: &str = "job_submit";
/// Worker-side claim notification (optional; inbox claim uses `claimer_node_id`).
pub const A2A_MESSAGE_JOB_CLAIM: &str = "job_claim";
/// Result payload from worker to submitter (convention; payload is JSON contract-defined).
pub const A2A_MESSAGE_JOB_RESULT: &str = "job_result";
/// Terminal failure from worker (convention).
pub const A2A_MESSAGE_JOB_FAIL: &str = "job_fail";

/// Body for the `leave_node` HTTP handler: remove a node id from the in-memory registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveRequest {
    /// Node id to remove (same as `NodeRecord::id`).
    pub id: String,
}

/// Request to deliver an A2A message to a local agent.
///
/// Type is defined in `vox-mesh-types` and re-exported here for backwards compatibility.
pub use vox_mesh_types::A2ADeliverRequest;

/// Persisted A2A delivery envelope in the control plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AStoredMessage {
    /// Monotonic message row id.
    pub id: u64,
    /// Sender agent ID.
    pub sender_agent_id: String,
    /// Receiver agent ID.
    pub receiver_agent_id: String,
    /// Message type / schema name.
    pub message_type: String,
    /// JSON or raw payload.
    pub payload: String,
    /// Control-plane wall time when stored (unix ms).
    pub created_unix_ms: u64,
    /// Whether the receiver has acked delivery.
    pub acknowledged: bool,
    /// Node id holding an inbox processing lease, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_holder_node_id: Option<String>,
    /// Wall time when [`Self::lease_holder_node_id`] expires (unix ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_expires_unix_ms: Option<u64>,
    /// Privacy / routing class copied from deliver (for worker claim policy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_class: Option<String>,
    /// Internal dedupe map key when [`A2ADeliverRequest::idempotency_key`] was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_dedupe_key: Option<String>,
    /// Copied from deliver when attestation fields are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_blake3_hex: Option<String>,
    /// Copied from deliver: Standard base64 Ed25519 signature over raw BLAKE3 digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_ed25519_sig_b64: Option<String>,
    /// Copied from deliver: JWE block containing forwarded Clavis secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jwe_payload: Option<String>,
    /// Task priority (0=lowest, 255=highest).
    #[serde(default = "default_priority")]
    pub priority: u8,
    /// Task kind for donation policy filtering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_kind: Option<String>,
    /// Optional target model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Node id that sent this message (if authenticated via node signature).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender_node_id: Option<String>,
    /// W3C `traceparent` copied from the deliver request (for cross-node propagation in S2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traceparent: Option<String>,
}

fn default_priority() -> u8 {
    128
}

/// Reply from the control plane after an A2A deliver attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ADeliverResponse {
    /// Whether the message was accepted for storage/delivery.
    pub accepted: bool,
    /// Assigned [`A2AStoredMessage::id`] when accepted.
    pub message_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshQueueStats {
    pub pending_count: usize,
    pub pending_by_kind: std::collections::HashMap<String, usize>,
    pub pending_by_priority: std::collections::HashMap<u8, usize>,
}

/// Inbox poll: identify the receiving agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AInboxRequest {
    /// Receiver agent ID.
    pub receiver_agent_id: String,
    /// When set, only return messages unleased, leased to this node, or with expired lease; may refresh lease on first matching row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimer_node_id: Option<String>,
    /// Optional maximum row count for non-claimer inbox fetches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_messages: Option<usize>,
    /// Optional cursor for non-claimer inbox fetches (return rows with `id < before_message_id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_message_id: Option<u64>,
}

/// Inbox poll: queued messages for the receiver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AInboxResponse {
    /// Pending messages (order is transport-defined).
    pub messages: Vec<A2AStoredMessage>,
}

/// Ack a delivered inbox message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AAckRequest {
    /// Receiver agent ID.
    pub receiver_agent_id: String,
    /// [`A2AStoredMessage::id`] to acknowledge.
    pub message_id: u64,
}

/// Grant or refresh a **remote execution** lease for an opaque `scope_key` (correlation id for a task /
/// workflow slice / resource slot). Distinct from per-A2A-row inbox leases; stored in-memory on the
/// control plane only. TTL uses the same wall clock as inbox leases (`a2a_lease_duration_ms`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteExecLeaseGrantRequest {
    /// Node id requesting the lease (must be registered via join).
    pub claimer_node_id: String,
    /// Opaque key identifying what is leased (e.g. `workflow_run:…` / `task:…`); trimmed; must be non-empty.
    pub scope_key: String,
}

/// Server response for [`RemoteExecLeaseGrantRequest`]: includes stable `lease_id` for renew/release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteExecLeaseGrantResponse {
    /// Monotonic decimal lease id (correlation handle).
    pub lease_id: String,
    /// Echo of normalized `scope_key`.
    pub scope_key: String,
    /// Holder after grant (same as request `claimer_node_id` when successful).
    pub holder_node_id: String,
    /// Wall time when the lease expires if not renewed (unix ms).
    pub expires_unix_ms: u64,
}

/// Extend an active remote execution lease held by `claimer_node_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteExecLeaseRenewRequest {
    /// [`RemoteExecLeaseGrantResponse::lease_id`].
    pub lease_id: String,
    /// Must match the current holder.
    pub claimer_node_id: String,
}

/// Release a remote execution lease held by `claimer_node_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteExecLeaseReleaseRequest {
    /// [`RemoteExecLeaseGrantResponse::lease_id`].
    pub lease_id: String,
    /// Must match the current holder.
    pub claimer_node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteExecLeaseRow {
    pub(super) lease_id: String,
    pub(super) scope_key: String,
    pub(super) holder_node_id: String,
    pub(super) expires_unix_ms: u64,
}

/// One non-expired remote execution lease for observability ([`GET /v1/populi/exec/leases`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteExecLeaseListItem {
    /// Lease id returned by grant.
    pub lease_id: String,
    /// Correlation key (e.g. `task:<id>`).
    pub scope_key: String,
    /// Node id of the current holder.
    pub holder_node_id: String,
    /// Wall-clock expiry (Unix ms).
    pub expires_unix_ms: u64,
}

/// Response for [`GET /v1/populi/exec/leases`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteExecLeaseListResponse {
    /// Active leases after server-side expiry sweep.
    pub leases: Vec<RemoteExecLeaseListItem>,
}

/// Extend an active inbox lease held by `claimer_node_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ALeaseRenewRequest {
    /// Receiver agent ID (must match row).
    pub receiver_agent_id: String,
    /// [`A2AStoredMessage::id`].
    pub message_id: u64,
    /// Node id that already holds the lease.
    pub claimer_node_id: String,
}

/// Operator quarantine toggle (blocks A2A claims for a node).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminQuarantineRequest {
    /// `NodeRecord::id`.
    pub node_id: String,
    /// When true, claimers with this node id cannot receive new leases.
    pub quarantined: bool,
}

/// Operator maintenance toggle (blocks new A2A claims for a node while enabled).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminMaintenanceRequest {
    /// `NodeRecord::id`.
    pub node_id: String,
    /// When true, claimers with this node id cannot receive new leases.
    pub maintenance: bool,
    /// When `maintenance` is true, clear drain automatically at this Unix ms (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance_until_unix_ms: Option<u64>,
    /// When `maintenance` is true and `maintenance_until_unix_ms` is unset, server sets deadline to `now + min(for_ms, MAX_MAINTENANCE_FOR_MS)]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance_for_ms: Option<u64>,
}

/// Operator removal of a remote execution lease row (does not require holder cooperation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminExecLeaseRevokeRequest {
    /// [`RemoteExecLeaseGrantResponse::lease_id`].
    pub lease_id: String,
}

/// Request a one-time bootstrap exchange for mesh join.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapExchangeRequest {
    /// Ephemeral bootstrap token provisioned by `vox populi up`.
    pub bootstrap_token: String,
}

/// Request to dispatch a .vox script for remote execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchRequest {
    /// Base64-encoded .vox source code or compiled bundle.
    pub source: String,
    /// Optional target node id for affinity; if unset, control plane picks a node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    /// Execution timeout in seconds.
    #[serde(default = "default_dispatch_timeout")]
    pub timeout_secs: u64,
    /// When true, source is treated as a .vox.bundle (compiled binary + manifest).
    #[serde(default)]
    pub is_bundle: bool,
    /// Optional BLAKE3 hash of the source bytes for integrity verification (Wave 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_blake3_hex: Option<String>,
    /// Required capability labels for Wave 5 routing (e.g. ["gpu", "region=us-east"]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_labels: Option<Vec<String>>,
    /// When true, dispatch returns immediately and results are stored in the mesh state (Wave 5).
    #[serde(default)]
    pub is_detached: bool,
    /// Task priority (0=lowest, 255=highest).
    #[serde(default = "default_priority")]
    pub priority: u8,
    /// Task kind (e.g. "text_infer").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_kind: Option<String>,
    /// Target model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Minimum VRAM required in MB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_vram_mb: Option<u32>,
}

fn default_dispatch_timeout() -> u64 {
    30
}

/// Response from a dispatch request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResponse {
    /// Whether the dispatch was successfully routed and executed.
    pub success: bool,
    /// Combined stdout/stderr from the remote execution.
    pub output: String,
    /// Whether the output was truncated due to length limits (Wave 4).
    #[serde(default)]
    pub is_truncated: bool,
    /// Execution duration in milliseconds (Wave 4).
    #[serde(default)]
    pub duration_ms: u64,
    /// Process exit code if available (Wave 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Optional error message if execution failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Node id where the execution happened.
    pub node_id: String,
    /// Optional expiration timestamp for the result (GC/TTL compaction loop).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_unix_ms: Option<u64>,
}

/// Response payload for bootstrap exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapExchangeResponse {
    /// Long-lived mesh bearer token (same as `VOX_MESH_TOKEN`).
    pub mesh_token: String,
    /// Optional scope id to join.
    pub scope_id: Option<String>,
}

/// Request to announce a federated mesh network directory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationAnnounceRequest {
    pub entry: vox_mesh_types::federation::MeshDirectoryEntry,
}

/// Response containing the known federated mesh directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationDirectoryResponse {
    pub entries: Vec<vox_mesh_types::federation::MeshDirectoryEntry>,
}

/// Shared registry state for the HTTP server (in-memory; optionally persisted by callers).
#[derive(Clone)]
pub struct PopuliTransportState {
    inner: Arc<RwLock<PopuliRegistryFile>>,
    a2a_messages: Arc<RwLock<Vec<A2AStoredMessage>>>,
    a2a_id_gen: Arc<AtomicU64>,
    exec_leases: Arc<RwLock<Vec<RemoteExecLeaseRow>>>,
    exec_lease_id_gen: Arc<AtomicU64>,
    /// JWT `jti` replay + A2A idempotency keys; optionally persisted (`mesh-replay-state.json`).
    pub(crate) mesh_replay: Arc<mesh_replay::MeshReplayState>,
    /// Durable mesh store (Turso via VoxDb). When `Some`, all A2A / lease / dispatch mutations
    /// are written through here in addition to the in-memory cache.
    pub(crate) mesh_store: Option<Arc<dyn store::MeshStore>>,
    a2a_store_path: Option<PathBuf>,
    exec_lease_store_path: Option<PathBuf>,
    pub(crate) federated_meshes: Arc<RwLock<Vec<vox_mesh_types::federation::MeshDirectoryEntry>>>,
    bootstrap_token: Option<Arc<str>>,
    bootstrap_expires_unix_ms: Option<u64>,
    bootstrap_used: Arc<AtomicBool>,
    /// When set, join/heartbeat must send the same `NodeRecord::scope_id`.
    pub required_scope: Option<Arc<str>>,
    /// Optional Ed25519 verify key from **`VOX_MESH_WORKER_RESULT_VERIFY_KEY`** for signed job results.
    pub(super) worker_result_verify_key: Option<[u8; 32]>,
    /// Wave 5: Async dispatch result storage for detached execution.
    #[cfg(feature = "transport")]
    pub(crate) dispatch_results: Arc<dashmap::DashMap<String, DispatchResponse>>,
    #[cfg(feature = "transport")]
    pub(crate) dispatch_results_store_path: Option<PathBuf>,
    /// Optional callback to verify if a given node_id is trusted.
    #[allow(clippy::type_complexity)]
    pub node_trust_verifier: Option<
        Arc<
            dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
                + Send
                + Sync,
        >,
    >,
    /// Optional VoxDb handle for kudos ledger and reputation tracking.
    pub db: Option<vox_db::VoxDb>,
    /// Federated mesh networks to announce ourselves to on startup.
    pub bootstrap_peers: Vec<String>,
}

impl Default for PopuliTransportState {
    fn default() -> Self {
        Self::new()
    }
}

impl PopuliTransportState {
    /// New empty in-memory registry; does **not** read `VOX_MESH_SCOPE_ID` (for tests).
    #[must_use]
    pub fn new() -> Self {
        Self::with_required_scope(None)
    }

    /// Override worker result attestation key (primarily tests; [`Self::new_for_serve`] reads Clavis otherwise).
    #[must_use]
    pub fn with_worker_result_verify_key(mut self, key: Option<[u8; 32]>) -> Self {
        self.worker_result_verify_key = key;
        self
    }

    /// Set the database handle for kudos and reputation.
    #[must_use]
    pub fn with_db(mut self, db: Option<vox_db::VoxDb>) -> Self {
        self.db = db;
        self
    }

    /// Attach a durable [`store::MeshStore`] for write-through persistence.
    #[must_use]
    pub fn with_mesh_store(mut self, store: Arc<dyn store::MeshStore>) -> Self {
        self.mesh_store = Some(store);
        self
    }

    /// Warm in-memory caches from the durable store (called once at serve startup).
    ///
    /// No-op when `mesh_store` is `None`.
    pub async fn init_from_mesh_store(&mut self) -> Result<(), store::MeshStoreError> {
        use std::sync::atomic::Ordering;
        let Some(ms) = self.mesh_store.clone() else {
            return Ok(());
        };

        let a2a = ms.load_all_a2a().await?;
        let next_id = a2a
            .iter()
            .map(|m| m.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        *self.a2a_messages.write().await = a2a;
        self.a2a_id_gen.store(next_id, Ordering::SeqCst);

        let leases = ms.list_exec_leases().await?;
        let next_lease_id = leases
            .iter()
            .filter_map(|r| r.lease_id.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        *self.exec_leases.write().await = leases;
        self.exec_lease_id_gen
            .store(next_lease_id, Ordering::SeqCst);

        #[cfg(feature = "transport")]
        {
            let dispatch = ms.load_all_dispatch_results().await?;
            self.dispatch_results = Arc::new(dashmap::DashMap::from_iter(dispatch));
        }

        Ok(())
    }

    /// New empty registry and optional required scope (trimmed; empty string becomes `None`).
    #[must_use]
    pub fn with_required_scope(scope: Option<String>) -> Self {
        let required_scope = scope
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(|s| Arc::from(s.into_boxed_str()));
        Self {
            inner: Arc::new(RwLock::new(PopuliRegistryFile {
                schema_version: 1,
                nodes: Vec::new(),
                queue_depth: None,
            })),
            a2a_messages: Arc::new(RwLock::new(Vec::new())),
            a2a_id_gen: Arc::new(AtomicU64::new(1)),
            exec_leases: Arc::new(RwLock::new(Vec::new())),
            exec_lease_id_gen: Arc::new(AtomicU64::new(1)),
            mesh_replay: mesh_replay::MeshReplayState::in_memory(),
            mesh_store: None,
            a2a_store_path: None,
            exec_lease_store_path: None,
            federated_meshes: Arc::new(RwLock::new(Vec::new())),
            bootstrap_token: None,
            bootstrap_expires_unix_ms: None,
            bootstrap_used: Arc::new(AtomicBool::new(false)),
            required_scope,
            worker_result_verify_key: None,
            #[cfg(feature = "transport")]
            dispatch_results: Arc::new(dashmap::DashMap::new()),
            #[cfg(feature = "transport")]
            dispatch_results_store_path: None,
            node_trust_verifier: None,
            db: None,
            bootstrap_peers: Vec::new(),
        }
    }

    /// Same as [`Self::new`] but sets [`Self::required_scope`] from **`VOX_MESH_SCOPE_ID`** when set.
    #[must_use]
    pub fn new_for_serve() -> Self {
        let mut s = Self::with_required_scope(crate::populi_scope_id_from_env());
        let store_path = store::a2a_store_path_from_env();
        let exec_lease_store_path = store::exec_lease_store_path_from_env(store_path.as_ref());
        let dispatch_results_store_path =
            store::dispatch_results_store_path_from_env(store_path.as_ref());
        if let Some(path) = &store_path
            && let Ok(existing) = store::load_a2a_store(path)
        {
            let next_id = existing
                .iter()
                .map(|m| m.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            s.a2a_messages = Arc::new(RwLock::new(existing));
            s.a2a_id_gen = Arc::new(AtomicU64::new(next_id));
        }
        if let Some(path) = &exec_lease_store_path
            && let Ok(existing) = store::load_exec_lease_store(path)
        {
            let next_lease_id = existing
                .iter()
                .filter_map(|r| r.lease_id.parse::<u64>().ok())
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            s.exec_leases = Arc::new(RwLock::new(existing));
            s.exec_lease_id_gen = Arc::new(AtomicU64::new(next_lease_id));
        }

        #[cfg(feature = "transport")]
        if let Some(path) = &dispatch_results_store_path
            && let Ok(existing) = store::load_dispatch_results_store(path)
        {
            s.dispatch_results = Arc::new(dashmap::DashMap::from_iter(existing));
        }

        s.a2a_store_path = store_path;
        s.exec_lease_store_path = exec_lease_store_path;
        #[cfg(feature = "transport")]
        {
            s.dispatch_results_store_path = dispatch_results_store_path;
        }
        s.bootstrap_token =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshBootstrapToken)
                .expose()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .map(Arc::from);
        s.bootstrap_expires_unix_ms =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshBootstrapExpiresUnixMs)
                .expose()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|ms| *ms > crate::now_ms());
        s
    }

    /// Set a one-time bootstrap token, overriding the `VoxMeshBootstrapToken` env value.
    ///
    /// When set, `POST /v1/populi/bootstrap/exchange` accepts this token exactly once and
    /// returns the long-lived mesh bearer token to the caller.  The token is consumed on
    /// first use; subsequent calls receive 410 Gone.
    pub fn with_bootstrap_token(mut self, token: impl Into<Arc<str>>) -> Self {
        self.bootstrap_token = Some(token.into());
        self.bootstrap_used = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self
    }

    /// Load initial snapshot from disk (best-effort) and apply scope from **`VOX_MESH_SCOPE_ID`**.
    pub async fn load_from_path(path: &std::path::Path) -> Result<Self, PopuliRegistryError> {
        let reg = if path.is_file() {
            let raw = vox_bounded_fs::read_utf8_path_capped(path)
                .map_err(|e| PopuliRegistryError::Io(std::io::Error::other(e.to_string())))?;
            serde_json::from_str(&raw).map_err(|e| PopuliRegistryError::Json(e.to_string()))?
        } else {
            PopuliRegistryFile {
                schema_version: 1,
                nodes: Vec::new(),
                queue_depth: None,
            }
        };
        let store_path = store::a2a_store_path_from_env();
        let exec_lease_store_path = store::exec_lease_store_path_from_env(store_path.as_ref());
        let dispatch_results_store_path =
            store::dispatch_results_store_path_from_env(store_path.as_ref());
        let rows = if let Some(sp) = &store_path {
            store::load_a2a_store(sp).unwrap_or_default()
        } else {
            Vec::new()
        };
        let exec_lease_rows = if let Some(sp) = &exec_lease_store_path {
            store::load_exec_lease_store(sp).unwrap_or_default()
        } else {
            Vec::new()
        };
        let next_id = rows
            .iter()
            .map(|m| m.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let replay_path = mesh_replay::mesh_replay_persist_path(store_path.as_ref());
        Ok(Self {
            inner: Arc::new(RwLock::new(reg)),
            a2a_messages: Arc::new(RwLock::new(rows)),
            a2a_id_gen: Arc::new(AtomicU64::new(next_id)),
            exec_leases: Arc::new(RwLock::new(exec_lease_rows.clone())),
            exec_lease_id_gen: Arc::new(AtomicU64::new(
                exec_lease_rows
                    .iter()
                    .filter_map(|r| r.lease_id.parse::<u64>().ok())
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1),
            )),
            mesh_replay: mesh_replay::MeshReplayState::load(replay_path),
            a2a_store_path: store_path,
            exec_lease_store_path,
            federated_meshes: Arc::new(RwLock::new(Vec::new())),
            bootstrap_token: None,
            bootstrap_expires_unix_ms: None,
            bootstrap_used: Arc::new(AtomicBool::new(false)),
            required_scope: crate::populi_scope_id_from_env()
                .map(|s| Arc::from(s.into_boxed_str())),
            worker_result_verify_key: worker_result_verify_key_resolved(),
            #[cfg(feature = "transport")]
            dispatch_results: if let Some(path) = &dispatch_results_store_path
                && let Ok(existing) = store::load_dispatch_results_store(path)
            {
                Arc::new(dashmap::DashMap::from_iter(existing))
            } else {
                Arc::new(dashmap::DashMap::new())
            },
            #[cfg(feature = "transport")]
            dispatch_results_store_path,
            node_trust_verifier: None,
            db: None,
            bootstrap_peers: Vec::new(),
            mesh_store: None,
        })
    }

    /// Spawns a background task that periodically announces this mesh to federated peers.
    pub fn start_federation_gossip(&self) {
        let state = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            let env = crate::populi_env();

            let scope_id = env
                .scope_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let control_url = env
                .control_addr
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:9847".to_string());
            let public = env.visibility.as_deref() == Some("public");

            // Resolve signing key from Clavis
            let signing_key =
                vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshFederationSigningKey)
                    .expose()
                    .and_then(|s| {
                        let bytes = data_encoding::BASE64.decode(s.trim().as_bytes()).ok()?;
                        if bytes.len() != 32 {
                            return None;
                        }
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&bytes);
                        Some(vox_crypto::facades::signing_key_from_bytes(&arr))
                    });

            let public_key = signing_key.as_ref().map(|k| {
                vox_crypto::facades::verifying_key_to_bytes(&vox_crypto::facades::to_verifying_key(
                    k,
                ))
            });

            // Derive task kinds from donation policy if present
            let task_kinds = env
                .donation_policy
                .as_ref()
                .map(|p| {
                    p.slots
                        .iter()
                        .map(|s| s.task_kind.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            tracing::info!(
                scope_id = %scope_id,
                control_url = %control_url,
                bootstrap_peers = state.bootstrap_peers.len(),
                signed = signing_key.is_some(),
                "Starting federation gossip loop"
            );

            loop {
                interval.tick().await;

                // Simple metric for load: count of pending A2A messages
                let queue_depth = state.a2a_messages.read().await.len();

                let mut entry = vox_mesh_types::federation::MeshDirectoryEntry {
                    scope_id: scope_id.clone(),
                    control_url: control_url.clone(),
                    region_label: None,
                    task_kinds: task_kinds.clone(),
                    public,
                    current_queue_depth: Some(queue_depth),
                    supported_priorities: None,
                    signature: None,
                    public_key,
                };

                if let Some(ref key) = signing_key {
                    let msg = entry.canonical_bytes();
                    entry.signature = Some(vox_crypto::facades::sign(key, &msg).to_vec());
                }

                let announce = FederationAnnounceRequest { entry };

                // Collect target peers: bootstrap + already known federated meshes
                let mut targets = state.bootstrap_peers.clone();
                {
                    let federated = state.federated_meshes.read().await;
                    for peer in federated.iter() {
                        if !targets.contains(&peer.control_url) {
                            targets.push(peer.control_url.clone());
                        }
                    }
                }

                for peer_url in targets {
                    // Don't announce to ourselves
                    if peer_url == control_url {
                        continue;
                    }

                    let client =
                        crate::http_client::PopuliHttpClient::new(&peer_url).with_env_token();
                    match client.federation_announce(&announce).await {
                        Ok(resp) => {
                            // Infectious discovery: merge entries learned from this peer
                            let mut federated = state.federated_meshes.write().await;
                            for peer_entry in resp.entries {
                                // Don't add ourselves or nodes we already know with newer info?
                                // For now, simple upsert by scope_id
                                if peer_entry.scope_id != scope_id {
                                    if let Some(pos) = federated
                                        .iter()
                                        .position(|e| e.scope_id == peer_entry.scope_id)
                                    {
                                        federated[pos] = peer_entry;
                                    } else {
                                        federated.push(peer_entry);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                peer_url = %peer_url,
                                error = %e,
                                "Failed to announce to federated peer"
                            );
                        }
                    }
                }
            }
        });
    }
}

fn worker_result_verify_key_resolved() -> Option<[u8; 32]> {
    let resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshWorkerResultVerifyKey);
    let raw = resolved.expose()?;
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    match result_attestation::parse_ed25519_public_key_bytes(t) {
        Ok(k) => Some(k),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "VOX_MESH_WORKER_RESULT_VERIFY_KEY is invalid; job_result attestation disabled"
            );
            None
        }
    }
}

/// Optional server-side staleness window: hide nodes whose `last_seen_unix_ms` is older than this
/// many milliseconds from [`crate::now_ms`]. Unset or `0` = no pruning (default).
#[must_use]
pub(super) fn server_stale_prune_ms() -> Option<u64> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshServerStalePruneMs)
        .expose()
        .and_then(|s| s.trim().parse().ok())
        .filter(|&n| n > 0)
}

/// Max in-memory A2A rows before oldest messages are dropped (persisted store is rewritten).
#[must_use]
pub(super) fn a2a_in_memory_cap() -> usize {
    const DEFAULT: usize = 50_000;
    /// Allow small caps for tests and single-node dev; operators should still use ≥100 in prod.
    const MIN: usize = 1;
    const MAX: usize = 500_000;
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshA2aMaxMessages)
        .expose()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .map(|n| n.clamp(MIN, MAX))
        .unwrap_or(DEFAULT)
}

/// Drop expired inbox leases in-place (requeue semantics).
pub(super) fn a2a_sweep_expired_leases(messages: &mut [A2AStoredMessage], now_ms: u64) {
    for m in messages.iter_mut() {
        if m.acknowledged {
            continue;
        }
        if m.lease_expires_unix_ms.is_some_and(|exp| exp <= now_ms) {
            m.lease_holder_node_id = None;
            m.lease_expires_unix_ms = None;
        }
    }
}

/// Drop expired remote execution leases (same wall clock as inbox leases).
pub(super) fn exec_lease_sweep(rows: &mut Vec<RemoteExecLeaseRow>, now_ms: u64) {
    rows.retain(|r| r.expires_unix_ms > now_ms);
}

/// Sweep expired dispatch results from the DashMap.
#[cfg(feature = "transport")]
pub(super) fn dispatch_results_sweep(
    map: &dashmap::DashMap<String, DispatchResponse>,
    now_ms: u64,
) {
    let mut to_remove = Vec::new();
    for entry in map.iter() {
        if let Some(exp) = entry.value().expires_unix_ms
            && exp <= now_ms
        {
            to_remove.push(entry.key().clone());
        }
    }
    for k in to_remove {
        map.remove(&k);
    }
}

/// Inbox lease duration in milliseconds (claimer flows). Override with **`VOX_MESH_A2A_LEASE_MS`**.
#[must_use]
pub(super) fn a2a_lease_duration_ms() -> u64 {
    const DEFAULT: u64 = 120_000;
    const MIN: u64 = 1_000;
    const MAX: u64 = 3_600_000;
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshA2aLeaseMs)
        .expose()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|n| n.clamp(MIN, MAX))
        .unwrap_or(DEFAULT)
}
