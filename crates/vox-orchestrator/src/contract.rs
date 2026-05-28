//! Canonical orchestration contract (v2-compatible payloads) for MCP, CLI, and DeI surfaces.
//!
//! Use these types when crossing subsystem boundaries so task requirements, sessions, and
//! migration flags stay aligned. See also `PLAN_TOOL_DAEMON_ALIGNMENT`.

use serde::{Deserialize, Serialize};
// schemars usage moved with mcp_tools to vox-orchestrator-mcp.

/// Version label for orchestration wire format (bump when breaking compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationContractVersion {
    /// Legacy task/session shapes (default).
    #[default]
    V1,
    /// Opt-in unified contract (adapters may translate to V1).
    V2,
}

pub use vox_repository::TaskCapabilityHints;

/// Session envelope for tools and Codex dual-write (repository-scoped).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionContractEnvelope {
    /// Stable repository id from `vox-repository`.
    pub repository_id: String,
    /// Opaque session id (JSONL or Codex).
    pub session_id: String,
    /// Optional MCP / IDE metadata (JSON-serializable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Staged migration toggles (defaults preserve legacy behavior).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrchestrationMigrationFlags {
    /// When true, prefer v2 contract paths where implemented.
    #[serde(default)]
    pub orchestration_v2_enabled: bool,
    /// When true, allow falling back to legacy handlers if v2 fails.
    #[serde(default = "vox_config::serde_defaults::default_true")]
    pub legacy_orchestration_fallback: bool,
}

impl Default for OrchestrationMigrationFlags {
    fn default() -> Self {
        Self {
            orchestration_v2_enabled: false,
            legacy_orchestration_fallback: true,
        }
    }
}

/// MCP planning tool names aligned index-wise with DeI `ai.plan.*` methods (new / replan / status).
pub const MCP_PLAN_TOOL_NAMES: &[&str] = &["vox_plan", "vox_replan", "vox_plan_status"];

/// DeI daemon JSON-RPC methods for the same planning operations (execute is separate).
pub const DEI_PLAN_METHODS_NEW_REPLAN_STATUS: &[&str] =
    &["ai.plan.new", "ai.plan.replan", "ai.plan.status"];

/// Documented mapping: `MCP_PLAN_TOOL_NAMES[i]` ↔ `DEI_PLAN_METHODS_NEW_REPLAN_STATUS[i]`.
#[must_use]
pub fn plan_tool_daemon_alignment_valid() -> bool {
    MCP_PLAN_TOOL_NAMES.len() == DEI_PLAN_METHODS_NEW_REPLAN_STATUS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_alignment_len_matches() {
        assert!(plan_tool_daemon_alignment_valid());
        assert_eq!(MCP_PLAN_TOOL_NAMES.len(), 3);
    }
}
