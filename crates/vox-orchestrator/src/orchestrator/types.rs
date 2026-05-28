use crate::types::{AgentId, TaskId};

#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    /// Orchestrator is turned off via configuration.
    #[error("Orchestrator is disabled")]
    Disabled,
    /// Orchestrator is in emergency stop state.
    #[error("Orchestrator is stopped")]
    Stopped,
    /// No additional agent slots remain.
    #[error("Maximum agents ({max}) reached")]
    MaxAgentsReached {
        /// Configured hard cap on concurrent agents.
        max: usize,
    },
    /// Lookup failed for the given agent id.
    #[error("Agent {0} not found")]
    AgentNotFound(AgentId),
    /// Parent agent for delegation spawn was not found.
    #[error("Delegation parent agent {0} not found")]
    DelegationParentNotFound(AgentId),
    /// Lookup failed for the given task id.
    #[error("Task {0} not found")]
    TaskNotFound(TaskId),
    /// File lock could not be acquired.
    #[error("Lock conflict: {0}")]
    LockConflict(#[from] crate::locks::LockConflict),
    /// Path violated scope / affinity rules.
    #[error("Scope denied: {0}")]
    ScopeDenied(String),
    /// Task was classified as blocked by approval policy.
    #[error("Approval blocked: {0}")]
    ApprovalBlocked(String),
    /// Completion attestation did not satisfy approval policy requirements.
    #[error("Approval attestation required: {0}")]
    ApprovalAttestationRequired(String),
    /// Undo/redo referenced a missing oplog entry.
    #[error("Operation not found")]
    OperationNotFound,
    /// Task behavioral validation failed.
    #[error("Task validation failed: {0}")]
    TaskValidationFailed(String),
    /// Persistent layer failure surfaced to callers.
    #[error("Database error: {0}")]
    DatabaseError(String),
    /// Handoff exceeded its validity window.
    #[error("Handoff from {agent_id} is stale (age: {age_ms}ms, timeout: {timeout_ms}ms)")]
    StaleHandoff {
        /// Sender of the stale handoff.
        agent_id: AgentId,
        /// Calculated age in milliseconds.
        age_ms: u64,
        /// Maximum allowed age before rejection.
        timeout_ms: u64,
    },
    /// Structured handoff invariant validation failed.
    #[error("Handoff invariant failed: {0}")]
    HandoffInvariant(String),
    /// Mesh accepted a lease-gated remote envelope but the local queue could not enter remote-hold (race).
    #[error(
        "Populi remote delegation could not be recorded after mesh accept; remote execution may still be active"
    )]
    PopuliRemoteHoldRace,
    /// Task was blocked due to extreme resource budget constraints.
    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),
    /// Task blocked because the agent appears to be in a doom-loop (cost-without-progress).
    #[error("Doom loop detected: {0}")]
    DoomLoop(String),
}

/// One step in a task's lifecycle timeline (ingress → route → verification → outcome).
/// ORCH-01 SPLIT TARGET: Types (OrchestratorError, TaskTraceStep, OrchestratorStatus, AgentSummary)
/// → move to orchestrator/types.rs when decomposing this file into sub-modules.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskTraceStep {
    /// Pipeline stage name (submit, route, verify, complete, …).
    pub stage: String,
    /// When this step was recorded (Unix ms).
    pub timestamp_ms: u64,
    /// Optional structured payload or error text.
    pub detail: Option<String>,
}

pub const MAX_TASK_TRACES: usize = 200;

/// Snapshot of the orchestrator state for display.
#[derive(Debug, serde::Serialize)]
pub struct OrchestratorStatus {
    /// Whether the orchestrator accepts new work.
    pub enabled: bool,
    /// Registered agents (static + dynamic).
    pub agent_count: usize,
    /// Tasks waiting across all queues.
    pub total_queued: usize,
    /// Tasks currently executing.
    pub total_in_progress: usize,
    /// Tasks finished since start (approximate counter).
    pub total_completed: usize,
    /// Distinct paths under lock.
    pub locked_files: usize,
    /// Aggregate lock wait / conflict events (policy-specific).
    pub total_contention: usize,
    /// Sum of weighted queue depths for scaling heuristics.
    pub total_weighted_load: f64,
    /// Smoothed forecast of near-future load.
    pub predicted_load: f64,
    /// Agents pinned or reserved for scaling policy.
    pub reserved_agents: usize,
    /// Ephemeral agents spawned for burst handling.
    pub dynamic_agents: usize,
    /// Tasks currently in Doubted state.
    pub total_doubted: usize,
    /// Shared context keys visible to dashboards.
    pub context_entries: std::collections::HashMap<String, crate::context::ContextEntry>,
    /// Maximum handoff count observed in any active task across all agents.
    pub max_handoff_count: u8,
    /// Per-agent rollups for UI tables.
    pub agents: Vec<AgentSummary>,
}

/// Summary info for one agent.
#[derive(Debug, serde::Serialize)]
pub struct AgentSummary {
    /// Agent id.
    pub id: AgentId,
    /// Display name.
    pub name: String,
    /// Tasks waiting in this agent's queue.
    pub queued: usize,
    /// Tasks in Doubted state for this agent.
    pub doubted_count: usize,
    /// Urgent-priority backlog depth.
    pub urgent_count: usize,
    /// Normal-priority backlog depth.
    pub normal_count: usize,
    /// Background-priority backlog depth.
    pub background_count: usize,
    /// Whether a task is actively running.
    pub in_progress: bool,
    /// Completed tasks attributed to this agent.
    pub completed: usize,
    /// Operator paused this agent.
    pub paused: bool,
    /// Files this agent currently owns for writing.
    pub owned_files: usize,
    /// True if spawned dynamically for overflow.
    pub dynamic: bool,
    /// Load score combining priorities and in-flight work.
    pub weighted_load: f64,
    /// Linked Codex session id when known.
    pub agent_session_id: Option<String>,
    /// Maximum handoff count observed in this agent's queue.
    pub max_handoff_count: u8,
    /// Active procedural skill (e.g. tdd) for the current task.
    pub active_skill: Option<String>,
    /// Current execution phase (Wave 2 OOPAV).
    pub current_phase: Option<crate::types::TaskPhase>,
}
