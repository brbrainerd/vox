//! Round-trip TCP daemon for `orch.ping` / `orch.task_status`.
//!
//! Daemon readiness waits on a successful `ping()` instead of a fixed sleep after spawn.

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use vox_orchestrator::{
    AgentTask, Orchestrator, OrchestratorConfig, TaskId, TaskPriority, orch_daemon,
};

async fn wait_until_async<F, Fut>(
    label: &str,
    timeout: Duration,
    interval: Duration,
    mut condition: F,
) where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if condition().await {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("{label}: timed out after {timeout:?}");
        }
        tokio::time::sleep(interval).await;
    }
}

/// Wall-clock ceiling so local TCP daemon tests cannot stall indefinitely if readiness RPC regresses.
const DAEMON_TEST_TIMEOUT: Duration = vox_config::timeouts::D_60S;

#[tokio::test]
async fn orchestrator_daemon_ping_and_task_status() {
    tokio::time::timeout(DAEMON_TEST_TIMEOUT, async {
        orchestrator_daemon_ping_and_task_status_inner().await;
    })
    .await
    .expect("orchestrator_daemon_ping_and_task_status exceeded wall-clock budget");
}

async fn orchestrator_daemon_ping_and_task_status_inner() {
    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    let aid = orch.spawn_agent("d1").expect("spawn");
    let tid = TaskId(4242);
    let task = AgentTask::new(tid, "daemon probe", TaskPriority::Normal, vec![]);
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let mut q = ql.write().unwrap();
        q.enqueue(task);
        let _ = q.dequeue();
    }
    orch.task_assignments.write().unwrap().insert(tid, aid);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let o = orch.clone();
    let bind_label = addr.to_string();
    let server = tokio::spawn(orch_daemon::serve_listener(
        listener,
        bind_label,
        "ut-repo".to_string(),
        o,
    ));

    let addr_str = addr.to_string();
    wait_until_async(
        "orchestrator daemon TCP accepting (`orch.ping`)",
        vox_config::timeouts::D_15S,
        vox_config::timeouts::D_5MS,
        || {
            let c = orch_daemon::OrchDaemonClient::new(addr_str.clone());
            async move { c.ping().await.is_ok() }
        },
    )
    .await;

    let client = orch_daemon::OrchDaemonClient::new(addr_str);
    let ping = client.ping().await.expect("ping");
    assert_eq!(ping["repository_id"], "ut-repo");
    assert_eq!(ping["protocol"], "vox.orchestrator_daemon/v1");

    let st = client.orchestrator_status().await.expect("status");
    assert!(st.get("agent_count").is_some());

    let ts = client.task_status(4242).await.expect("task_status");
    assert_eq!(ts["status"], "InProgress");

    let spawned = client
        .spawn_agent_named("rpc-spawn")
        .await
        .expect("spawn_agent");
    assert!(spawned["agent_id"].as_u64().is_some());

    let ids = client.agent_ids().await.expect("agent_ids");
    assert!(ids["agent_ids"].as_array().is_some());

    let wj = client.workspace_journey().await.expect("workspace_journey");
    assert_eq!(wj["daemon_repository_id"], "ut-repo");
    assert!(wj.get("workspace_journey_store_mode").is_some());

    server.abort();
}

#[tokio::test]
async fn orchestrator_daemon_subscribe_streams_status_event() {
    tokio::time::timeout(DAEMON_TEST_TIMEOUT, async {
        orchestrator_daemon_subscribe_streams_status_event_inner().await;
    })
    .await
    .expect("orchestrator_daemon_subscribe_streams_status_event exceeded wall-clock budget");
}

async fn orchestrator_daemon_subscribe_streams_status_event_inner() {
    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    orch.spawn_agent("sub1").expect("spawn");

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let server = tokio::spawn(orch_daemon::serve_listener(
        listener,
        addr.to_string(),
        "ut-repo".to_string(),
        orch.clone(),
    ));
    let addr_str = addr.to_string();
    wait_until_async(
        "orchestrator daemon TCP accepting (`orch.ping`)",
        vox_config::timeouts::D_15S,
        vox_config::timeouts::D_5MS,
        || {
            let c = orch_daemon::OrchDaemonClient::new(addr_str.clone());
            async move { c.ping().await.is_ok() }
        },
    )
    .await;

    // Subscribing must push at least one structured Event frame carrying an
    // orchestrator status snapshot — the daemon initiates the push, the client
    // does not poll.
    let client = orch_daemon::OrchDaemonClient::new(addr_str);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(8);
    let sub = tokio::spawn(async move {
        let _ = client.subscribe(tx).await;
    });

    let first = tokio::time::timeout(vox_config::timeouts::D_15S, rx.recv())
        .await
        .expect("first subscribe event within budget")
        .expect("subscribe channel produced an event");
    assert!(
        first.get("agent_count").is_some(),
        "first subscribe event must be a status snapshot, got: {first}"
    );

    drop(rx);
    sub.abort();
    server.abort();
}

#[tokio::test]
async fn orchestrator_daemon_subscribe_events_streams_agent_events() {
    tokio::time::timeout(DAEMON_TEST_TIMEOUT, async {
        orchestrator_daemon_subscribe_events_inner().await;
    })
    .await
    .expect("orchestrator_daemon_subscribe_events exceeded wall-clock budget");
}

