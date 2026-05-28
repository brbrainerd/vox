//! Integration coverage for validation helpers and stable metric-type constants.

use vox_telemetry::{
    METRIC_TYPE_BENCHMARK_EVENT, ResearchMetricEvent, TelemetryEvent, validate_research_metric_row,
};

#[test]
fn validate_research_metric_row_accepts_well_formed_row() {
    validate_research_metric_row(
        "bench:smoke",
        METRIC_TYPE_BENCHMARK_EVENT,
        Some(r#"{"k":"v"}"#),
    )
    .expect("valid row");
}

#[test]
fn validate_research_metric_row_rejects_empty_session() {
    let err = validate_research_metric_row("", METRIC_TYPE_BENCHMARK_EVENT, None)
        .expect_err("empty session_id must fail");
    assert!(
        err.to_string().contains("session_id"),
        "unexpected error: {err}"
    );
}

#[test]
fn research_metric_event_constructible_cross_module() {
    let ev = TelemetryEvent::ResearchMetric(ResearchMetricEvent {
        session_id: "mcp:int".into(),
        metric_type: METRIC_TYPE_BENCHMARK_EVENT.into(),
        metric_value: Some(1.0),
        metadata_json: None,
    });
    match ev {
        TelemetryEvent::ResearchMetric(inner) => {
            assert_eq!(inner.metric_type, METRIC_TYPE_BENCHMARK_EVENT);
        }
        _ => panic!("wrong variant"),
    }
}
