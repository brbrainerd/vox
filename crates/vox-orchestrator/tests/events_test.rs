use std::path::PathBuf;
use tokio::time::{timeout, Duration};
use vox_orchestrator::{
    events::{AgentEventKind, EventBus},
    AgentId,
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

    let event = timeout(Duration::from_secs(1), rx.recv())
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
