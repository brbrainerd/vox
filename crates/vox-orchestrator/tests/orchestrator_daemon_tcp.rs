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
const D_20MS: Duration = Duration::from_millis(20);

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
                session_id: None,
            });
            tokio::time::sleep(D_20MS).await;
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

/// T0.2: a daemon configured with an auth token must reject a client that
/// presents no token (or the wrong token) before dispatching *any* method —
/// proving the auth gate actually distinguishes authenticated from
/// unauthenticated callers rather than being a no-op.
#[tokio::test]
async fn orchestrator_daemon_rejects_missing_or_wrong_token() {
    tokio::time::timeout(DAEMON_TEST_TIMEOUT, async {
        orchestrator_daemon_rejects_missing_or_wrong_token_inner().await;
    })
    .await
    .expect("orchestrator_daemon_rejects_missing_or_wrong_token exceeded wall-clock budget");
}

async fn orchestrator_daemon_rejects_missing_or_wrong_token_inner() {
    use std::sync::Arc as StdArc;
    use vox_foundation::protocol::{DispatchPayload, DispatchRequest, DispatchResponse};

    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let expected_token: StdArc<str> = StdArc::from("t0.2-secret-token");
    let server = tokio::spawn(orch_daemon::serve_listener_with_extra(
        listener,
        addr.to_string(),
        "ut-repo".to_string(),
        orch,
        None,
        Some(expected_token.clone()),
    ));
    let addr_str = addr.to_string();

    // Wait for the daemon to accept connections at all (using the *correct*
    // token so this readiness probe doesn't itself get auth-rejected forever).
    wait_until_async(
        "orchestrator daemon TCP accepting (authenticated `orch.ping`)",
        vox_config::timeouts::D_15S,
        vox_config::timeouts::D_5MS,
        || {
            let c = orch_daemon::OrchDaemonClient::with_token(
                addr_str.clone(),
                expected_token.to_string(),
            );
            async move { c.ping().await.is_ok() }
        },
    )
    .await;

    // A raw connection presenting NO token must be rejected with an error
    // response, not a normal dispatch result — regardless of method, including
    // orch.tool_call-shaped and plain orch.ping requests.
    async fn send_raw(addr: &str, req: &DispatchRequest) -> DispatchResponse {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
        let (read_half, mut write_half) = stream.split();
        let mut line = serde_json::to_string(req).unwrap();
        line.push('\n');
        write_half.write_all(line.as_bytes()).await.unwrap();
        write_half.flush().await.unwrap();
        let mut reader = BufReader::new(read_half);
        let mut resp_line = String::new();
        reader.read_line(&mut resp_line).await.unwrap();
        serde_json::from_str(resp_line.trim()).unwrap()
    }

    let no_token_req = DispatchRequest {
        id: "no-token".to_string(),
        method: vox_foundation::protocol::orch_daemon_method::PING.to_string(),
        params: serde_json::json!({}),
        auth_token: None,
        permission_mode: None,
    };
    let resp = send_raw(&addr_str, &no_token_req).await;
    match resp.payload {
        DispatchPayload::Error { message, .. } => {
            assert!(
                message.to_lowercase().contains("unauthorized")
                    || message.to_lowercase().contains("token"),
                "expected an auth-shaped error, got: {message}"
            );
        }
        other => panic!("expected Error payload for missing token, got: {other:?}"),
    }

    let wrong_token_req = DispatchRequest {
        id: "wrong-token".to_string(),
        method: vox_foundation::protocol::orch_daemon_method::PING.to_string(),
        params: serde_json::json!({}),
        auth_token: Some("not-the-real-token".to_string()),
        permission_mode: None,
    };
    let resp = send_raw(&addr_str, &wrong_token_req).await;
    assert!(
        matches!(resp.payload, DispatchPayload::Error { .. }),
        "expected Error payload for wrong token, got: {:?}",
        resp.payload
    );

    // A raw connection presenting NO token against a SUBSCRIBE-family method
    // must also be rejected — the auth gate must cover the streaming
    // subscribe paths, not just the "normal" dispatch branch.
    let subscribe_no_token = DispatchRequest {
        id: "sub-no-token".to_string(),
        method: vox_foundation::protocol::orch_daemon_method::SUBSCRIBE.to_string(),
        params: serde_json::json!({}),
        auth_token: None,
        permission_mode: None,
    };
    let resp = send_raw(&addr_str, &subscribe_no_token).await;
    assert!(
        matches!(resp.payload, DispatchPayload::Error { .. }),
        "expected SUBSCRIBE without a token to be rejected before streaming, got: {:?}",
        resp.payload
    );

    server.abort();
}

