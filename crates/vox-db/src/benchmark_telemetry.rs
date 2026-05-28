//! Unified benchmark telemetry via `research_metrics` (no extra DDL).
//!
//! Session key pattern: `bench:<repository_id>`; metric type: `benchmark_event`.
//! Serialized `BenchmarkEventMeta` is subject to `crate::research_metrics_contract::RESEARCH_METRICS_METADATA_JSON_MAX_BYTES`
//! via [`crate::VoxDb::append_research_metric`] → [`crate::research_metrics_contract::validate_research_metric_row`].

use serde::Serialize;

use crate::research_metrics_contract::{METRIC_TYPE_BENCHMARK_EVENT, TelemetryWriteOptions};
use crate::{StoreError, VoxDb};

/// Payload for one benchmark observation.
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkEventMeta {
    /// Logical benchmark name (`vox_run_warm`, `populi_fim_p95`, `eval_local`, `build_timings`, ...).
    pub name: String,
    /// Repository id from `vox-repository` when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,
    /// Unit for `metric_value` when set (`seconds`, `milliseconds`, `ratio`, …). See telemetry SSOT doc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_value_unit: Option<String>,
    /// Free-form details (durations, pass rates, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl VoxDb {
    /// Append one benchmark row to `research_metrics` under `bench:<repository_id>`.
    pub async fn record_benchmark_event(
        &self,
        repository_id: &str,
        name: &str,
        metric_value: Option<f64>,
        metric_value_unit: Option<&str>,
        details: Option<serde_json::Value>,
    ) -> Result<i64, StoreError> {
        let meta = BenchmarkEventMeta {
            name: name.to_string(),
            repository_id: Some(repository_id.to_string()),
            metric_value_unit: metric_value_unit.map(String::from),
            details,
        };
        let json =
            serde_json::to_string(&meta).map_err(|e| StoreError::Serialization(e.to_string()))?;
        let tw = TelemetryWriteOptions::new(repository_id);
        self.append_research_metric(
            &tw.session_bench(),
            METRIC_TYPE_BENCHMARK_EVENT,
            metric_value,
            Some(&json),
        )
        .await
    }
}
