//! Task priority, status, categories, and agent task model.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::Instant;

use super::ids::{TaskId, is_zero_f64, now_unix_ms};

fn default_victory_condition() -> crate::VictoryCondition {
    crate::VictoryCondition::CompilationOnly
}

/// Maximum number of times a task can be handed off before it is considered an infinite loop.
pub const MAX_A2A_BOUNCE: u8 = 5;

/// Financial and temporal budget constraints for a task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Budget {
    /// Maximum allowed cost for the task in USD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cost_usd: Option<f64>,
    /// Maximum allowed wall-clock latency for the task in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_latency_ms: Option<u64>,
}

/// One turn in a task's conversational history (for agent-to-agent context).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTurn {
    /// Agent that performed this turn.
    pub agent_id: super::ids::AgentId,
    /// Human-readable agent name.
    pub agent_name: String,
    /// Final condensed summary/report from the agent.
    pub message: String,
    /// Unix timestamp (ms) when turn was recorded.
    pub timestamp_ms: u64,
}

/// Priority level for a task. Higher priority tasks are dequeued first.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Background work — lowest priority.
    Background = 0,
    /// Normal priority — default.
    Normal = 1,
    /// Urgent — preempts normal tasks.
    Urgent = 2,
}

impl TaskPriority {
    /// Map a wire `u8` (e.g. the `priority` field of a `HopperSync` op) to a
    /// `TaskPriority`. Matches the enum's `repr` (0/1/2); any out-of-range value
    /// falls back to `Normal` rather than failing — replication should degrade
    /// gracefully, not drop an admission.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Background,
            2 => Self::Urgent,
            _ => Self::Normal,
        }
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Background => write!(f, "background"),
            Self::Normal => write!(f, "normal"),
            Self::Urgent => write!(f, "urgent"),
        }
    }
}

/// Current execution status of a task.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Waiting in the queue to be picked up.
    Queued,
    /// Currently being executed by an agent.
    InProgress,
    /// Successfully completed.
    Completed,
    /// Failed with an error reason.
    Failed(String),
    /// Blocked waiting for another task to complete.
    Blocked(TaskId),
    /// Blocked waiting for human approval.
    BlockedOnApproval,
    /// Explicitly cancelled by user or system.
    Cancelled,
    /// Flagged by a human as "Suspect", awaiting high-audit resolution.
    Doubted(Option<String>),
}

/// Execution phase of the agentic loop (OOPAV).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskPhase {
    /// Initial environment and task inspection.
    Inspect,
    /// Localizing the problem to specific files or code blocks.
    Localize,
    /// Forming a hypothesis for the fix or implementation.
    Hypothesize,
    /// Performing the actual code modification or tool execution.
    Act,
    /// Verifying the results (e.g. running tests).
    Verify,
    /// Final decision and summary generation.
    Decide,
}

impl TaskPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inspect => "inspect",
            Self::Localize => "localize",
            Self::Hypothesize => "hypothesize",
            Self::Act => "act",
            Self::Verify => "verify",
            Self::Decide => "decide",
        }
    }
}

impl std::str::FromStr for TaskPhase {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "inspect" => Ok(Self::Inspect),
            "localize" => Ok(Self::Localize),
            "hypothesize" => Ok(Self::Hypothesize),
            "act" => Ok(Self::Act),
            "verify" => Ok(Self::Verify),
            "decide" => Ok(Self::Decide),
            _ => Err(format!("Unknown TaskPhase: {}", s)),
        }
    }
}

impl fmt::Display for TaskPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::InProgress => write!(f, "in-progress"),
            Self::Completed => write!(f, "completed"),
            Self::Failed(reason) => write!(f, "failed: {}", reason),
            Self::Blocked(dep) => write!(f, "blocked on {}", dep),
            Self::BlockedOnApproval => write!(f, "blocked on approval"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Doubted(reason) => {
                if let Some(r) = reason {
                    write!(f, "doubted: {}", r)
                } else {
                    write!(f, "doubted")
                }
            }
        }
    }
}

/// Kind of access an agent requires on a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessKind {
    /// Read-only access (multiple agents can hold simultaneously).
    Read,
    /// Exclusive write access (only one agent at a time).
    Write,
}

/// A file path paired with the access kind required for a task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileAffinity {
    /// Path the task touches.
    pub path: PathBuf,
    /// Required lock / sharing mode.
    pub access: AccessKind,
}

impl FileAffinity {
    /// Read-only affinity for `path`.
    pub fn read(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            access: AccessKind::Read,
        }
    }

    /// Exclusive write affinity for `path`.
    pub fn write(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            access: AccessKind::Write,
        }
    }
}

pub use crate::models::generated::TaskCategory;

/// Populi mesh holds execution authority for this task; local actors must not dequeue it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PopuliRemoteDelegate {
    /// Same key as [`crate::a2a::RemoteTaskEnvelope::idempotency_key`] for cancel/result correlation.
    pub idempotency_key: String,
    /// Populi execution lease id when lease APIs are active for this task class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_id: Option<String>,
    /// Claimer node identity used for lease renew/release calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimer_node_id: Option<String>,
}

