//! Shared MCP chat model resolution (registry + token-budget hint).
//!
//! Callers pass a [`McpChatModelResolution`](crate::llm_bridge::McpChatModelResolution); when
//! `context_fill_ratio` is unset, it is filled from the global MCP LLM budget agent (`AgentId(0)`).

use crate::llm_bridge::{
    McpChatModelResolution, mcp_global_llm_context_fill_ratio, resolve_mcp_chat_model,
};
use crate::server_state::ServerState;
use vox_orchestrator::models::ModelSpec;

/// Resolve model from sticky override + registry; fills `context_fill_ratio` when omitted.
pub async fn resolve_chat_llm_model(
    state: &ServerState,
    user_prompt: &str,
    mut resolution: McpChatModelResolution,
    user_id: Option<&str>,
) -> Result<(ModelSpec, bool), String> {
    let pref = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => return Err(e.to_string()),
    };
    let orch = &state.orchestrator;
    if resolution.context_fill_ratio.is_none() {
        resolution.context_fill_ratio = mcp_global_llm_context_fill_ratio(orch);
    }
    resolve_mcp_chat_model(state, user_prompt, pref.as_deref(), resolution, user_id).await
}
