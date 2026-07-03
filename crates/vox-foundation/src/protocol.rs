//! Pure-types L0 leaf for the orchestrator daemon wire protocol (dispatch request/payload shapes, method ids).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Method ids for `vox-orchestrator-d` (newline-delimited [`DispatchRequest`] / [`DispatchPayload::Result`]).
pub mod orch_daemon_method {
    /// Handshake + `repository_id` for the bound orchestrator instance.
    pub const PING: &str = "orch.ping";
    /// Full orchestrator status JSON snapshot (`OrchestratorStatus` in `vox-orchestrator`).
    pub const STATUS: &str = "orch.status";
    /// Params: `{"task_id": u64}` → `{"status": "Completed"|...}` or error.
    pub const TASK_STATUS: &str = "orch.task_status";
    /// Params: `{"name": "..."}` → `{"agent_id": u64}` or error.
    pub const SPAWN_AGENT: &str = "orch.spawn_agent";
    /// Params: `{}` → `{"agent_ids": [u64, ...]}`.
    pub const AGENT_IDS: &str = "orch.agent_ids";
    /// Params: submit-task payload mirroring `Orchestrator::submit_task_with_agent`.
    pub const SUBMIT_TASK: &str = "orch.submit_task";
    /// Params: `{"task_id": u64, "attestation": {...?}}` → `{"ok": true}`.
    pub const COMPLETE_TASK: &str = "orch.complete_task";
    /// Params: `{"task_id": u64, "reason": "..."}` → `{"ok": true}`.
    pub const FAIL_TASK: &str = "orch.fail_task";
    /// Params: `{"task_id": u64}` → `{"ok": true}`.
    pub const CANCEL_TASK: &str = "orch.cancel_task";
    /// Params: `{"task_id": u64}` → `{"ok": true}`.
    /// Signals an in-progress local task to abort (fires its `CancellationToken`).
    /// Unlike `CANCEL_TASK` this works on tasks that are already running, not just queued.
    pub const INTERRUPT_TASK: &str = "orch.interrupt_task";
    /// Params: `{"task_id": u64, "priority": "urgent|normal|background"}`.
    pub const REORDER_TASK: &str = "orch.reorder_task";
    /// Params: `{}` → `{"tasks": [{id, description, priority, status, lifecycle,
    /// agent_id, session_id, estimated_complexity, depends_on, write_files}, ...]}`.
    /// Lists every queued or in-progress task across all agents.
    pub const LIST_TASKS: &str = "orch.list_tasks";
    /// Params: `{"task_id": u64, "description": "..."}` → `{"ok": true}`.
    /// Rewrites the description of a queued (not in-progress) task.
    pub const EDIT_TASK: &str = "orch.edit_task";
    /// Params: `{"agent_id": u64}` → `{"drained_count": u64}`.
    pub const DRAIN_AGENT: &str = "orch.drain_agent";
    /// Params: `{}` → `{"rebalanced": u64}`.
    pub const REBALANCE: &str = "orch.rebalance";
    /// Params: dynamic/static spawn payload (name + optional delegation metadata).
    pub const SPAWN_AGENT_EXT: &str = "orch.spawn_agent_ext";
    /// Params: `{"agent_id": u64}` → `{"remaining_tasks": u64}`.
    pub const RETIRE_AGENT: &str = "orch.retire_agent";
    /// Params: `{"agent_id": u64}` → `{"ok": true}`.
    pub const PAUSE_AGENT: &str = "orch.pause_agent";
    /// Params: `{"agent_id": u64}` → `{"ok": true}`.
    pub const RESUME_AGENT: &str = "orch.resume_agent";
    /// Params: `{"task_id": u64, "reason": "..."?}` → `{"ok": true}`.
    pub const DOUBT_TASK: &str = "orch.doubt_task";
    /// Params: `{"task_id": u64, "reason": "..."} ` → `{"ok": true}`.
    pub const OVERRULE_TASK: &str = "orch.overrule_task";
    /// Params: `{"task_id": u64}` → `{"ok": true}`.
    /// Approves the current planning phase, advancing the PAV loop to Acting.
    pub const APPROVE_PLAN: &str = "orch.approve_plan";
    /// Params: `{"task_id": u64}` → `{"ok": true}`.
    /// Skips the Verifying phase; the task completes immediately.
    pub const SKIP_VERIFY: &str = "orch.skip_verify";
    /// Params: `{"task_id": u64}` → `{"ok": true}`.
    /// Forces a Verifying phase even when risk would normally skip it.
    pub const FORCE_VERIFY: &str = "orch.force_verify";
    /// Params: `{}` → workspace journey store diagnostics (`.vox/store.db` vs canonical).
    pub const WORKSPACE_JOURNEY: &str = "orch.workspace_journey";
    /// Params: `{}` → `{"ok": true}`. Triggers a hot-reload of Vox.toml configuration.
    pub const RELOAD_CONFIG: &str = "orch.reload_config";
    /// Params: `{"op_id": "uuid"}` → `{"ok": true}`
    pub const UNDO_OPERATION: &str = "orch.undo_operation";
    /// Params: `{"op_id": "uuid"}` → `{"ok": true}`
    pub const REDO_OPERATION: &str = "orch.redo_operation";
    /// Params: `{}` → a long-lived stream of [`super::DispatchPayload::Event`] frames
    /// (each carrying an orchestrator status snapshot), pushed by the daemon until
    /// the client disconnects. Unlike every other method this does not return a
    /// single terminal `Result`; the connection stays open and frames are emitted
    /// whenever the status changes.
    pub const SUBSCRIBE: &str = "orch.subscribe";
    /// Params: `{}` → a long-lived push stream of [`super::DispatchPayload::Event`]
    /// frames, one per `AgentEvent` emitted on the orchestrator's event bus
    /// (token streaming, task lifecycle, agent lifecycle, …). Like
    /// [`SUBSCRIBE`] the connection stays open until the client disconnects, but
    /// frames are pushed by the broadcast bus (no polling) and carry the
    /// serialized `AgentEvent` (`{ id, timestamp_ms, kind: { type, … } }`).
    pub const SUBSCRIBE_EVENTS: &str = "orch.subscribe_events";
    /// Params: `{"name": "<tool>", "args": {...}}` → the MCP tool's JSON result
    /// envelope. Dispatched against the daemon's MCP `ServerState` so callers
    /// (e.g. the GUI) run tools through the one shared orchestrator rather than a
    /// second in-process instance. Served via an `ExtraDispatch` hook.
    pub const TOOL_CALL: &str = "orch.tool_call";
    /// Params: `{}` → `{"approvals": [...]}` — HITL approvals awaiting a decision
    /// in the daemon's `ServerState`. Served via `ExtraDispatch`.
    pub const LIST_PENDING_APPROVALS: &str = "orch.list_pending_approvals";
    /// Params: `{"approval_id": "...", "outcome": "approved"|"rejected"|"modified"}`
    /// → `{"resolved": bool, ...}`. Wakes a parked dangerous-tool call in the
    /// daemon. Served via `ExtraDispatch`.
    pub const RESOLVE_APPROVAL: &str = "orch.resolve_approval";
    /// Params: `{}` → live VCS isolation status JSON
    /// (`{"strategy_default", "per_agent", "active_conflicts"}`), read from the
    /// daemon's single shared orchestrator so conflicts + per-agent overrides
    /// reflect real state. Mirrors `GET /api/v2/vcs/isolation`.
    pub const VCS_ISOLATION_STATUS: &str = "orch.vcs_isolation_status";
    /// Params: `{"strategy_default": String?, "agent_id": u64?, "strategy": String|null?}`
    /// — set the default and/or a per-agent override (`strategy: null` with an
    /// `agent_id` clears that override). At least one of `strategy_default` /
    /// `agent_id` must be present. Returns the fresh isolation status JSON.
    /// Mirrors `POST /api/v2/vcs/isolation/strategy`.
    pub const VCS_ISOLATION_SET_STRATEGY: &str = "orch.vcs_isolation_set_strategy";
    /// Params: `{}` → `{"tree": [{task_id, agent_id, parent_task_id?, reason, source_task_id}]}`
    /// — the current subagent delegation tree from `AgentDelegationBinding` topology records.
    /// Served via `ExtraDispatch`.
    pub const SUBAGENT_TREE: &str = "orch.subagent_tree";
    /// Params: `{"task_id": u64, "policy": "any"|"local_only"|{"exclude": ["node1",...]}}` → `{"ok": true}`.
    /// Updates the `mesh_policy` of a queued task. Served via `ExtraDispatch`.
    pub const SET_MESH_POLICY: &str = "orch.set_mesh_policy";
    /// Params: `{}` → `{"agents": [{"id", "name", "signal": BudgetSignal}]}` —
    /// per-agent budget/drift signal from the daemon's shared `BudgetManager`
    /// (T2.3 follow-up: `vox safety status`). Mirrors
    /// `BudgetManager::agent_budget_signal` for every agent in
    /// `Orchestrator::status().agents`.
    pub const SAFETY_BUDGET_SIGNALS: &str = "orch.safety_budget_signals";
    /// Params: `{"agent_id": u64?}` → `{"receipts": [{"receipt_id", "agent_id", "tool_name"}]}`
    /// — snapshot of the daemon's shared cryptographic tool receipt ledger,
    /// optionally filtered to one agent (T2.3 follow-up: `vox safety ledger`).
    pub const SAFETY_LEDGER: &str = "orch.safety_ledger";
    /// Params: `{}` → `{"locks": [{"resource_id", "kind", "holder", "expires_ms"}]}`
    /// — snapshot of the daemon's shared generic resource lock manager (T2.3
    /// follow-up: `vox safety locks`).
    pub const SAFETY_LOCKS: &str = "orch.safety_locks";
    /// Params: `{}` → `{"snapshot": AttentionBudget, "config": {"attention_enabled",
    /// "attention_budget_ms", "attention_alert_threshold"}}` — the daemon's shared
    /// real-time cognitive attention budget/threshold summary (T2.3 follow-up:
    /// `vox attention snapshot`). Mirrors `BudgetManager::attention_snapshot`.
    pub const ATTENTION_SNAPSHOT: &str = "orch.attention_snapshot";
}

