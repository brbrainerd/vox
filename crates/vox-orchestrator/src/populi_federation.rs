//! Read-only view of a remote mens HTTP control plane (orchestrator / MCP federation).
//!
//! Populated by embedders (e.g. `vox-mcp` background poll). Does **not** perform HTTP itself.

use serde::{Deserialize, Serialize};
use vox_repository::TaskCapabilityHints;

/// Capability hints copied from a remote mens `NodeRecord` JSON for experimental routing logs.
///
/// Routing remains **in-process only**; this struct informs `RoutingService` when
/// `OrchestratorConfig::populi_routing_experimental` is enabled.
#[derive(Debug, Clone, PartialEq)]
pub struct RemotePopuliRoutingHint {
    /// [`vox_populi::NodeRecord::id`].
    pub node_id: String,
    /// Full remote capability snapshot copied from `NodeRecord.capabilities`.
    pub capabilities: TaskCapabilityHints,
    /// Labels from the remote node's `vox_orchestrator::TaskCapabilityHints::labels`.
    pub labels: Vec<String>,
    /// Remote node advertises CUDA.
    pub gpu_cuda: bool,
    /// Remote node advertises Metal.
    pub gpu_metal: bool,
    /// Remote node advertises minimum VRAM (MiB), if known.
    pub min_vram_mb: Option<u32>,
    /// Layer A raw visible GPU count when available.
    pub gpu_total_count: Option<u32>,
    /// Layer A healthy GPU count when available.
    pub gpu_healthy_count: Option<u32>,
    /// Layer B allocatable GPU count when available.
    pub gpu_allocatable_count: Option<u32>,
    /// Source marker (`probed`, `advertised`, ...).
    pub gpu_inventory_source: Option<String>,
    /// Truth layer marker (`layer_a_verified`, `layer_b_allocatable`, `layer_c_advertised`).
    pub gpu_truth_layer: Option<String>,
    /// NVIDIA kernel driver version when reported on the remote `NodeRecord`.
    pub nvidia_driver_version: Option<String>,
    /// CUDA driver version when reported on the remote `NodeRecord`.
    pub cuda_driver_version: Option<String>,
    /// Remote worker-reported GPU readiness (`false` excludes GPU mesh eligibility).
    pub gpu_readiness_ok: Option<bool>,
    /// Machine-readable readiness detail when `gpu_readiness_ok` is `false`.
    pub gpu_readiness_reason: Option<String>,
    /// Unix ms when readiness was last evaluated on the remote node.
    pub gpu_readiness_checked_unix_ms: Option<u64>,
    /// Labels related to training workloads (`workload=mens-train`, pool tags, etc.).
    pub training_labels: Vec<String>,
    /// Node is in maintenance/drain mode and should not receive new work.
    pub maintenance: bool,
    /// Node is quarantined and ineligible for new claims.
    pub quarantined: bool,
    /// Last heartbeat is older than [`crate::config::OrchestratorConfig::stale_threshold_ms`]
    /// at federation poll time (partition / crashed worker).
    pub heartbeat_stale: bool,
}

impl RemotePopuliRoutingHint {
    /// Eligible for experimental federation routing visibility (not quarantined, not draining, heartbeat fresh).
    #[inline]
    #[must_use]
    pub fn is_federation_schedulable(&self) -> bool {
        !self.quarantined && !self.maintenance && !self.heartbeat_stale
    }

    /// Advertises CUDA/Metal and has non-zero healthy/allocatable GPU counts when those fields are present.
    #[inline]
    #[must_use]
    pub fn is_federation_gpu_eligible(&self) -> bool {
        if self.gpu_readiness_ok == Some(false) {
            return false;
        }
        if self.gpu_allocatable_count.is_some_and(|n| n == 0) {
            return false;
        }
        if self.gpu_healthy_count.is_some_and(|n| n == 0) {
            return false;
        }
        self.gpu_cuda || self.gpu_metal
    }
}

/// Deltas after applying a new federation hint snapshot (for MCP / embedder follow-ups).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopuliRoutingHintUpdate {
    /// Count of schedulable remote nodes before the update.
    pub prev_schedulable: usize,
    /// Count of schedulable remote nodes after the update.
    pub new_schedulable: usize,
    /// Count of GPU-eligible remote nodes before the update.
    pub prev_gpu_eligible: usize,
    /// Count of GPU-eligible remote nodes after the update.
    pub new_gpu_eligible: usize,
}

/// One line per remote node for status payloads (no full capability struct).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PopuliNodeBrief {
    /// Node id from control plane JSON.
    pub id: String,
    /// Epoch ms from `NodeRecord::last_seen_unix_ms`.
    pub last_seen_unix_ms: u64,
}

/// Cached result of `GET /v1/populi/nodes` (read-only federation).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemotePopuliSnapshot {
    /// Wall clock when the snapshot was taken (Unix ms).
    pub fetched_at_unix_ms: u64,
    /// Whether the last fetch parsed successfully.
    pub ok: bool,
    /// Number of nodes in the registry file.
    pub node_count: usize,
    /// `PopuliRegistryFile::schema_version` when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,
    /// Error message when `ok` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Short node list for dashboards.
    #[serde(default)]
    pub nodes_brief: Vec<PopuliNodeBrief>,
}

impl RemotePopuliSnapshot {
    /// Successful parse.
    #[must_use]
    pub fn success(
        fetched_at_unix_ms: u64,
        schema_version: u32,
        nodes_brief: Vec<PopuliNodeBrief>,
    ) -> Self {
        let node_count = nodes_brief.len();
        Self {
            fetched_at_unix_ms,
            ok: true,
            node_count,
            schema_version: Some(schema_version),
            error: None,
            nodes_brief,
        }
    }

    /// Failed fetch (HTTP / JSON).
    #[must_use]
    pub fn failure(fetched_at_unix_ms: u64, message: impl Into<String>) -> Self {
        Self {
            fetched_at_unix_ms,
            ok: false,
            node_count: 0,
            schema_version: None,
            error: Some(message.into()),
            nodes_brief: Vec::new(),
        }
    }
}
