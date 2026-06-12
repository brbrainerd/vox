//! Plugin lifecycle telemetry events. Per
//! docs/src/architecture/telemetry-trust-ssot.md, emitted via tracing.
//!
//! Plugin **load failures** additionally emit a
//! [`METRIC_TYPE_PLUGIN_LOAD_FAILURE`]
//! `research_metrics` row through the global telemetry recorder so the failure
//! is visible in offline analysis (the tracing line alone is not durable).

use tracing::info;
use vox_telemetry::{
    METRIC_TYPE_PLUGIN_LOAD_FAILURE, ResearchMetricEvent, TelemetryEvent, record_event,
};

pub fn discovered(id: &str, version: &str, payload_kind: &str, abi_or_format_version: u32) {
    info!(
        event = "plugin.discovered",
        id, version, payload_kind, abi_or_format_version,
    );
}

pub fn loaded(id: &str, version: &str, payload_kind: &str, load_ms: u128) {
    info!(event = "plugin.loaded", id, version, payload_kind, load_ms = %load_ms);
}

pub fn load_failed(id: &str, version: &str, error_kind: &str) {
    info!(event = "plugin.load_failed", id, version, error_kind);
    let metadata_json = serde_json::json!({
        "plugin_id": id,
        "plugin_version": version,
        "error_kind": error_kind,
    })
    .to_string();
    record_event!(&TelemetryEvent::ResearchMetric(ResearchMetricEvent {
        session_id: format!("plugin:{id}"),
        metric_type: METRIC_TYPE_PLUGIN_LOAD_FAILURE.into(),
        metric_value: None,
        metadata_json: Some(metadata_json),
    }));
}

pub fn abi_mismatch(id: &str, plugin_abi: u32, host_abi: u32) {
    info!(event = "plugin.abi_mismatch", id, plugin_abi, host_abi);
    let metadata_json = serde_json::json!({
        "plugin_id": id,
        "error_kind": "abi_mismatch",
        "plugin_abi": plugin_abi,
        "host_abi": host_abi,
    })
    .to_string();
    record_event!(&TelemetryEvent::ResearchMetric(ResearchMetricEvent {
        session_id: format!("plugin:{id}"),
        metric_type: METRIC_TYPE_PLUGIN_LOAD_FAILURE.into(),
        metric_value: None,
        metadata_json: Some(metadata_json),
    }));
}
