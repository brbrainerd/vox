//! Console → agent messaging: send a free-form note from the console operator to
//! a running agent's A2A inbox, reusing the orchestrator's `send_to_db` path.
//!
//! Sender identity: the console operator is the human at the keyboard, not an
//! orchestrator-spawned agent. Orchestrator agent ids come from an `AtomicU64`
//! generator starting at 1, so `AgentId(0)` is reserved here as the console
//! operator sentinel.

use vox_orchestrator::a2a::{send_to_db_with_breaker, A2AMessageType};
use vox_orchestrator::types::{AgentId, MessagePriority};

/// The reserved sender id for messages originating from the console operator.
pub const CONSOLE_OPERATOR_AGENT_ID: u64 = 0;

/// Parse a receiver agent id string into an `AgentId`. Accepts both the display
/// form ("A-05") and a bare number ("5"), matching `AgentId`'s `FromStr`.
pub fn parse_receiver(raw: &str) -> Result<AgentId, String> {
    raw.trim()
        .parse::<AgentId>()
        .map_err(|_| format!("invalid agent id: {raw:?}"))
}

/// Send a free-form note to an agent's A2A inbox. Returns the new message id.
#[tauri::command]
pub async fn send_to_agent(agent_id: String, body: String) -> Result<String, String> {
    if body.trim().is_empty() {
        return Err("message body is empty".into());
    }
    let receiver = parse_receiver(&agent_id)?;
    let config = vox_db::DbConfig::resolve_for_mesh().map_err(|e| e.to_string())?;
    let db = vox_db::Codex::connect(config).await.map_err(|e| e.to_string())?;
    send_to_db_with_breaker(
        &db,
        AgentId(CONSOLE_OPERATOR_AGENT_ID),
        receiver,
        A2AMessageType::FreeForm,
        body,
        MessagePriority::Normal,
        None,
        "",
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_number() {
        assert_eq!(parse_receiver("5").unwrap(), AgentId(5));
    }

    #[test]
    fn parses_display_form() {
        assert_eq!(parse_receiver("A-07").unwrap(), AgentId(7));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_receiver("not-an-id").is_err());
    }
}
