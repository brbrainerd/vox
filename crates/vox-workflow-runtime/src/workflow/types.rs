//! Workflow planning types: mens control ops and planned activity descriptors.

/// Control-plane sub-step for a [`PopuliActivity`] (URL always comes from env / `Vox.toml`, not source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopuliHttpOp {
    /// `POST` heartbeat with the current node record.
    Heartbeat,
    /// Log only; still runs local registry publish when mens is enabled.
    Noop,
    /// `POST /v1/populi/join` for this process record.
    Join,
    /// `GET /v1/populi/nodes` (counts in journal only; no arbitrary URLs).
    Snapshot,
    /// `POST /v1/populi/dispatch` for remote task execution.
    Dispatch,
    /// `GET /v1/populi/dispatch/result/{dispatch_id}` for remote task polling.
    Wait,
}

/// One planned activity invocation extracted from workflow HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedActivity {
    /// Activity name as referenced in the workflow body.
    pub name: String,
    /// When true, run the mens / Populi HTTP step (`execute_populi_step` when feature `mens` is on).
    pub mens: bool,
    /// Idempotency / journal key from `with { activity_id: "…" }` when set.
    pub activity_id: Option<String>,
    /// Wall-clock timeout for mens HTTP sub-steps from `with { timeout: … }` (milliseconds).
    pub timeout_ms: Option<u64>,
    /// Additional attempts after the first one for interpreted mesh activity execution.
    pub retries: u32,
    /// Delay before the first retry after a failed interpreted mesh activity attempt.
    pub initial_backoff_ms: Option<u64>,
    /// Populi control-plane operation when [`Self::mens`] is true.
    pub populi_op: PopuliHttpOp,
    /// Optional labels for mesh routing (e.g. `gpu`, `region=us-east-1`).
    pub required_labels: Option<Vec<String>>,
    /// When true, dispatch as a detached task and poll for completion.
    pub is_detached: bool,
    /// P2-T5: structural argument values for dedup-cache keying. Empty when planning-time args are unavailable.
    pub arguments: Vec<serde_json::Value>,
    /// P2-T5: dedup window from `@activity(dedup = "…")` in milliseconds. Default: 24h.
    pub dedup_window_ms: Option<u64>,
}

/// P2-T5: derive a hex SHA3-512 hash of the canonicalized argument list.
pub fn compute_structural_arg_hash(args: &[serde_json::Value]) -> String {
    let canonical = serde_json::Value::Array(args.to_vec()).to_string();
    vox_db::hash::content_hash(canonical.as_bytes())
}

/// Replay-oriented node for interpreted durable workflow execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayNode {
    /// Execute one activity step and persist/replay by `activity_id`.
    Activity(PlannedActivity),
    /// `workflow.version("change-id", min, max)` patch-marker (P2-T2).
    WorkflowPatch {
        /// Stable identifier for this version gate (e.g. `"add-retry-v2"`).
        change_id: String,
        /// Minimum workflow version that takes the new code path.
        min: u32,
        /// Maximum workflow version that takes the new code path (inclusive).
        max: u32,
    },
}

/// Linear replay IR produced from workflow HIR for the interpreted runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowReplayIr {
    /// Ordered replay nodes for deterministic interpreted execution.
    pub nodes: Vec<ReplayNode>,
}

/// Mens-tagged activity (name convention: `mesh_*`, plus [`PopuliHttpOp`]).
#[derive(Debug, Clone)]
pub struct PopuliActivity {
    /// Activity name from source.
    pub name: String,
    /// Resolved mens HTTP operation.
    pub populi_op: PopuliHttpOp,
    /// Timeout for populi HTTP client (defaults inside `execute_populi_step` when unset).
    pub timeout_ms: Option<u64>,
    /// Stable id for journal / idempotency (`with { activity_id }` or generated).
    pub activity_id: String,
    /// Mesh routing labels.
    pub required_labels: Option<Vec<String>>,
    /// Asynchronous execution.
    pub is_detached: bool,
}