/// Optional hints applied at enqueue time and merged into [`AgentTask`] for routing / telemetry.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TaskEnqueueHints {
    /// When set, overrides default task category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_category: Option<TaskCategory>,
    /// Estimated complexity 1–10; clamped when merged onto the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<u8>,
    /// Optional trace identifier for cross-system correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Optional budget constraints for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,
    /// Non-binding preference string (e.g. tier hint); stored on [`AgentTask::model_preference`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_preference: Option<String>,
    /// If set, stored on [`AgentTask::model_override`] for labeling and downstream routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_override: Option<String>,
    /// Interaction mode hint (`plan` | `act` | `verify`); stored on [`AgentTask::mode`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Optional reconstruction campaign id for long-horizon grouped runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<String>,
    /// Optional benchmark tier for progressive reconstruction gating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_tier: Option<crate::reconstruction::ReconstructionBenchmarkTier>,
    /// Optional explicit specialization role for multi-agent protocol runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_role: Option<crate::reconstruction::AgentExecutionRole>,
    /// Optional logical thread id preserving branch continuity inside a session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Optional portable harness contract supplied by the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_spec_json: Option<String>,
    /// Optional tool declaration hints (e.g. `[[tool:vox_run_tests]]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_hints: Vec<String>,
    /// Optional research intent hints (e.g. `[[research:vector]]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub research_hints: Vec<String>,
    /// Optional labels for mesh capability routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_labels: Option<Vec<String>>,
    /// True if the mesh task should detach for asynchronous execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_detached: Option<bool>,
    /// Whether this task requires human approval before execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_approval: Option<bool>,
    /// Pre-computed Socrates tracking from the planner phase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socrates_context: Option<crate::socrates::SocratesTaskContext>,
    /// Optional manifest of blob/image attachments for visual auditing or multi-modal continuation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_manifest: Option<crate::attachment_manifest::AttachmentManifest>,
    /// Optional procedural skill to guide the agent (e.g. `superpowers:tdd`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_skill: Option<String>,
    /// Optional tenant ID for budget tracking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// Drive Console clutch label (`free`|`efficiency`|`balanced`|`genius`); parsed in [`AgentTask::apply_hints`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clutch: Option<String>,
    /// Drive Console risk label (`high`|`moderate`|`low`); parsed in [`AgentTask::apply_hints`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,
    /// When set, overrides [`AgentTask::grounding_check_enabled`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grounding_check_enabled: Option<bool>,
    /// Trigger-source label (`interactive`|`automated`|`subagent`|`mesh`); parsed
    /// in [`AgentTask::apply_hints`]. `None` = unset; the generic MCP submission
    /// path defaults this to `Interactive` at the resolver, not here (unset stays
    /// unset so `resolved_policy()` can tell "explicitly interactive" from
    /// "caller didn't say").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_source: Option<String>,
    /// Chat session that issued the submit call (Phase D Task D1 durable
    /// lineage); stored on [`AgentTask::chat_session_id`] for correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_session_id: Option<String>,
}

/// Attribution record for which model was actually used to execute a task.
///
/// Populated at the inference/dispatch site and stashed on [`AgentTask`] so the
/// completion handler can copy it onto [`CompletionAttestation`] without a separate
/// lookup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedModelRecord {
    /// The model id that ran this task (e.g. `"anthropic/claude-opus-4-5"`).
    pub model_id: String,
    /// Provider family label from `backend_telemetry_labels` (e.g. `"anthropic"`).
    pub provider: String,
    /// Short description of why this model was chosen (serialised `SelectionReason`).
    pub selection_reason: String,
    /// Input tokens consumed, if known at record time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_tokens: Option<u64>,
    /// Wall-clock latency in milliseconds, if measured at record time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

/// Completion-time attestation metadata supplied by clients (e.g. MCP) for policy checks.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompletionAttestation {
    /// Human-readable completion summary used for no-write policy validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_summary: Option<String>,
    /// Optional list of checks the caller claims were run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks_passed: Vec<String>,
    /// Evidence references that must appear in the session [`crate::ContextEnvelope`] (substring match).
    /// Also see `[[voxcite:...]]` markers in [`Self::completion_summary`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_citations: Vec<String>,
    /// Optional artifacts produced by the task (workspace-relative paths preferred).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_paths: Vec<PathBuf>,
    /// Explicit declaration that output avoids placeholders / stubs.
    #[serde(default)]
    pub declared_non_placeholder: bool,
    /// Allow risky completion with explicit reason (audited and logged).
    #[serde(default)]
    pub force_risky: bool,
    /// Required when `force_risky` is true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_risky_reason: Option<String>,
    /// Observer summary produced at task exit (Task 65).
    ///
    /// Populated by the MCP completion handler when an `Observer` was active for this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_summary: Option<crate::observer::ObservationSummary>,
    /// Model that actually completed this task (e.g. "anthropic/claude-opus").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completing_model: Option<String>,
    /// Provider route for the completing model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Why this model was selected (`SelectionReason` rendered as a short string).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<String>,
    /// Input tokens sent for this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_tokens: Option<u64>,
    /// Output tokens received for this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_tokens: Option<u64>,
    /// End-to-end latency in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    /// Optional pointer to a captured request/response digest (privacy-gated).
    /// Only populated when the user enables I/O capture; never stores raw payload inline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_digest_ref: Option<String>,
}

#[cfg(test)]
mod attribution_tests {
    use super::*;

    #[test]
    fn attestation_roundtrips_attribution() {
        let a = CompletionAttestation {
            completing_model: Some("anthropic/claude-opus".into()),
            provider: Some("anthropic".into()),
            selection_reason: Some("scored".into()),
            request_tokens: Some(4200),
            response_tokens: Some(1100),
            latency_ms: Some(820),
            ..Default::default()
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: CompletionAttestation = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.completing_model.as_deref(),
            Some("anthropic/claude-opus")
        );
        assert_eq!(back.request_tokens, Some(4200));
    }

    #[test]
    fn old_attestation_without_attribution_still_parses() {
        // Backward compatibility: a payload predating these fields must deserialize.
        let old = r#"{"declared_non_placeholder":true}"#;
        let a: CompletionAttestation = serde_json::from_str(old).unwrap();
        assert!(a.completing_model.is_none());
        assert!(a.declared_non_placeholder);
    }

    #[test]
    fn attestation_serializes_new_fields_as_optional() {
        // All new fields None → they must be absent from the JSON output.
        let a = CompletionAttestation::default();
        let json = serde_json::to_string(&a).unwrap();
        assert!(
            !json.contains("completing_model"),
            "completing_model should be absent: {json}"
        );
        assert!(
            !json.contains("provider"),
            "provider should be absent: {json}"
        );
        assert!(
            !json.contains("selection_reason"),
            "selection_reason should be absent: {json}"
        );
        assert!(
            !json.contains("request_tokens"),
            "request_tokens should be absent: {json}"
        );
        assert!(
            !json.contains("response_tokens"),
            "response_tokens should be absent: {json}"
        );
        assert!(
            !json.contains("latency_ms"),
            "latency_ms should be absent: {json}"
        );
        assert!(
            !json.contains("io_digest_ref"),
            "io_digest_ref should be absent: {json}"
        );
    }

