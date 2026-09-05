//! Real-time event bus for agent activity broadcasting.
//!
//! Publishes structured `AgentEvent`s over a tokio broadcast channel.
//! Consumers (dashboard SSE, monitors, gamify hooks) subscribe and receive
//! events as they happen — no polling, no JSONL heuristics.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

use crate::types::{AgentId, PrioritySource, TaskId, TaskPriority};

/// Opaque identifier for a hopper intake item (Hp-T1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HopperItemId(pub String);

/// Source of authority for a reprioritization event (Hp-T2, SSOT §3.5).
///
/// `Developer` dominates `Orchestrator` dominates `LearningPolicy`.
///
/// **Deprecated alias.** Use [`PrioritySource`] from `vox-orchestrator-types`
/// directly. This alias is retained for backward compatibility.
pub type ReprioritizationActor = PrioritySource;

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// Monotonically increasing event ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub u64);

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "E-{:06}", self.0)
    }
}

/// What an agent is currently doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActivity {
    /// Writing code / editing files.
    Writing,
    /// Reading / searching files.
    Reading,
    /// Running a command or tool.
    Executing,
    /// Thinking / planning (waiting for LLM response).
    Thinking,
    /// Waiting for user input or permission.
    WaitingForInput,
    /// Idle — no active task.
    Idle,
}

impl std::fmt::Display for AgentActivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Writing => write!(f, "writing"),
            Self::Reading => write!(f, "reading"),
            Self::Executing => write!(f, "executing"),
            Self::Thinking => write!(f, "thinking"),
            Self::WaitingForInput => write!(f, "waiting_for_input"),
            Self::Idle => write!(f, "idle"),
        }
    }
}

impl std::str::FromStr for AgentActivity {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "writing" => Ok(Self::Writing),
            "reading" => Ok(Self::Reading),
            "executing" => Ok(Self::Executing),
            "thinking" => Ok(Self::Thinking),
            "waiting_for_input" => Ok(Self::WaitingForInput),
            "idle" => Ok(Self::Idle),
            _ => Err(format!("unknown activity: {s}")),
        }
    }
}

/// A structured event emitted by the orchestrator.
///
/// Each event carries a unique ID, timestamp, and typed payload.
/// This replaces Pixel Agents' heuristic-based JSONL parsing with
/// deterministic, structured events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    /// Unique event identifier.
    pub id: EventId,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Event payload.
    pub kind: AgentEventKind,
}

/// Compiler stage identifier used in [`AgentEventKind::BuildStage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildStageKind {
    Lex,
    Parse,
    Hir,
    Typecheck,
    Codegen,
}

