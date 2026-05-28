//! JSON argument shapes for DEI MCP tools.

use schemars::JsonSchema;
use serde::Deserialize;

/// MCP arguments: serialize one agent's task queue as JSON.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueueStatusParams {
    /// Target agent id.
    pub agent_id: u64,
}

/// MCP arguments: cancel helper used by some JSON-RPC shims (prefer [`crate::params::CancelTaskParams`]).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CancelTaskParams {
    /// Task id to cancel.
    pub task_id: u64,
}

/// MCP arguments: fetch Gamify event rows for one agent id string.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentEventsParams {
    /// Agent id as u64 (stringified for DB lookup).
    pub agent_id: u64,
}

/// MCP arguments: cap rows pulled per pseudo-agent when listing spend history.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CostHistoryParams {
    /// Per-agent SQL `LIMIT` before global merge (VS Code extension historically sent `buckets`).
    #[serde(alias = "buckets")]
    pub limit_per_agent: Option<i64>,
}

/// MCP arguments: cap merged event rows from Codex + transient buffer.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PollEventsParams {
    /// Maximum rows after sorting newest-first.
    pub limit: Option<i64>,
}

/// MCP arguments: lightweight task submit (string description + optional affinities).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitTaskParams {
    /// Raw task description (not canonicalized here).
    pub description: String,
    /// Optional file path hints (`write(...)` affinity strings).
    pub affinites: Option<Vec<String>>,
    /// Optional forced routing target.
    pub agent_id: Option<u64>,
    /// Optional session link (for chat/workflow grouping in Mens).
    pub session_id: Option<String>,
    /// Optional tenant identifier for budget attribution.
    #[serde(default)]
    pub tenant_id: Option<String>,
}

/// MCP arguments: correlate IDE session ids with orchestrator agents.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HeartbeatParams {
    /// Client session string previously mapped via `map_agent_session`.
    pub session_id: String,
}

/// MCP arguments: attribute spend to the agent tied to a session id.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordCostParams {
    /// Session key used to resolve the target agent.
    pub session_id: String,
    /// LLM provider slug (`openrouter`, ...).
    pub provider: String,
    /// Concrete model name/id.
    pub model: String,
    /// Total USD charged for the call.
    pub cost_usd: f64,
    /// Prompt tokens billed.
    pub input_tokens: u32,
    /// Completion tokens billed.
    pub output_tokens: u32,
}

/// MCP arguments: attention analytics lookback.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AttentionSummaryParams {
    /// Hours to look back (default 24).
    pub hours: Option<u64>,
}

/// MCP arguments: agent-to-agent handoff lineage limits.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HandoffLineageParams {
    /// Maximum lineage rows to return.
    pub limit: Option<u64>,
}
