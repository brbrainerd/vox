//! Canonical telemetry constants, event types, and validation helpers.
//!
//! All `METRIC_TYPE_*` constants and [`validate_research_metric_row`] previously
//! lived in `vox-db::research_metrics_contract`. That module now re-exports
//! everything from here so downstream code continues to compile unchanged.
//!
//! - Human contract: `docs/src/reference/telemetry-metric-contract.md`
//! - Sensitivity classes (S0–S3): `docs/src/architecture/telemetry-retention-sensitivity-ssot.md`
//! - Taxonomy / owners: `docs/src/architecture/telemetry-taxonomy-contracts-ssot.md`
//! - Design: `docs/src/architecture/telemetry-unification-design-2026.md`

// ── TelemetryError ────────────────────────────────────────────────────────

/// Error returned by [`validate_research_metric_row`] and sink operations.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("telemetry validation: {0}")]
    Validation(String),
}

// ── Payload size and field length limits ────────────────────────────────────

/// Upper bound on `metadata_json` payload size.
pub const RESEARCH_METRICS_METADATA_JSON_MAX_BYTES: usize = 256 * 1024;
/// `session_id` length cap.
pub const RESEARCH_METRICS_SESSION_ID_MAX_CHARS: usize = 512;
/// `metric_type` length cap.
pub const RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS: usize = 128;

// ── Canonical `metric_type` values — existing (S0–S2) ────────────────────────

/// S0 — Coarse benchmark names, timings, ratios.
pub const METRIC_TYPE_BENCHMARK_EVENT: &str = "benchmark_event";
/// S0–S1 — Compiler / fixture ids.
pub const METRIC_TYPE_SYNTAX_K_EVENT: &str = "syntax_k_event";
/// S1 (payload may reach S2) — MCP surface calibration.
pub const METRIC_TYPE_SOCRATES_SURFACE: &str = "socrates_surface";
/// S2 — BM25/vector fusion stats.
pub const METRIC_TYPE_MEMORY_HYBRID_FUSION: &str = "memory_hybrid_fusion";
/// S1–S2 — Durable workflow journal JSON.
pub const METRIC_TYPE_WORKFLOW_JOURNAL_ENTRY: &str = "workflow_journal_entry";
/// S1 — Mesh/registry audit; must not store bearer tokens.
pub const METRIC_TYPE_POPULI_CONTROL_EVENT: &str = "populi_control_event";
/// S1–S2 — Questioning KPIs.
pub const METRIC_TYPE_QUESTIONING_EVENT: &str = "questioning_event";
/// S1 — Tool execution tracking (duration, timeouts, compute/cost).
pub const METRIC_TYPE_AGENT_EXEC_TIME: &str = "agent_exec_time";
/// S1 — Routing policy / capability-gate events from runtime/orchestrator.
pub const METRIC_TYPE_MODEL_ROUTE_EVENT: &str = "model_route_event";

// ── Orchestrator policy decision metrics (D1–D10) — S1 ────────────────────

/// D6: Circuit breaker trip — doom-loop detected.
pub const METRIC_TYPE_CIRCUIT_BREAKER_TRIP: &str = "orch.circuit_breaker.trip";
/// D3: Socrates confidence fusion decision.
pub const METRIC_TYPE_SOCRATES_FUSION: &str = "orch.socrates.fusion";
/// D1: Model tier routing decision (Economy / Standard / Strong).
pub const METRIC_TYPE_MODEL_TIER_ROUTE: &str = "orch.routing.tier";
/// D2: Plan-mode vs. ReAct mode decision.
pub const METRIC_TYPE_PLAN_MODE_DECISION: &str = "orch.plan.mode_decision";
/// D9: HITL interrupt — human-in-the-loop escalation triggered.
pub const METRIC_TYPE_HITL_INTERRUPT: &str = "orch.hitl.interrupt";
/// D5: Risk matrix score event.
pub const METRIC_TYPE_RISK_SCORE: &str = "orch.risk.score";
/// D8: Privacy routing decision.
pub const METRIC_TYPE_PRIVACY_ROUTE_DECISION: &str = "orch.privacy.route_decision";
/// D7: Cache hit prediction for provider routing.
pub const METRIC_TYPE_CACHE_HIT_PREDICTION: &str = "orch.cache.hit_prediction";
/// D7: Budget gate decision (Proceed / Downgrade / Halt).
pub const METRIC_TYPE_BUDGET_DECISION: &str = "orch.budget.decision";
/// D10: Calibration loop run observation.
pub const METRIC_TYPE_CALIBRATION_RUN: &str = "orch.calibration.run";
/// D10: Calibration drift alert — predicted vs. observed confidence diverging.
pub const METRIC_TYPE_DRIFT_ALERT: &str = "orch.calibration.drift_alert";
/// D10: Contextual bandit arm update.
pub const METRIC_TYPE_BANDIT_UPDATE: &str = "orch.calibration.bandit_update";
/// D4: Sub-agent dispatched.
pub const METRIC_TYPE_SUBAGENT_DISPATCH: &str = "orch.subagent.dispatch";
/// D4: Sub-agent chain depth exceeded configured cap.
pub const METRIC_TYPE_CHAIN_DEPTH_ALERT: &str = "orch.subagent.chain_depth_alert";
/// S1 — AgentOS guardrail denied a tool preflight (mutation / destructive heuristic).
pub const METRIC_TYPE_AGENTOS_GUARDRAIL_DENY: &str = "orch.agentos.guardrail_deny";

// ── New metric types (Phase B–D) — defined here, used when emit sites are added ─

/// S1 — Per-LLM-call record: tokens, cache, latency, cost, error class. (Phase B)
pub const METRIC_TYPE_MODEL_CALL_EVENT: &str = "model_call_event";
/// S1 — Top-level task completion rollup: total tokens, cost, wall time, depth. (Phase C)
pub const METRIC_TYPE_TASK_ROOT_SUMMARY: &str = "task.root_summary";
/// S0 — Build summary mirrored from build_run after `vox ci build-timings`. (Phase D)
pub const METRIC_TYPE_BUILD_SUMMARY_EVENT: &str = "build.summary";
/// S1 — Generic subsystem error / retry event. (Phase D)
pub const METRIC_TYPE_ERROR_EVENT: &str = "telemetry.error";

/// S1 — `vox-plugin-host` failed to load a cdylib plugin (missing symbol, ABI
/// mismatch, panic at init). Emitted via `ResearchMetric` envelope; payload
/// shape is documented at the emit site in `vox-plugin-host::loader`.
pub const METRIC_TYPE_PLUGIN_LOAD_FAILURE: &str = "plugin.load_failure";

/// S1 — `vox-actor-runtime` sandbox / activity terminated a run after exceeding
/// its configured timeout. Distinct from `agent_exec_time` (which only tracks
/// successful-or-failed durations, not timeout-terminated runs). Emitted via
/// `ResearchMetric` envelope; payload shape is documented at the emit site
/// in `vox-actor-runtime::activity`.
pub const METRIC_TYPE_SANDBOX_TIMEOUT_KILL: &str = "sandbox.timeout_kill";

/// S1 — LLM-provider prompt-cache lookup missed (the call ran without a cache
/// hit). Counterpart to `cache_read_input_tokens` on `ModelCallEvent` so miss
/// rates can be computed online instead of offline. Emitted via `ResearchMetric`
/// envelope from `vox-orchestrator-mcp::llm_bridge::infer`.
pub const METRIC_TYPE_ORCH_CACHE_MISS: &str = "orch.cache.miss";

