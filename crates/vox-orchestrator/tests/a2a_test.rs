use vox_orchestrator::a2a::{A2AMessageType, MessageBus};
use vox_orchestrator::AgentId;

#[test]
fn test_a2a_communication() {
    let mut bus = MessageBus::new(10);
    let agent1 = AgentId(1);
    let agent2 = AgentId(2);

    bus.register_agent(agent1);
    bus.register_agent(agent2);

    // Broadcast check
    let _msg_id = bus.broadcast(agent1, A2AMessageType::FreeForm, "Hello everyone");

    let inbox1 = bus.inbox(agent1);
    assert_eq!(inbox1.len(), 0); // Sender doesn't receive broadcast

    let inbox2 = bus.inbox(agent2);
    assert_eq!(inbox2.len(), 1);
    assert_eq!(inbox2[0].sender, agent1);
    assert_eq!(inbox2[0].msg_type, A2AMessageType::FreeForm);
    assert_eq!(inbox2[0].payload, "Hello everyone");
}