/// The different kinds of events the orchestrator can emit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEventKind {
    /// A new agent was spawned.
    AgentSpawned {
        agent_id: AgentId,
        name: String,
    },
    /// An agent was retired/removed.
    AgentRetired {
        agent_id: AgentId,
    },
    /// Agent heartbeat received.
    AgentHeartbeat {
        agent_id: AgentId,
        activity: AgentActivity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active_skill: Option<String>,
    },
    /// An agent's activity changed.
    ActivityChanged {
        agent_id: AgentId,
        activity: AgentActivity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active_skill: Option<String>,
    },
    /// An agent's operating mode changed (Strategic, Execution, Verification).
    OperatingModeChanged {
        agent_id: AgentId,
        mode: crate::context_envelope::OperatingMode,
    },
    /// A feedback request was created.
    FeedbackRequested {
        feedback_id: String,
        kind: String,
        gates: Vec<u64>,
        surface: String,
    },
    /// A feedback request was resolved.
    FeedbackResolved {
        feedback_id: String,
    },

    /// A task was submitted to the queue.
    TaskSubmitted {
        task_id: TaskId,
        agent_id: AgentId,
        description: String,
        /// Optional session link (for chat/workflow grouping in Mens).
        session_id: Option<String>,
    },
    /// A task started executing.
    TaskStarted {
        task_id: TaskId,
        agent_id: AgentId,
        /// Optional session link (for chat/workflow grouping in Mens).
        session_id: Option<String>,
    },
    /// A task transitioned to a new execution phase (Inspect, Act, Verify, etc.).
    TaskPhaseChanged {
        task_id: TaskId,
        agent_id: AgentId,
        phase: crate::types::TaskPhase,
    },
    /// A task completed successfully.
    TaskCompleted {
        task_id: TaskId,
        agent_id: AgentId,
        /// Optional session link (for chat/workflow grouping in Mens).
        session_id: Option<String>,
        /// Optional audit report (from Doubt resolution).
        audit_report: Option<String>,
    },
    /// A task failed.
    TaskFailed {
        task_id: TaskId,
        agent_id: AgentId,
        error: String,
        /// Optional session link (for chat/workflow grouping in Mens).
        session_id: Option<String>,
        /// Optional audit report (from Doubt resolution).
        audit_report: Option<String>,
    },

    /// A task was delegated (handed off) from one agent to another.
    TaskDelegated {
        parent_agent_id: AgentId,
        child_agent_id: AgentId,
        task_id: TaskId,
        reason: String,
    },
    /// A task was flagged as suspect by a human user.
    TaskDoubted {
        task_id: TaskId,
        agent_id: AgentId,
        reason: Option<String>,
    },
    /// A suspect task was resolved by the Resolution Agent.
    TaskResolved {
        task_id: TaskId,
        agent_id: AgentId,
        validated: bool,
        report: String,
    },

    /// A tool execution timed out autonomously.
    ToolTimedOut {
        agent_id: AgentId,
        tool_key: String,
        attempted_budget_ms: u64,
    },

    /// An autonomous agent's own `@tool` intent line (emitted while
    /// executing a task) was actually dispatched through the real MCP tool
    /// gate (T1.5 follow-up — `AiTaskProcessor::process` +
    /// `runtime::ToolDispatcher`), as opposed to only being logged as a
    /// tracing breadcrumb.
    ToolCallDispatched {
        task_id: TaskId,
        agent_id: AgentId,
        tool_name: String,
        /// `true` if the dispatch call itself returned `Ok` (the tool result
        /// may still encode a `success: false` `ToolResult` payload inside
        /// that `Ok` string — this only reflects whether dispatch completed
        /// without an `Err`).
        ok: bool,
    },

    /// A file lock was acquired.
    LockAcquired {
        agent_id: AgentId,
        path: PathBuf,
        exclusive: bool,
    },
    /// A file lock was released.
    LockReleased {
        agent_id: AgentId,
        path: PathBuf,
    },

    /// An agent went idle (no pending tasks).
    AgentIdle {
        agent_id: AgentId,
    },
    /// An agent started working again.
    AgentBusy {
        agent_id: AgentId,
    },

    /// An inter-agent message was sent.
    MessageSent {
        from: AgentId,
        to: Option<AgentId>,
        summary: String,
    },

    /// A cost was incurred (LLM API call).
    ///
    /// **MCP:** when Codex is attached, persisted usage is SSOT in `provider_usage`; bus emission is
    /// gated by **`VOX_MCP_LLM_COST_EVENTS`** (default off with DB) to avoid dashboards double-counting.
    CostIncurred {
        agent_id: AgentId,
        provider: String,
        model: String,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: f64,
        /// Structured temporal context (date, server_idle_secs)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        temporal_context: Option<serde_json::Value>,
    },

    /// Global emergency stop triggered.
    EmergencyStop {
        reason: Option<String>,
    },

    /// Auto-continuation was triggered for an idle agent.
    ContinuationTriggered {
        agent_id: AgentId,
        strategy: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active_skill: Option<String>,
    },

    /// A plan handoff between agents.
    PlanHandoff {
        from: AgentId,
        to: AgentId,
        plan_summary: String,
        #[serde(default)]
        has_context_envelope: bool,
        #[serde(default)]
        has_harness_spec: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread_id: Option<String>,
        /// NNT Wave 1: Consumers must sort available agent pools by the shortest affinity distance
        /// from this role when `to` is AgentId(0) (any available agent).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_role: Option<crate::topology::AgentRole>,
    },

    /// A scope violation was detected.
    ScopeViolation {
        agent_id: AgentId,
        path: PathBuf,
        reason: String,
    },

    /// Context window compaction was triggered.
    CompactionTriggered {
        agent_id: AgentId,
        tokens_before: usize,
        tokens_after: usize,
        strategy: String,
    },

    /// Pre-compaction memory flush completed.
    MemoryFlushed {
        agent_id: AgentId,
        facts_flushed: usize,
    },

    /// A new session was created.
    SessionCreated {
        agent_id: AgentId,
        session_id: String,
    },

    /// A session was reset.
    SessionReset {
        agent_id: AgentId,
        session_id: String,
        turns_cleared: usize,
    },

    /// Workspace / DB snapshot captured for undo or conflict tracking.
    SnapshotCaptured {
        agent_id: AgentId,
        snapshot_id: String,
        file_count: usize,
        description: String,
        /// Optional session link (for chat/workflow grouping in Mens).
        session_id: Option<String>,
    },
    /// Overlapping edits detected between agents.
    ConflictDetected {
        path: PathBuf,
        agent_ids: Vec<AgentId>,
        conflict_id: String,
    },
    /// Undo stack applied.
    OperationUndone {
        agent_id: AgentId,
        operation_id: String,
    },
    /// Redo stack applied.
    OperationRedone {
        agent_id: AgentId,
        operation_id: String,
    },
    /// Handoff could not be completed (e.g. spawn failure).
    AgentHandoffRejected {
        from: AgentId,
        reason: String,
    },
    /// Handoff completed; target agent resumed work.
    AgentHandoffAccepted {
        agent_id: AgentId,
        from: AgentId,
        plan_summary: String,
        #[serde(default)]
        has_context_envelope: bool,
        #[serde(default)]
        has_harness_spec: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread_id: Option<String>,
    },
    /// Load balancer moved tasks due to urgent queue depth.
    UrgentRebalanceTriggered {
        moved: usize,
    },
    /// Streaming LLM token chunk (debug / UI).
    TokenStreamed {
        agent_id: AgentId,
        text: String,
        /// Chat session this token belongs to (Task G1). `None` for the
        /// pre-existing background-task streaming path
        /// (`Orchestrator::run_phase_stream_with_bus`), which has no chat
        /// session concept — only the sync `vox_chat_message` path
        /// (`agent_loop::run_agent_turn`) sets this, so the frontend's
        /// `resolveSessionForEvent` can route the frame directly instead of
        /// falling back to the `agentToTask` scan.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    /// Opt-in, non-blocking post-reply grounding/hallucination check
    /// (chat gate policy) completed for a chat task. Never delays the
    /// reply — it is emitted after the reply has already streamed.
    GroundingCheckCompleted {
        agent_id: AgentId,
        task_id: TaskId,
        confidence: f64,
        flagged: bool,
    },

    /// Prompt injection / safety gate rejected input (MCP).
    InjectionDetected {
        detail: String,
    },
    /// Canonicalized prompt produced warnings (MCP task submit).
    PromptConflictDetected {
        task_id: TaskId,
        warnings: Vec<String>,
    },
    /// Planning router chose a strategy for a submitted goal.
    PlanningRouted {
        strategy: String,
        complexity: u8,
        confidence: f32,
        rationale: String,
    },
    /// A new plan session was created.
    PlanSessionCreated {
        plan_session_id: String,
        strategy: String,
        version: i64,
    },
    /// A branch/replan version was created.
    PlanVersionCreated {
        plan_session_id: String,
        version: i64,
        parent_version: Option<i64>,
    },
    /// Failure triggered a replan branch.
    ReplanTriggered {
        plan_session_id: String,
        node_id: String,
        reason: String,
        next_version: i64,
    },
    /// Planner requested workflow runtime handoff.
    WorkflowHandoffRequested {
        plan_session_id: String,
        workflow_name: String,
    },
    /// Workflow handoff finished and yielded a task id.
    WorkflowHandoffCompleted {
        plan_session_id: String,
        task_id: u64,
    },
    /// Durable workflow lifecycle (MCP / dashboard).
    WorkflowStarted {
        workflow_id: String,
    },
    WorkflowCompleted {
        workflow_id: String,
    },
    WorkflowFailed {
        workflow_id: String,
        error: String,
    },
    ActivityStarted {
        activity_id: String,
    },
    ActivityCompleted {
        activity_id: String,
    },
    ActivityRetried {
        activity_id: String,
        attempt: u32,
    },
    ConflictResolved {
        conflict_id: String,
        resolution_strategy: String,
    },
    WorkspaceCreated {
        agent_id: AgentId,
        root: PathBuf,
    },
    /// Endpoint reliability observation from LLM call (feeds EWMA in Codex).
    EndpointReliabilityObservation {
        /// Provider endpoint URL.
        endpoint_url: String,
        /// Model identifier.
        model_id: String,
        /// Proxy signal for hallucination risk (0.0–1.0).
        hallucination_proxy: f64,
        /// Ratio of contradictory claims detected (0.0–1.0).
        contradiction_ratio: f64,
        /// 1.0 for infra failures (rate-limit/timeout), 0.0 otherwise.
        infra_failure: f64,
        /// True when the failure was a rate-limit response.
        rate_limit_hit: bool,
        /// True when the call timed out.
        timeout_hit: bool,
    },
    /// The entire orchestrator (all agents) has been idle.
    OrchestratorIdle {
        /// Milliseconds of absolute silence across all agents.
        idle_ms: u64,
    },
    /// A task was timed out and removed from the queue.
    TaskExpired {
        /// Expired task ID.
        task_id: TaskId,
        /// Agent ID that was holding it.
        agent_id: AgentId,
        /// Age in milliseconds.
        age_ms: u64,
    },

    /// OAPV Observer anomaly detected (mens_observer_observations) for MCP dashboards.
    MensObserverObservation {
        agent_id: AgentId,
        observation_type: String,
        queue_depth: usize,
    },

    /// AST Healing was applied automatically on a file.
    AutoHealApplied {
        agent_id: AgentId,
        /// Path of the healed file
        path: PathBuf,
        /// Description of the healing action
        description: String,
        /// The new source after healing
        new_source: String,
    },
    /// A fix was suggested automatically for a diagnostic error.
    AutoHealSuggested {
        agent_id: AgentId,
        path: PathBuf,
        diagnostic: String,
        fix_suggestion: String,
    },

    /// Attention budget threshold alert
    AttentionBudgetAlert {
        agent_id: AgentId,
        threshold: f64,
        spent_ms: u64,
        max_ms: u64,
    },
    /// A general budget alert for tokens/cost self-correction
    BudgetAlert {
        agent_id: AgentId,
        signal: crate::budget::BudgetSignal,
    },
    /// Attention budget was explicitly reset
    AttentionBudgetReset {
        agent_id: AgentId,
        new_max_ms: u64,
        reason: String,
    },
    /// Agent trust level was manually overridden
    TrustOverride {
        agent_id: AgentId,
        tier: String,
        reason: String,
    },
    /// Attention policy configuration was hot-reloaded
    AttentionConfigReloaded,

    /// Warning emitted when context bytes are dropped due to limits
    ContextTruncated {
        session_id: String,
        section: String,
        chars_dropped: usize,
    },

    /// LLM request completed inside planning or context phases
    LlmCallCompleted {
        session_id: String,
        duration_ms: u64,
        prompt_tokens: u32,
        completion_tokens: u32,
    },
    /// Observer recorded a structural health report for a file (Task 56).
    ObservationRecorded {
        agent_id: AgentId,
        task_id: TaskId,
        file_path: std::path::PathBuf,
        lsp_error_count: usize,
        parse_rate: f32,
        construct_coverage: f32,
        recommended_action: String,
    },
    /// The Orient phase completed its risk analysis.
    OrientCompleted {
        agent_id: AgentId,
        task_id: TaskId,
        risk_band: String,
        evidence_gap: f64,
    },
    /// Autonomous research was performed (Tavily).
    ResearchExecuted {
        agent_id: Option<AgentId>,
        task_id: Option<TaskId>,
        queries: Vec<String>,
        results_count: usize,
    },
    /// Lane G synthesized research evidence.
    ResearchSynthesisExecuted {
        agent_id: Option<AgentId>,
        task_id: Option<TaskId>,
        model_id: String,
        provider: String,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: f64,
        content_preview: String,
    },
    /// A task was flagged for potential drift or doubt (Doom-loop protection).
    DoubtReported {
        agent_id: AgentId,
        task_id: TaskId,
        reason: String,
    },
    /// Semantic drift was confirmed; agent may be halted.
    SemanticDriftDetected {
        agent_id: AgentId,
        iterations: usize,
        cost_usd: f64,
    },

    // -----------------------------------------------------------------------
    // Dashboard live-data variants (Task 0.6)
    // -----------------------------------------------------------------------
    /// A compiler stage transitioned (lex → parse → hir → typecheck → codegen).
    ///
    /// Powers the Forge pipeline view on the dashboard.
    BuildStage {
        run_id: String,
        stage: BuildStageKind,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        diagnostic_count: u32,
    },
    /// Throttled ~1 Hz throughput tick for mesh activity / status-bar queue indicator.
    ///
    /// Powers the Mesh activity strip and status-bar queue indicator.
    ThroughputTick {
        ts_ms: u64,
        tokens_per_sec: f32,
        active_runs: u32,
    },
    /// Per-model cost tick emitted when cost crosses a batching boundary.
    ///
    /// Powers the Models cost horizon and status-bar cost display.
    CostTick {
        ts_ms: u64,
        delta_usd: f64,
        total_24h_usd: f64,
        model: String,
    },
    /// A file's diagnostic counts changed.
    ///
    /// Powers the Code surface file-tree diagnostic dots.
    FileDiagChanged {
        path: String,
        error_count: u32,
        warn_count: u32,
    },
    /// Mesh topology changed: agents joined/left or an edge became active/inactive.
    ///
    /// Powers the Mesh topology re-render on the dashboard.
    MeshTopologyChanged {
        added_nodes: Vec<String>,
        removed_nodes: Vec<String>,
        changed_edges: u32,
    },

    // -----------------------------------------------------------------------
    // Hopper variants (Hp-T2, SSOT §3.5)
    // -----------------------------------------------------------------------
    /// Emitted when a developer or policy reorders a task in flight.
    TaskReprioritized {
        task_id: TaskId,
        old_priority: TaskPriority,
        new_priority: TaskPriority,
        actor: ReprioritizationActor,
        reason: Option<String>,
        session_id: Option<String>,
    },

    /// Emitted when the hopper admits an intake item and binds it to an agent queue.
    HopperItemAdmitted {
        item_id: HopperItemId,
        classified_priority: TaskPriority,
        classified_affinity: Vec<PathBuf>,
        confidence: f32,
        session_id: Option<String>,
    },

    /// Emitted when a developer overrides the orchestrator's classified priority.
    HopperItemOverridden {
        item_id: HopperItemId,
        original_priority: TaskPriority,
        developer_priority: TaskPriority,
        delta_seconds_since_admit: u64,
    },

    /// Emitted when a developer cancels an item in the hopper.
    HopperItemCancelled {
        item_id: HopperItemId,
    },

    // -----------------------------------------------------------------------
    // Mesh spend + action events (P4-T1, P4-T6, P4-T7)
    // -----------------------------------------------------------------------
    /// Per-node budget tick — emitted at most once per second per node.
    /// Powers the spend gauges (P4-T6) on the topology canvas.
    MeshNodeBudget {
        node_id: String,
        cost_usd_24h: f64,
        cost_cap_usd: f64,
        token_count_24h: u64,
    },

    /// A destructive mesh action (kill/pause/drain/replay) was committed.
    /// Always paired with a signed audit-log entry.
    MeshActionCommitted {
        node_id: String,
        action: MeshAction,
        actor: String,
        signed_audit_id: String,
    },

    // -----------------------------------------------------------------------
    // Plan/Act/Verify loop (Track D)
    // -----------------------------------------------------------------------
    /// A task's PAV (Plan/Act/Verify) phase advanced. Distinct from
    /// `TaskPhaseChanged` which carries the OOPAV debug phase (`TaskPhase`).
    /// The GUI subscribes via the same agent-events broadcast stream.
    PavPhaseChanged {
        task_id: TaskId,
        phase: crate::planning::phase_loop::PavPhase,
    },
}

