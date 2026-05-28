//! Integration test for the `orch.task.cancelled` telemetry event.
//!
//! Owns its own test binary so the process-wide `set_global_recorder`
//! `OnceLock` write doesn't collide with siblings.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use vox_orchestrator::config::OrchestratorConfig;
use vox_orchestrator::orchestrator::Orchestrator;
use vox_orchestrator::types::FileAffinity;
use vox_telemetry::{
    METRIC_TYPE_ORCH_TASK_CANCELLED, TelemetryEvent, TelemetryRecorder, set_global_recorder,
};

#[derive(Default)]
struct CapturingRecorder {
    events: Mutex<Vec<TelemetryEvent>>,
}

impl TelemetryRecorder for CapturingRecorder {
    fn record(&self, event: &TelemetryEvent) {
        self.events.lock().expect("mutex").push(event.clone());
    }
}

fn recorder() -> &'static Arc<CapturingRecorder> {
    static INNER: OnceLock<Arc<CapturingRecorder>> = OnceLock::new();
    INNER.get_or_init(|| {
        let r = Arc::new(CapturingRecorder::default());
        set_global_recorder(r.clone());
        r
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_task_emits_orch_task_cancelled_event() {
    let rec = recorder();
    rec.events.lock().expect("mutex").clear();

    let orch = Orchestrator::new(OrchestratorConfig::for_testing());
    let path = Path::new("state_inv/telemetry_cancel.rs");
    let tid = orch
        .submit_task(
            "cancel-me",
            vec![FileAffinity::write(path)],
            None,
            None,
            None,
        )
        .await
        .expect("submit");
    orch.cancel_task(tid).expect("cancel");

    let events = rec.events.lock().expect("mutex");
    let cancellations: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            TelemetryEvent::ResearchMetric(r)
                if r.metric_type == METRIC_TYPE_ORCH_TASK_CANCELLED =>
            {
                Some(r)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        cancellations.len(),
        1,
        "expected exactly 1 orch.task.cancelled event; saw {events:?}"
    );
    let row = cancellations[0];
    assert_eq!(row.session_id, format!("orch:task:{}", tid.0));
    let meta = row.metadata_json.as_deref().expect("metadata present");
    assert!(meta.contains(&format!("\"task_id\":{}", tid.0)));
    assert!(meta.contains("\"path\":\"queue\""));
}