    #[test]
    fn selected_model_record_survives_round_trip() {
        let rec = SelectedModelRecord {
            model_id: "anthropic/claude-opus-4-5".to_string(),
            provider: "anthropic".to_string(),
            selection_reason: "Scored".to_string(),
            request_tokens: Some(1234),
            latency_ms: Some(500),
        };
        let json = serde_json::to_string(&rec).unwrap();
        let back: SelectedModelRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model_id, "anthropic/claude-opus-4-5");
        assert_eq!(back.provider, "anthropic");
        assert_eq!(back.selection_reason, "Scored");
        assert_eq!(back.request_tokens, Some(1234));
        assert_eq!(back.latency_ms, Some(500));
    }

    #[test]
    fn selected_model_record_optional_fields_absent_when_none() {
        let rec = SelectedModelRecord {
            model_id: "openai/gpt-4o".to_string(),
            provider: "openai".to_string(),
            selection_reason: "LocalOnly".to_string(),
            request_tokens: None,
            latency_ms: None,
        };
        let json = serde_json::to_string(&rec).unwrap();
        assert!(
            !json.contains("request_tokens"),
            "request_tokens should be absent: {json}"
        );
        assert!(
            !json.contains("latency_ms"),
            "latency_ms should be absent: {json}"
        );
    }
}

/// Per-task mesh execution policy.
///
/// Controls which mesh nodes are eligible to run this task. The default (`Any`) allows
/// any available node. `LocalOnly` forces local execution. `Exclude` filters out named
/// nodes from the candidate set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeshPolicy {
    #[default]
    Any,
    LocalOnly,
    Exclude(Vec<String>),
}

impl MeshPolicy {
    /// Returns `true` when `node_id` is eligible to execute this task under the policy.
    ///
    /// `"local"` is the conventional id for the current node.
    #[must_use]
    pub fn allows_node(&self, node_id: &str) -> bool {
        match self {
            Self::Any => true,
            Self::LocalOnly => node_id == "local",
            Self::Exclude(list) => !list.iter().any(|n| n == node_id),
        }
    }
}

/// Description of a task before it is assigned an ID and routed in the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDescriptor {
    /// Human-readable work summary.
    pub description: String,
    /// Optional queue priority override.
    pub priority: Option<TaskPriority>,
    /// Files read or written by this task.
    pub file_manifest: Vec<FileAffinity>,
    /// Dependencies on tasks already in the orchestrator.
    pub depends_on: Vec<TaskId>,
    /// Intra-batch dependencies by index in the same submit call.
    pub temp_deps: Vec<usize>,
    /// Optional capability requirements for routing (same semantics as [`AgentTask::capability_requirements`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_requirements: Option<crate::contract::TaskCapabilityHints>,
    /// Optional session link (for chat/workflow grouping in Mens).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Optional logical thread id preserving branch continuity for handoff or remote execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Whether this task requires human approval before execution.
    #[serde(default)]
    pub requires_approval: bool,
    /// Explicit testing requirement for this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_decision: Option<crate::planning::TestDecision>,
    /// Optional tenant ID for budget tracking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