/// S1 — A task was explicitly cancelled mid-execution (user interrupt, parent
/// timeout, lease loss, etc.). Emitted via `ResearchMetric` envelope from
/// `vox-orchestrator::orchestrator::agent::lifecycle_ops::cancel_task`.
pub const METRIC_TYPE_ORCH_TASK_CANCELLED: &str = "orch.task.cancelled";

/// S1 — AI-first `@ai` fixture: intent routing metadata (task category, strengths).
pub const METRIC_TYPE_FIXTURE_MODEL_INTENT: &str = "fixture.model.intent_resolved";
/// S1 — AI-first `@prompt` fixture: cascade dispatch observation.
pub const METRIC_TYPE_FIXTURE_PROMPT_DISPATCH: &str = "fixture.prompt.dispatch";
/// S1 — AI-first `@search` fixture: retrieval dispatch observation.
pub const METRIC_TYPE_FIXTURE_SEARCH_DISPATCH: &str = "fixture.search.dispatch";
/// S1 — AI-first `@hole` fixture: compile-time hole observed (staleness / inventory).
pub const METRIC_TYPE_FIXTURE_HOLE_OBSERVED: &str = "fixture.hole.observed";

// ── Session id prefixes ──────────────────────────────────────────────────────

pub const SESSION_PREFIX_BENCH: &str = "bench:";
pub const SESSION_PREFIX_SYNTAXK: &str = "syntaxk:";
pub const SESSION_PREFIX_MCP: &str = "mcp:";
pub const SESSION_PREFIX_WORKFLOW: &str = "workflow:";
pub const SESSION_PREFIX_MENS: &str = "mens:";
pub const SESSION_PREFIX_ROUTE: &str = "route:";

/// Hybrid retrieval telemetry uses a fixed session id (not `mcp:<repository_id>`).
pub const SESSION_ID_MEMORY_HYBRID_FUSION: &str = "socrates:retrieval";

// ── TelemetryWriteOptions ────────────────────────────────────────────────────

/// Repository-scoped session_id builder. Use with `METRIC_TYPE_*` constants.
#[derive(Debug, Clone, Copy)]
pub struct TelemetryWriteOptions<'a> {
    pub repository_id: &'a str,
}

impl<'a> TelemetryWriteOptions<'a> {
    #[inline]
    pub fn new(repository_id: &'a str) -> Self {
        Self { repository_id }
    }

    #[inline]
    pub fn session_bench(&self) -> String {
        format!("{SESSION_PREFIX_BENCH}{}", self.repository_id)
    }

    #[inline]
    pub fn session_syntaxk(&self) -> String {
        format!("{SESSION_PREFIX_SYNTAXK}{}", self.repository_id)
    }

    #[inline]
    pub fn session_mcp(&self) -> String {
        format!("{SESSION_PREFIX_MCP}{}", self.repository_id)
    }

    #[inline]
    pub fn session_workflow(&self) -> String {
        format!("{SESSION_PREFIX_WORKFLOW}{}", self.repository_id)
    }

    #[inline]
    pub fn session_mens(&self) -> String {
        format!("{SESSION_PREFIX_MENS}{}", self.repository_id)
    }

    #[inline]
    pub fn session_route(&self) -> String {
        format!("{SESSION_PREFIX_ROUTE}{}", self.repository_id)
    }

    /// Session ID for `vox.lint.*` events (P2.1). Aggregator buckets by repo.
    #[inline]
    pub fn session_lint(&self) -> String {
        format!("{SESSION_PREFIX_LINT}{}", self.repository_id)
    }

    /// Session ID for `vox.repair.*` events (P2.1). One session id per repair
    /// invocation; the [`RepairOutcomeEvent`] closes it.
    #[inline]
    pub fn session_repair(&self) -> String {
        format!("{SESSION_PREFIX_REPAIR}{}", self.repository_id)
    }

    /// Session ID for `vox.audit.*` events (A11).
    #[inline]
    pub fn session_audit(&self) -> String {
        format!("{SESSION_PREFIX_AUDIT}{}", self.repository_id)
    }
}

// ── Validation ───────────────────────────────────────────────────────────────

fn valid_metric_type_chars(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':'))
}

/// Validate inputs before `INSERT INTO research_metrics`.
///
/// Called by `vox_db::VoxDb::append_research_metric` on every write.
pub fn validate_research_metric_row(
    session_id: &str,
    metric_type: &str,
    metadata_json: Option<&str>,
) -> Result<(), TelemetryError> {
    if session_id.is_empty() {
        return Err(TelemetryError::Validation(
            "research_metrics: session_id must be non-empty".into(),
        ));
    }
    if session_id.len() > RESEARCH_METRICS_SESSION_ID_MAX_CHARS {
        return Err(TelemetryError::Validation(format!(
            "research_metrics: session_id exceeds {} characters",
            RESEARCH_METRICS_SESSION_ID_MAX_CHARS
        )));
    }
    if metric_type.is_empty() {
        return Err(TelemetryError::Validation(
            "research_metrics: metric_type must be non-empty".into(),
        ));
    }
    if metric_type.len() > RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS {
        return Err(TelemetryError::Validation(format!(
            "research_metrics: metric_type exceeds {} characters",
            RESEARCH_METRICS_METRIC_TYPE_MAX_CHARS
        )));
    }
    if !valid_metric_type_chars(metric_type) {
        return Err(TelemetryError::Validation(format!(
            "research_metrics: metric_type {metric_type:?} contains disallowed characters"
        )));
    }
    if let Some(m) = metadata_json {
        if m.len() > RESEARCH_METRICS_METADATA_JSON_MAX_BYTES {
            return Err(TelemetryError::Validation(format!(
                "research_metrics: metadata_json exceeds {} bytes",
                RESEARCH_METRICS_METADATA_JSON_MAX_BYTES
            )));
        }
        if metric_type == METRIC_TYPE_MODEL_ROUTE_EVENT
            && (!m.contains("\"trace_id\"") || !m.contains("\"route_policy_profile\""))
        {
            return Err(TelemetryError::Validation(
                "research_metrics: model_route_event metadata_json must include trace_id and route_policy_profile".into(),
            ));
        }
    }
    Ok(())
}

// ── TelemetryEvent ─────────────────────────────────────────────────────────

