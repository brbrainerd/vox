//! Multi-modal Visual Retrieval-Augmented Generation (RAG) tools.
//!
//! Demonstrates the standardized workflow for extending Vox capabilities
//! without relying on forbidden language-level macros or plugins. All external
//! intelligence is routed through the standard MCP interface and mapped via
//! the `vox-skills` registry.

use crate::params::{ToolResult, VoxVisualRagQueryParams, VoxVisualRagQueryResponse};
use crate::server_state::ServerState;

/// Dispatches a multi-modal visual RAG query to the configured intelligence backend.
pub async fn visual_rag_query(_state: &ServerState, params: VoxVisualRagQueryParams) -> String {
    if params.image_paths.is_empty() && params.image_base64.is_none() {
        return ToolResult::<VoxVisualRagQueryResponse>::err(
            "MISSING_MODALITY: A visual RAG query requires at least one image path or base64 payload.",
        )
        .to_json_compact();
    }

    let image_count =
        params.image_paths.len() + params.image_base64.as_ref().map(|v| v.len()).unwrap_or(0);

    tracing::warn!(
        target: "vox_mcp::rag",
        query = %params.query,
        images = image_count,
        "vox_visual_rag_query is not implemented — multimodal retrieval is planned; use vox_search or memory retrieval tools instead"
    );

    ToolResult::<VoxVisualRagQueryResponse>::err(
        "NOT_IMPLEMENTED: Visual RAG is not wired yet. Use vox_search, vox_memory_retrieval, or vox_repo_search for text retrieval until multimodal RAG lands.",
    )
    .to_json()
}