/// A unit of work to be executed by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    /// Unique task identifier.
    pub id: TaskId,
    /// Human-readable description of the work.
    pub description: String,
    /// Execution priority.
    pub priority: TaskPriority,
    /// Current status.
    pub status: TaskStatus,
    /// Files this task needs to read or write.
    pub file_manifest: Vec<FileAffinity>,
    /// The victory condition tier required to pass verification.
    #[serde(default = "default_victory_condition")]
    pub victory_condition: crate::VictoryCondition,
    /// Tasks that must complete before this one can start.
    pub depends_on: Vec<TaskId>,
    /// Estimated complexity (1-10 scale).
    pub estimated_complexity: u8,
    /// Model preference string (if any).
    pub model_preference: Option<String>,
    /// Explicit override for the model to use.
    pub model_override: Option<String>,
    /// Interaction mode requested at submit time (`plan` | `act` | `verify`).
    /// Advisory: routing/verification policies may consult it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Task category to help select the best model.
    pub task_category: TaskCategory,
    /// Explicit testing requirement decision if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_decision: Option<crate::planning::TestDecision>,
    /// Optional trace identifier for cross-system correlation (FIX-14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Optional budget constraints for the task (FIX-18).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,
    /// Number of times this task has been re-routed due to validation failures.
    pub debug_iterations: u8,
    /// Number of times this task has failed Toestub gates.
    #[serde(default)]
    pub toestub_iterations: u8,
    /// Number of times this task has failed Socrates evidence checks.
    #[serde(default)]
    pub socrates_iterations: u8,
    /// Optional tool declaration hints extracted from description (e.g. `[[tool:vox_run_tests]]`).
    #[serde(default)]
    pub tool_hints: Vec<String>,
    /// Optional research intent hints extracted from description (e.g. `[[research:vector]]`).
    #[serde(default)]
    pub research_hints: Vec<String>,
    /// Number of retry attempts (for timeout/failure recovery).
    pub retry_count: u32,
    /// When the task was created (not serialized — reconstructed on load).
    #[serde(skip)]
    pub created_at: Option<Instant>,
    /// Unix timestamp (ms) when this task object was first created (vcs/serialization safe).
    pub created_at_ms: u64,
    /// Unix timestamp (ms) when agent began executing this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    /// Unix timestamp (ms) of the last expensive operation (e.g. full build).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_expensive_op_ms: Option<u64>,
    /// Optional Socrates evidence contract for factual completion gating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socrates: Option<crate::socrates::SocratesTaskContext>,
    /// Optional GPU / hardware routing hints for distributed execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_requirements: Option<crate::contract::TaskCapabilityHints>,
    /// Optional session link (for chat/workflow grouping in Mens).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Chat session that issued the submit call (Phase D Task D1 durable
    /// lineage). Distinct from `session_id` above (Mens telemetry grouping,
    /// caller-supplied) — this one is injected server-side by `run_agent_turn`
    /// for calls dispatched inside a chat turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_session_id: Option<String>,
    /// Optional logical thread id preserving branch continuity for handoff or remote execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Effective attention weight computed at gate time (Phase 15). 0.0 = not yet computed.
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub attention_weight: f64,
    /// Approval tier assigned by the attention gate (Phase 15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_tier: Option<crate::attention::ApprovalTier>,
    /// Optional planning session this task belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_session_id: Option<String>,
    /// Optional planning node this task implements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_node_id: Option<String>,
    /// Optional planning version for this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_version: Option<u32>,
    /// Serialized execution policy generated by planner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_policy_json: Option<String>,
    /// Optional human resolution report (VALIDATED/OVERRULED summary).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_report: Option<String>,
    /// Optional campaign id for grouped reconstruction attempts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<String>,
    /// Optional benchmark tier for this task when campaign scoring is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_tier: Option<crate::reconstruction::ReconstructionBenchmarkTier>,
    /// Optional explicit execution role (planner/builder/verifier/reproducer/researcher).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_role: Option<crate::reconstruction::AgentExecutionRole>,
    /// Optional portable harness contract attached to the task for relay, audit, and replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_spec_json: Option<String>,
    /// When set, this task was handed to Populi A2A remote execution; local queue must not run it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub populi_remote_delegate: Option<PopuliRemoteDelegate>,
    /// Rolling window of observer reports for this task, capped at 20 entries (Task 58).
    ///
    /// Populated by the `Observer` each time `observe_file` / `observe_rust_file` is called
    /// for this task. Intentionally excluded from the hot serialization path via `skip_serializing_if`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observation_history: Vec<vox_db::store::ObservationReport>,
    /// Number of times this task was handed off between agents (A2A bounce guard).
    #[serde(default)]
    pub handoff_count: u8,
    /// Structured execution history for context injection (Surgical Injection).
    #[serde(default)]
    pub transcript: Vec<TaskTurn>,
    /// Current execution phase (Wave 2 OOPAV).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_phase: Option<TaskPhase>,
    /// Optional manifest of blob/image attachments for visual auditing or multi-modal continuation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_manifest: Option<crate::attachment_manifest::AttachmentManifest>,
    /// Procedural skill currently guiding the agent's behavior (e.g. `superpowers:test-driven-development`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_skill: Option<String>,
    /// Optional tenant ID for budget tracking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// Attribution record written by the inference/dispatch layer and copied to
    /// [`CompletionAttestation`] at task completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_model_record: Option<SelectedModelRecord>,
    /// Live Plan/Act/Verify loop state (Track D). `None` for tasks that predate
    /// the PAV loop or were submitted without a clutch override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pav_loop: Option<crate::planning::phase_loop::PavLoopState>,
    /// Per-task mesh execution policy (local-only / exclude peers).
    #[serde(default)]
    pub mesh_policy: MeshPolicy,
    /// Node that actually executed this task (audit). `None` = local or not yet run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_node_id: Option<String>,
    /// User-selected clutch ("how much gas") profile from the Drive Console.
    /// `None` = no override; routing falls back to the neutral `Balanced` resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clutch_profile: Option<crate::mode::ClutchProfile>,
    /// User-selected risk posture ("acceptable risk") from the Drive Console.
    /// `None` = no override; gating falls back to the neutral `Moderate` resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_posture: Option<crate::mode::RiskPosture>,
    /// Opt-in, per-task toggle for the non-blocking post-reply grounding
    /// check (chat gate policy). Defaults to `false`; only meaningful for
    /// chat-origin tasks run by `ChatTaskProcessor`.
    #[serde(default)]
    pub grounding_check_enabled: bool,
    /// Who/what started this task. `None` = unknown; `resolved_policy()` treats
    /// unset the same as `Interactive` (today's most common caller).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_source: Option<crate::mode::TriggerSource>,
}

impl AgentTask {
    /// Create a new task with the given parameters.
    pub fn new(
        id: TaskId,
        description: impl Into<String>,
        priority: TaskPriority,
        file_manifest: Vec<FileAffinity>,
    ) -> Self {
        let description = description.into();
        let (tool_hints, research_hints) = Self::parse_description_hints(&description);
        let mut task_category = TaskCategory::default();
        if description.contains("[[category:visus]]") {
            task_category = TaskCategory::Visus;
        } else if description.contains("[[category:research]]") {
            task_category = TaskCategory::Research;
        } else if description.contains("[[category:codegen]]") {
            task_category = TaskCategory::CodeGen;
        }

        Self {
            id,
            description,
            priority,
            status: TaskStatus::Queued,
            file_manifest,
            depends_on: Vec::new(),
            estimated_complexity: 5,
            model_preference: None,
            model_override: None,
            mode: None,
            test_decision: None,
            trace_id: None,
            budget: None,
            task_category,
            debug_iterations: 0,
            toestub_iterations: 0,
            socrates_iterations: 0,
            tool_hints,
            research_hints,
            campaign_id: None,
            retry_count: 0,
            created_at: Some(Instant::now()),
            created_at_ms: now_unix_ms(),
            started_at_ms: None,
            last_expensive_op_ms: None,
            socrates: None,
            capability_requirements: None,
            session_id: None,
            chat_session_id: None,
            thread_id: None,
            attention_weight: 0.0,
            approval_tier: None,
            plan_session_id: None,
            plan_node_id: None,
            plan_version: None,
            execution_policy_json: None,
            benchmark_tier: None,
            execution_role: None,
            harness_spec_json: None,
            audit_report: None,
            populi_remote_delegate: None,
            victory_condition: crate::VictoryCondition::CompilationOnly,
            observation_history: Vec::new(),
            handoff_count: 0,
            transcript: Vec::new(),
            current_phase: None,
            attachment_manifest: None,
            active_skill: None,
            tenant_id: None,
            selected_model_record: None,
            pav_loop: None,
            mesh_policy: MeshPolicy::Any,
            executor_node_id: None,
            clutch_profile: None,
            risk_posture: None,
            grounding_check_enabled: false,
            trigger_source: None,
        }
    }

