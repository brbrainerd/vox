//! MCP tools: episodic memory, knowledge graph queries, and session-scoped recall.
//!
//! **Agents calling these tools:** Use stable, namespaced `key` strings for writes; pair
//! `MemoryStoreParams::agent_id` with the orchestrator’s agent id when available. Cap graph
//! fan-out with [`KnowledgeQueryParams::limit`](crate::memory::KnowledgeQueryParams::limit) so tool responses stay bounded. For authoritative
//! Codex semantics, see repo-root `AGENTS.md` §2.2.1 and ADR 004—not every tool doc repeats the
//! data-plane glossary.

mod config;
mod handlers_compaction;
mod handlers_memory;
mod handlers_preferences;
mod handlers_session;
mod params;
mod retrieval;

#[cfg(test)]
mod tests;

pub use handlers_compaction::*;
pub use handlers_memory::*;
pub use handlers_preferences::*;
pub use handlers_session::*;
pub use params::*;
pub use retrieval::{
    RetrievalBundle, RetrievalEvidenceEnvelope, RetrievalTriggerMode, run_retrieval_bundle,
};

use std::path::Path;

/// AgentOS semantic-fs bridge: rank workspace paths from an `intent` string.
pub fn semantic_fs_discover(
    repo_root: &Path,
    intent: &str,
    limit: usize,
    policy: &vox_search::SearchPolicy,
) -> Vec<serde_json::Value> {
    vox_search::semantic_fs::discover_files_for_intent(repo_root, intent, limit, policy)
}