/// T0.2: the flip side — a client presenting the CORRECT token gets normal
/// dispatch behavior against an auth-configured daemon.
#[tokio::test]
async fn orchestrator_daemon_accepts_correct_token() {
    tokio::time::timeout(DAEMON_TEST_TIMEOUT, async {
        orchestrator_daemon_accepts_correct_token_inner().await;
    })
    .await
    .expect("orchestrator_daemon_accepts_correct_token exceeded wall-clock budget");
}

async fn orchestrator_daemon_accepts_correct_token_inner() {
    use std::sync::Arc as StdArc;

    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    orch.spawn_agent("auth-ok").expect("spawn");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let expected_token: StdArc<str> = StdArc::from("t0.2-secret-token-2");
    let server = tokio::spawn(orch_daemon::serve_listener_with_extra(
        listener,
        addr.to_string(),
        "ut-repo".to_string(),
        orch,
        None,
        Some(expected_token.clone()),
    ));
    let addr_str = addr.to_string();

    let client = orch_daemon::OrchDaemonClient::with_token(addr_str, expected_token.to_string());
    wait_until_async(
        "orchestrator daemon TCP accepting (authenticated `orch.ping`)",
        vox_config::timeouts::D_15S,
        vox_config::timeouts::D_5MS,
        || {
            let c = client.clone();
            async move { c.ping().await.is_ok() }
        },
    )
    .await;

    let ping = client
        .ping()
        .await
        .expect("authenticated ping must succeed");
    assert_eq!(ping["repository_id"], "ut-repo");

    let status = client
        .orchestrator_status()
        .await
        .expect("authenticated orchestrator_status must succeed");
    assert!(status.get("agent_count").is_some());

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

/// T1.3 RED: a client subscribing to `orch.subscribe_events` with
/// `from_offset = N` must receive every durable Tier-A op with `op_id > N`
/// (replay-envelope frames, each carrying `replay: true` and `op_id`) BEFORE
/// any live event, then transition to live tailing exactly like a
/// `subscribe_events` call without an offset.
#[tokio::test]
async fn orchestrator_daemon_subscribe_events_replays_from_offset() {
    tokio::time::timeout(DAEMON_TEST_TIMEOUT, async {
        orchestrator_daemon_subscribe_events_replays_from_offset_inner().await;
    })
    .await
    .expect("orchestrator_daemon_subscribe_events_replays_from_offset exceeded wall-clock budget");
}

async fn orchestrator_daemon_subscribe_events_replays_from_offset_inner() {
    use vox_db::{DbConfig, VoxDb};
    use vox_orchestrator::events::AgentEventKind;
    use vox_orchestrator::oplog::OperationKind;

    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    orch.init_db(db.clone()).await.expect("init_db");
    let aid = orch.spawn_agent("replay-agent").expect("spawn");

    // Record three durable Tier-A ops directly (mirrors how MCP/daemon call
    // sites durably record before broadcasting — see vcs_ops.rs's
    // record_operation). The first op's id becomes `from_offset`; the client
    // should NOT see it replayed (offset is exclusive), only the two after it.
    let op1 = orch
        .record_operation(
            aid,
            OperationKind::TaskDoubted {
                task_id: 1,
                reason: Some("pre-offset".into()),
            },
            "pre-offset op (must not be replayed)",
            None,
            None,
            None,
            None,
        )
        .await;
    let op2 = orch
        .record_operation(
            aid,
            OperationKind::TaskDoubted {
                task_id: 2,
                reason: Some("post-offset-1".into()),
            },
            "post-offset op 1",
            None,
            None,
            None,
            None,
        )
        .await;
    let op3 = orch
        .record_operation(
            aid,
            OperationKind::TaskDoubted {
                task_id: 3,
                reason: Some("post-offset-2".into()),
            },
            "post-offset op 2",
            None,
            None,
            None,
            None,
        )
        .await;
    assert!(op2.0 > op1.0 && op3.0 > op2.0, "op ids must be increasing");

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
    let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    let client_for_sub = client.clone();
    let sub = tokio::spawn(async move {
        let _ = client_for_sub.subscribe_events_from_offset(op1.0, tx).await;
    });

    // First two frames must be the replay of op2 and op3, oldest-first, each
    // tagged `replay: true`, and NOT op1 (which is at/before the offset).
    let first = tokio::time::timeout(vox_config::timeouts::D_15S, rx.recv())
        .await
        .expect("first replay frame within budget")
        .expect("channel produced a frame");
    assert_eq!(first["replay"], true, "first frame must be a replay frame");
    assert_eq!(first["op_id"].as_u64(), Some(op2.0));

    let second = tokio::time::timeout(vox_config::timeouts::D_15S, rx.recv())
        .await
        .expect("second replay frame within budget")
        .expect("channel produced a frame");
    assert_eq!(
        second["replay"], true,
        "second frame must be a replay frame"
    );
    assert_eq!(second["op_id"].as_u64(), Some(op3.0));

    // Now emit a live Tier-B event; it must arrive AFTER the replay frames,
    // proving the transition from replay to live-tail.
    let orch_emit = orch.clone();
    let emitter = tokio::spawn(async move {
        loop {
            orch_emit.event_bus().emit(AgentEventKind::TokenStreamed {
                agent_id: aid,
                text: "post-replay-live".to_string(),
                session_id: None,
            });
            tokio::time::sleep(D_20MS).await;
        }
    });

    let third = tokio::time::timeout(vox_config::timeouts::D_15S, rx.recv())
        .await
        .expect("live frame within budget")
        .expect("channel produced a frame");
    assert!(
        third.get("replay").is_none(),
        "third frame must be a LIVE frame (no replay tag), got: {third}"
    );
    assert_eq!(third["kind"]["type"], "token_streamed");

    emitter.abort();
    drop(rx);
    sub.abort();
    server.abort();
}

/// T1.3 regression: a client subscribing to `orch.subscribe_events` WITHOUT
/// `from_offset` must behave exactly as before — live-tail only, no replay,
/// even when durable Tier-A history exists in the DB.
#[tokio::test]
async fn orchestrator_daemon_subscribe_events_without_offset_is_live_tail_only() {
    tokio::time::timeout(DAEMON_TEST_TIMEOUT, async {
        orchestrator_daemon_subscribe_events_without_offset_inner().await;
    })
    .await
    .expect(
        "orchestrator_daemon_subscribe_events_without_offset_is_live_tail_only exceeded wall-clock budget",
    );
}

async fn orchestrator_daemon_subscribe_events_without_offset_inner() {
    use vox_db::{DbConfig, VoxDb};
    use vox_orchestrator::events::AgentEventKind;
    use vox_orchestrator::oplog::OperationKind;

    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    orch.init_db(db.clone()).await.expect("init_db");
    let aid = orch.spawn_agent("no-offset-agent").expect("spawn");

    // Durable history exists before the subscriber connects...
    let _ = orch
        .record_operation(
            aid,
            OperationKind::TaskDoubted {
                task_id: 99,
                reason: Some("should never be replayed".into()),
            },
            "pre-existing durable op",
            None,
            None,
            None,
            None,
        )
        .await;

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

    // ...but a plain `subscribe_events` (no offset) must ONLY ever see the
    // live event emitted below, never the pre-existing durable op.
    let client = orch_daemon::OrchDaemonClient::new(addr_str);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    let client_for_sub = client.clone();
    let sub = tokio::spawn(async move {
        let _ = client_for_sub.subscribe_events(tx).await;
    });

    let orch_emit = orch.clone();
    let emitter = tokio::spawn(async move {
        loop {
            orch_emit.event_bus().emit(AgentEventKind::TokenStreamed {
                agent_id: aid,
                text: "live-only".to_string(),
                session_id: None,
            });
            tokio::time::sleep(D_20MS).await;
        }
    });

    let first = tokio::time::timeout(vox_config::timeouts::D_15S, rx.recv())
        .await
        .expect("live frame within budget")
        .expect("channel produced a frame");
    assert!(
        first.get("replay").is_none(),
        "first (and only expected) frame must be LIVE, not a replay frame, got: {first}"
    );
    assert_eq!(first["kind"]["type"], "token_streamed");

    emitter.abort();
    drop(rx);
    sub.abort();
    server.abort();
}
