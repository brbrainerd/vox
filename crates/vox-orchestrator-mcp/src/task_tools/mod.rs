//! Task management tool handlers for the Vox MCP server.
//!
//! Covers: submit, status, complete, fail, cancel, reorder, drain, and publish.
//!
//! ## Policy side effects
//! Submit/status paths participate in **interruption / attention policy** (`super::attention_policy`): they may call
//! [`ServerState::record_attention_event`](crate::server_state::ServerState::record_attention_event) and read trust snapshots when
//! questioning backlog or human-confirmation gates apply. That is **budget-plane telemetry**, not remote product analytics.

use crate::params::{PublishMessageParams, ToolResult};
use crate::server_state::ServerState;
use vox_orchestrator::TaskId;

pub mod submission;
pub use submission::*;
pub mod lifecycle;
pub use lifecycle::*;
pub mod query;
pub use query::*;

/// Publish a message to the bulletin board.
pub async fn publish_message(state: &ServerState, _params: PublishMessageParams) -> String {
    let orch = &state.orchestrator;
    let board = orch.bulletin();
    board.publish(vox_orchestrator::AgentMessage::DependencyReady { task_id: TaskId(0) });
    ToolResult::ok("message published".to_string()).to_json()
}

#[cfg(test)]
mod tests;