/// A single telemetry observation that may be routed to one or more sinks.
///
/// New variants are added in later phases; each variant maps to one or more
/// `METRIC_TYPE_*` constants. All variants are `non_exhaustive` so older sinks
/// can ignore unknown events safely via `_ => {}` arms.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TelemetryEvent {
    /// Wraps the existing research_metrics row. Passed through ResearchMetricsSink unchanged.
    ResearchMetric(ResearchMetricEvent),
    /// Per-LLM-call record (Phase B).
    ModelCall(ModelCallEvent),
    /// Top-level task completion rollup (Phase C).
    TaskRootSummary(TaskRootSummaryEvent),
    /// Build summary mirrored from `vox ci build-timings` (Phase D).
    BuildSummary(BuildSummaryEvent),
    /// Subsystem error / retry event (Phase D).
    Error(ErrorEvent),
    /// AI-first language fixtures (`@ai`, `@prompt`, `@subagent`, `@search`, `@hole`).
    AiFixture(AiFixtureEvent),
    /// CR-L8 corpus-feedback: one `vox-code-audit` finding (P2.1).
    LintFinding(LintFindingEvent),
    /// CR-L8 corpus-feedback: autofix accepted or rejected (P2.1).
    /// The wrapping `metric_type` disambiguates `applied` vs `rejected`.
    LintAutofix(LintAutofixEvent),
    /// CR-L8 corpus-feedback: one outer-loop attempt of `vox repair` (P2.1).
    RepairAttempt(RepairAttemptEvent),
    /// CR-L8 corpus-feedback: terminal outcome of a `vox repair` session (P2.1).
    RepairOutcome(RepairOutcomeEvent),
    /// Per `vox audit <thing>` run (A11; contract-required per
    /// `contracts/ci/vox-audit-contract.v1.yaml` §telemetry).
    AuditRun(AuditRunEvent),
    /// One `select()` call — L0 of the model-autonomic system.
    /// `metric_type = METRIC_TYPE_SELECTION_DECISION`.
    /// See [`docs/src/architecture/model-autonomic-system-2026.md`] §4.
    SelectionDecision(SelectionDecisionEvent),
    /// A model id appeared in an upstream catalog that wasn't in the prior
    /// snapshot. L1 of the model-autonomic system.
    /// `metric_type = METRIC_TYPE_MODEL_DISCOVERY`.
    ModelDiscovery(DiscoveryEvent),
    /// Classifier LLM emitted tier/strengths/confidence for a model.
    /// L2 of the model-autonomic system.
    /// `metric_type = METRIC_TYPE_MODEL_CLASSIFICATION`.
    ModelClassification(ClassificationEvent),
    /// A model crossed a confidence boundary (Provisional→Shadowed→Confirmed).
    /// L2/L3 of the model-autonomic system.
    /// `metric_type = METRIC_TYPE_CONFIDENCE_PROMOTION`.
    ConfidencePromotion(ConfidencePromotionEvent),
    /// One `vox doctor --project <path>` health-check run.
    /// `metric_type = METRIC_TYPE_DOCTOR_PROJECT_CHECK`.
    /// CR-L7's third leg (after `vox new` + `vox deploy`): the doctor mode
    /// invoked by the deploy integration test to verify post-deploy health.
    DoctorProjectCheck(DoctorProjectCheckEvent),
}

/// Payload aligned with `contracts/telemetry/fixture-model-intent-resolved.v1.schema.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FixtureModelIntentResolvedEvent {
    pub task_category: String,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier_max: Option<String>,
    pub resolved_model_hint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

/// Payload aligned with `contracts/telemetry/orch-subagent-dispatch.v1.schema.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SubagentDispatchTelemetryPayload {
    pub metric_type: String,
    pub decision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch_latency_ms: Option<u64>,
}

/// Payload aligned with `contracts/telemetry/fixture-prompt-dispatch.v1.schema.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PromptDispatchTelemetryEvent {
    pub stage: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub redact_count: u32,
}

/// Payload aligned with `contracts/telemetry/fixture-search-dispatch.v1.schema.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SearchDispatchTelemetryEvent {
    pub corpus: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
}

/// Payload aligned with `contracts/telemetry/fixture-hole-observed.v1.schema.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HoleObservedTelemetryEvent {
    pub cache_key: String,
    pub observation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
}

/// Tagged union for AI fixture telemetry (serialized as JSON `metadata_json` in research_metrics).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "fixture_kind", rename_all = "snake_case")]
pub enum AiFixtureEvent {
    ModelIntent(FixtureModelIntentResolvedEvent),
    SubagentDispatch(SubagentDispatchTelemetryPayload),
    PromptDispatch(PromptDispatchTelemetryEvent),
    SearchDispatch(SearchDispatchTelemetryEvent),
    HoleObserved(HoleObservedTelemetryEvent),
}

/// Back-compat alias for older docs / actor-runtime re-exports.
pub type OrchSubagentDispatchEvent = SubagentDispatchTelemetryPayload;

/// Payload for a `TelemetryEvent::ResearchMetric`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResearchMetricEvent {
    pub session_id: String,
    pub metric_type: String,
    pub metric_value: Option<f64>,
    pub metadata_json: Option<String>,
}

/// Per-LLM-call performance record. Persisted as `research_metrics` row with
/// `metric_type = METRIC_TYPE_MODEL_CALL_EVENT`.
///
/// Unlocks (in aggregate): cache hit rate, cost-per-task, p95 latency by model and route,
/// token efficiency. Sensitivity: **S1 (OperationalTracing)**.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelCallEvent {
    pub model: String,
    pub provider: String,
    pub route_profile: Option<String>,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    /// Anthropic prompt-cache: tokens read from cache (~10× cheaper than fresh input).
    pub cache_read_input_tokens: Option<u32>,
    /// Anthropic prompt-cache: tokens written to create the cache (~25% premium).
    pub cache_creation_input_tokens: Option<u32>,
    pub latency_ms: u64,
    pub cost_usd: f64,
    pub cost_source: String,
    pub error_class: Option<String>,
    pub retry_attempt: u32,
    pub task_id: Option<u64>,
    pub parent_task_id: Option<u64>,
    pub trace_id: Option<String>,
    pub caller_agent_id: Option<String>,
}

/// Top-level task completion rollup. Persisted as `research_metrics` row with
/// `metric_type = METRIC_TYPE_TASK_ROOT_SUMMARY`.
///
/// One row per top-level task. Aggregates totals across all child agent calls
/// and LLM calls within the task. Token/cost aggregates are populated in Phase D
/// (wall_time_ms is 0 as a placeholder until task start time is threaded through).
/// Sensitivity: **S1 (OperationalTracing)**.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskRootSummaryEvent {
    pub task_id: u64,
    pub trace_id: String,
    pub repository_id: Option<String>,
    /// "completed" | "failed" | "doubted" | "cancelled"
    pub outcome: String,
    pub wall_time_ms: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub child_call_count: u32,
    pub max_span_depth: u16,
    pub subagent_fanout: u32,
}

/// Build pipeline summary. Persisted as `research_metrics` row with
/// `metric_type = METRIC_TYPE_BUILD_SUMMARY_EVENT`.
///
/// Emitted after `vox ci build-timings` completes or fails.
/// Sensitivity: **S0 (OperationalMetrics — no user content)**.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildSummaryEvent {
    /// Identifier for the build run (CI job ID, local session ID, etc.).
    pub build_id: String,
    /// "success" | "failure" | "cancelled"
    pub outcome: String,
    /// Total wall-clock time of the build in milliseconds.
    pub wall_time_ms: u64,
    /// Number of crates compiled.
    pub crates_compiled: u32,
    /// Number of compile errors, if any.
    pub error_count: u32,
    /// Top-level invocation context: "ci" | "local" | "watch".
    pub invocation_context: Option<String>,
}

/// Generic subsystem error / retry event. Persisted as `research_metrics` row with
/// `metric_type = METRIC_TYPE_ERROR_EVENT`.
///
/// Emitted at the call site of notable errors: HTTP failures, 429 rate-limits,
/// circuit-breaker trips. Carries enough context to filter by subsystem and
/// error kind without embedding user content.
/// Sensitivity: **S1 (OperationalTracing)**.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorEvent {
    /// Dot-separated subsystem path, e.g. "llm.http", "orch.circuit_breaker".
    pub subsystem: String,
    /// Short kebab-case error class, e.g. "rate-limited", "connection-timeout".
    pub error_class: String,
    /// HTTP status code, if applicable.
    pub http_status: Option<u16>,
    /// Which retry attempt this is (0 = first failure, 1 = after first retry, …).
    pub retry_attempt: u32,
    /// Whether the call was ultimately retried after this error.
    pub retried: bool,
    /// Optional model identifier if the error occurred during an LLM call.
    pub model: Option<String>,
    /// Optional provider identifier.
    pub provider: Option<String>,
    /// Optional task id from the ambient TraceContext.
    pub task_id: Option<u64>,
    /// Optional trace id from the ambient TraceContext.
    pub trace_id: Option<String>,
}

