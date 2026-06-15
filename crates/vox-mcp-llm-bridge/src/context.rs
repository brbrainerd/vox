//! Trait abstracting the MCP server state needed by the LLM inference bridge.
//!
//! [`ServerState`] in `vox-orchestrator-mcp` implements this trait so the
//! bridge crate does not depend back on orchestrator-mcp internals.

use std::sync::Arc;

use vox_orchestrator::{BudgetManager, OrchestratorConfig, Orchestrator};

/// Narrow interface over MCP server state required by the LLM inference bridge.
///
/// Implement this on your server-state type and pass `&dyn McpServerContext`
/// to the bridge functions. Only the fields actually read by `infer.rs` and
/// `model_route_policy/resolve.rs` are exposed.
pub trait McpServerContext: Send + Sync {
    /// The optional VoxDb handle (absent when running without a Codex database).
    fn db(&self) -> Option<&Arc<vox_db::VoxDb>>;

    /// The in-process orchestrator used for model-registry and event-bus access.
    fn orchestrator(&self) -> &Arc<Orchestrator>;

    /// Static orchestrator configuration (tuning knobs, feature flags).
    fn orchestrator_config(&self) -> &OrchestratorConfig;

    /// In-process budget manager for token/cost gates.
    fn budget_manager(&self) -> &Arc<BudgetManager>;

    /// Shared HTTP client for all provider calls.
    fn http_client(&self) -> &reqwest::Client;

    /// Read the sticky MCP chat model override, returning a cloned `Option<String>`.
    ///
    /// Returning a clone avoids exposing the `Arc<RwLock<…>>` internals across the
    /// crate boundary while still giving the bridge a consistent snapshot.
    fn mcp_chat_model_override(&self) -> Option<String>;
}
