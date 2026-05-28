use crate::task::TaskKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DonationSlot {
    pub task_kind: TaskKind,
    pub max_concurrent: u8,
    pub weight_pct: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerDonationPolicy {
    pub slots: Vec<DonationSlot>,
    pub nsfw_allowed: bool,
    pub max_job_duration_secs: u64,
    pub public_mesh_opt_in: bool,
    /// Minimum priority required to accept a job from the public mesh.
    pub min_priority: u8,
    /// Optional whitelist of scopes this node is willing to donate to.
    /// If None, and public_mesh_opt_in is true, it accepts from any scope.
    pub allowed_scopes: Option<Vec<String>>,
    /// Optional whitelist of user IDs allowed to run tasks on this node.
    pub allowed_users: Option<Vec<String>>,
    /// Optional blacklist of user IDs explicitly denied from running tasks.
    pub denied_users: Option<Vec<String>>,
    /// Optional list of federated mesh networks (scope IDs) to explicitly allow.
    pub allowed_mesh_networks: Option<Vec<String>>,
    /// If `true`, this node is willing to accept workloads marked as handling
    /// sensitive data (e.g. PII, health records). Defaults to `false` for
    /// backwards compatibility with serialized policies that lack this field.
    #[serde(default)]
    pub accept_sensitive_workloads: bool,
    /// Optional redundancy / replication policy for declared-deterministic tasks (P6-T4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redundancy: Option<crate::redundancy::RedundancyPolicy>,
    /// Whether this node accepts mesh inference workloads (Mn-T7).
    #[serde(default)]
    pub accepts_inference_workloads: bool,
    /// Whether this node accepts distributed training workloads (Mn-T7). CUDA training path only.
    #[serde(default)]
    pub accepts_training_workloads: bool,
    /// Advertised CUDA tier for planners (`0` = none / unknown).
    #[serde(default)]
    pub cuda_tier: u8,
    /// Advertised Metal tier for planners (`0` = none / unknown).
    #[serde(default)]
    pub metal_tier: u8,
    /// Minimum VRAM (GiB) this node claims for training/inference scheduling hints.
    #[serde(default)]
    pub vram_min_gb: u32,
    /// Distinct from [`Self::accept_sensitive_workloads`]: gates *training* data sensitivity (Mn-T7).
    #[serde(default)]
    pub accepts_sensitive_training_data: bool,
}
