//! [`ResearchMetricsSink`] — writes telemetry events to the `research_metrics` table
//! via [`crate::VoxDb::append_research_metric`].
//!
//! Handles `ResearchMetric`, `ModelCall`, `TaskRootSummary`, `BuildSummary`, and `Error` variants;
//! unknown variants are ignored.

use std::sync::Arc;

use vox_telemetry::{
    AiFixtureEvent, BuildSummaryEvent, ErrorEvent, METRIC_TYPE_BUILD_SUMMARY_EVENT,
    METRIC_TYPE_ERROR_EVENT, METRIC_TYPE_FIXTURE_HOLE_OBSERVED, METRIC_TYPE_FIXTURE_MODEL_INTENT,
    METRIC_TYPE_FIXTURE_PROMPT_DISPATCH, METRIC_TYPE_FIXTURE_SEARCH_DISPATCH,
    METRIC_TYPE_MODEL_CALL_EVENT, METRIC_TYPE_SUBAGENT_DISPATCH, METRIC_TYPE_TASK_ROOT_SUMMARY,
    ModelCallEvent, ResearchMetricEvent, TaskRootSummaryEvent, TelemetryEvent, TelemetryRecorder,
};

/// `TelemetryRecorder` sink backed by a live `VoxDb` connection.
///
/// `record` spawns a background tokio task for the async DB write so the caller
/// is never blocked. Write failures are logged at WARN and swallowed — telemetry
/// must never surface as a user-visible error.
pub struct ResearchMetricsSink {
    db: Arc<crate::VoxDb>,
}

impl ResearchMetricsSink {
    pub fn new(db: crate::VoxDb) -> Self {
        Self { db: Arc::new(db) }
    }
}

impl TelemetryRecorder for ResearchMetricsSink {
    fn record(&self, event: &TelemetryEvent) {
        match event {
            TelemetryEvent::ResearchMetric(e) => {
                let db = Arc::clone(&self.db);
                let e: ResearchMetricEvent = e.clone();
                tokio::spawn(async move {
                    if let Err(err) = db
                        .append_research_metric(
                            &e.session_id,
                            &e.metric_type,
                            e.metric_value,
                            e.metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: append_research_metric failed");
                    }
                });
            }
            TelemetryEvent::ModelCall(e) => {
                let db = Arc::clone(&self.db);
                let e: ModelCallEvent = e.clone();
                tokio::spawn(async move {
                    let session_id = format!(
                        "model:{}",
                        e.task_id.map(|id| id.to_string()).unwrap_or_default()
                    );
                    let metadata_json = serde_json::to_string(&e).ok();
                    if let Err(err) = db
                        .append_research_metric(
                            &session_id,
                            METRIC_TYPE_MODEL_CALL_EVENT,
                            Some(e.cost_usd),
                            metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: ModelCall write failed");
                    }
                });
            }
            TelemetryEvent::TaskRootSummary(e) => {
                let db = Arc::clone(&self.db);
                let e: TaskRootSummaryEvent = e.clone();
                tokio::spawn(async move {
                    let session_id = format!("task:{}", e.task_id);
                    let metadata_json = match serde_json::to_string(&e) {
                        Ok(s) => Some(s),
                        Err(err) => {
                            tracing::warn!(
                                ?err,
                                "ResearchMetricsSink: task_root_summary serialize failed"
                            );
                            return;
                        }
                    };
                    if let Err(err) = db
                        .append_research_metric(
                            &session_id,
                            METRIC_TYPE_TASK_ROOT_SUMMARY,
                            Some(e.total_cost_usd),
                            metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: task_root_summary write failed");
                    }
                });
            }
            TelemetryEvent::BuildSummary(e) => {
                let db = Arc::clone(&self.db);
                let e: BuildSummaryEvent = e.clone();
                tokio::spawn(async move {
                    let session_id = format!("build:{}", e.build_id);
                    let metadata_json = serde_json::to_string(&e).ok();
                    if let Err(err) = db
                        .append_research_metric(
                            &session_id,
                            METRIC_TYPE_BUILD_SUMMARY_EVENT,
                            None,
                            metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: BuildSummary write failed");
                    }
                });
            }
            TelemetryEvent::Error(e) => {
                let db = Arc::clone(&self.db);
                let e: ErrorEvent = e.clone();
                tokio::spawn(async move {
                    let session_id = format!("error:{}", e.subsystem);
                    let metadata_json = serde_json::to_string(&e).ok();
                    if let Err(err) = db
                        .append_research_metric(
                            &session_id,
                            METRIC_TYPE_ERROR_EVENT,
                            None,
                            metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: Error write failed");
                    }
                });
            }
            TelemetryEvent::AiFixture(ev) => {
                let db = Arc::clone(&self.db);
                let ev = ev.clone();
                tokio::spawn(async move {
                    let (session_id, metric_type, metadata_json) = match &ev {
                        AiFixtureEvent::ModelIntent(m) => {
                            let sid = format!(
                                "fixture:intent:{}",
                                m.trace_id.as_deref().unwrap_or("unknown")
                            );
                            let meta = serde_json::to_string(&ev).ok();
                            (sid, METRIC_TYPE_FIXTURE_MODEL_INTENT, meta)
                        }
                        AiFixtureEvent::SubagentDispatch(p) => {
                            let sid = format!(
                                "fixture:subagent:{}",
                                p.session_id.as_deref().unwrap_or("unknown")
                            );
                            let meta = serde_json::to_string(&ev).ok();
                            (sid, METRIC_TYPE_SUBAGENT_DISPATCH, meta)
                        }
                        AiFixtureEvent::PromptDispatch(p) => {
                            let sid = format!("fixture:prompt:{}", p.stage);
                            let meta = serde_json::to_string(&ev).ok();
                            (sid, METRIC_TYPE_FIXTURE_PROMPT_DISPATCH, meta)
                        }
                        AiFixtureEvent::SearchDispatch(s) => {
                            let sid = format!("fixture:search:{}", s.corpus);
                            let meta = serde_json::to_string(&ev).ok();
                            (sid, METRIC_TYPE_FIXTURE_SEARCH_DISPATCH, meta)
                        }
                        AiFixtureEvent::HoleObserved(h) => {
                            let sid = format!("fixture:hole:{}", h.cache_key);
                            let meta = serde_json::to_string(&ev).ok();
                            (sid, METRIC_TYPE_FIXTURE_HOLE_OBSERVED, meta)
                        }
                    };
                    if let Err(err) = db
                        .append_research_metric(
                            &session_id,
                            metric_type,
                            None,
                            metadata_json.as_deref(),
                        )
                        .await
                    {
                        tracing::warn!(?err, "ResearchMetricsSink: AiFixture write failed");
                    }
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_metrics_sink_is_recorder() {
        fn _assert_recorder<T: vox_telemetry::TelemetryRecorder>() {}
        _assert_recorder::<ResearchMetricsSink>();
    }
}
