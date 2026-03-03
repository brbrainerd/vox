//! RPC parameter and response types for all Vox MCP tools.
//!
//! All `#[derive(Deserialize)]` structs accepted by tool handlers live here.
//! All `#[derive(Serialize)]` response types live here.
//! The generic [`ToolResult<T>`] envelope lives here.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Generic tool result envelope
// ---------------------------------------------------------------------------

/// A standard envelope for all tool responses.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResult<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ToolResult<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| {
            format!("{{\"success\":false,\"error\":\"serialization failed: {e}\"}}")
        })
    }
}

// ---------------------------------------------------------------------------
// File / task request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct FileSpec {
    pub path: String,
    pub access: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitTaskParams {
    pub description: String,
    pub files: Vec<FileSpec>,
    pub priority: Option<String>,
    pub agent_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitTaskResponse {
    pub task_id: u64,
    pub agent_id: u64,
    /// Whether the task description was canonicalized (order-invariant, normalized).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_canonicalized: Option<bool>,
    /// Conflict warnings from the prompt canonicalization pipeline (for transparency).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_warnings: Option<Vec<String>>,
    /// Hash of the original prompt for debug/traceability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_prompt_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskStatusParams {
    pub task_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct CompleteTaskParams {
    pub task_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct FailTaskParams {
    pub task_id: u64,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct CancelTaskParams {
    pub task_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct ReorderTaskParams {
    pub task_id: u64,
    pub priority: String,
}

#[derive(Debug, Deserialize)]
pub struct DrainAgentParams {
    pub agent_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct MapAgentSessionParams {
    pub agent_id: u64,
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ValidateFileParams {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct RunTestsParams {
    pub crate_name: String,
    pub test_filter: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublishMessageParams {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct AgentInfo {
    pub id: u64,
    pub name: String,
    pub queued: usize,
    pub completed: usize,
    pub paused: bool,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub agent_count: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub agents: Vec<AgentInfo>,
    /// Current scaling profile (conservative / balanced / aggressive) for transparency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaling_profile: Option<String>,
    /// Effective scale-up threshold (scaling_threshold * profile multiplier) for explainability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_scale_up_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub companion: Option<vox_gamify::companion::Companion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown_summary: Option<String>,
    // -- VCS health stats --
    /// Total snapshots stored in the snapshot ring-buffer.
    pub snapshot_count: usize,
    /// Total operations in the operation log.
    pub oplog_count: usize,
    /// Number of active (unresolved) file conflicts.
    pub active_conflicts: usize,
    /// Number of active agent workspaces.
    pub active_workspaces: usize,
    /// Number of tracked logical changes.
    pub active_changes: usize,
}

#[derive(Debug, Serialize)]
pub struct DiagnosticInfo {
    pub severity: String,
    pub message: String,
    pub source: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub count: usize,
    pub diagnostics: Vec<DiagnosticInfo>,
}