/// Destructive mesh actions that trigger a confirmation modal and signed audit-log entry.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshAction {
    Kill,
    Pause,
    Drain,
    Replay,
}

// ---------------------------------------------------------------------------
// Tier classification (T1.2: two-tier append-before-broadcast)
// ---------------------------------------------------------------------------
//
// **Tier A** (durable-before-broadcast): dispatch-lifecycle transitions that
// T1.1 wired into the durable op-log via `Orchestrator::record_operation` —
// task submit/complete/fail, HITL approval request/resolve, soft-HITL
// feedback request/resolve, task-doubt, and hopper admit/assign/complete.
// These fire at most a few times per second, so paying an oplog append (and,
// via `append_to_db_with_breaker`, an eventual durable write) per event is
// cheap. Production call sites MUST durably record a Tier-A transition
// *before* broadcasting the corresponding `AgentEventKind` on the
// [`EventBus`] — see `Orchestrator::record_operation`'s doc comment and the
// call sites in `vox-orchestrator-mcp/src/{dispatch,feedback_tools}.rs`,
// `vox-orchestrator/src/orchestrator/{task_dispatch,agent/doubt,dispatch}.rs`,
// and `vox-orchestrator/src/orch_daemon/mod.rs`.
//
// **Tier B** (broadcast-only, never durably journaled): everything else —
// `TokenStreamed`, heartbeats, throughput/cost ticks, lock/diag chatter, and
// any other high-frequency or purely observational event. These can fire
// hundreds of times per second (e.g. one `TokenStreamed` per LLM token);
// fsync-per-append here was explicitly rejected as a design (it would bloat
// the op-log and stall the hot path on disk I/O). Tier-B events are pure
// broadcast: no durable record, no undo/redo, no replay. The recovery
// contract for a crashed/restarted daemon is "resume from the last committed
// Tier-A transition" — never mid-token, never mid-heartbeat.
//
// `AgentEventKind` is not `#[non_exhaustive]`, so [`is_tier_a`] is written as
// an exhaustive match: adding a new variant without extending this match is a
// compile error, which is the enforcement mechanism (a future contributor is
// forced to make an explicit tier decision rather than silently defaulting).

