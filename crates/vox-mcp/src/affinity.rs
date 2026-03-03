use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{ServerState, ToolResult};
use vox_orchestrator::AgentId;

#[derive(Debug, Deserialize)]
pub struct FileOwnerParams {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct MyFilesParams {
    pub agent_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct ClaimFileParams {
    pub agent_id: u64,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct TransferFileParams {
    pub from_agent: u64,
    pub to_agent: u64,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct FileOwnerResponse {
    pub path: String,
    pub owner: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct MyFilesResponse {
    pub agent_id: u64,
    pub files: Vec<String>,
}

pub async fn file_owner(state: &ServerState, params: FileOwnerParams) -> String {
    let orch = state.orchestrator.lock().await;

    let path = PathBuf::from(&params.path);
    let owner = orch.affinity_map().lookup(&path).map(|id| id.0);

    ToolResult::ok(FileOwnerResponse {
        path: params.path,
        owner,
    })
    .to_json()
}

pub async fn my_files(state: &ServerState, params: MyFilesParams) -> String {
    let orch = state.orchestrator.lock().await;

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

pub async fn claim_file(state: &ServerState, params: ClaimFileParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    let path = PathBuf::from(&params.path);
    let agent_id = AgentId(params.agent_id);

    if let Some(existing) = orch.affinity_map().lookup(&path) {
        if existing != agent_id {
            return ToolResult::<String>::err(format!(
                "File already owned by agent {}",
                existing.0
            ))
            .to_json();
        }
    }

    orch.affinity_map_mut().assign(&path, agent_id);
    ToolResult::ok(format!("Successfully claimed {}", params.path)).to_json()
}

pub async fn transfer_file(state: &ServerState, params: TransferFileParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    let path = PathBuf::from(&params.path);
    let from_id = AgentId(params.from_agent);
    let to_id = AgentId(params.to_agent);

    if let Some(existing) = orch.affinity_map().lookup(&path) {
        if existing != from_id {
            return ToolResult::<String>::err(format!(
                "File is owned by {}, not {}",
                existing.0, from_id.0
            ))
            .to_json();
        }
    } else {
        return ToolResult::<String>::err("File is not currently owned by anyone").to_json();
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