    /// Extract structured hints from double-bracketed tags in the description.
    ///
    /// Matches `[[tool:name]]` and `[[research:topic]]`.
    pub fn parse_description_hints(description: &str) -> (Vec<String>, Vec<String>) {
        let mut tools = Vec::new();
        let mut research = Vec::new();

        // Simple manual scan to avoid heavy regex in core task types if possible.
        let mut start = 0;
        while let Some(open) = description[start..].find("[[") {
            let open_pos = start + open;
            if let Some(close) = description[open_pos..].find("]]") {
                let close_pos = open_pos + close;
                let inner = &description[open_pos + 2..close_pos];
                if let Some(colon) = inner.find(':') {
                    let kind = &inner[..colon];
                    let value = inner[colon + 1..].trim();
                    if !value.is_empty() {
                        match kind {
                            "tool" => tools.push(value.to_string()),
                            "research" => research.push(value.to_string()),
                            "category" => {
                                // Category hints are handled at the dispatch/creation layer
                                // but we store them here if needed for telemetry.
                            }
                            _ => {}
                        }
                    }
                }
                start = close_pos + 2;
            } else {
                break;
            }
        }

        (tools, research)
    }

    /// Attach a session ID to this task.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add a dependency on another task.
    pub fn depends_on(mut self, dep: TaskId) -> Self {
        self.depends_on.push(dep);
        self.status = TaskStatus::Blocked(dep);
        self
    }

    /// Set estimated complexity (clamped to 1-10).
    pub fn complexity(mut self, c: u8) -> Self {
        self.estimated_complexity = c.clamp(1, 10);
        self
    }

    /// Set task category.
    pub fn category(mut self, cat: TaskCategory) -> Self {
        self.task_category = cat;
        self
    }

    /// Check if all dependencies are in the given completed set.
    pub fn is_ready(&self, completed: &[TaskId]) -> bool {
        self.depends_on.iter().all(|dep| completed.contains(dep))
    }

    /// List of files this task will write to.
    pub fn write_files(&self) -> Vec<&PathBuf> {
        self.file_manifest
            .iter()
            .filter(|f| f.access == AccessKind::Write)
            .map(|f| &f.path)
            .collect()
    }

    /// Merge hints into the task object.
    pub fn apply_hints(&mut self, h: &TaskEnqueueHints) {
        if let Some(c) = h.complexity {
            self.estimated_complexity = c.clamp(1, 10);
        }
        if let Some(ref m) = h.model_override {
            self.model_override = Some(m.clone());
        }
        if let Some(ref p) = h.model_preference {
            self.model_preference = Some(p.clone());
        }
        if let Some(ref m) = h.mode {
            self.mode = Some(m.clone());
        }
        if let Some(cat) = h.task_category {
            self.task_category = cat;
        }
        if let Some(ref campaign_id) = h.campaign_id {
            let trimmed = campaign_id.trim();
            if !trimmed.is_empty() {
                self.campaign_id = Some(trimmed.to_string());
            }
        }
        if let Some(tier) = h.benchmark_tier {
            self.benchmark_tier = Some(tier);
        }
        if let Some(role) = h.execution_role {
            self.execution_role = Some(role);
        }
        if let Some(ref thread_id) = h.thread_id {
            let trimmed = thread_id.trim();
            if !trimmed.is_empty() {
                self.thread_id = Some(trimmed.to_string());
            }
        }
        if let Some(ref chat_session_id) = h.chat_session_id {
            let trimmed = chat_session_id.trim();
            if !trimmed.is_empty() {
                self.chat_session_id = Some(trimmed.to_string());
            }
        }
        if !h.tool_hints.is_empty() {
            self.tool_hints.extend(h.tool_hints.clone());
        }
        if !h.research_hints.is_empty() {
            self.research_hints.extend(h.research_hints.clone());
        }
        if let Some(ref harness_spec_json) = h.harness_spec_json {
            let trimmed = harness_spec_json.trim();
            if !trimmed.is_empty() {
                self.harness_spec_json = Some(trimmed.to_string());
            }
        }
        if let Some(ref labels) = h.required_labels {
            if !labels.is_empty() {
                let mut reqs = self.capability_requirements.take().unwrap_or_default();
                reqs.labels.extend(labels.clone());
                self.capability_requirements = Some(reqs);
            }
        }
        if let Some(req_apprv) = h.requires_approval {
            if req_apprv {
                self.status = TaskStatus::BlockedOnApproval;
            }
        }
        if let Some(ref soc) = h.socrates_context {
            self.socrates = Some(soc.clone());
        }
        if let Some(ref attachment_manifest) = h.attachment_manifest {
            self.attachment_manifest = Some(attachment_manifest.clone());
        }
        if let Some(ref trace_id) = h.trace_id {
            self.trace_id = Some(trace_id.clone());
        }
        if let Some(ref budget) = h.budget {
            self.budget = Some(budget.clone());
        }
        if let Some(ref skill) = h.active_skill {
            self.active_skill = Some(skill.clone());
        }
        if let Some(ref tenant_id) = h.tenant_id {
            self.tenant_id = Some(tenant_id.clone());
        }
        if let Some(ref clutch) = h.clutch {
            if let Some(profile) = crate::mode::ClutchProfile::from_label(clutch) {
                self.clutch_profile = Some(profile);
            }
        }
        if let Some(ref risk) = h.risk {
            if let Some(posture) = crate::mode::RiskPosture::from_label(risk) {
                self.risk_posture = Some(posture);
            }
        }
        if let Some(enabled) = h.grounding_check_enabled {
            self.grounding_check_enabled = enabled;
        }
        if let Some(ref source) = h.trigger_source {
            if let Some(parsed) = crate::mode::TriggerSource::from_label(source) {
                self.trigger_source = Some(parsed);
            }
        }
    }

