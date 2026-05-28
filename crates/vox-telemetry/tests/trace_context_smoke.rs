//! Cross-module smoke for `span` + `config` surfaces (`vox-telemetry`).
//!
//! Exercises `TraceContext` parent/child propagation and the all-off
//! [`TelemetryConfig`] preset without relying on any process-wide env var
//! state — these surfaces sit on the L1 facade boundary and must remain
//! callable by every emitter without a runtime.

use vox_telemetry::{TelemetryConfig, TraceContext};

#[test]
fn trace_context_child_inherits_trace_id_and_increments_depth() {
    let root = TraceContext::root(42);
    assert_eq!(root.task_id, Some(42));
    assert!(root.parent_task_id.is_none());
    assert_eq!(root.span_depth, 0);

    let child = root.child(43, "agent.beta");
    assert_eq!(child.task_id, Some(43));
    assert_eq!(child.parent_task_id, Some(42));
    assert_eq!(child.trace_id, root.trace_id, "trace id must propagate");
    assert_eq!(child.span_depth, 1);
    assert_eq!(child.caller_agent_id.as_deref(), Some("agent.beta"));

    let grandchild = child.child(44, "agent.gamma");
    assert_eq!(grandchild.parent_task_id, Some(43));
    assert_eq!(grandchild.span_depth, 2);
    assert_eq!(grandchild.trace_id, root.trace_id);
}

#[test]
fn telemetry_config_all_off_is_fully_disabled() {
    let cfg = TelemetryConfig::all_off();
    assert!(!cfg.enabled);
    assert!(!cfg.remote_upload);
    assert!(!cfg.research_metrics);
    assert!(!cfg.model_calls);
    assert!(!cfg.agent_orchestration);
    assert!(!cfg.build);
    assert!(!cfg.errors);
}

#[test]
fn telemetry_config_default_on_keeps_remote_upload_off() {
    let cfg = TelemetryConfig::default_on();
    assert!(cfg.enabled);
    assert!(
        !cfg.remote_upload,
        "ADR 023: remote upload must be opt-in even when all categories are on"
    );
    assert!(cfg.research_metrics && cfg.model_calls && cfg.errors);
}
