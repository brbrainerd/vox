//! Integration test for the `sandbox.timeout_kill` telemetry event.
//!
//! Owns its own test binary so the process-wide `set_global_recorder`
//! `OnceLock` write doesn't collide with siblings.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use vox_actor_runtime::activity::{
    ActivityError, ActivityOptions, ActivityResult, execute_activity,
};
use vox_telemetry::{
    METRIC_TYPE_SANDBOX_TIMEOUT_KILL, TelemetryEvent, TelemetryRecorder, set_global_recorder,
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

#[tokio::test]
async fn terminal_timeout_emits_sandbox_timeout_kill_event() {
    let rec = recorder();
    rec.events.lock().expect("mutex").clear();

    let opts = ActivityOptions::new().with_timeout(Duration::from_millis(5));
    let result = execute_activity::<_, _, (), &str>("hang-forever", &opts, || async {
        tokio::time::sleep(vox_config::timeouts::D_60S).await;
        Ok(())
    })
    .await;
    assert!(
        matches!(result, ActivityResult::Failed(ActivityError::Timeout(_))),
        "expected Timeout result, got {result:?}"
    );

    let events = rec.events.lock().expect("mutex");
    let kills: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            TelemetryEvent::ResearchMetric(r)
                if r.metric_type == METRIC_TYPE_SANDBOX_TIMEOUT_KILL =>
            {
                Some(r)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        kills.len(),
        1,
        "expected exactly 1 sandbox.timeout_kill event; saw {events:?}"
    );
    let kill = kills[0];
    assert_eq!(kill.session_id.split(':').next(), Some("sandbox"));
    let meta = kill.metadata_json.as_deref().expect("metadata present");
    assert!(meta.contains("\"activity_name\":\"hang-forever\""));
    assert!(meta.contains("\"terminal\":true"));
    assert!(meta.contains("\"attempt\":1"));
}
