//! Build-observability summary/row types.
//!
//! Moved from `vox_db::store::ops_build`. The query functions
//! (`VoxDb::query_build_health` etc.) stay in vox-db; only the pure-data
//! result types live here.

use serde::{Deserialize, Serialize};

/// Persisted dependency-shape summary for a build run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildDependencyShape {
    /// Build run id this snapshot is attached to.
    pub run_id: i64,
    /// Raw JSON payload (bounded summary from CLI collector).
    pub dependency_json: serde_json::Value,
}

/// Summary returned by `vox_db::VoxDb::query_build_health`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildHealthSummary {
    /// Total wall-clock of the latest run in milliseconds.
    pub total_ms: i64,
    /// Number of crates compiled (not served from cache) in the latest run.
    pub compiled: i64,
    /// Number of crates served from cache in the latest run.
    pub cached: i64,
    /// Up to 5 slowest non-fresh crates in the latest run.
    pub slowest: Vec<CrateSample>,
    /// Number of warnings emitted during the latest run.
    pub warning_count: i64,
    /// Whether the dep-graph fingerprint changed vs the previous run.
    pub dep_changed: bool,
}

/// One crate timing sample (subset used in health/regression reports).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateSample {
    /// Crate name (package id short form).
    pub name: String,
    /// Elapsed compile time in milliseconds (`None` when only freshness is known).
    pub elapsed_ms: Option<i64>,
    /// Actionable hint for this crate, if any.
    pub hint: Option<String>,
}

/// One regression row returned by `vox_db::VoxDb::query_build_regressions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionRow {
    /// Crate name.
    pub name: String,
    /// Elapsed time in the latest run (ms).
    pub elapsed_ms: i64,
    /// Rolling average across previous runs (ms).
    pub avg_ms: f64,
    /// `elapsed_ms / avg_ms` ratio (≥ 1.5 → regression).
    pub ratio: f64,
    /// Actionable hint for this crate.
    pub hint: Option<String>,
}
