//! Persisted record shapes for `vox harness eval --live` runs (chat harness continuous eval
//! design, 2026-08-02). Mirrors `research.rs`'s `ResearchEvalRunRecord`/`ResearchEvalSampleRecord`
//! split: one row per eval invocation, N child rows per golden task result, plus a per-model-
//! selection-decision child table for cost-tier drift tracking.

use serde::{Deserialize, Serialize};

/// One row per `vox harness eval --live` invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessEvalRunRecord {
    pub run_id: String,
    pub triggered_by: String,
    pub git_sha: String,
    pub git_branch: String,
    pub changed_files: Vec<String>,
    pub config_version: Option<String>,
    pub samples_per_task: i64,
    pub task_count: i64,
    pub pass_count: i64,
    pub fail_count: i64,
    pub skip_count: i64,
    pub total_cost_usd: f64,
    pub started_at_ms: i64,
    pub finished_at_ms: i64,
}

/// One row per golden task per run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessEvalTaskResultRecord {
    pub run_id: String,
    pub task_id: String,
    pub category: String,
    pub checker_kind: String,
    pub status: String,
    pub pass_samples: i64,
    pub total_samples: i64,
    pub latency_p50_ms: Option<i64>,
    pub cost_usd: Option<f64>,
    pub failure_detail: Option<String>,
    pub recorded_at_ms: i64,
}

/// One row per model-selection decision observed during a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelectionEventRecord {
    pub run_id: String,
    pub task_id: String,
    pub model_id: String,
    pub cost_tier: String,
    pub selection_reason: String,
    pub was_privacy_gated: bool,
    pub recorded_at_ms: i64,
}
