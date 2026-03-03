use vox_orchestrator::{types::TaskId, AgentId, HandoffPayload};

#[test]
fn test_handoff_payload_serialization() {
    let handoff = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "Initial Setup")
        .with_pending(vec![TaskId(5)])
        .with_metadata("db_url", "local");

    let json = handoff.to_json();
    assert!(json.contains("Initial Setup"));
    assert!(json.contains("local"));

    let parsed = HandoffPayload::from_json(&json).unwrap();
    assert_eq!(parsed.from_agent, AgentId(1));
    assert_eq!(parsed.to_agent, Some(AgentId(2)));
    assert_eq!(parsed.pending_tasks.len(), 1);
    assert_eq!(parsed.metadata.len(), 1);
}
