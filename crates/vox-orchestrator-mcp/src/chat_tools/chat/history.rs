use super::super::params::{ChatHistoryParams, ChatTranscriptEntry};
use super::hydrate::context_history_or_hydrate;
use crate::params::ToolResult;
use crate::server_state::ServerState;
use crate::session_identity::normalize_chat_session_id;

/// Return the full chat history for a session.
///
/// Pass `params.session_id` to retrieve the isolated transcript for a specific session.
/// Empty / omitted id normalizes to `"default"`, matching `chat_message`.
pub async fn chat_history(state: &ServerState, params: ChatHistoryParams) -> String {
    let (session_id, _) = normalize_chat_session_id(Some(params.session_id.as_str()));
    if let Some(tid) = params.trace_id.as_deref().filter(|s| !s.trim().is_empty()) {
        tracing::debug!(target: "vox_mcp::chat_history", trace_id = %tid, session = %session_id, "chat_history request");
    }
    let history_key = format!("chat_history:{session_id}");
    let history: Vec<ChatTranscriptEntry> =
        context_history_or_hydrate(state, history_key.as_str(), session_id.as_str()).await;
    ToolResult::ok(history).to_json()
}
