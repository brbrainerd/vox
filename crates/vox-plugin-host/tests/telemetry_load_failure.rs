//! Integration test for the `plugin.load_failure` telemetry event.
//!
//! Owns its own test binary so the process-wide `set_global_recorder`
//! `OnceLock` write doesn't collide with siblings (per `vox-telemetry`'s
//! first-writer-wins contract).

use std::sync::{Arc, Mutex, OnceLock};

use vox_plugin_host::telemetry;
use vox_telemetry::{
    METRIC_TYPE_PLUGIN_LOAD_FAILURE, TelemetryEvent, TelemetryRecorder, set_global_recorder,
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

#[test]
fn load_failed_emits_plugin_load_failure_event() {
    let rec = recorder();
    // Drain any prior events to keep this test independent.
    rec.events.lock().expect("mutex").clear();

    telemetry::load_failed("acme-codegen", "1.2.3", "init");
    telemetry::abi_mismatch("acme-codegen", 7, 9);

    let events = rec.events.lock().expect("mutex");
    let load_failure_rows: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            TelemetryEvent::ResearchMetric(r) if r.metric_type == METRIC_TYPE_PLUGIN_LOAD_FAILURE => Some(r),
            _ => None,
        })
        .collect();
    assert_eq!(
        load_failure_rows.len(),
        2,
        "expected exactly 2 plugin.load_failure events (init + abi_mismatch); saw {events:?}"
    );

    // First event: init failure.
    let init = load_failure_rows[0];
    assert_eq!(init.session_id, "plugin:acme-codegen");
    let init_meta = init.metadata_json.as_deref().expect("metadata present");
    assert!(init_meta.contains("\"plugin_id\":\"acme-codegen\""));
    assert!(init_meta.contains("\"error_kind\":\"init\""));
    assert!(init_meta.contains("\"plugin_version\":\"1.2.3\""));

    // Second event: ABI mismatch carries plugin_abi / host_abi.
    let abi = load_failure_rows[1];
    let abi_meta = abi.metadata_json.as_deref().expect("metadata present");
    assert!(abi_meta.contains("\"error_kind\":\"abi_mismatch\""));
    assert!(abi_meta.contains("\"plugin_abi\":7"));
    assert!(abi_meta.contains("\"host_abi\":9"));
}