// ───────────────────────────────────────────────────────────────────────────
// CR-L8 corpus-feedback events (council ratified 2026-05-15, P2.1).
//
// Feed the quarterly export pipeline at
// `contracts/reports/corpus-feedback/<quarter>.json` per
// `docs/src/architecture/v1-llm-target-implementation-plan-2026.md` §1.3 P2.1.
//
// Sensitivity: **S1 (OperationalTracing)** — rule-id + span info are not
// secret-bearing; consumers MUST NOT include source text in `metadata_json`.
// ───────────────────────────────────────────────────────────────────────────

/// S1 — `vox-code-audit` emitted one [`LintFindingEvent`] per finding.
pub const METRIC_TYPE_LINT_FINDING: &str = "vox.lint.finding";

/// S1 — `vox-code-audit` autofix accepted by user / pipeline.
pub const METRIC_TYPE_LINT_AUTOFIX_APPLIED: &str = "vox.lint.autofix_applied";

/// S1 — `vox-code-audit` autofix rejected by user / pipeline.
pub const METRIC_TYPE_LINT_AUTOFIX_REJECTED: &str = "vox.lint.autofix_rejected";

/// S1 — `vox repair` LLM attempt observation (one event per outer-loop attempt).
pub const METRIC_TYPE_REPAIR_ATTEMPT: &str = "vox.repair.attempt";

/// S1 — `vox repair` terminal outcome (one event per session).
pub const METRIC_TYPE_REPAIR_OUTCOME: &str = "vox.repair.outcome";

/// Session-ID prefix for `vox.lint.*` events.
pub const SESSION_PREFIX_LINT: &str = "lint:";

/// Session-ID prefix for `vox.repair.*` events.
pub const SESSION_PREFIX_REPAIR: &str = "repair:";

/// Payload aligned with `metric_type = METRIC_TYPE_LINT_FINDING`.
///
/// One event per [`crate::types::TelemetryEvent::LintFinding`]; aggregated by
/// the CR-L8 quarterly export pipeline into the "Top-50 firing diagnostics"
/// section of `contracts/reports/corpus-feedback/<quarter>.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LintFindingEvent {
    /// Rule id as returned by `DetectionRule::id()` (e.g. `retired/decorator-usage`).
    pub rule_id: String,
    /// Stable diagnostic id from `vox_code_audit::diagnostics::catalog`
    /// (e.g. `vox/retired/decorator-usage`). `None` when the rule has no
    /// catalog binding yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
    /// Lowercase severity name: `info` | `warning` | `error` | `critical`.
    pub severity: String,
    /// Relative path within the repo (no absolute paths — keeps the event S1).
    pub relative_path: String,
    /// 1-based line number where the finding fires.
    pub line: u32,
    /// True when the rule provided a structured autofix descriptor.
    pub autofix_available: bool,
    /// Confidence tag: `high` | `medium` | `low` (informational; aligns with
    /// the lint Phase-2 plan's confidence ladder).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    /// Repository identifier (typically the workspace root's basename) so the
    /// aggregator can roll up by repo. Optional for ad-hoc runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,
}

/// Outcome of an autofix offer — applied (accepted) or rejected.
///
/// Used by both [`METRIC_TYPE_LINT_AUTOFIX_APPLIED`] and
/// [`METRIC_TYPE_LINT_AUTOFIX_REJECTED`] event payloads (the metric_type
/// disambiguates the outcome; the payload shape is shared for aggregator
/// simplicity).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LintAutofixEvent {
    pub rule_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
    /// `"applied"` | `"rejected"` — must match the wrapping `metric_type`.
    pub outcome: String,
    /// Free-form reason when rejected (e.g., "user dismissed", "conflict",
    /// "low-confidence"); empty / absent when applied cleanly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub relative_path: String,
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,
}

/// One outer-loop attempt of `vox repair`. Multiple attempts may share a
/// session id; the [`RepairOutcomeEvent`] closes the session.
///
/// `PartialEq` only (no `Eq`) because `cost_usd: f64` does not implement `Eq`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RepairAttemptEvent {
    /// 1-based attempt index within the session.
    pub attempt_number: u32,
    /// Diagnostics observed at the start of this attempt.
    pub diagnostics_in: u32,
    /// Diagnostics observed at the end of this attempt.
    pub diagnostics_out: u32,
    /// Files the LLM patch touched in this attempt.
    pub files_touched: u32,
    /// Cost (USD) attributable to this attempt across the panel.
    pub cost_usd: f64,
    /// Wall-clock duration of this attempt.
    pub duration_ms: u64,
    /// `panel_member_id` of the LLM that produced the patch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel_member_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,
}

/// S1 — One [`AuditRunEvent`] per `vox audit <thing>` run.
///
/// Required per `contracts/ci/vox-audit-contract.v1.yaml` §telemetry. Council
/// ratified 2026-05-15 (A11 contract-compliance follow-on).
pub const METRIC_TYPE_AUDIT_RUN: &str = "vox.audit.run";

/// Session-ID prefix for `vox.audit.*` events.
pub const SESSION_PREFIX_AUDIT: &str = "audit:";

/// Per-`vox audit <thing>` run observation.
///
/// Aligned with `contracts/ci/vox-audit-contract.v1.yaml` §telemetry
/// `required_fields`: `corpus_hash`, `panel_version`, `duration_seconds`,
/// `outcome`, `cumulative_cost_usd`, `unreachable_panel_member_count`. The
/// `thing` field disambiguates which gate the event is for (the metric_type
/// in research_metrics is the constant; consumers use `thing` to slice).
///
/// `PartialEq` only (no `Eq`) because `cumulative_cost_usd: f64` does not
/// implement `Eq`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AuditRunEvent {
    /// CR-L gate name (e.g., `retirement`, `humaneval`, `corpus-feedback`,
    /// or `all` for the umbrella run).
    pub thing: String,
    /// `"ok"` | `"bar_missed"` | `"infra_error"` | `"invalid_input"` matching
    /// the [`crate::types::TelemetryEvent`] exit-code-to-string mapping.
    pub outcome: String,
    /// BLAKE3 content hash of the corpus / contract this gate measured against.
    pub corpus_hash: String,
    /// Number of fixtures / events the gate observed.
    pub corpus_size: u32,
    pub duration_seconds: f64,
    /// Cumulative LLM cost across the panel for this run. Zero for
    /// non-cost-metered gates (retirement, aci-default, corpus-feedback).
    pub cumulative_cost_usd: f64,
    /// Number of panel members that were unreachable during this run
    /// (rate-limited, API down, etc.). Zero for non-cost-metered gates.
    #[serde(default)]
    pub unreachable_panel_member_count: u32,
    /// Pinned panel version (e.g. `2026-05-15`). `None` for non-LLM-panel gates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel_version: Option<String>,
    /// Was this run inside `vox audit all` (umbrella) vs a direct gate call?
    #[serde(default)]
    pub umbrella_run: bool,
    /// Optional repository identifier (for per-workspace rollups).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,
}