    /// Resolve the clutch profile to its safety/quality gates, falling back to the
    /// neutral `Balanced` resolution when no override was supplied.
    #[must_use]
    pub fn resolved_clutch(&self) -> crate::mode::ResolvedClutch {
        self.clutch_profile
            .unwrap_or(crate::mode::ClutchProfile::Balanced)
            .resolve()
    }

    /// Resolve the risk posture to its safety gates, falling back to the neutral
    /// `Moderate` resolution when no override was supplied.
    #[must_use]
    pub fn resolved_risk(&self) -> crate::mode::ResolvedRisk {
        self.risk_posture
            .unwrap_or(crate::mode::RiskPosture::Moderate)
            .resolve()
    }

    /// Resolve this task's effective (clutch, risk) pair using the full
    /// precedence chain: explicit hint > this task's category policy > this
    /// task's trigger-source policy > the neutral global default. `overrides`
    /// is the live `OrchestratorConfig::snapshot().task_policy` — callers fetch
    /// it once per resolution rather than this method reaching for it itself,
    /// keeping `AgentTask` free of a config-snapshot dependency in its own type.
    #[must_use]
    pub fn resolved_policy(
        &self,
        overrides: &crate::config::TaskPolicyOverrides,
    ) -> (crate::mode::ClutchProfile, crate::mode::RiskPosture) {
        let (category_clutch, category_risk) =
            crate::mode::effective_category_policy(overrides, self.task_category);
        let source = self
            .trigger_source
            .unwrap_or(crate::mode::TriggerSource::Interactive);
        let (source_clutch, source_risk) = crate::mode::effective_source_policy(overrides, source);
        crate::mode::resolve_task_policy(
            self.clutch_profile,
            self.risk_posture,
            category_clutch,
            category_risk,
            source_clutch,
            source_risk,
        )
    }

    /// Mark the task as started, recording the start timestamp.
    pub fn start(&mut self) -> &mut Self {
        self.started_at_ms = Some(now_unix_ms());
        self
    }

    /// Record that an expensive operation occurred during this task.
    pub fn record_expensive_op(&mut self) {
        self.last_expensive_op_ms = Some(now_unix_ms());
    }

    /// Milliseconds since the last expensive operation in this task, if any.
    pub fn elapsed_since_last_expensive_op_ms(&self) -> Option<u64> {
        self.last_expensive_op_ms
            .map(|t| now_unix_ms().saturating_sub(t))
    }

    /// Append a turn to the task's transcript, maintaining a rolling window to prevent context bloat.
    pub fn append_turn(&mut self, agent_id: super::ids::AgentId, name: String, message: String) {
        self.transcript.push(TaskTurn {
            agent_id,
            agent_name: name,
            message,
            timestamp_ms: now_unix_ms(),
        });
        // Hard limit on transcript depth to ensure LLM prompt density.
        if self.transcript.len() > 10 {
            self.transcript.remove(0);
        }
    }

    /// Enforce state machine transitions for the task status.
    pub fn transition_to(&mut self, new_status: TaskStatus) -> Result<(), String> {
        // Allow self-transitions
        if std::mem::discriminant(&self.status) == std::mem::discriminant(&new_status) {
            self.status = new_status;
            return Ok(());
        }

        match (&self.status, &new_status) {
            (TaskStatus::Queued, TaskStatus::InProgress | TaskStatus::Cancelled) => {}
            (
                TaskStatus::InProgress,
                TaskStatus::Completed
                | TaskStatus::Failed(_)
                | TaskStatus::Cancelled
                | TaskStatus::Blocked(_)
                | TaskStatus::BlockedOnApproval
                | TaskStatus::Doubted(_)
                | TaskStatus::Queued,
            ) => {}
            (TaskStatus::Blocked(_), TaskStatus::Queued | TaskStatus::Cancelled) => {}
            (
                TaskStatus::BlockedOnApproval,
                TaskStatus::Queued | TaskStatus::Cancelled | TaskStatus::InProgress,
            ) => {}
            (TaskStatus::Failed(_), TaskStatus::Queued | TaskStatus::Cancelled) => {}
            (TaskStatus::Doubted(_), TaskStatus::Queued | TaskStatus::Cancelled) => {}
            _ => {
                return Err(format!(
                    "Invalid state transition from {} to {}",
                    self.status, new_status
                ));
            }
        }
        self.status = new_status;
        Ok(())
    }