/// Returns `true` if `kind` is a **Tier A** event: a dispatch-lifecycle
/// transition that must be durably recorded (via `Orchestrator::record_operation`)
/// *before* being broadcast on the [`EventBus`]. See the module-level tier
/// documentation above.
#[must_use]
pub fn is_tier_a(kind: &AgentEventKind) -> bool {
    match kind {
        // Task lifecycle — mirrors OperationKind::{TaskSubmit,TaskComplete,TaskFail}.
        AgentEventKind::TaskSubmitted { .. }
        | AgentEventKind::TaskCompleted { .. }
        | AgentEventKind::TaskFailed { .. }
        // Soft-HITL feedback — mirrors OperationKind::{FeedbackRequested,FeedbackResolved}.
        | AgentEventKind::FeedbackRequested { .. }
        | AgentEventKind::FeedbackResolved { .. }
        // Doom-loop doubt — mirrors OperationKind::TaskDoubted.
        | AgentEventKind::TaskDoubted { .. }
        // Hopper admit — mirrors OperationKind::HopperAdmit (HopperAssign/HopperComplete
        // have no direct AgentEventKind broadcast counterpart today).
        | AgentEventKind::HopperItemAdmitted { .. } => true,

        // Everything else is Tier B: broadcast-only, never durably journaled.
        AgentEventKind::AgentSpawned { .. }
        | AgentEventKind::AgentRetired { .. }
        | AgentEventKind::AgentHeartbeat { .. }
        | AgentEventKind::ActivityChanged { .. }
        | AgentEventKind::OperatingModeChanged { .. }
        | AgentEventKind::TaskStarted { .. }
        | AgentEventKind::TaskPhaseChanged { .. }
        | AgentEventKind::TaskDelegated { .. }
        | AgentEventKind::TaskResolved { .. }
        | AgentEventKind::ToolTimedOut { .. }
        | AgentEventKind::LockAcquired { .. }
        | AgentEventKind::LockReleased { .. }
        | AgentEventKind::AgentIdle { .. }
        | AgentEventKind::AgentBusy { .. }
        | AgentEventKind::MessageSent { .. }
        | AgentEventKind::CostIncurred { .. }
        | AgentEventKind::EmergencyStop { .. }
        | AgentEventKind::ContinuationTriggered { .. }
        | AgentEventKind::PlanHandoff { .. }
        | AgentEventKind::ScopeViolation { .. }
        | AgentEventKind::CompactionTriggered { .. }
        | AgentEventKind::MemoryFlushed { .. }
        | AgentEventKind::SessionCreated { .. }
        | AgentEventKind::SessionReset { .. }
        | AgentEventKind::SnapshotCaptured { .. }
        | AgentEventKind::ConflictDetected { .. }
        | AgentEventKind::OperationUndone { .. }
        | AgentEventKind::OperationRedone { .. }
        | AgentEventKind::AgentHandoffRejected { .. }
        | AgentEventKind::AgentHandoffAccepted { .. }
        | AgentEventKind::UrgentRebalanceTriggered { .. }
        | AgentEventKind::TokenStreamed { .. }
        | AgentEventKind::InjectionDetected { .. }
        | AgentEventKind::PromptConflictDetected { .. }
        | AgentEventKind::PlanningRouted { .. }
        | AgentEventKind::PlanSessionCreated { .. }
        | AgentEventKind::PlanVersionCreated { .. }
        | AgentEventKind::ReplanTriggered { .. }
        | AgentEventKind::WorkflowHandoffRequested { .. }
        | AgentEventKind::WorkflowHandoffCompleted { .. }
        | AgentEventKind::WorkflowStarted { .. }
        | AgentEventKind::WorkflowCompleted { .. }
        | AgentEventKind::WorkflowFailed { .. }
        | AgentEventKind::ActivityStarted { .. }
        | AgentEventKind::ActivityCompleted { .. }
        | AgentEventKind::ActivityRetried { .. }
        | AgentEventKind::ConflictResolved { .. }
        | AgentEventKind::WorkspaceCreated { .. }
        | AgentEventKind::EndpointReliabilityObservation { .. }
        | AgentEventKind::OrchestratorIdle { .. }
        | AgentEventKind::TaskExpired { .. }
        | AgentEventKind::MensObserverObservation { .. }
        | AgentEventKind::AutoHealApplied { .. }
        | AgentEventKind::AutoHealSuggested { .. }
        | AgentEventKind::AttentionBudgetAlert { .. }
        | AgentEventKind::BudgetAlert { .. }
        | AgentEventKind::AttentionBudgetReset { .. }
        | AgentEventKind::TrustOverride { .. }
        | AgentEventKind::AttentionConfigReloaded
        | AgentEventKind::ContextTruncated { .. }
        | AgentEventKind::LlmCallCompleted { .. }
        | AgentEventKind::ObservationRecorded { .. }
        | AgentEventKind::OrientCompleted { .. }
        | AgentEventKind::ResearchExecuted { .. }
        | AgentEventKind::ResearchSynthesisExecuted { .. }
        | AgentEventKind::DoubtReported { .. }
        | AgentEventKind::SemanticDriftDetected { .. }
        | AgentEventKind::BuildStage { .. }
        | AgentEventKind::ThroughputTick { .. }
        | AgentEventKind::CostTick { .. }
        | AgentEventKind::FileDiagChanged { .. }
        | AgentEventKind::MeshTopologyChanged { .. }
        | AgentEventKind::TaskReprioritized { .. }
        | AgentEventKind::HopperItemOverridden { .. }
        | AgentEventKind::HopperItemCancelled { .. }
        | AgentEventKind::MeshNodeBudget { .. }
        | AgentEventKind::MeshActionCommitted { .. }
        | AgentEventKind::PavPhaseChanged { .. }
        | AgentEventKind::ToolCallDispatched { .. }
        | AgentEventKind::GroundingCheckCompleted { .. } => false,
    }
}

