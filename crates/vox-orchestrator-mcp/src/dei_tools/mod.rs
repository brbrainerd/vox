//! Live DEI orchestrator inspection and lightweight control surfaces exposed as MCP tools.
//!
//! Covers agent queues, lock/budget summaries, Gamify-backed history, affinity graphs,
//! JSON config patch, task submission shims, session heartbeats, and cost recording.

pub mod params;

mod control;
mod orchestrator_snapshot;
mod vcs_runtime;

pub use control::{
    agent_events, attention_summary, budget_status, cancel_task, config_get, config_set,
    cost_history, drain_agent, file_graph, handoff_lineage, lock_status, map_agent_session,
    orchestrator_start, pause_agent, queue_status, rebalance, reorder_task, resume_agent,
    retire_agent, spawn_agent,
};
pub use orchestrator_snapshot::orchestrator_status;
pub use params::*;
pub use vcs_runtime::{
    check_file_owner, heartbeat, poll_events, record_cost, submit_task, vcs_status,
};
