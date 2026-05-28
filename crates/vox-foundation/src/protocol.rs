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
    /// Params: `{"task_id": u64, "priority": "urgent|normal|background"}`.
    pub const REORDER_TASK: &str = "orch.reorder_task";
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
    /// Params: `{}` → workspace journey store diagnostics (`.vox/store.db` vs canonical).
    pub const WORKSPACE_JOURNEY: &str = "orch.workspace_journey";
    /// Params: `{}` → `{"ok": true}`. Triggers a hot-reload of Vox.toml configuration.
    pub const RELOAD_CONFIG: &str = "orch.reload_config";
    /// Params: `{"op_id": "uuid"}` → `{"ok": true}`
    pub const UNDO_OPERATION: &str = "orch.undo_operation";
    /// Params: `{"op_id": "uuid"}` → `{"ok": true}`
    pub const REDO_OPERATION: &str = "orch.redo_operation";
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
}

/// Outgoing request from thin clients to Dei-style JSON-line daemons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchRequest {
    pub id: String,
    pub method: String,
    pub params: Value,
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