/// Bare variant names of every Tier-B `AgentEventKind` (the `false` arms of
/// [`is_tier_a`]), for consumers that need the set as strings rather than
/// pattern-matchable values (e.g. `tests/tier_b_never_durable_guard.rs`'s
/// source-grep, which checks for these names appearing as `OperationKind::`
/// constructor arguments at durable-write call sites — `OperationKind` is a
/// different, `#[non_exhaustive]` enum in another crate with no structural
/// link to `AgentEventKind`, so this can't be derived via a shared trait/derive
/// without adding an `EnumIter`-style dependency).
///
/// **Keep in sync with [`is_tier_a`]'s match arms by hand** — there is no
/// compiler enforcement linking this list to the match. If you add a new
/// `AgentEventKind` variant, update `is_tier_a`'s match (compiler-enforced,
/// since it's exhaustive) *and*, if the variant is Tier B, add its name here.
#[rustfmt::skip]
pub const TIER_B_KIND_NAMES: &[&str] = &[
    "AgentSpawned", "AgentRetired", "AgentHeartbeat", "ActivityChanged",
    "OperatingModeChanged", "TaskStarted", "TaskPhaseChanged", "TaskDelegated",
    "TaskResolved", "ToolTimedOut", "LockAcquired", "LockReleased", "AgentIdle",
    "AgentBusy", "MessageSent", "CostIncurred", "EmergencyStop",
    "ContinuationTriggered", "PlanHandoff", "ScopeViolation",
    "CompactionTriggered", "MemoryFlushed", "SessionCreated", "SessionReset",
    "SnapshotCaptured", "ConflictDetected", "OperationUndone", "OperationRedone",
    "AgentHandoffRejected", "AgentHandoffAccepted", "UrgentRebalanceTriggered",
    "TokenStreamed", "InjectionDetected", "PromptConflictDetected",
    "PlanningRouted", "PlanSessionCreated", "PlanVersionCreated",
    "ReplanTriggered", "WorkflowHandoffRequested", "WorkflowHandoffCompleted",
    "WorkflowStarted", "WorkflowCompleted", "WorkflowFailed", "ActivityStarted",
    "ActivityCompleted", "ActivityRetried", "ConflictResolved",
    "WorkspaceCreated", "EndpointReliabilityObservation", "OrchestratorIdle",
    "TaskExpired", "MensObserverObservation", "AutoHealApplied",
    "AutoHealSuggested", "AttentionBudgetAlert", "BudgetAlert",
    "AttentionBudgetReset", "TrustOverride", "AttentionConfigReloaded",
    "ContextTruncated", "LlmCallCompleted", "ObservationRecorded",
    "OrientCompleted", "ResearchExecuted", "ResearchSynthesisExecuted",
    "DoubtReported", "SemanticDriftDetected", "BuildStage", "ThroughputTick",
    "CostTick", "FileDiagChanged", "MeshTopologyChanged", "TaskReprioritized",
    "HopperItemOverridden", "HopperItemCancelled", "MeshNodeBudget",
    "MeshActionCommitted", "PavPhaseChanged", "ToolCallDispatched",
    "GroundingCheckCompleted",
];

