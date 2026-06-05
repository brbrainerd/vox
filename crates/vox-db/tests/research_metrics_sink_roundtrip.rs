//! Round-trip tests for [`vox_db::telemetry_sink::ResearchMetricsSink`].
//!
//! Verifies that calling `record(event)` on the sink — the entry point the rest
//! of the workspace uses via `record_event!` — causes a row to land in
//! `research_metrics` for each handled `TelemetryEvent` variant. Without these
//! tests, the sink's dispatch logic (variant → serialize → write) is invisible
//! to CI and a regression would only surface in production via missing rows.
//!
//! `model_call_event_roundtrip.rs` exercises the underlying `append_research_metric`
//! API directly; this file exercises the sink layer that sits in front of it.

use std::sync::Arc;

use vox_db::telemetry_sink::ResearchMetricsSink;
use vox_db::{DbConfig, VoxDb};
use vox_telemetry::{
    BuildSummaryEvent, ErrorEvent, ModelCallEvent, ResearchMetricEvent, TaskRootSummaryEvent,
    TelemetryEvent, TelemetryRecorder,
};

/// Poll the DB for up to ~3 s, returning the rows whose `session_id` starts
/// with `session_id_prefix`. Sink writes are intentionally fire-and-forget so
/// tests need a deterministic way to observe them.
async fn wait_for_session_rows(
    db: &Arc<VoxDb>,
    session_id_prefix: &str,
) -> Vec<(String, String, Option<f64>, Option<String>)> {
    for _ in 0..60 {
        if let Ok(rows) = db
            .list_research_metrics_by_session(session_id_prefix, None, 10)
            .await
            && !rows.is_empty()
        {
            return rows;
        }
        tokio::time::sleep(vox_config::timeouts::D_50MS).await;
    }
    Vec::new()
}

async fn wait_for_type_rows(
    db: &Arc<VoxDb>,
    metric_type: &str,
) -> Vec<(String, Option<f64>, Option<String>)> {
    for _ in 0..60 {
        if let Ok(rows) = db.list_research_metrics_by_type(metric_type, "%", 10).await
            && !rows.is_empty()
        {
            return rows;
        }
        tokio::time::sleep(vox_config::timeouts::D_50MS).await;
    }
    Vec::new()
}

/// Build a fresh in-memory DB and a sink that writes to the same instance.
/// The sink takes a `VoxDb` value which it then wraps in its own Arc, so we
/// give it a clone of our handle.
async fn fresh_db_and_sink() -> (Arc<VoxDb>, ResearchMetricsSink) {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let db_arc = Arc::new(db);
    let sink = ResearchMetricsSink::new((*db_arc).clone());
    (db_arc, sink)
}

#[tokio::test(flavor = "multi_thread")]
async fn sink_writes_research_metric_event() {
    let (db, sink) = fresh_db_and_sink().await;
    let event = TelemetryEvent::ResearchMetric(ResearchMetricEvent {
        session_id: "sink-test-rm".into(),
        metric_type: "test.generic_metric".into(),
        metric_value: Some(42.0),
        metadata_json: Some(r#"{"k":"v"}"#.into()),
    });

    sink.record(&event);

    let rows = wait_for_session_rows(&db, "sink-test-rm").await;
    assert!(!rows.is_empty(), "ResearchMetric row never appeared");
    let (sid, mtype, mv, _meta) = &rows[0];
    assert_eq!(sid, "sink-test-rm");
    assert_eq!(mtype, "test.generic_metric");
    assert_eq!(*mv, Some(42.0));
}

#[tokio::test(flavor = "multi_thread")]
async fn sink_writes_model_call_event() {
    let (db, sink) = fresh_db_and_sink().await;
    let event = TelemetryEvent::ModelCall(ModelCallEvent {
        model: "claude-sonnet-4-6".into(),
        provider: "Anthropic".into(),
        route_profile: None,
        selection_rationale: None,
        prompt_tokens: 100,
        completion_tokens: 50,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        latency_ms: 250,
        cost_usd: 0.001,
        cost_source: "estimated".into(),
        error_class: None,
        retry_attempt: 0,
        task_id: Some(99),
        parent_task_id: None,
        trace_id: None,
        caller_agent_id: None,
    });

    sink.record(&event);

    let rows = wait_for_session_rows(&db, "model:99").await;
    assert!(!rows.is_empty(), "ModelCall row never appeared");
    let (_sid, mtype, _mv, meta) = &rows[0];
    assert_eq!(mtype, vox_telemetry::METRIC_TYPE_MODEL_CALL_EVENT);
    let meta = meta.as_deref().expect("metadata_json");
    assert!(meta.contains("claude-sonnet-4-6"), "metadata: {meta}");
}

#[tokio::test(flavor = "multi_thread")]
async fn sink_writes_task_root_summary() {
    let (db, sink) = fresh_db_and_sink().await;
    let event = TelemetryEvent::TaskRootSummary(TaskRootSummaryEvent {
        task_id: 7,
        trace_id: "trace-7".into(),
        repository_id: None,
        outcome: "completed".into(),
        wall_time_ms: 5000,
        total_input_tokens: 1000,
        total_output_tokens: 200,
        total_cost_usd: 0.025,
        child_call_count: 3,
        max_span_depth: 1,
        subagent_fanout: 0,
    });

    sink.record(&event);

    let rows = wait_for_session_rows(&db, "task:7").await;
    assert!(!rows.is_empty(), "TaskRootSummary row never appeared");
    let (_sid, mtype, _mv, _meta) = &rows[0];
    assert_eq!(mtype, vox_telemetry::METRIC_TYPE_TASK_ROOT_SUMMARY);
}

#[tokio::test(flavor = "multi_thread")]
async fn sink_writes_build_summary() {
    let (db, sink) = fresh_db_and_sink().await;
    let event = TelemetryEvent::BuildSummary(BuildSummaryEvent {
        build_id: "ci-build-42".into(),
        outcome: "success".into(),
        wall_time_ms: 60_000,
        crates_compiled: 104,
        error_count: 0,
        invocation_context: Some("ci".into()),
    });

    sink.record(&event);

    let rows = wait_for_type_rows(&db, vox_telemetry::METRIC_TYPE_BUILD_SUMMARY_EVENT).await;
    assert!(!rows.is_empty(), "BuildSummary row never appeared");
    let (_sid, _mv, meta) = &rows[0];
    let meta = meta.as_deref().expect("metadata_json");
    assert!(meta.contains("ci-build-42"), "metadata: {meta}");
}

#[tokio::test(flavor = "multi_thread")]
async fn sink_writes_error_event() {
    let (db, sink) = fresh_db_and_sink().await;
    let event = TelemetryEvent::Error(ErrorEvent {
        subsystem: "llm.http".into(),
        error_class: "rate-limited".into(),
        http_status: Some(429),
        retry_attempt: 1,
        retried: true,
        model: Some("claude-sonnet-4-6".into()),
        provider: Some("Anthropic".into()),
        task_id: Some(11),
        trace_id: None,
    });

    sink.record(&event);

    let rows = wait_for_type_rows(&db, vox_telemetry::METRIC_TYPE_ERROR_EVENT).await;
    assert!(!rows.is_empty(), "Error row never appeared");
    let (_sid, _mv, meta) = &rows[0];
    let meta = meta.as_deref().expect("metadata_json");
    assert!(meta.contains("rate-limited"), "metadata: {meta}");
}
