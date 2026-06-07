//! [`NodeRecord`], [`PopuliRegistryFile`], and their stateless helpers.
//!
//! Extracted from `vox-populi/src/node_registry.rs` per ADR-042. The file-backed
//! persistence layer (`vox-populi::LocalRegistry`) remains in `vox-populi`.

use serde::{Deserialize, Serialize};
use vox_repository::TaskCapabilityHints;

/// One participant in the populi view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRecord {
    /// Stable node id (operator- or env-assigned).
    pub id: String,
    /// Host capabilities (CPU + optional GPU hints).
    pub capabilities: TaskCapabilityHints,
    /// Optional listen address for control or data plane (phase 3+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_addr: Option<String>,
    /// `CARGO_PKG_VERSION` of `vox-populi` / embedding crate at registration time.
    pub version: String,
    /// Wall-clock last update (epoch ms).
    pub last_seen_unix_ms: u64,
    /// Populi tenancy / cluster id; must match server `PopuliTransportState::required_scope`
    /// when the server enforces scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_id: Option<String>,
    /// Worker visibility for scheduling policy (`private` or `public` when set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    /// Logical pool id (`pool=…` mesh label normalization).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_id: Option<String>,
    /// Trust tier for public mesh policy (`new`, `probation`, `trusted`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_tier: Option<String>,
    /// Declared workload classes (`infer`, `train`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_classes: Option<Vec<String>>,
    /// Privacy class advertised by this node (`public_ok`, `trusted_only`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_class: Option<String>,
    /// Advertised models loaded into VRAM on this node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_llm_models: Option<Vec<String>>,
    /// When true, scheduler should not place new work here (drain-only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance: Option<bool>,
    /// When set and `maintenance` is true, maintenance is treated as cleared at this
    /// Unix ms (lazy sweep + gate checks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance_until_unix_ms: Option<u64>,
    /// Optional cloud / bridge provider tag (`runpod`, `vast`, …) for hybrid workers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Total number of GPU devices visible on this node (when probed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_total_count: Option<u32>,
    /// Number of currently healthy GPUs on this node (when probed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_healthy_count: Option<u32>,
    /// Number of currently allocatable GPUs after local reservations (Layer B).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_allocatable_count: Option<u32>,
    /// Source of GPU inventory values (`probed`, `advertised`, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_inventory_source: Option<String>,
    /// Truth-layer marker (`layer_a_verified`, `layer_b_allocatable`, `layer_c_advertised`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_truth_layer: Option<String>,
    /// NVIDIA kernel driver version (NVML `sys_driver_version`), when probe-backed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nvidia_driver_version: Option<String>,
    /// CUDA driver version (`major.minor` from NVML), when probe-backed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuda_driver_version: Option<String>,
    /// Worker-reported GPU readiness for scheduling (NVML probe or pilot self-check).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_readiness_ok: Option<bool>,
    /// Short machine-readable reason when [`Self::gpu_readiness_ok`] is `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_readiness_reason: Option<String>,
    /// Unix ms when readiness was last evaluated on the worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_readiness_checked_unix_ms: Option<u64>,
    /// When true, server rejects new A2A claims for this node (admin quarantine only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantined: Option<bool>,
    /// Host architecture triple (e.g. `x86_64-pc-windows-msvc`) for cross-compilation (Wave 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_triple: Option<String>,
    /// Real-time CPU usage percentage (0.0–100.0) for Wave 5 load balancing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_usage_pct: Option<f32>,
    /// Available system memory in bytes for Wave 5 resource-aware scheduling.
    pub memory_free_bytes: Option<u64>,
    /// The user ID of the node owner (assigned securely from the join token).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_vox_user_id: Option<String>,
    /// Models advertised by this node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advertised_models: Option<Vec<vox_mesh_types::ModelAdvertisement>>,
    /// Donation policy for GPU compute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub donation_policy: Option<vox_mesh_types::WorkerDonationPolicy>,
    /// Ed25519 public key used to verify attestation signatures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ed25519_pub_key_b64: Option<String>,
    /// Names of hardware probes that returned an error during the last probe pipeline run.
    /// `None` means no failures occurred, or the summary was not produced by a pipeline run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_failures: Option<Vec<String>>,
}

/// Serializable registry file (`.vox/cache/populi/local-registry.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PopuliRegistryFile {
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// Known nodes (typically one for local-only mode).
    pub nodes: Vec<NodeRecord>,
    /// Wave 5: Global pending job count across all receiver agent inboxes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_depth: Option<usize>,
}

/// Upper bound for operator `maintenance_for_ms` (`POST /v1/populi/admin/maintenance`).
pub const MAX_MAINTENANCE_FOR_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// Drop nodes whose `last_seen_unix_ms` is older than `now - max_stale_ms`.
/// `max_stale_ms = None` or `0` returns the file unchanged.
#[must_use]
pub fn filter_registry_by_max_stale_ms(
    mut file: PopuliRegistryFile,
    max_stale_ms: Option<u64>,
) -> PopuliRegistryFile {
    let Some(threshold) = max_stale_ms.filter(|n| *n > 0) else {
        return file;
    };
    let now = now_ms();
    file.nodes
        .retain(|n| now.saturating_sub(n.last_seen_unix_ms) <= threshold);
    file
}

