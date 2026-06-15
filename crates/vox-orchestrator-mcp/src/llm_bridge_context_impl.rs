//! Implementation of [`vox_mcp_llm_bridge::McpServerContext`] for [`ServerState`].
//!
//! This thin impl exposes exactly the six fields that the bridge's inference loop
//! needs — nothing more. Keeping the impl here (not in `vox-mcp-llm-bridge`) avoids
//! a circular crate dependency: the bridge crate has no dep on orchestrator-mcp.

use std::sync::Arc;

use vox_mcp_llm_bridge::McpServerContext;
use vox_orchestrator::{BudgetManager, OrchestratorConfig, Orchestrator};

use crate::server_state::ServerState;

impl McpServerContext for ServerState {
    fn db(&self) -> Option<&Arc<vox_db::VoxDb>> {
        self.db.as_ref()
    }

    fn orchestrator(&self) -> &Arc<Orchestrator> {
        &self.orchestrator
    }

    fn orchestrator_config(&self) -> &OrchestratorConfig {
        &self.orchestrator_config
    }

    fn budget_manager(&self) -> &Arc<BudgetManager> {
        &self.budget_manager
    }

    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    fn mcp_chat_model_override(&self) -> Option<String> {
        self.mcp_chat_model_override.read().clone()
    }
}
