//! MCP tools for file affinity: resolve owners, claim paths, transfer ownership, list files.
//!
//! All handlers return JSON via `vox_orchestrator::ToolResult`. Mutating calls update the in-memory
//! orchestrator affinity map (not the filesystem).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_orchestrator::AgentId;

const REM_AFFINITY_CLAIM: &str = "Release the path, pick another file, or negotiate transfer with the owning agent via affinity tools.";
const REM_AFFINITY_TRANSFER: &str =
    "Use `file_owner` / `my_files` to verify current ownership before transferring.";

/// MCP arguments: workspace-relative or absolute path to inspect for an owning agent.
#[derive(Debug, Deserialize)]
pub struct FileOwnerParams {
    /// Repository file path as sent by the client (string form).
    pub path: String,
}

/// MCP arguments: list paths currently assigned to one agent.
#[derive(Debug, Deserialize)]
pub struct MyFilesParams {
    /// Numeric orchestrator agent id.
    pub agent_id: u64,
}

/// MCP arguments: agent takes exclusive affinity for `path` if unowned or already owned by them.
#[derive(Debug, Deserialize)]
pub struct ClaimFileParams {
    /// Agent claiming exclusive affinity.
    pub agent_id: u64,
    /// File path to assign.
    pub path: String,
}

/// MCP arguments: move affinity from `from_agent` to `to_agent` when the source owns the file.
#[derive(Debug, Deserialize)]
pub struct TransferFileParams {
    /// Current owner agent id.
    pub from_agent: u64,
    /// Recipient agent id.
    pub to_agent: u64,
    /// File path to move between agents.
    pub path: String,
}

/// JSON fragment: resolved owner for a single path (`owner` is `None` if unassigned).
#[derive(Debug, Serialize)]
pub struct FileOwnerResponse {
    /// Path that was queried.
    pub path: String,
    /// Owning agent id when the affinity map has an entry.
    pub owner: Option<u64>,
}

/// JSON fragment: paths the affinity map associates with `agent_id`.
#[derive(Debug, Serialize)]
pub struct MyFilesResponse {
    /// Agent whose file list is returned.
    pub agent_id: u64,
    /// Owned paths as strings.
    pub files: Vec<String>,
}

/// Return JSON describing which agent (if any) owns `params.path` in the affinity map.
pub async fn file_owner(state: &ServerState, params: FileOwnerParams) -> String {
    let orch = &state.orchestrator;

    let path = PathBuf::from(&params.path);
    let owner = orch.affinity_map().lookup(&path).map(|id| id.0);

    ToolResult::ok(FileOwnerResponse {
        path: params.path,
        owner,
    })
    .to_json()
}

/// Return JSON listing all file paths owned by `params.agent_id`.
pub async fn my_files(state: &ServerState, params: MyFilesParams) -> String {
    let orch = &state.orchestrator;

    let files = orch
        .affinity_map()
        .files_for_agent(AgentId(params.agent_id));
    let files_str: Vec<String> = files.into_iter().map(|p| p.display().to_string()).collect();

    ToolResult::ok(MyFilesResponse {
        agent_id: params.agent_id,
        files: files_str,
    })
    .to_json()
}

/// Assign `params.path` to `params.agent_id` when not owned by another agent; mutates affinity map.
pub async fn claim_file(state: &ServerState, params: ClaimFileParams) -> String {
    let orch = &state.orchestrator;

    let path = PathBuf::from(&params.path);
    let agent_id = AgentId(params.agent_id);

    if let Some(existing) = orch.affinity_map().lookup(&path) {
        if existing != agent_id {
            return ToolResult::<String>::err_with_remediation(
                format!("File already owned by agent {}", existing.0),
                REM_AFFINITY_CLAIM,
            )
            .to_json();
        }
    }

    orch.affinity_map_mut().assign(&path, agent_id);
    ToolResult::ok(format!("Successfully claimed {}", params.path)).to_json()
}

/// Release then re-assign `params.path` from `from_agent` to `to_agent`; errors if ownership mismatches.
pub async fn transfer_file(state: &ServerState, params: TransferFileParams) -> String {
    let orch = &state.orchestrator;

    let path = PathBuf::from(&params.path);
    let from_id = AgentId(params.from_agent);
    let to_id = AgentId(params.to_agent);

    if let Some(existing) = orch.affinity_map().lookup(&path) {
        if existing != from_id {
            return ToolResult::<String>::err_with_remediation(
                format!("File is owned by {}, not {}", existing.0, from_id.0),
                REM_AFFINITY_TRANSFER,
            )
            .to_json();
        }
    } else {
        return ToolResult::<String>::err_with_remediation(
            "File is not currently owned by anyone",
            REM_AFFINITY_TRANSFER,
        )
        .to_json();
    }

    // Perform transfer
    orch.affinity_map_mut().release(&path);
    orch.affinity_map_mut().assign(&path, to_id);

    ToolResult::ok(format!(
        "Successfully transferred {} to agent {}",
        params.path, to_id.0
    ))
    .to_json()
}
