//! Pure model resolution for MCP chat: registry lookup, free-tier enforcement, context signals.

mod free_tier_adapter;
mod policy;
mod resolve;
mod types;

#[cfg(test)]
#[allow(unsafe_code)] // tests use `set_var` / `remove_var` under a global lock (Rust 2024)
mod tests;

pub use resolve::{
    mcp_global_llm_context_fill_ratio, mcp_provider_telemetry_labels, resolve_mcp_chat_model,
    resolve_mcp_chat_model_sync, resolve_mcp_chat_model_with_rationale,
};
pub use types::McpChatModelResolution;