/// Terminal outcome of a `vox repair` session (one per session).
///
/// `PartialEq` only (no `Eq`) because `total_cost_usd: f64` does not implement `Eq`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RepairOutcomeEvent {
    /// `"success"` | `"partial"` | `"abandoned"` | `"infra_error"`.
    pub final_state: String,
    pub attempts_used: u32,
    pub attempts_budget: u32,
    pub total_cost_usd: f64,
    pub total_duration_ms: u64,
    /// Number of diagnostics still firing at end of session (0 on success).
    pub residual_diagnostics: u32,
    /// Optional human-readable note (e.g. error class when `infra_error`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,
}

// ─── Model-autonomic system events (L0–L3) ────────────────────────────────
//
// Council-ratified 2026-05-15. SSOT: `docs/src/architecture/model-autonomic-system-2026.md`.
// All four events together form the feedback flywheel that auto-classifies new
// frontier models and promotes them into the registry without manual PRs.

/// L0 — one `select(intent, registry)` decision.
pub const METRIC_TYPE_SELECTION_DECISION: &str = "vox.model.selection_decision";
/// L1 — a model id was seen in upstream catalog that wasn't in prior snapshot.
pub const METRIC_TYPE_MODEL_DISCOVERY: &str = "vox.model.discovery";
/// L2 — classifier LLM tagged a model with tier/strengths/confidence.
pub const METRIC_TYPE_MODEL_CLASSIFICATION: &str = "vox.model.classification";
/// L2/L3 — a model crossed a confidence-state boundary.
pub const METRIC_TYPE_CONFIDENCE_PROMOTION: &str = "vox.model.confidence_promotion";

/// Session-ID prefix for `vox.model.*` autonomic-system events.
pub const SESSION_PREFIX_MODEL_AUTONOMIC: &str = "model_autonomic:";

// ─── vox doctor — project-health events (CR-L7) ───────────────────────────

/// Per-`vox doctor --project <path>` health-check observation.
///
/// CR-L7 names `vox doctor` as the third leg of `vox new → vox deploy →
/// vox doctor` and asks for `vox.doctor.*` telemetry alongside structured
/// JSON output. This is the project-scope health-check event; the existing
/// `vox doctor` (environment / cross-compile preflight) is a separate
/// surface and does not emit this event.
pub const METRIC_TYPE_DOCTOR_PROJECT_CHECK: &str = "vox.doctor.project_check";

/// Payload for [`crate::types::TelemetryEvent::DoctorProjectCheck`].
///
/// `PartialEq` only (no `Eq`) because `duration_seconds: f64` does not
/// implement `Eq`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct DoctorProjectCheckEvent {
    /// Absolute or workspace-relative project root that was scanned.
    pub project_root: String,
    /// Total `.vox` files discovered under `project_root`.
    pub files_total: u32,
    /// Files that produced zero error-severity diagnostics.
    pub files_passing: u32,
    /// Files that produced at least one error-severity diagnostic.
    pub files_failing: u32,
    /// `"green"` | `"red"` | `"infra_error"`.
    pub outcome: String,
    /// Wall-clock duration of the entire check run.
    pub duration_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,
}

/// One `select()` decision. Carries enough context to reconstruct *why* this
/// model was chosen for this caller at this moment without re-running the
/// scorer. Drives the L3 council report.
///
/// Sensitivity: **S1 (OperationalTracing)** — no prompt content, no PII.
/// `PartialEq` only (no `Eq`) because the axes are stored as `u8` triples
/// (which would actually be `Eq`-able), but the field is kept open for
/// future float weights — pre-emptively `PartialEq`-only matches the rest
/// of the model events in this module.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SelectionDecisionEvent {
    /// Free-form caller identifier (e.g. `"repair-loop"`, `"research"`).
    /// `None` when caller didn't set `SelectionIntent::caller_hint`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_caller: Option<String>,
    /// `TaskCategory` as a snake_case string (avoids cross-crate enum drift).
    pub task: String,
    /// `(cost, responsiveness, intelligence)` axis weights at decision time.
    pub axes: (u8, u8, u8),
    /// Model id chosen.
    pub chosen_model: String,
    /// Why this model was chosen: `"premium_alias"` | `"scored"` |
    /// `"local_only"` | `"env_override"`.
    pub reason: String,
    /// Optional alias-key when reason = `"premium_alias"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_alias_key: Option<String>,
    /// Optional repository id for per-workspace rollups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,
}

/// L1 — a model id appeared in an upstream catalog that wasn't in the prior
/// snapshot. Emitted once per (source, model_id) per refresh cycle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DiscoveryEvent {
    /// `"openrouter"` | `"litellm"` | `"anthropic_direct"` | `"populi_mesh"`.
    pub source: String,
    pub model_id: String,
    /// Optional model description text from upstream (useful for L2 classifier).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional context-window size reported by upstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_context_tokens: Option<u32>,
}

/// L2 — classifier LLM emitted tier/strengths/confidence for a model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ClassificationEvent {
    pub model_id: String,
    /// Which classifier produced this judgement (e.g. `"anthropic/claude-haiku-4.5"`).
    pub classifier_model: String,
    /// `ModelTier` as a snake_case string.
    pub tier: String,
    /// `StrengthTag`s as snake_case strings.
    pub strengths: Vec<String>,
    /// 0.0–1.0 self-reported confidence from the classifier.
    pub confidence: f32,
}

/// L2/L3 — a model crossed a confidence-state boundary
/// (Provisional → Shadowed → Confirmed, or any state → Deprecated).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ConfidencePromotionEvent {
    pub model_id: String,
    /// Previous state (`"provisional"` | `"shadowed"` | `"confirmed"` | `"deprecated"`).
    pub from: String,
    pub to: String,
    /// What evidence triggered the promotion: `"scoreboard_threshold"` |
    /// `"council_approval"` | `"shadow_eval"` | `"failure_threshold"`.
    pub evidence: String,
}

// ─── Effort audit events (S1 — `vox audit effort`) ────────────────────────
//
// One run of `vox audit effort` walks a commit range, asks a judge model to
// score each commit's likely agent-token spend, and emits four kinds of
// telemetry: one `RunStarted`, one `CommitJudged` per commit observation,
// one `RunCompleted` on success, or one `RunFailed` on terminal failure.
// SSOT: `docs/superpowers/specs/2026-05-28-effort-audit-core-design.md` §3.

/// One emit per `vox audit effort` invocation, before any commits are walked.
pub const METRIC_TYPE_AUDIT_EFFORT_RUN_STARTED: &str = "audit.effort.run.started";
/// One emit per judged commit (success, skip, or per-commit judge failure).
pub const METRIC_TYPE_AUDIT_EFFORT_COMMIT_JUDGED: &str = "audit.effort.commit.judged";
/// One emit on successful run completion.
pub const METRIC_TYPE_AUDIT_EFFORT_RUN_COMPLETED: &str = "audit.effort.run.completed";
/// One emit on terminal failure (git walk, output write, etc.).
pub const METRIC_TYPE_AUDIT_EFFORT_RUN_FAILED: &str = "audit.effort.run.failed";

/// `RunStarted` — emitted once at the top of every `vox audit effort` run,
/// before any commits are walked.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AuditEffortRunStartedEvent {
    /// Stable id assigned to this run (used to correlate the other three
    /// event kinds back to this start). ULID-ish; opaque to consumers.
    pub run_id: String,
    /// Human-readable commit range that was requested
    /// (e.g. `"30 days ago..HEAD"`, `"HEAD~50..HEAD"`).
    pub range: String,
    /// Model id selected to judge commits in this run (or `"mock"` for the
    /// deterministic mock judge used in tests).
    pub judge_model_id: String,
}

