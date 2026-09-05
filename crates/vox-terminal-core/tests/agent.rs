use vox_orchestrator::events::{AgentEvent, AgentEventKind, EventId};
use vox_orchestrator::types::AgentId;
use vox_terminal_core::agent::{AgentAdapterConfig, translate_event};
use vox_terminal_core::session::SessionEvent;

fn fake_agent_id() -> AgentId {
    AgentId(42)
}

#[test]
fn token_streamed_maps_to_agent_message() {
    let ev = AgentEvent {
        id: EventId(1),
        timestamp_ms: 0,
        kind: AgentEventKind::TokenStreamed {
            agent_id: fake_agent_id(),
            text: "hello world".into(),
            session_id: None,
        },
    };
    let cfg = AgentAdapterConfig { agent_id: None };
    let translated = translate_event(&ev, &cfg);
    assert!(
        matches!(&translated, Some(SessionEvent::AgentMessage { text }) if text == "hello world"),
        "got: {translated:?}"
    );
}

#[test]
fn non_token_events_return_none() {
    let ev = AgentEvent {
        id: EventId(2),
        timestamp_ms: 0,
        kind: AgentEventKind::AgentRetired {
            agent_id: fake_agent_id(),
        },
    };
    let cfg = AgentAdapterConfig { agent_id: None };
    assert!(translate_event(&ev, &cfg).is_none());
}