pub mod dei_method {
    pub const AI_CHECK: &str = "ai.check";
    pub const AI_FIX: &str = "ai.fix";
    pub const AI_REVIEW: &str = "ai.review";
    pub const AI_GENERATE: &str = "ai.generate";
    pub const CONFIG_GET: &str = "config.get";
    pub const AI_PLAN_NEW: &str = "ai.plan.new";
    pub const AI_PLAN_REPLAN: &str = "ai.plan.replan";
    pub const AI_PLAN_STATUS: &str = "ai.plan.status";
    pub const AI_PLAN_EXECUTE: &str = "ai.plan.execute";
    /// `research.run` — enqueue a SCIENTIA research run inside the persistent
    /// orchestrator daemon. Params:
    /// `{"query": String, "scope": "web|local|both"?, "max_sources": usize?,
    ///   "verify_claims": bool?, "site_scope": String?, "session_id": i64?}`.
    /// Returns `{"session_id": i64, "task_id": String, "status": "running"}`
    /// immediately while the daemon advances the session to a terminal
    /// `completed`/`failed` status in the background.
    pub const RESEARCH_RUN: &str = "research.run";
}

/// Outgoing request from thin clients to Dei-style JSON-line daemons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchRequest {
    pub id: String,
    pub method: String,
    pub params: Value,
    /// Shared-secret daemon auth token (T0.2). A TOP-LEVEL field, deliberately
    /// separate from `params`: `params` is caller/tool-composed JSON, so the
    /// auth token must never be settable by tool-call-composing code — only
    /// the transport layer (`OrchDaemonClient`) sets this. `#[serde(default)]`
    /// so existing serialized requests without the field still deserialize
    /// (they simply fail the daemon's auth check rather than the parse).
    #[serde(default)]
    pub auth_token: Option<String>,
    /// GUI-selected permission mode (T0.3: `"ask" | "accept_edits" |
    /// "accept_all" | "plan"`), consulted by the dangerous-tool HITL gate in
    /// `vox-orchestrator-mcp`'s dispatch (`orch.tool_call`). Same isolation
    /// rationale as `auth_token`: a TOP-LEVEL field, separate from `params`,
    /// set only by the transport layer (`OrchDaemonClient`) — never
    /// reachable from tool-call `params` JSON the LLM agent composes, so a
    /// model can never self-select an auto-approving mode. `#[serde(default)]`
    /// so a missing/absent field resolves to `None`, which the gate treats
    /// as the fail-safe `ask` mode (today's always-park behavior).
    #[serde(default)]
    pub permission_mode: Option<String>,
}

/// Incoming response envelope from Dei-style JSON-line daemons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResponse {
    pub id: String,
    pub payload: DispatchPayload,
}

/// Payload variants for streaming and final Dei responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DispatchPayload {
    Result {
        value: Value,
    },
    /// Non-terminal structured event frame: a JSON payload pushed mid-stream
    /// (e.g. an orchestrator status snapshot from [`orch_daemon_method::SUBSCRIBE`]).
    /// Distinct from `Chunk` (text-only) and `Result` (single terminal value).
    Event {
        value: Value,
    },
    Error {
        message: String,
        code: i32,
    },
    Chunk {
        text: String,
    },
    Progress {
        percent: f32,
        status: String,
    },
    Log {
        level: String,
        msg: String,
    },
    Diag {
        severity: String,
        message: String,
        file: String,
        line: u32,
        col: u32,
    },
    Artifact {
        path: String,
    },
    Done {
        exit: i32,
    },
}