/// `CommitJudged` — one emit per commit observation. Finding fields are
/// `Option` so a per-commit judge failure or shape-based skip still produces
/// an event without inventing fake scores.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AuditEffortCommitJudgedEvent {
    /// Correlates back to [`AuditEffortRunStartedEvent::run_id`].
    pub run_id: String,
    /// Full commit SHA (hex).
    pub commit_sha: String,
    /// Model id that produced the finding (or `"mock"`).
    pub judge_model_id: String,
    /// Wall-clock latency of the judge call for this commit.
    pub latency_ms: u64,
    /// Total tokens (input + output) consumed by the judge for this commit.
    pub tokens_consumed_by_judge: u64,
    /// 0–100 waste score; `None` when the commit was skipped or the judge
    /// failed for this commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waste_score: Option<u8>,
    /// Free-form category tag (e.g. `"flailing"`, `"over_engineered"`,
    /// `"good_signal"`). `None` when no finding was produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waste_category: Option<String>,
    /// Suggested remediation kind tag (e.g. `"split_commit"`,
    /// `"add_test_first"`). `None` when no finding was produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_remediation_kind: Option<String>,
}

/// `RunCompleted` — emitted once on successful run completion. Carries
/// roll-up counters for the manifest + dashboard. `PartialEq` only because
/// `hybrid_coverage_percent: f64` does not implement `Eq`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AuditEffortRunCompletedEvent {
    /// Correlates back to [`AuditEffortRunStartedEvent::run_id`].
    pub run_id: String,
    /// Total commits the walker observed in the requested range.
    pub commits_in_range: u64,
    /// Commits the judge actually scored (a finding was produced).
    pub commits_judged: u64,
    /// Commits skipped (shape filter, per-commit judge failure, etc.).
    pub commits_skipped: u64,
    /// Cumulative input tokens spent by the judge across the run.
    pub judge_total_input_tokens: u64,
    /// Cumulative output tokens spent by the judge across the run.
    pub judge_total_output_tokens: u64,
    /// Percent of judged commits that the hybrid transcript correlator
    /// could associate with real agent token spend (0.0–100.0).
    pub hybrid_coverage_percent: f64,
}

/// `RunFailed` — emitted once on terminal failure (i.e. the run did not
/// produce a manifest). Per-commit judge failures do NOT emit this — they
/// appear as a `CommitJudged` event with `waste_score = None`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AuditEffortRunFailedEvent {
    /// Correlates back to [`AuditEffortRunStartedEvent::run_id`].
    pub run_id: String,
    /// Discriminator tag for the failure class (e.g. `"GitWalkFailed"`,
    /// `"OutputWriteFailed"`, `"RangeResolutionFailed"`).
    pub error_kind: String,
    /// Short human-readable message (no PII; no raw repo paths beyond the
    /// repo root marker).
    pub message: String,
}

// --- audit.route.* events (vox audit effort-route, S2) -----------------------
// Lifecycle mirrors audit.effort.*: one `RunStarted` per invocation, one
// `ClusterDecided` per routed cluster, one `RunCompleted` on success, or one
// `RunFailed` on terminal failure.
// SSOT: `docs/superpowers/specs/2026-05-30-effort-route-design.md`.

/// One emit per `vox audit effort-route` invocation, before any clusters route.
pub const METRIC_TYPE_AUDIT_ROUTE_RUN_STARTED: &str = "audit.route.run.started";
/// One emit per routed cluster (decide + verify outcome).
pub const METRIC_TYPE_AUDIT_ROUTE_CLUSTER_DECIDED: &str = "audit.route.cluster.decided";
/// One emit on successful run completion.
pub const METRIC_TYPE_AUDIT_ROUTE_RUN_COMPLETED: &str = "audit.route.run.completed";
/// One emit on terminal failure (load, schema mismatch, output write, etc.).
pub const METRIC_TYPE_AUDIT_ROUTE_RUN_FAILED: &str = "audit.route.run.failed";

/// `RunStarted` — emitted once at the top of every `vox audit effort-route`
/// run, before any clusters are routed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AuditRouteRunStartedEvent {
    /// Stable id assigned to this run (used to correlate the other three
    /// event kinds back to this start). Opaque to consumers.
    pub run_id: String,
    /// Path to the S1 `findings.jsonl` this run consumed.
    pub findings_path: String,
    /// Model id selected to re-judge clusters in this run (or `"mock"`).
    pub judge_model_id: String,
    /// Whether the selected judge model is permitted to author Vox source.
    pub judge_model_vox_capable: bool,
}

/// `ClusterDecided` — one emit per routed cluster. Carries the decided form
/// and the verify outcome so the dashboard can track refutation rates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AuditRouteClusterDecidedEvent {
    /// Correlates back to [`AuditRouteRunStartedEvent::run_id`].
    pub run_id: String,
    /// Stable id of the cluster that was routed.
    pub cluster_id: String,
    /// Number of member findings in the cluster.
    pub member_count: u64,
    /// Decided artifact form tag (e.g. `"CiGate"`, `"AgentsMdRule"`, `"None"`).
    pub artifact_form: String,
    /// Decide-pass confidence (0.0–1.0).
    pub confidence: f64,
    /// Whether the adversarial verify pass left the decision standing.
    pub verified: bool,
}

/// `RunCompleted` — emitted once on successful run completion. Carries
/// roll-up counters for the manifest + dashboard.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AuditRouteRunCompletedEvent {
    /// Correlates back to [`AuditRouteRunStartedEvent::run_id`].
    pub run_id: String,
    /// Findings that survived load + filter.
    pub findings_loaded: u64,
    /// Deterministic buckets formed before sub-clustering.
    pub buckets: u64,
    /// Clusters routed (decide + verify).
    pub clusters_routed: u64,
    /// Clusters whose decision survived adversarial verification.
    pub verified: u64,
}