    /// Predict the number of tokens this task will consume based on its complexity and category.
    pub fn estimated_token_count(&self) -> u64 {
        let base = match self.task_category {
            TaskCategory::CodeGen => 2000,
            TaskCategory::Research => 4000,
            TaskCategory::Visus => 8000,
            _ => 1000,
        };
        let complexity_mult = f64::from(self.estimated_complexity).powi(2) / 25.0; // 5 is 1.0, 10 is 4.0
        (base as f64 * complexity_mult).round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::super::ids::TaskId;
    use super::*;

    #[test]
    fn task_priority_ordering() {
        assert!(TaskPriority::Urgent > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Background);
    }

    #[test]
    fn file_affinity_constructors() {
        let r = FileAffinity::read("foo.rs");
        assert_eq!(r.access, AccessKind::Read);
        let w = FileAffinity::write("bar.rs");
        assert_eq!(w.access, AccessKind::Write);
    }

    #[test]
    fn apply_hints_parses_clutch_labels() {
        for (label, expected) in [
            ("free", crate::mode::ClutchProfile::Free),
            ("Efficiency", crate::mode::ClutchProfile::Efficiency),
            ("BALANCED", crate::mode::ClutchProfile::Balanced),
            ("genius", crate::mode::ClutchProfile::Genius),
        ] {
            let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
            let hints = TaskEnqueueHints {
                clutch: Some(label.to_string()),
                ..Default::default()
            };
            task.apply_hints(&hints);
            assert_eq!(task.clutch_profile, Some(expected), "label {label}");
        }
    }

    #[test]
    fn apply_hints_parses_risk_labels() {
        for (label, expected) in [
            ("high", crate::mode::RiskPosture::High),
            ("Moderate", crate::mode::RiskPosture::Moderate),
            ("LOW", crate::mode::RiskPosture::Low),
        ] {
            let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
            let hints = TaskEnqueueHints {
                risk: Some(label.to_string()),
                ..Default::default()
            };
            task.apply_hints(&hints);
            assert_eq!(task.risk_posture, Some(expected), "label {label}");
        }
    }

    #[test]
    fn apply_hints_unknown_clutch_risk_leaves_none() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        let hints = TaskEnqueueHints {
            clutch: Some("turbo".to_string()),
            risk: Some("reckless".to_string()),
            ..Default::default()
        };
        task.apply_hints(&hints);
        assert_eq!(task.clutch_profile, None);
        assert_eq!(task.risk_posture, None);
    }

    #[test]
    fn apply_hints_parses_trigger_source_labels() {
        for (label, expected) in [
            ("interactive", crate::mode::TriggerSource::Interactive),
            ("Automated", crate::mode::TriggerSource::Automated),
            ("SUBAGENT", crate::mode::TriggerSource::Subagent),
            ("mesh", crate::mode::TriggerSource::Mesh),
        ] {
            let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
            let hints = TaskEnqueueHints {
                trigger_source: Some(label.to_string()),
                ..Default::default()
            };
            task.apply_hints(&hints);
            assert_eq!(task.trigger_source, Some(expected), "label {label}");
        }
    }

    #[test]
    fn apply_hints_unknown_trigger_source_leaves_none() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        let hints = TaskEnqueueHints {
            trigger_source: Some("turbo".to_string()),
            ..Default::default()
        };
        task.apply_hints(&hints);
        assert_eq!(task.trigger_source, None);
    }

    #[test]
    fn new_task_has_no_trigger_source_by_default() {
        let task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        assert_eq!(task.trigger_source, None);
    }

    #[test]
    fn resolved_clutch_risk_fall_back_to_neutral_defaults() {
        let task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        assert_eq!(task.clutch_profile, None);
        assert_eq!(task.risk_posture, None);
        assert_eq!(
            task.resolved_clutch(),
            crate::mode::ClutchProfile::Balanced.resolve()
        );
        assert_eq!(
            task.resolved_risk(),
            crate::mode::RiskPosture::Moderate.resolve()
        );
    }

    #[test]
    fn resolved_policy_falls_back_to_neutral_defaults_when_nothing_set() {
        let task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        let overrides = crate::config::TaskPolicyOverrides::default();
        let (clutch, risk) = task.resolved_policy(&overrides);
        assert_eq!(clutch, crate::mode::ClutchProfile::Balanced);
        assert_eq!(risk, crate::mode::RiskPosture::Moderate);
    }

    #[test]
    fn resolved_policy_explicit_hint_wins_over_source_override() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        task.clutch_profile = Some(crate::mode::ClutchProfile::Genius);
        task.trigger_source = Some(crate::mode::TriggerSource::Automated);

        let mut source = std::collections::HashMap::new();
        source.insert(
            "Automated".to_string(),
            crate::config::TaskPolicyEntry {
                clutch: Some("free".to_string()),
                risk: Some("high".to_string()),
            },
        );
        let overrides = crate::config::TaskPolicyOverrides {
            category: std::collections::HashMap::new(),
            source,
        };

        let (clutch, _risk) = task.resolved_policy(&overrides);
        assert_eq!(
            clutch,
            crate::mode::ClutchProfile::Genius,
            "explicit clutch hint must win"
        );
    }

    #[test]
    fn resolved_policy_uses_source_override_when_no_explicit_hint() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        task.trigger_source = Some(crate::mode::TriggerSource::Automated);

        let mut source = std::collections::HashMap::new();
        source.insert(
            "Automated".to_string(),
            crate::config::TaskPolicyEntry {
                clutch: Some("free".to_string()),
                risk: Some("high".to_string()),
            },
        );
        let overrides = crate::config::TaskPolicyOverrides {
            category: std::collections::HashMap::new(),
            source,
        };

        let (clutch, risk) = task.resolved_policy(&overrides);
        assert_eq!(clutch, crate::mode::ClutchProfile::Free);
        assert_eq!(risk, crate::mode::RiskPosture::High);
    }

    #[test]
    fn agent_task_clutch_risk_serde_round_trip() {
        let mut task = AgentTask::new(TaskId(7), "t", TaskPriority::Normal, vec![]);
        task.clutch_profile = Some(crate::mode::ClutchProfile::Genius);
        task.risk_posture = Some(crate::mode::RiskPosture::Low);
        let json = serde_json::to_string(&task).unwrap();
        let back: AgentTask = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.clutch_profile,
            Some(crate::mode::ClutchProfile::Genius)
        );
        assert_eq!(back.risk_posture, Some(crate::mode::RiskPosture::Low));
    }

    #[test]
    fn agent_task_dependency_check() {
        let task = AgentTask::new(TaskId(1), "test task", TaskPriority::Normal, vec![])
            .depends_on(TaskId(10))
            .depends_on(TaskId(20));

        assert!(!task.is_ready(&[TaskId(10)]));
        assert!(task.is_ready(&[TaskId(10), TaskId(20)]));
    }

    #[test]
    fn serialization_roundtrip() {
        let task = AgentTask::new(
            TaskId(1),
            "fix parser",
            TaskPriority::Urgent,
            vec![FileAffinity::write("src/parser.rs")],
        );
        let json = serde_json::to_string(&task).expect("serialize");
        let back: AgentTask = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, task.id);
        assert_eq!(back.priority, task.priority);
        assert_eq!(back.description, task.description);
    }

    #[test]
    fn task_start_sets_started_at_ms() {
        let mut task = AgentTask::new(TaskId(1), "test", TaskPriority::Normal, vec![]);
        assert!(task.started_at_ms.is_none(), "should not be started yet");
        task.start();
        assert!(
            task.started_at_ms.is_some(),
            "start() must populate started_at_ms"
        );
        let ts = task.started_at_ms.unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(
            now.saturating_sub(ts) < 5_000,
            "started_at_ms should be recent"
        );
    }

    #[test]
    fn expensive_op_elapsed_ms_is_monotone() {
        let mut task = AgentTask::new(TaskId(2), "test", TaskPriority::Normal, vec![]);
        assert!(task.elapsed_since_last_expensive_op_ms().is_none());
        task.record_expensive_op();
        let elapsed = task.elapsed_since_last_expensive_op_ms();
        assert!(elapsed.is_some(), "should have elapsed after recording");
        assert!(elapsed.unwrap() < 1_000, "should be < 1s in test");
    }

    #[test]
    fn task_start_idempotent_timestamp_stable() {
        let mut task = AgentTask::new(TaskId(3), "test", TaskPriority::Normal, vec![]);
        task.start();
        let first = task.started_at_ms.unwrap();
        task.start();
        let second = task.started_at_ms.unwrap();
        assert!(
            second >= first,
            "second start should not go backward in time"
        );
    }

    #[test]
    fn enqueue_hints_roundtrip_preserves_campaign_tier_and_role() {
        let hints = TaskEnqueueHints {
            task_category: Some(TaskCategory::Testing),
            complexity: Some(7),
            model_preference: Some("free".to_string()),
            model_override: Some("model-x".to_string()),
            mode: None,
            campaign_id: Some("camp-123".to_string()),
            benchmark_tier: Some(crate::reconstruction::ReconstructionBenchmarkTier::CrateRegen),
            execution_role: Some(crate::reconstruction::AgentExecutionRole::Verifier),
            thread_id: Some("thread-123".to_string()),
            harness_spec_json: Some("{\"schema_version\":1}".to_string()),
            tool_hints: vec![],
            research_hints: vec![],
            required_labels: None,
            is_detached: None,
            requires_approval: None,
            socrates_context: None,
            attachment_manifest: None,
            trace_id: None,
            budget: None,
            active_skill: None,
            tenant_id: None,
            clutch: None,
            risk: None,
            grounding_check_enabled: None,
            trigger_source: None,
            chat_session_id: None,
        };
        let json = serde_json::to_string(&hints).expect("serialize hints");
        let back: TaskEnqueueHints = serde_json::from_str(&json).expect("deserialize hints");
        assert_eq!(back.campaign_id.as_deref(), Some("camp-123"));
        assert_eq!(
            back.benchmark_tier,
            Some(crate::reconstruction::ReconstructionBenchmarkTier::CrateRegen)
        );
        assert_eq!(
            back.execution_role,
            Some(crate::reconstruction::AgentExecutionRole::Verifier)
        );
        assert_eq!(back.thread_id.as_deref(), Some("thread-123"));
        assert_eq!(
            back.harness_spec_json.as_deref(),
            Some("{\"schema_version\":1}")
        );
    }
}

