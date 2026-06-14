//! `vox-telemetry` — L1 telemetry facade.
//!
//! Zero domain dependencies. Every emitter in the workspace depends on this crate
//! for the `record_event!` macro and `METRIC_TYPE_*` constants. Sinks live in
//! higher-layer crates (`vox-db`, `vox-cli`) and register themselves at binary
//! startup via [`set_global_recorder`].
//!
//! # Quick start for producers
//!
//! ```rust,ignore
//! use vox_telemetry::{record_event, TelemetryEvent, ResearchMetricEvent, METRIC_TYPE_BENCHMARK_EVENT};
//!
//! record_event!(&TelemetryEvent::ResearchMetric(ResearchMetricEvent {
//!     session_id: "bench:myrepo".into(),
//!     metric_type: METRIC_TYPE_BENCHMARK_EVENT.into(),
//!     metric_value: Some(42.0),
//!     metadata_json: None,
//! }));
//! ```
//!
//! If no recorder is registered, `record_event!` is a no-op.

pub mod aggregator;
pub mod config;
pub mod no_op;
pub mod recorder;
pub mod semcov_wave27_tests;
pub mod span;
pub mod types;

// ── Public re-exports ─────────────────────────────────────────────────────

pub use aggregator::{fill_task_root_summary, record_task_started};
pub use config::{TelemetryConfig, is_master_enabled, org_policy_disabled};
pub use no_op::NoOpRecorder;
pub use recorder::{CompositeRecorder, TelemetryRecorder, global_recorder, set_global_recorder};
pub use span::{TRACE_CTX, TraceContext, current_trace_ctx};
pub use types::{
    // event types
    AiFixtureEvent,
    // vox audit effort (S1) — `audit.effort.*` events
    AuditEffortCommitJudgedEvent,
    AuditEffortRunCompletedEvent,
    AuditEffortRunFailedEvent,
    AuditEffortRunStartedEvent,
    // vox audit effort-route (S2) — `audit.route.*` events
    AuditRouteClusterDecidedEvent,
    AuditRouteRunCompletedEvent,
    AuditRouteRunFailedEvent,
    AuditRouteRunStartedEvent,
    // vox audit run telemetry (A11)
    AuditRunEvent,
    BuildSummaryEvent,
    // model-autonomic system (L0/L1/L2/L3) — 2026-05-15
    ClassificationEvent,
    ConfidencePromotionEvent,
    DiscoveryEvent,
    ErrorEvent,
    FixtureModelIntentResolvedEvent,
    HoleObservedTelemetryEvent,
    // CR-L8 corpus-feedback (P2.1)
    LintAutofixEvent,
    LintFindingEvent,
    METRIC_TYPE_AGENT_EXEC_TIME,
    // existing metric types
    METRIC_TYPE_AGENTOS_GUARDRAIL_DENY,
    // vox audit effort (S1) — `audit.effort.*` metric_type constants
    METRIC_TYPE_AUDIT_EFFORT_COMMIT_JUDGED,
    METRIC_TYPE_AUDIT_EFFORT_RUN_COMPLETED,
    METRIC_TYPE_AUDIT_EFFORT_RUN_FAILED,
    METRIC_TYPE_AUDIT_EFFORT_RUN_STARTED,
    // vox audit effort-route (S2) — `audit.route.*` metric_type constants
    METRIC_TYPE_AUDIT_ROUTE_CLUSTER_DECIDED,
    METRIC_TYPE_AUDIT_ROUTE_RUN_COMPLETED,
    METRIC_TYPE_AUDIT_ROUTE_RUN_FAILED,
    METRIC_TYPE_AUDIT_ROUTE_RUN_STARTED,
    METRIC_TYPE_AUDIT_RUN,
    METRIC_TYPE_BANDIT_UPDATE,
    METRIC_TYPE_BENCHMARK_EVENT,
    METRIC_TYPE_BUDGET_DECISION,
    // new metric types (Phase B–D emit sites)
    METRIC_TYPE_BUILD_SUMMARY_EVENT,
    METRIC_TYPE_CACHE_HIT_PREDICTION,
    METRIC_TYPE_CALIBRATION_RUN,
    METRIC_TYPE_CHAIN_DEPTH_ALERT,
    METRIC_TYPE_CIRCUIT_BREAKER_TRIP,
    METRIC_TYPE_CONFIDENCE_PROMOTION,
    METRIC_TYPE_DRIFT_ALERT,
    METRIC_TYPE_ERROR_EVENT,
    METRIC_TYPE_FIXTURE_HOLE_OBSERVED,
    METRIC_TYPE_FIXTURE_MODEL_INTENT,
    METRIC_TYPE_FIXTURE_PROMPT_DISPATCH,
    METRIC_TYPE_FIXTURE_SEARCH_DISPATCH,
    METRIC_TYPE_HITL_INTERRUPT,
    METRIC_TYPE_LINT_AUTOFIX_APPLIED,
    METRIC_TYPE_LINT_AUTOFIX_REJECTED,
    METRIC_TYPE_LINT_FINDING,
    METRIC_TYPE_MEMORY_HYBRID_FUSION,
    METRIC_TYPE_MODEL_CALL_EVENT,
    METRIC_TYPE_MODEL_CLASSIFICATION,
    METRIC_TYPE_MODEL_DISCOVERY,
    METRIC_TYPE_MODEL_ROUTE_EVENT,
    METRIC_TYPE_MODEL_TIER_ROUTE,
    METRIC_TYPE_ORCH_CACHE_MISS,
    METRIC_TYPE_ORCH_TASK_CANCELLED,
    METRIC_TYPE_PLAN_MODE_DECISION,
    METRIC_TYPE_PLUGIN_LOAD_FAILURE,
    METRIC_TYPE_POPULI_CONTROL_EVENT,
    METRIC_TYPE_PRIVACY_ROUTE_DECISION,
    METRIC_TYPE_QUESTIONING_EVENT,
    METRIC_TYPE_REPAIR_ATTEMPT,
    METRIC_TYPE_REPAIR_OUTCOME,
    METRIC_TYPE_RISK_SCORE,
    METRIC_TYPE_SANDBOX_TIMEOUT_KILL,
    METRIC_TYPE_SELECTION_DECISION,
    METRIC_TYPE_SOCRATES_FUSION,
    METRIC_TYPE_SOCRATES_SURFACE,
    METRIC_TYPE_SUBAGENT_DISPATCH,
    METRIC_TYPE_SYNTAX_K_EVENT,
    METRIC_TYPE_TASK_ROOT_SUMMARY,
    METRIC_TYPE_WORKFLOW_JOURNAL_ENTRY,
    ModelCallEvent,
    OrchSubagentDispatchEvent,
    PromptDispatchTelemetryEvent,
    // size limits
    RESEARCH_METRICS_METADATA_JSON_MAX_BYTES,
    RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS,
    RESEARCH_METRICS_SESSION_ID_MAX_CHARS,
    RepairAttemptEvent,
    RepairOutcomeEvent,
    ResearchMetricEvent,
    // session prefixes
    SESSION_ID_MEMORY_HYBRID_FUSION,
    SESSION_PREFIX_AUDIT,
    SESSION_PREFIX_BENCH,
    SESSION_PREFIX_LINT,
    SESSION_PREFIX_MCP,
    SESSION_PREFIX_MENS,
    SESSION_PREFIX_MODEL_AUTONOMIC,
    SESSION_PREFIX_REPAIR,
    SESSION_PREFIX_ROUTE,
    SESSION_PREFIX_SYNTAXK,
    SESSION_PREFIX_WORKFLOW,
    SearchDispatchTelemetryEvent,
    SelectionDecisionEvent,
    SubagentDispatchTelemetryPayload,
    TaskRootSummaryEvent,
    // error
    TelemetryError,
    TelemetryEvent,
    // write helpers
    TelemetryWriteOptions,
    model_call_event_to_gen_ai_attributes,
    validate_research_metric_row,
};

// ── record_event! macro ───────────────────────────────────────────────────

/// Emit a telemetry event through the global recorder.
///
/// No-op (zero cost) when no recorder has been registered via [`set_global_recorder`].
///
/// ```text
/// record_event!(&TelemetryEvent::ResearchMetric(ResearchMetricEvent { ... }));
/// ```
#[macro_export]
macro_rules! record_event {
    ($event:expr) => {
        if let Some(r) = $crate::global_recorder() {
            r.record($event);
        }
    };
}
