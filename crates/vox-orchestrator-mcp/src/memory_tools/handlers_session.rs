use crate::params::ToolResult;
use crate::server_state::ServerState;

use super::params::{SessionCompactParams, SessionCreateParams, SessionIdParams, SessionInfo};

const REM_SESSION_OP: &str = "Verify the agent id and orchestrator session manager health; restart MCP if state is corrupted.";
const REM_SESSION_ID: &str =
    "Call `session_list` and pass an existing `session_id`; ids are case-sensitive.";

/// Create a new session for an agent (async).
pub async fn session_create(state: &ServerState, params: SessionCreateParams) -> String {
    let mut mgr = state.session_manager.lock().await;
    match mgr.create(
        vox_orchestrator::AgentId(params.agent_id),
        params.tenant_id,
    ) {
        Ok(id) => ToolResult::ok(id).to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_SESSION_OP).to_json()
        }
    }
}

/// List all sessions (async).
pub async fn session_list(state: &ServerState) -> String {
    let mgr = state.session_manager.lock().await;
    let sessions: Vec<SessionInfo> = mgr
        .list_sessions()
        .iter()
        .map(|s| SessionInfo::from_session(*s))
        .collect();
    ToolResult::ok(sessions).to_json()
}

/// Reset a session (clear history, keep metadata) (async).
pub async fn session_reset(state: &ServerState, params: SessionIdParams) -> String {
    let mut mgr = state.session_manager.lock().await;
    match mgr.reset(&params.session_id) {
        Ok(cleared) => ToolResult::ok(format!(
            "Session '{}' reset: {} turns cleared.",
            params.session_id, cleared
        ))
        .to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_SESSION_OP).to_json()
        }
    }
}

/// Compact a session with a summary (async).
pub async fn session_compact(state: &ServerState, params: SessionCompactParams) -> String {
    let mut mgr = state.session_manager.lock().await;
    match mgr.compact(&params.session_id, &params.summary) {
        Ok(removed) => ToolResult::ok(format!(
            "Session '{}' compacted: {} turns replaced with summary.",
            params.session_id, removed
        ))
        .to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_SESSION_OP).to_json()
        }
    }
}

/// Get info about a specific session (async).
pub async fn session_info(state: &ServerState, params: SessionIdParams) -> String {
    let mgr = state.session_manager.lock().await;
    match mgr.get(&params.session_id) {
        Some(s) => ToolResult::ok(SessionInfo::from_session(s)).to_json(),
        None => ToolResult::<String>::err_with_remediation(
            format!("Session '{}' not found.", params.session_id),
            REM_SESSION_ID,
        )
        .to_json(),
    }
}

/// Cleanup archived sessions (async).
pub async fn session_cleanup(state: &ServerState) -> String {
    let mut mgr = state.session_manager.lock().await;
    mgr.tick_lifecycle();
    match mgr.cleanup() {
        Ok(n) => ToolResult::ok(format!("{n} sessions cleaned up.")).to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_SESSION_OP).to_json()
        }
    }
}
