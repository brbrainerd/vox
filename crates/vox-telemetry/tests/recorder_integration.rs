//! Integration-style exercise of recorder wiring (`vox-telemetry`).

use std::sync::{Arc, Mutex};

use vox_telemetry::{
    CompositeRecorder, METRIC_TYPE_BENCHMARK_EVENT, NoOpRecorder, ResearchMetricEvent,
    SESSION_PREFIX_BENCH, TelemetryEvent, TelemetryRecorder, validate_research_metric_row,
};

struct CaptureRecorder {
    hits: Mutex<Vec<String>>,
}

impl TelemetryRecorder for CaptureRecorder {
    fn record(&self, event: &TelemetryEvent) {
        let mut g = self.hits.lock().expect("hits mutex");
        g.push(format!("{event:?}"));
    }
}

#[test]
fn validate_research_metric_row_accepts_benchmark_metric() {
    let sid = format!("{SESSION_PREFIX_BENCH}integration");
    assert!(validate_research_metric_row(&sid, METRIC_TYPE_BENCHMARK_EVENT, None).is_ok());
}

#[test]
fn composite_recorder_fanout_and_noop() {
    let cap = Arc::new(CaptureRecorder {
        hits: Mutex::new(Vec::new()),
    });
    let comp = CompositeRecorder::new(vec![
        Arc::new(NoOpRecorder) as Arc<dyn TelemetryRecorder>,
        cap.clone() as Arc<dyn TelemetryRecorder>,
    ]);

    let ev = TelemetryEvent::ResearchMetric(ResearchMetricEvent {
        session_id: format!("{SESSION_PREFIX_BENCH}fanout"),
        metric_type: METRIC_TYPE_BENCHMARK_EVENT.into(),
        metric_value: Some(2.0),
        metadata_json: None,
    });
    comp.record(&ev);

    let n = cap.hits.lock().unwrap().len();
    assert_eq!(n, 1, "expected capture recorder to observe one event");
}