/// `RunFailed` — emitted once on terminal failure (i.e. the run did not
/// produce recommendations). Per-cluster decide/verify failures do NOT emit
/// this — they appear as a `ClusterDecided` event with `verified = false`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AuditRouteRunFailedEvent {
    /// Correlates back to [`AuditRouteRunStartedEvent::run_id`].
    pub run_id: String,
    /// Discriminator tag for the failure class (e.g. `"LoadFailed"`,
    /// `"SchemaMismatch"`, `"OutputWriteFailed"`).
    pub error_kind: String,
    /// Short human-readable message (no PII; no raw repo paths beyond the
    /// repo root marker).
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_session_and_metric_type() {
        assert!(validate_research_metric_row("", "t", None).is_err());
        assert!(validate_research_metric_row("s", "", None).is_err());
    }

    #[test]
    fn accepts_colon_in_metric_type() {
        assert!(validate_research_metric_row("sess", "mcp:foo_bar", None).is_ok());
    }

    #[test]
    fn rejects_disallowed_metric_type_chars() {
        assert!(validate_research_metric_row("s", "bad type", None).is_err());
        assert!(validate_research_metric_row("s", "bad/type", None).is_err());
    }

    #[test]
    fn enforces_metadata_size_cap() {
        let big = "x".repeat(RESEARCH_METRICS_METADATA_JSON_MAX_BYTES + 1);
        assert!(validate_research_metric_row("s", "t", Some(&big)).is_err());
        let ok = "x".repeat(RESEARCH_METRICS_METADATA_JSON_MAX_BYTES);
        assert!(validate_research_metric_row("s", "t", Some(&ok)).is_ok());
    }

    #[test]
    fn telemetry_write_options_builds_expected_sessions() {
        let o = TelemetryWriteOptions::new("rid42");
        assert_eq!(o.session_bench(), "bench:rid42");
        assert_eq!(o.session_syntaxk(), "syntaxk:rid42");
        assert_eq!(o.session_mcp(), "mcp:rid42");
        assert_eq!(o.session_workflow(), "workflow:rid42");
        assert_eq!(o.session_mens(), "mens:rid42");
        assert_eq!(o.session_route(), "route:rid42");
    }

    #[test]
    fn new_phase_b_d_constants_pass_validation() {
        for mt in [
            METRIC_TYPE_MODEL_CALL_EVENT,
            METRIC_TYPE_TASK_ROOT_SUMMARY,
            METRIC_TYPE_BUILD_SUMMARY_EVENT,
            METRIC_TYPE_ERROR_EVENT,
            METRIC_TYPE_FIXTURE_MODEL_INTENT,
            METRIC_TYPE_FIXTURE_PROMPT_DISPATCH,
            METRIC_TYPE_FIXTURE_SEARCH_DISPATCH,
            METRIC_TYPE_FIXTURE_HOLE_OBSERVED,
            METRIC_TYPE_PLUGIN_LOAD_FAILURE,
            METRIC_TYPE_SANDBOX_TIMEOUT_KILL,
            METRIC_TYPE_ORCH_CACHE_MISS,
            METRIC_TYPE_ORCH_TASK_CANCELLED,
        ] {
            assert!(
                validate_research_metric_row("sess", mt, None).is_ok(),
                "{mt} failed validation"
            );
        }
    }

    #[test]
    fn ai_fixture_event_round_trip() {
        let ev = AiFixtureEvent::ModelIntent(FixtureModelIntentResolvedEvent {
            task_category: "CodeGen".into(),
            strengths: vec!["codegen".into()],
            tier_max: Some("Pro".into()),
            resolved_model_hint: "openrouter/auto".into(),
            trace_id: Some("t1".into()),
        });
        let outer = TelemetryEvent::AiFixture(ev.clone());
        let json = serde_json::to_string(&outer).expect("serialize");
        let back: TelemetryEvent = serde_json::from_str(&json).expect("deserialize");
        let TelemetryEvent::AiFixture(AiFixtureEvent::ModelIntent(m)) = back else {
            panic!("variant lost");
        };
        assert_eq!(m.task_category, "CodeGen");
        assert_eq!(m.resolved_model_hint, "openrouter/auto");
    }

    // ─── CR-L8 corpus-feedback events (P2.1) ──────────────────────────────

    #[test]
    fn lint_finding_event_round_trip() {
        let e = LintFindingEvent {
            rule_id: "retired/decorator-usage".into(),
            diagnostic_id: Some("vox/retired/decorator-usage".into()),
            severity: "warning".into(),
            relative_path: "examples/old.vox".into(),
            line: 42,
            autofix_available: true,
            confidence: Some("high".into()),
            repository_id: Some("vox".into()),
        };
        let outer = TelemetryEvent::LintFinding(e.clone());
        let json = serde_json::to_string(&outer).expect("serialize");
        let back: TelemetryEvent = serde_json::from_str(&json).expect("deserialize");
        let TelemetryEvent::LintFinding(back) = back else {
            panic!("LintFinding variant lost in round trip");
        };
        assert_eq!(back, e);
    }

    #[test]
    fn lint_autofix_event_round_trip() {
        let e = LintAutofixEvent {
            rule_id: "retired/decorator-usage".into(),
            diagnostic_id: Some("vox/retired/decorator-usage".into()),
            outcome: "applied".into(),
            reason: None,
            relative_path: "src/main.vox".into(),
            line: 7,
            repository_id: None,
        };
        let outer = TelemetryEvent::LintAutofix(e.clone());
        let json = serde_json::to_string(&outer).expect("serialize");
        let back: TelemetryEvent = serde_json::from_str(&json).expect("deserialize");
        let TelemetryEvent::LintAutofix(back) = back else {
            panic!("LintAutofix variant lost");
        };
        assert_eq!(back, e);
    }

    #[test]
    fn repair_attempt_event_round_trip() {
        let e = RepairAttemptEvent {
            attempt_number: 2,
            diagnostics_in: 5,
            diagnostics_out: 1,
            files_touched: 3,
            cost_usd: 0.42,
            duration_ms: 8200,
            panel_member_id: Some("claude-sonnet".into()),
            repository_id: Some("vox".into()),
        };
        let outer = TelemetryEvent::RepairAttempt(e.clone());
        let json = serde_json::to_string(&outer).expect("serialize");
        let back: TelemetryEvent = serde_json::from_str(&json).expect("deserialize");
        let TelemetryEvent::RepairAttempt(back) = back else {
            panic!("RepairAttempt variant lost");
        };
        assert_eq!(back, e);
    }

    #[test]
    fn repair_outcome_event_round_trip() {
        let e = RepairOutcomeEvent {
            final_state: "success".into(),
            attempts_used: 2,
            attempts_budget: 3,
            total_cost_usd: 0.72,
            total_duration_ms: 14500,
            residual_diagnostics: 0,
            note: None,
            repository_id: Some("vox".into()),
        };
        let outer = TelemetryEvent::RepairOutcome(e.clone());
        let json = serde_json::to_string(&outer).expect("serialize");
        let back: TelemetryEvent = serde_json::from_str(&json).expect("deserialize");
        let TelemetryEvent::RepairOutcome(back) = back else {
            panic!("RepairOutcome variant lost");
        };
        assert_eq!(back, e);
    }

    #[test]
    fn audit_run_event_round_trip() {
        let e = AuditRunEvent {
            thing: "retirement".into(),
            outcome: "ok".into(),
            corpus_hash: "blake3:abc".into(),
            corpus_size: 16,
            duration_seconds: 0.42,
            cumulative_cost_usd: 0.0,
            unreachable_panel_member_count: 0,
            panel_version: None,
            umbrella_run: false,
            repository_id: Some("vox".into()),
        };
        let outer = TelemetryEvent::AuditRun(e.clone());
        let json = serde_json::to_string(&outer).expect("ser");
        let back: TelemetryEvent = serde_json::from_str(&json).expect("de");
        let TelemetryEvent::AuditRun(back) = back else {
            panic!("AuditRun variant lost");
        };
        assert_eq!(back, e);
    }

    #[test]
    fn audit_effort_event_round_trips() {
        // RunStarted — minimal shape, asserts the field referenced by the
        // plan's exemplar serializes literally.
        let started = AuditEffortRunStartedEvent {
            run_id: "01HW7".into(),
            range: "30 days ago..HEAD".into(),
            judge_model_id: "mock".into(),
        };
        let j = serde_json::to_string(&started).expect("serialize");
        let back: AuditEffortRunStartedEvent = serde_json::from_str(&j).expect("deserialize");
        assert!(j.contains("01HW7"));
        assert_eq!(back, started);

        // CommitJudged — both the finding-present and finding-absent shapes,
        // since the Optional fields are the load-bearing design choice.
        let judged_full = AuditEffortCommitJudgedEvent {
            run_id: "01HW7".into(),
            commit_sha: "abcdef0123".into(),
            judge_model_id: "mock".into(),
            latency_ms: 142,
            tokens_consumed_by_judge: 1200,
            waste_score: Some(73),
            waste_category: Some("flailing".into()),
            suggested_remediation_kind: Some("split_commit".into()),
        };
        let j = serde_json::to_string(&judged_full).expect("serialize");
        let back: AuditEffortCommitJudgedEvent = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back, judged_full);

        let judged_skipped = AuditEffortCommitJudgedEvent {
            run_id: "01HW7".into(),
            commit_sha: "abcdef0123".into(),
            judge_model_id: "mock".into(),
            latency_ms: 0,
            tokens_consumed_by_judge: 0,
            waste_score: None,
            waste_category: None,
            suggested_remediation_kind: None,
        };
        let j = serde_json::to_string(&judged_skipped).expect("serialize");
        // Skip-serializing Option::None means these keys must not appear.
        assert!(!j.contains("waste_score"));
        assert!(!j.contains("waste_category"));
        assert!(!j.contains("suggested_remediation_kind"));
        let back: AuditEffortCommitJudgedEvent = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back, judged_skipped);

        // RunCompleted — carries the f64 hybrid_coverage_percent.
        let completed = AuditEffortRunCompletedEvent {
            run_id: "01HW7".into(),
            commits_in_range: 50,
            commits_judged: 48,
            commits_skipped: 2,
            judge_total_input_tokens: 12_000,
            judge_total_output_tokens: 4_500,
            hybrid_coverage_percent: 62.5,
        };
        let j = serde_json::to_string(&completed).expect("serialize");
        let back: AuditEffortRunCompletedEvent = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back, completed);

        // RunFailed.
        let failed = AuditEffortRunFailedEvent {
            run_id: "01HW7".into(),
            error_kind: "GitWalkFailed".into(),
            message: "unable to resolve HEAD".into(),
        };
        let j = serde_json::to_string(&failed).expect("serialize");
        let back: AuditEffortRunFailedEvent = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back, failed);
    }

    #[test]
    fn audit_route_event_round_trips() {
        let started = AuditRouteRunStartedEvent {
            run_id: "01HW7".into(),
            findings_path: "target/audit/effort/01HW7/findings.jsonl".into(),
            judge_model_id: "mock".into(),
            judge_model_vox_capable: false,
        };
        let j = serde_json::to_string(&started).expect("serialize");
        let back: AuditRouteRunStartedEvent = serde_json::from_str(&j).expect("deserialize");
        assert!(j.contains("01HW7"));
        assert_eq!(back, started);

        let decided = AuditRouteClusterDecidedEvent {
            run_id: "01HW7".into(),
            cluster_id: "01HW7-0".into(),
            member_count: 3,
            artifact_form: "CiGate".into(),
            confidence: 0.82,
            verified: true,
        };
        let j = serde_json::to_string(&decided).expect("serialize");
        let back: AuditRouteClusterDecidedEvent = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back, decided);

        let completed = AuditRouteRunCompletedEvent {
            run_id: "01HW7".into(),
            findings_loaded: 12,
            buckets: 5,
            clusters_routed: 6,
            verified: 4,
        };
        let j = serde_json::to_string(&completed).expect("serialize");
        let back: AuditRouteRunCompletedEvent = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back, completed);

        let failed = AuditRouteRunFailedEvent {
            run_id: "01HW7".into(),
            error_kind: "SchemaMismatch".into(),
            message: "findings.jsonl schema_version 9.9 != 1.0".into(),
        };
        let j = serde_json::to_string(&failed).expect("serialize");
        let back: AuditRouteRunFailedEvent = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back, failed);
    }

    #[test]
    fn audit_route_metric_types_pass_validation() {
        for mt in [
            METRIC_TYPE_AUDIT_ROUTE_RUN_STARTED,
            METRIC_TYPE_AUDIT_ROUTE_CLUSTER_DECIDED,
            METRIC_TYPE_AUDIT_ROUTE_RUN_COMPLETED,
            METRIC_TYPE_AUDIT_ROUTE_RUN_FAILED,
        ] {
            assert!(
                validate_research_metric_row("audit:vox", mt, None).is_ok(),
                "metric_type {mt} failed validation"
            );
        }
    }

    #[test]
    fn audit_effort_metric_types_pass_validation() {
        // All four metric_type constants must pass the session/metric-type
        // char + length validation that gate every research_metrics row.
        for mt in [
            METRIC_TYPE_AUDIT_EFFORT_RUN_STARTED,
            METRIC_TYPE_AUDIT_EFFORT_COMMIT_JUDGED,
            METRIC_TYPE_AUDIT_EFFORT_RUN_COMPLETED,
            METRIC_TYPE_AUDIT_EFFORT_RUN_FAILED,
        ] {
            assert!(
                validate_research_metric_row("audit:vox", mt, None).is_ok(),
                "metric_type {mt} failed validation"
            );
        }
    }

    #[test]
    fn audit_session_helper_composes_correctly() {
        let opts = TelemetryWriteOptions::new("my-repo");
        assert_eq!(opts.session_audit(), "audit:my-repo");
    }

    #[test]
    fn audit_run_metric_type_passes_validation() {
        assert!(
            validate_research_metric_row("audit:vox", METRIC_TYPE_AUDIT_RUN, None).is_ok(),
            "metric_type {METRIC_TYPE_AUDIT_RUN} failed validation"
        );
    }

    #[test]
    fn cr_l8_metric_types_pass_validation() {
        for mt in [
            METRIC_TYPE_LINT_FINDING,
            METRIC_TYPE_LINT_AUTOFIX_APPLIED,
            METRIC_TYPE_LINT_AUTOFIX_REJECTED,
            METRIC_TYPE_REPAIR_ATTEMPT,
            METRIC_TYPE_REPAIR_OUTCOME,
        ] {
            assert!(
                validate_research_metric_row("lint:vox", mt, None).is_ok(),
                "metric_type {mt} failed validation"
            );
        }
    }

    #[test]
    fn cr_l8_session_helpers_compose_correctly() {
        let opts = TelemetryWriteOptions::new("vox");
        assert_eq!(opts.session_lint(), "lint:vox");
        assert_eq!(opts.session_repair(), "repair:vox");
    }

    #[test]
    fn lint_finding_optional_fields_skip_when_none() {
        let e = LintFindingEvent {
            rule_id: "stub".into(),
            diagnostic_id: None,
            severity: "info".into(),
            relative_path: "x.rs".into(),
            line: 1,
            autofix_available: false,
            confidence: None,
            repository_id: None,
        };
        let json = serde_json::to_string(&e).expect("serialize");
        assert!(
            !json.contains("\"diagnostic_id\""),
            "None diagnostic_id should skip in JSON; got {json}"
        );
        assert!(!json.contains("\"confidence\""));
        assert!(!json.contains("\"repository_id\""));
    }

    #[test]
    fn model_call_event_serialize_round_trip() {
        let e = ModelCallEvent {
            model: "claude-opus-4-7".into(),
            provider: "anthropic".into(),
            route_profile: Some("strong".into()),
            prompt_tokens: 1234,
            completion_tokens: 567,
            cache_read_input_tokens: Some(800),
            cache_creation_input_tokens: Some(50),
            latency_ms: 2400,
            cost_usd: 0.0152,
            cost_source: "provider_reported".into(),
            error_class: None,
            retry_attempt: 0,
            task_id: Some(42),
            parent_task_id: None,
            trace_id: Some("abc-123".into()),
            caller_agent_id: None,
        };
        let event = TelemetryEvent::ModelCall(e.clone());
        let json = serde_json::to_string(&event).expect("serialize");
        let back: TelemetryEvent = serde_json::from_str(&json).expect("deserialize");
        let TelemetryEvent::ModelCall(back) = back else {
            panic!("variant lost in round trip")
        };
        assert_eq!(back.model, e.model);
        assert_eq!(back.cache_read_input_tokens, e.cache_read_input_tokens);
        assert_eq!(back.trace_id, e.trace_id);
    }
}
