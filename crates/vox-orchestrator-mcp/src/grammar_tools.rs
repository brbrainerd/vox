//! Grammar and Observer MCP tools.
//!
//! - `vox_export_grammar_ebnf` — export the Vox EBNF grammar (Task 39, restored wire)
//! - `vox_observer_status` — return observer summary for a task_id (Task 69)

use crate::params::ToolResult;
use crate::server_state::ServerState;
use serde::Deserialize;

pub async fn export_grammar_ebnf(_state: &ServerState) -> String {
    let ebnf = vox_grammar_export::ebnf::emit_ebnf();
    ToolResult::ok(ebnf).to_json()
}

#[derive(Debug, Deserialize)]
pub struct ObserverStatusParams {
    /// Task ID to summarize observations for.
    pub task_id: String,
}

/// Return the observer health summary for `task_id`.
///
/// The `Observer` instance lives per-server; this reflects whatever has been
/// accumulated for `task_id` in the current process lifetime.
pub async fn observer_status(state: &ServerState, params: ObserverStatusParams) -> String {
    let summary = state.observer.summarize(&params.task_id);
    match serde_json::to_string(&summary) {
        Ok(json) => ToolResult::ok(json).to_json(),
        Err(e) => ToolResult::<String>::err(format!("serialization error: {e}")).to_json(),
    }
}
