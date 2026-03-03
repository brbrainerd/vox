use vox_orchestrator::{AgentId, ContinuationEngine, ContinuationStrategy, EventBus};

#[test]
fn test_continuation_engine() {
    let mut engine = ContinuationEngine::new(0, 5);
    let bus = EventBus::new(16);
    let agent_id = AgentId(1);

    let prompt1 = engine.generate_continuation(agent_id, ContinuationStrategy::Continue, 1, &bus);
    assert!(prompt1.is_some());
    assert_eq!(prompt1.unwrap().agent_id, agent_id);
    assert_eq!(engine.continuation_count(agent_id), 1);

    // Testing cooling down (set cooldown to 10s for agent 2)
    let mut cooled_engine = ContinuationEngine::new(10000, 5);
    let prompt2 =
        cooled_engine.generate_continuation(AgentId(2), ContinuationStrategy::Continue, 1, &bus);
    assert!(prompt2.is_some());

    // Attempting second immediately should fail due to cooldown
    let prompt3 =
        cooled_engine.generate_continuation(AgentId(2), ContinuationStrategy::Continue, 1, &bus);
    assert!(prompt3.is_none());

    // But working after reset
    cooled_engine.reset_cooldown(AgentId(2));
    let prompt4 =
        cooled_engine.generate_continuation(AgentId(2), ContinuationStrategy::Continue, 1, &bus);
    assert!(prompt4.is_some());
}
