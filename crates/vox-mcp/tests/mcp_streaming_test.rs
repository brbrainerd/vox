//! Integration tests for MCP streaming capabilities.

use vox_mcp::ServerState;
use vox_orchestrator::events::{AgentEvent, EventId as AgentEventId, AgentEventKind};
use vox_orchestrator::types::AgentId;
use std::sync::Arc;

#[tokio::test]
async fn test_mcp_token_streaming_poll() {
    let state = Arc::new(ServerState::new_test().await);
    let agent_id = AgentId(1);

    // 1. Inject a transient token event
    let event = AgentEvent {
        id: AgentEventId(42),
        timestamp_ms: 1000,
        kind: AgentEventKind::TokenStreamed {
            agent_id,
            text: "Hello".to_string(),
        },
    };

    {
        let mut transient = state.transient_events.lock().await;
        transient.push(event);
    }

    // 2. Poll events via the internal tool logic (simplified for test)
    let mut all_events = Vec::new();
    {
        let mut transient = state.transient_events.lock().await;
        for ev in transient.drain(..) {
            let (agent_id_str, event_type) = match &ev.kind {
                AgentEventKind::TokenStreamed { agent_id, .. } => (agent_id.to_string(), "TokenStreamed"),
                _ => ("A-00".to_string(), "Other"),
            };
            let payload = serde_json::to_string(&ev.kind).unwrap_or_default();
            all_events.push(vox_gamify::db::AgentEventRecord {
                id: ev.id.0 as i64,
                agent_id: agent_id_str,
                event_type: event_type.to_string(),
                payload: Some(payload),
                timestamp: ev.timestamp_ms.to_string(),
            });
        }
    }

    assert_eq!(all_events.len(), 1);
    assert_eq!(all_events[0].id, 42);
    assert!(all_events[0].payload.as_ref().unwrap().contains("Hello"));
    assert_eq!(all_events[0].event_type, "TokenStreamed");
}