/// Returns `true` if `kind` is a **Tier B** event (broadcast-only, never
/// durably journaled). The exact complement of [`is_tier_a`].
#[must_use]
pub fn is_tier_b(kind: &AgentEventKind) -> bool {
    !is_tier_a(kind)
}

// ---------------------------------------------------------------------------
// Event bus
// ---------------------------------------------------------------------------

/// Thread-safe event bus for broadcasting agent events.
///
/// Uses a tokio broadcast channel under the hood. Multiple consumers
/// (dashboard, monitor, gamify hooks) can subscribe independently.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
    id_gen: std::sync::Arc<AtomicU64>,
}

impl EventBus {
    /// Create a new event bus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            id_gen: std::sync::Arc::new(AtomicU64::new(1)),
        }
    }

    /// Emit an event. Returns the assigned EventId.
    pub fn emit(&self, kind: AgentEventKind) -> EventId {
        let id = EventId(self.id_gen.fetch_add(1, Ordering::Relaxed));
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let event = AgentEvent {
            id,
            timestamp_ms,
            kind,
        };

        tracing::debug!(event_id = %id, "event emitted: {:?}", event.kind);
        let _ = self.sender.send(event);
        id
    }

    /// Subscribe to events. Returns a receiver for all future events.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get the next event ID that will be assigned.
    pub fn next_event_id(&self) -> u64 {
        self.id_gen.load(Ordering::Relaxed)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn emit_and_receive() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let id = bus.emit(AgentEventKind::AgentSpawned {
            agent_id: AgentId(1),
            name: "builder".to_string(),
        });

        assert_eq!(id, EventId(1));

        let event = tokio::time::timeout(vox_config::timeouts::D_100MS, rx.recv())
            .await
            .expect("should not timeout")
            .expect("should receive");

        assert_eq!(event.id, EventId(1));
        assert!(event.timestamp_ms > 0);
        match event.kind {
            AgentEventKind::AgentSpawned { agent_id, name } => {
                assert_eq!(agent_id, AgentId(1));
                assert_eq!(name, "builder");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn multiple_subscribers_receive() {
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        bus.emit(AgentEventKind::AgentIdle {
            agent_id: AgentId(2),
        });

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[test]
    fn event_serialization_roundtrip() {
        let event = AgentEvent {
            id: EventId(42),
            timestamp_ms: 1234567890,
            kind: AgentEventKind::CostIncurred {
                agent_id: AgentId(1),
                provider: "openrouter".to_string(),
                model: "claude-3".to_string(),
                input_tokens: 100,
                output_tokens: 50,
                cost_usd: 0.005,
                temporal_context: None,
            },
        };

        let json = serde_json::to_string(&event).expect("serialize");
        let back: AgentEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, EventId(42));
    }

    #[test]
    fn sequential_event_ids() {
        let bus = EventBus::new(16);
        let id1 = bus.emit(AgentEventKind::AgentIdle {
            agent_id: AgentId(1),
        });
        let id2 = bus.emit(AgentEventKind::AgentBusy {
            agent_id: AgentId(1),
        });
        assert_eq!(id1, EventId(1));
        assert_eq!(id2, EventId(2));
    }

    #[test]
    fn feedback_events_serialize_snake_case_tag() {
        let e = AgentEventKind::FeedbackRequested {
            feedback_id: "F-000001".into(),
            kind: "clarification".into(),
            gates: vec![7],
            surface: "needs_you".into(),
        };
        let j = serde_json::to_string(&e).unwrap();
        assert!(j.contains("\"type\":\"feedback_requested\"")); // NOT "FeedbackRequested"
        assert!(j.contains("needs_you"));
    }

    #[tokio::test]
    async fn pav_phase_changed_event_fires_on_advance() {
        use crate::planning::phase_loop::{PavPhase, PhaseLoop};
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let task_id = TaskId(999);
        let mut loop_ = PhaseLoop::new();

        // Advance Planning → Acting
        loop_.advance_to_acting();
        bus.emit(AgentEventKind::PavPhaseChanged {
            task_id,
            phase: loop_.phase(),
        });

        let event = tokio::time::timeout(vox_config::timeouts::D_100MS, rx.recv())
            .await
            .expect("should not timeout")
            .expect("received");

        match event.kind {
            AgentEventKind::PavPhaseChanged {
                task_id: tid,
                phase,
            } => {
                assert_eq!(tid, TaskId(999));
                assert_eq!(phase, PavPhase::Acting);
            }
            _ => panic!("expected PavPhaseChanged, got {:?}", event.kind),
        }
    }

    #[test]
    fn pav_phase_changed_serializes_cleanly() {
        use crate::planning::phase_loop::PavPhase;
        let event = AgentEvent {
            id: EventId(77),
            timestamp_ms: 1_000_000,
            kind: AgentEventKind::PavPhaseChanged {
                task_id: TaskId(42),
                phase: PavPhase::Verifying,
            },
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            json.contains("pav_phase_changed"),
            "serde tag missing: {json}"
        );
        assert!(json.contains("verifying"), "phase value missing: {json}");
        let back: AgentEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, EventId(77));
    }

    // -----------------------------------------------------------------------
    // T1.2: tier classification
    // -----------------------------------------------------------------------

    #[test]
    fn token_streamed_is_tier_b() {
        let kind = AgentEventKind::TokenStreamed {
            agent_id: AgentId(1),
            text: "hello".into(),
            session_id: None,
        };
        assert!(
            is_tier_b(&kind),
            "TokenStreamed must be Tier B (broadcast-only, never durably journaled)"
        );
        assert!(!is_tier_a(&kind), "is_tier_a must be the exact complement");
    }

    /// Task G1: the sync chat path needs `TokenStreamed` to carry `session_id`
    /// so the GUI's `resolveSessionForEvent` can route a frame directly to the
    /// chat bubble, instead of only being reachable via the background-task
    /// `agentToTask` scan. Field must serialize (not `skip_serializing_if`
    /// away a `Some`) so the wire frame the GUI reads actually carries it.
    #[test]
    fn token_streamed_carries_session_id_when_set() {
        let event = AgentEvent {
            id: EventId(1),
            timestamp_ms: 0,
            kind: AgentEventKind::TokenStreamed {
                agent_id: AgentId(1),
                text: "hi".into(),
                session_id: Some("sess-42".into()),
            },
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            json.contains("\"session_id\":\"sess-42\""),
            "session_id must be present on the wire frame: {json}"
        );
        let back: AgentEvent = serde_json::from_str(&json).expect("deserialize");
        match back.kind {
            AgentEventKind::TokenStreamed { session_id, .. } => {
                assert_eq!(session_id.as_deref(), Some("sess-42"));
            }
            other => panic!("expected TokenStreamed, got {other:?}"),
        }
    }

    /// `session_id: None` (the pre-existing background-task streaming path)
    /// must NOT serialize a `session_id` key at all — additive change,
    /// existing consumers that don't know the field must see the same wire
    /// shape as before.
    #[test]
    fn token_streamed_omits_session_id_when_absent() {
        let event = AgentEvent {
            id: EventId(2),
            timestamp_ms: 0,
            kind: AgentEventKind::TokenStreamed {
                agent_id: AgentId(1),
                text: "hi".into(),
                session_id: None,
            },
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            !json.contains("session_id"),
            "session_id must be omitted from the wire frame when None: {json}"
        );
    }

    #[test]
    fn task_completed_is_tier_a() {
        let kind = AgentEventKind::TaskCompleted {
            task_id: TaskId(1),
            agent_id: AgentId(1),
            session_id: None,
            audit_report: None,
        };
        assert!(
            is_tier_a(&kind),
            "TaskCompleted must be Tier A (durable-before-broadcast): it mirrors \
             OperationKind::TaskComplete, which T1.1 durably records"
        );
        assert!(!is_tier_b(&kind), "is_tier_b must be the exact complement");
    }

    #[test]
    fn t1_1_durable_variants_are_all_tier_a() {
        // Every AgentEventKind that mirrors a T1.1 durable OperationKind
        // (task lifecycle, feedback, doubt, hopper-admit) must classify Tier A.
        let tier_a_kinds = [
            AgentEventKind::TaskSubmitted {
                task_id: TaskId(1),
                agent_id: AgentId(1),
                description: "d".into(),
                session_id: None,
            },
            AgentEventKind::TaskCompleted {
                task_id: TaskId(1),
                agent_id: AgentId(1),
                session_id: None,
                audit_report: None,
            },
            AgentEventKind::TaskFailed {
                task_id: TaskId(1),
                agent_id: AgentId(1),
                error: "e".into(),
                session_id: None,
                audit_report: None,
            },
            AgentEventKind::FeedbackRequested {
                feedback_id: "F-1".into(),
                kind: "doubt".into(),
                gates: vec![],
                surface: "needs_you".into(),
            },
            AgentEventKind::FeedbackResolved {
                feedback_id: "F-1".into(),
            },
            AgentEventKind::TaskDoubted {
                task_id: TaskId(1),
                agent_id: AgentId(1),
                reason: None,
            },
            AgentEventKind::HopperItemAdmitted {
                item_id: HopperItemId("hp-1".into()),
                classified_priority: TaskPriority::Normal,
                classified_affinity: vec![],
                confidence: 0.5,
                session_id: None,
            },
        ];
        for kind in &tier_a_kinds {
            assert!(is_tier_a(kind), "expected Tier A: {kind:?}");
        }
    }

    #[test]
    fn high_frequency_kinds_are_all_tier_b() {
        // Heartbeats, throughput/cost ticks, lock/diag chatter must never be Tier A
        // (would defeat the whole point of keeping the durable oplog low-frequency).
        let tier_b_kinds = [
            AgentEventKind::TokenStreamed {
                agent_id: AgentId(1),
                text: "x".into(),
                session_id: None,
            },
            AgentEventKind::AgentHeartbeat {
                agent_id: AgentId(1),
                activity: AgentActivity::Idle,
                active_skill: None,
            },
            AgentEventKind::ThroughputTick {
                ts_ms: 0,
                tokens_per_sec: 0.0,
                active_runs: 0,
            },
            AgentEventKind::CostTick {
                ts_ms: 0,
                delta_usd: 0.0,
                total_24h_usd: 0.0,
                model: "m".into(),
            },
            AgentEventKind::LockAcquired {
                agent_id: AgentId(1),
                path: PathBuf::from("f"),
                exclusive: true,
            },
            AgentEventKind::LockReleased {
                agent_id: AgentId(1),
                path: PathBuf::from("f"),
            },
        ];
        for kind in &tier_b_kinds {
            assert!(is_tier_b(kind), "expected Tier B: {kind:?}");
        }
    }
}