async fn orchestrator_daemon_subscribe_events_inner() {
    use vox_orchestrator::events::AgentEventKind;

    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let server = tokio::spawn(orch_daemon::serve_listener(
        listener,
        addr.to_string(),
        "ut-repo".to_string(),
        orch.clone(),
    ));
    let addr_str = addr.to_string();
    wait_until_async(
        "orchestrator daemon TCP accepting (`orch.ping`)",
        vox_config::timeouts::D_15S,
        vox_config::timeouts::D_5MS,
        || {
            let c = orch_daemon::OrchDaemonClient::new(addr_str.clone());
            async move { c.ping().await.is_ok() }
        },
    )
    .await;

    // Subscribe to the live agent-event stream (distinct from orch.subscribe's
    // status snapshots).
    let client = orch_daemon::OrchDaemonClient::new(addr_str);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    let sub = tokio::spawn(async move {
        let _ = client.subscribe_events(tx).await;
    });

    // The bus is a broadcast channel with no replay, so the daemon only sees
    // events emitted after its subscription is established. Emit on an interval
    // until the subscriber observes one.
    let orch_emit = orch.clone();
    let emitter = tokio::spawn(async move {
        loop {
            orch_emit.event_bus().emit(AgentEventKind::TokenStreamed {
                agent_id: vox_orchestrator::AgentId(7),
                text: "hello".to_string(),
            });
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });

    let first = tokio::time::timeout(vox_config::timeouts::D_15S, rx.recv())
        .await
        .expect("agent event within budget")
        .expect("event channel produced a frame");
    assert_eq!(
        first["kind"]["type"], "token_streamed",
        "expected a token_streamed agent event, got: {first}"
    );
    assert_eq!(first["kind"]["text"], "hello");

    emitter.abort();
    drop(rx);
    sub.abort();
    server.abort();
}

#[tokio::test]
async fn orchestrator_daemon_task_and_agent_write_methods() {
    tokio::time::timeout(DAEMON_TEST_TIMEOUT, async {
        orchestrator_daemon_task_and_agent_write_methods_inner().await;
    })
    .await
    .expect("orchestrator_daemon_task_and_agent_write_methods exceeded wall-clock budget");
}

async fn orchestrator_daemon_task_and_agent_write_methods_inner() {
    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    let aid = orch.spawn_agent("writer").expect("spawn");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let server = tokio::spawn(orch_daemon::serve_listener(
        listener,
        addr.to_string(),
        "ut-repo".to_string(),
        orch.clone(),
    ));
    let addr_str = addr.to_string();
    wait_until_async(
        "orchestrator daemon TCP accepting (`orch.ping`)",
        vox_config::timeouts::D_15S,
        vox_config::timeouts::D_5MS,
        || {
            let c = orch_daemon::OrchDaemonClient::new(addr_str.clone());
            async move { c.ping().await.is_ok() }
        },
    )
    .await;
    let client = orch_daemon::OrchDaemonClient::new(addr_str);

    let submitted = client
        .submit_task(serde_json::json!({
            "description": "rpc submit task",
            "file_manifest": [],
            "priority": "Normal",
            "target_agent": "writer",
        }))
        .await
        .expect("submit_task");
    let task_id = submitted["task_id"].as_u64().expect("task_id");
    let _ = client
        .reorder_task(task_id, "urgent")
        .await
        .expect("reorder_task");
    let _ = client.cancel_task(task_id).await.expect("cancel_task");

    let submitted2 = client
        .submit_task(serde_json::json!({
            "description": "rpc submit fail task",
            "file_manifest": [],
            "priority": "Normal",
            "target_agent": "writer",
        }))
        .await
        .expect("submit_task_2");
    let task_id2 = submitted2["task_id"].as_u64().expect("task_id2");
    // `fail_task` / `complete_task` apply to the agent's in-progress task; dequeue first
    // so the RPC exercises real queue semantics (queued-only tasks are not mark_failed).
    {
        let ql = orch.agent_queue(aid).expect("queue for fail path");
        let mut q = ql.write().unwrap();
        let t = q.dequeue().expect("dequeue task 2");
        assert_eq!(t.id.0, task_id2);
    }
    let _ = client
        .fail_task(task_id2, "expected fail".to_string())
        .await
        .expect("fail_task");

    let submitted3 = client
        .submit_task(serde_json::json!({
            "description": "rpc submit complete task",
            "file_manifest": [],
            "priority": "Normal",
            "target_agent": "writer",
        }))
        .await
        .expect("submit_task_3");
    let task_id3 = submitted3["task_id"].as_u64().expect("task_id3");
    {
        let ql = orch.agent_queue(aid).expect("queue for complete path");
        let mut q = ql.write().unwrap();
        let t = q.dequeue().expect("dequeue task 3");
        assert_eq!(t.id.0, task_id3);
    }
    let _ = client
        .complete_task(task_id3, None)
        .await
        .expect("complete_task");

    let drained = client.drain_agent(aid.0).await.expect("drain_agent");
    assert!(drained["drained_count"].as_u64().is_some());
    let rebalance = client.rebalance().await.expect("rebalance");
    assert!(rebalance["rebalanced"].as_u64().is_some());

    let dyn_spawned = client
        .spawn_agent_ext(serde_json::json!({
            "name": "dyn-rpc",
            "dynamic": true,
            "parent_agent_id": aid.0,
            "delegation_reason": "unit-test",
            "source_task_id": task_id3,
        }))
        .await
        .expect("spawn_agent_ext");
    let dyn_id = dyn_spawned["agent_id"].as_u64().expect("dyn_id");
    let _ = client.pause_agent(dyn_id).await.expect("pause_agent");
    let _ = client.resume_agent(dyn_id).await.expect("resume_agent");
    let retired = client.retire_agent(dyn_id).await.expect("retire_agent");
    assert!(retired["remaining_tasks"].as_u64().is_some());

    server.abort();
}
