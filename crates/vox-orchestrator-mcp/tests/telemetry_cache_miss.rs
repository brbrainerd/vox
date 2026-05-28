//! Integration test for the `orch.cache.miss` telemetry event.
//!
//! Owns its own test binary so the process-wide `set_global_recorder`
//! `OnceLock` write doesn't collide with the sibling `trace_propagation`
//! binary (each integration test file is its own binary).

use std::sync::{Arc, Mutex, OnceLock};

use vox_orchestrator_mcp::llm_bridge::emit_cache_miss_if_applicable;
use vox_telemetry::{
    METRIC_TYPE_ORCH_CACHE_MISS, TelemetryEvent, TelemetryRecorder, set_global_recorder,
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
fn cache_miss_emits_orch_cache_miss_event() {
    let rec = recorder();
    rec.events.lock().expect("mutex").clear();

    // Case 1: `cache_read_input_tokens = None` ⇒ miss.
    emit_cache_miss_if_applicable(
        "claude-opus-4-7",
        "Anthropic",
        "mcp_chat",
        1234,
        567,
        None,
        None,
        Some(42),
        "trace-abc",
    );

    // Case 2: `cache_read_input_tokens = Some(0)` ⇒ also miss.
    emit_cache_miss_if_applicable(
        "claude-opus-4-7",
        "Anthropic",
        "mcp_chat",
        100,
        50,
        Some(0),
        Some(0),
        Some(43),
        "trace-def",
    );

    // Case 3: `cache_read_input_tokens = Some(800)` ⇒ hit, no event.
    emit_cache_miss_if_applicable(
        "claude-opus-4-7",
        "Anthropic",
        "mcp_chat",
        1000,
        100,
        Some(800),
        Some(50),
        Some(44),
        "trace-ghi",
    );

    let events = rec.events.lock().expect("mutex");
    let misses: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            TelemetryEvent::ResearchMetric(r) if r.metric_type == METRIC_TYPE_ORCH_CACHE_MISS => {
                Some(r)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        misses.len(),
        2,
        "expected exactly 2 orch.cache.miss events (None + Some(0)); cache hit should not fire one. saw {events:?}"
    );

    let first = misses[0];
    assert_eq!(first.session_id, "mcp:claude-opus-4-7");
    let meta = first.metadata_json.as_deref().expect("metadata present");
    assert!(meta.contains("\"model\":\"claude-opus-4-7\""));
    assert!(meta.contains("\"tool\":\"mcp_chat\""));
    assert!(meta.contains("\"prompt_tokens\":1234"));
    assert!(meta.contains("\"trace_id\":\"trace-abc\""));
}
