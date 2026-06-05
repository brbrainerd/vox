use std::path::PathBuf;
use tokio::time::timeout;
use vox_orchestrator::{
    AgentId,
    events::{AgentEventKind, EventBus},
};

#[tokio::test]
async fn test_event_bus() {
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    bus.emit(AgentEventKind::LockAcquired {
        agent_id: AgentId(1),
        path: PathBuf::from("src/main.rs"),
        exclusive: true,
    });

    let event = timeout(vox_config::timeouts::D_1S, rx.recv())
        .await
        .expect("timeout")
        .expect("recv");
    if let AgentEventKind::LockAcquired {
        agent_id,
        path,
        exclusive,
    } = event.kind
    {
        assert_eq!(agent_id, AgentId(1));
        assert_eq!(path.to_str().unwrap(), "src/main.rs");
        assert!(exclusive);
    } else {
        panic!("Wrong event kind");
    }
}
