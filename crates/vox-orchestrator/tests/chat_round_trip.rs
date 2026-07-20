//! No-cost chat round-trip: SUBMIT_TASK over the in-process daemon must
//! produce `task_started` AND `task_completed` event frames without any
//! resubmit, even when routing spawns the agent at submit time (its actor
//! handle does not exist until the fleet's next sync tick, so the submit-time
//! `ProcessQueue` notify is dropped). The `AgentFleet` supervisor tick is
//! responsible for nudging agents with queued work — this test reproduces the
//! "first chat message queues forever" stall red-before-fix.
//!
//! Zero LLM / network cost: [`StubTaskProcessor`] completes immediately.

#![cfg(feature = "runtime")]

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use vox_actor_runtime::scheduler::Scheduler;
use vox_orchestrator::runtime::{AgentFleet, StubTaskProcessor};
use vox_orchestrator::{Orchestrator, OrchestratorConfig, orch_daemon};

/// Wall-clock ceiling so the test cannot stall CI indefinitely.
const TEST_TIMEOUT: Duration = vox_config::timeouts::D_60S;
/// Generous deadline for the round trip (the fleet tick is 1s).
const ROUND_TRIP_DEADLINE: Duration = vox_config::timeouts::D_15S;

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

#[tokio::test]
async fn chat_submit_round_trip_completes_without_resubmit() {
    tokio::time::timeout(TEST_TIMEOUT, async {
        chat_submit_round_trip_inner().await;
    })
    .await
    .expect("chat_submit_round_trip_completes_without_resubmit exceeded wall-clock budget");
}

async fn chat_submit_round_trip_inner() {
    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));

    // Fleet supervisor loop, exactly as the daemon runs it — but with the
    // zero-cost stub processor instead of the LLM-backed one.
    let scheduler = Arc::new(Scheduler::new());
    let fleet_orch = orch.clone();
    let fleet = tokio::spawn(async move {
        let fleet = AgentFleet::new(scheduler, fleet_orch, Arc::new(StubTaskProcessor));
        fleet.run().await;
    });

    // In-process daemon on an ephemeral port.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let server = tokio::spawn(orch_daemon::serve_listener_with_extra(
        listener,
        addr.to_string(),
        "ut-repo".to_string(),
        orch.clone(),
        None,
        None,
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

    // Subscribe to the live agent-event stream BEFORE submitting (broadcast
    // bus has no replay).
    let client = orch_daemon::OrchDaemonClient::new(addr_str.clone());
    let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    let sub_client = client.clone();
    let sub = tokio::spawn(async move {
        let _ = sub_client.subscribe_events(tx).await;
    });

    // Chat-shaped submit, exactly what the GUI sends: a session_id, an
    // explicit `"priority": null` (regression cover for the null-priority
    // fix), and NO target_agent — routing must spawn a dynamic agent at
    // submit time, before its actor handle exists.
    let submitted = client
        .submit_task(serde_json::json!({
            "description": "chat round trip probe (stub, zero cost)",
            "file_manifest": [],
            "priority": null,
            "session_id": "chat-session-round-trip",
        }))
        .await
        .expect("submit_task");
    let task_id = submitted["task_id"].as_u64().expect("task_id");

    // The full round trip must arrive without any second submit: first
    // task_started, then task_completed, both for OUR task.
    let mut started = false;
    let mut completed = false;
    let deadline = tokio::time::Instant::now() + ROUND_TRIP_DEADLINE;
    while !(started && completed) {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!(
                "round trip stalled: task {task_id} started={started} completed={completed} \
                 within {ROUND_TRIP_DEADLINE:?} — queued task was never drained (no \
                 ProcessQueue nudge reached the agent actor)"
            );
        }
        let frame = tokio::time::timeout(remaining, rx.recv())
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "round trip stalled: task {task_id} started={started} completed={completed} \
                     within {ROUND_TRIP_DEADLINE:?} — queued task was never drained (no \
                     ProcessQueue nudge reached the agent actor)"
                )
            })
            .expect("event stream closed unexpectedly");
        let kind = frame["kind"]["type"].as_str().unwrap_or("");
        let frame_task = frame["kind"]["task_id"].as_u64();
        if frame_task == Some(task_id) {
            match kind {
                "task_started" => started = true,
                "task_completed" => completed = true,
                _ => {}
            }
        }
    }
    assert!(started && completed);

    fleet.abort();
    drop(rx);
    sub.abort();
    server.abort();
}
