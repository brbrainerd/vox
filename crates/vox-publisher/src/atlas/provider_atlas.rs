//! Provider Atlas finding for grand-network opt-in discovery (P6-T6).
//!
//! A `ProviderAtlasFinding` is a structured observation emitted by the
//! Scientia discovery feedback loop. It records a single provider's
//! self-reported capabilities and opt-in status at a point in time.
//!
//! Findings are collected by `vox-populi/src/mens/discovery_publish.rs`
//! (gated behind `mesh-discovery-publish`) and published via the Atlas
//! submission pipeline.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Observation
// ---------------------------------------------------------------------------

/// A raw telemetry observation that feeds into a `ProviderAtlasFinding`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasObservation {
    /// Node identifier of the observing provider.
    pub node_id: String,
    /// ISO-8601 timestamp of the observation.
    pub observed_at: String,
    /// Observed task kinds (e.g. `"text_infer"`, `"image_gen"`).
    pub task_kinds: Vec<String>,
    /// Average GPU wall-clock utilisation (0.0 – 1.0) over the observation window.
    pub gpu_utilisation: f64,
    /// Number of tasks completed since the previous observation.
    pub tasks_completed_delta: u64,
    /// Number of tasks that failed since the previous observation.
    pub tasks_failed_delta: u64,
    /// Optional VRAM in MB reported by the node.
    pub vram_mb: Option<u32>,
    /// Whether the node is currently opted into the grand volunteer network.
    pub public_mesh_opt_in: bool,
}

// ---------------------------------------------------------------------------
// Finding
// ---------------------------------------------------------------------------

/// A structured Provider Atlas finding aggregating one or more observations
/// into a single claim about a provider's behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAtlasFinding {
    /// Stable identifier for this finding (typically `"provider.<node_id>.<epoch>"`).
    pub id: String,
    /// The provider node ID this finding covers.
    pub node_id: String,
    /// ISO-8601 start of the observation window.
    pub window_start: String,
    /// ISO-8601 end of the observation window.
    pub window_end: String,
    /// Total tasks completed in the window.
    pub tasks_completed: u64,
    /// Total tasks failed in the window.
    pub tasks_failed: u64,
    /// Success rate in the window (0.0 – 1.0).
    pub success_rate: f64,
    /// Declared task kinds (union across all observations).
    pub declared_task_kinds: Vec<String>,
    /// True if the provider was opted into the grand network for the whole window.
    pub consistently_opted_in: bool,
    /// Average VRAM in MB across observations (None if never reported).
    pub avg_vram_mb: Option<f64>,
    /// Arbitrary metadata key-value pairs for extensibility.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Incrementally builds a `ProviderAtlasFinding` from a stream of observations.
#[derive(Debug, Default)]
pub struct ProviderAtlasFindingBuilder {
    pub node_id: String,
    pub window_start: String,
    pub window_end: String,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub task_kinds: std::collections::HashSet<String>,
    pub all_opted_in: bool,
    pub opted_in_count: usize,
    pub total_count: usize,
    pub vram_sum: f64,
    pub vram_count: usize,
}

impl ProviderAtlasFindingBuilder {
    /// Create a new builder for the given node.
    pub fn new(node_id: impl Into<String>, window_start: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            window_start: window_start.into(),
            all_opted_in: true,
            ..Default::default()
        }
    }

    /// Ingest one observation.
    pub fn observe(&mut self, obs: &AtlasObservation) {
        self.tasks_completed += obs.tasks_completed_delta;
        self.tasks_failed += obs.tasks_failed_delta;
        for kind in &obs.task_kinds {
            self.task_kinds.insert(kind.clone());
        }
        if obs.public_mesh_opt_in {
            self.opted_in_count += 1;
        } else {
            self.all_opted_in = false;
        }
        self.total_count += 1;
        self.window_end = obs.observed_at.clone();
        if let Some(v) = obs.vram_mb {
            self.vram_sum += v as f64;
            self.vram_count += 1;
        }
    }

    /// Build the `ProviderAtlasFinding`.
    pub fn build(self) -> ProviderAtlasFinding {
        let total = (self.tasks_completed + self.tasks_failed).max(1);
        let success_rate = self.tasks_completed as f64 / total as f64;
        let avg_vram_mb = if self.vram_count > 0 {
            Some(self.vram_sum / self.vram_count as f64)
        } else {
            None
        };
        let epoch = self.window_end.replace([':', '-', 'T', 'Z', '.'], "");
        let id = format!("provider.{}.{}", self.node_id, epoch);
        let mut task_kinds: Vec<String> = self.task_kinds.into_iter().collect();
        task_kinds.sort();

        ProviderAtlasFinding {
            id,
            node_id: self.node_id,
            window_start: self.window_start,
            window_end: self.window_end,
            tasks_completed: self.tasks_completed,
            tasks_failed: self.tasks_failed,
            success_rate,
            declared_task_kinds: task_kinds,
            consistently_opted_in: self.all_opted_in && self.total_count > 0,
            avg_vram_mb,
            metadata: Default::default(),
        }
    }
}