#[cfg(test)]
mod mesh_policy_tests {
    use super::*;

    #[test]
    fn mesh_policy_defaults_to_any() {
        assert_eq!(MeshPolicy::default(), MeshPolicy::Any);
    }

    #[test]
    fn local_only_forbids_remote() {
        assert!(!MeshPolicy::LocalOnly.allows_node("peer-7"));
        assert!(MeshPolicy::LocalOnly.allows_node("local"));
    }

    #[test]
    fn exclude_peer_blocks_named() {
        let p = MeshPolicy::Exclude(vec!["peer-7".into()]);
        assert!(!p.allows_node("peer-7"));
        assert!(p.allows_node("peer-9"));
    }

    #[test]
    fn any_allows_all_nodes() {
        assert!(MeshPolicy::Any.allows_node("peer-7"));
        assert!(MeshPolicy::Any.allows_node("local"));
        assert!(MeshPolicy::Any.allows_node("remote-xyz"));
    }

    #[test]
    fn mesh_policy_roundtrips_via_serde() {
        let p = MeshPolicy::Exclude(vec!["peer-1".into(), "peer-2".into()]);
        let json = serde_json::to_string(&p).unwrap();
        let back: MeshPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn agent_task_has_mesh_policy_field_with_default_any() {
        let task = AgentTask::new(TaskId(1), "test", TaskPriority::Normal, vec![]);
        assert_eq!(task.mesh_policy, MeshPolicy::Any);
        assert!(task.executor_node_id.is_none());
    }
}

#[cfg(test)]
mod semcov_pure_tests {
    use super::*;

    // Catches: parse_description_hints mis-routing a [[research:...]] tag into the
    // tools bucket (or vice-versa), or dropping the value after the colon.
    #[test]
    fn parse_description_hints_separates_tool_and_research() {
        let (tools, research) =
            AgentTask::parse_description_hints("do [[tool:ripgrep]] then [[research:rust async]]");
        assert_eq!(tools, vec!["ripgrep".to_string()]);
        assert_eq!(research, vec!["rust async".to_string()]);
    }

    // Catches: complexity() not clamping out-of-range input, letting a 0 or 99
    // through and skewing weighted_load / scaling math.
    #[test]
    fn complexity_clamps_to_one_through_ten() {
        let lo = AgentTask::new(TaskId(1), "x", TaskPriority::Normal, vec![]).complexity(0);
        assert_eq!(lo.estimated_complexity, 1);
        let hi = AgentTask::new(TaskId(2), "x", TaskPriority::Normal, vec![]).complexity(99);
        assert_eq!(hi.estimated_complexity, 10);
    }
}