/// Merge `incoming` (control-plane truth) into `local`, deduped by `id`, keeping
/// the record with the greater `last_seen_unix_ms`. Ties resolve to `incoming`
/// (the control plane is authoritative). Never mutates timestamps. `schema_version`
/// becomes the max; `queue_depth` prefers the live `incoming` value. Output node
/// order is sorted by `id` for deterministic on-disk + test output.
#[must_use]
pub fn merge_registry_by_last_seen(
    local: PopuliRegistryFile,
    incoming: PopuliRegistryFile,
) -> PopuliRegistryFile {
    use std::collections::HashMap;
    let mut by_id: HashMap<String, NodeRecord> = HashMap::new();
    for n in local.nodes {
        by_id.insert(n.id.clone(), n);
    }
    for n in incoming.nodes {
        // Incoming wins unless the local record is strictly fresher (so ties go to
        // the authoritative control plane).
        let keep_local = by_id
            .get(&n.id)
            .is_some_and(|existing| existing.last_seen_unix_ms > n.last_seen_unix_ms);
        if !keep_local {
            by_id.insert(n.id.clone(), n);
        }
    }
    let mut nodes: Vec<NodeRecord> = by_id.into_values().collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    PopuliRegistryFile {
        schema_version: local.schema_version.max(incoming.schema_version),
        nodes,
        queue_depth: incoming.queue_depth.or(local.queue_depth),
    }
}

/// Whether this node should block **new** claims / exec lease grant+renew (drain semantics).
#[must_use]
pub fn node_maintenance_blocks_new_work(now_ms: u64, n: &NodeRecord) -> bool {
    if n.maintenance != Some(true) {
        return false;
    }
    if let Some(until) = n.maintenance_until_unix_ms
        && now_ms >= until
    {
        return false;
    }
    true
}

/// Clear [`NodeRecord::maintenance`] / deadline when the deadline has passed.
pub fn sweep_expired_maintenance_on_nodes(nodes: &mut [NodeRecord], now_ms: u64) {
    for n in nodes.iter_mut() {
        if n.maintenance == Some(true) && n.maintenance_until_unix_ms.is_some_and(|u| now_ms >= u) {
            n.maintenance = None;
            n.maintenance_until_unix_ms = None;
        }
    }
}

/// Registry I/O errors. Defined here so `vox-populi-types` consumers can match on them
/// without depending on `vox-populi`.
#[derive(Debug, thiserror::Error)]
pub enum PopuliRegistryError {
    /// Filesystem error.
    #[error("populi registry I/O: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse/serialize error.
    #[error("populi registry JSON: {0}")]
    Json(String),
    /// HTTP control plane error.
    #[error("populi HTTP: {0}")]
    Http(String),
    /// HTTP control plane status error with structured code/context.
    #[error("populi HTTP {status} ({context}){body_suffix}")]
    HttpStatus {
        /// HTTP status code (`404`, `409`, ...).
        status: u16,
        /// Short operation context (`exec_lease_renew`, `a2a_inbox`, ...).
        context: String,
        /// Optional response body snippet.
        body_suffix: String,
    },
}

impl PopuliRegistryError {
    /// Status code when this error came from an HTTP status failure.
    #[must_use]
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::HttpStatus { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Convenience predicate for status-code branching.
    #[must_use]
    pub fn is_http_status(&self, code: u16) -> bool {
        self.status_code() == Some(code)
    }
}

// ── private util ─────────────────────────────────────────────────────────────

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    fn node(id: &str, last_seen: u64, version: &str) -> NodeRecord {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "capabilities": {},
            "version": version,
            "last_seen_unix_ms": last_seen,
        }))
        .expect("minimal NodeRecord")
    }

    fn file(schema: u32, queue: Option<usize>, nodes: Vec<NodeRecord>) -> PopuliRegistryFile {
        PopuliRegistryFile {
            schema_version: schema,
            nodes,
            queue_depth: queue,
        }
    }

    #[test]
    fn merge_unions_keeps_fresher_and_is_deterministic() {
        let local = file(
            1,
            Some(2),
            vec![node("a", 100, "local"), node("b", 500, "local")],
        );
        let incoming = file(
            2,
            Some(7),
            vec![node("a", 200, "remote"), node("c", 50, "remote")],
        );

        let merged = merge_registry_by_last_seen(local, incoming);

        // Union of ids, sorted deterministically.
        let ids: Vec<&str> = merged.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
        // 'a' conflict → incoming is fresher (200 > 100) → remote wins.
        let a = merged.nodes.iter().find(|n| n.id == "a").unwrap();
        assert_eq!(a.version, "remote");
        // 'b' only local → kept; 'c' only incoming → inserted.
        assert_eq!(
            merged.nodes.iter().find(|n| n.id == "b").unwrap().version,
            "local"
        );
        // schema = max, queue_depth = live incoming.
        assert_eq!(merged.schema_version, 2);
        assert_eq!(merged.queue_depth, Some(7));
    }

    #[test]
    fn merge_tie_resolves_to_incoming_authoritative() {
        let local = file(1, None, vec![node("x", 300, "local")]);
        let incoming = file(1, None, vec![node("x", 300, "remote")]);
        let merged = merge_registry_by_last_seen(local, incoming);
        assert_eq!(merged.nodes.len(), 1);
        assert_eq!(
            merged.nodes[0].version, "remote",
            "on an equal last_seen tie the control plane (incoming) wins"
        );
    }

    #[test]
    fn merge_keeps_strictly_fresher_local() {
        let local = file(1, None, vec![node("y", 999, "local")]);
        let incoming = file(1, None, vec![node("y", 100, "remote")]);
        let merged = merge_registry_by_last_seen(local, incoming);
        assert_eq!(merged.nodes[0].version, "local");
    }
}
