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
use vox_orchestrator::events::AgentEventKind;
use vox_orchestrator::routing_processor::RoutingTaskProcessor;
use vox_orchestrator::runtime::{AgentFleet, StubTaskProcessor, TaskProcessor};
use vox_orchestrator::types::{AgentId, AgentTask, TaskId, TaskPhase};
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

/// Test-only stand-in for [`vox_orchestrator::runtime::AiTaskProcessor`]'s
/// 6-phase Inspect/Localize/Hypothesize/Act/Verify/Decide loop, minus the
/// real LLM calls: emits one `TaskPhaseChanged` per phase, in order, then
/// succeeds. Proves the *event-count shape* of the agentic path without any
/// network cost.
struct SixPhaseStubProcessor {
    event_bus: vox_orchestrator::events::EventBus,
}

#[async_trait::async_trait]
impl TaskProcessor for SixPhaseStubProcessor {
    async fn process(
        &self,
        agent_id: AgentId,
        task: AgentTask,
        _cancel: Arc<std::sync::atomic::AtomicBool>,
    ) -> anyhow::Result<TaskId> {
        for phase in [
            TaskPhase::Inspect,
            TaskPhase::Localize,
            TaskPhase::Hypothesize,
            TaskPhase::Act,
            TaskPhase::Verify,
            TaskPhase::Decide,
        ] {
            self.event_bus.emit(AgentEventKind::TaskPhaseChanged {
                task_id: task.id,
                agent_id,
                phase,
            });
        }
        Ok(task.id)
    }
}

/// Test-only stand-in for a single-call fast-path chat processor, minus the
/// real LLM call: emits exactly one `TaskPhaseChanged` (`Decide`, its only
/// terminal phase) then succeeds. (Fix Task 4, gui-axis-chat-harness-fixes:
/// the dedicated `ChatTaskProcessor` this originally stood in for was
/// deleted; this stub still validates `RoutingTaskProcessor`'s dispatch
/// behavior, which is unaffected by that deletion.)
struct OnePhaseStubProcessor {
    event_bus: vox_orchestrator::events::EventBus,
}

#[async_trait::async_trait]
impl TaskProcessor for OnePhaseStubProcessor {
    async fn process(
        &self,
        agent_id: AgentId,
        task: AgentTask,
        _cancel: Arc<std::sync::atomic::AtomicBool>,
    ) -> anyhow::Result<TaskId> {
        self.event_bus.emit(AgentEventKind::TaskPhaseChanged {
            task_id: task.id,
            agent_id,
            phase: TaskPhase::Decide,
        });
        Ok(task.id)
    }
}

/// End-to-end proof (over the real in-process daemon + `RoutingTaskProcessor`,
/// zero LLM cost) that a `task_category: "chat"` submission takes the
/// single-call fast path — exactly one `TaskPhaseChanged` event — while a
/// default (agentic) submission still goes through all six phases of the
/// full pipeline. Mirrors `routing_processor.rs`'s own
/// `chat_category_routes_to_chat_processor_others_to_agentic` unit test
/// (Task A3), but drives it through the real daemon submit/event-subscribe
/// path this file already exercises, so it also proves the routing decision
/// is reachable end-to-end, not just correct in isolation.
#[tokio::test]
async fn chat_category_task_emits_exactly_one_phase_change_not_six() {
    tokio::time::timeout(TEST_TIMEOUT, async {
        chat_category_task_emits_exactly_one_phase_change_not_six_inner().await;
    })
    .await
    .expect("chat_category_task_emits_exactly_one_phase_change_not_six exceeded wall-clock budget");
}

async fn chat_category_task_emits_exactly_one_phase_change_not_six_inner() {
    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));

    let scheduler = Arc::new(Scheduler::new());
    let fleet_orch = orch.clone();
    let fleet_event_bus = orch.event_bus.clone();
    let fleet = tokio::spawn(async move {
        let agentic = Arc::new(SixPhaseStubProcessor {
            event_bus: fleet_event_bus.clone(),
        });
        let chat = Arc::new(OnePhaseStubProcessor {
            event_bus: fleet_event_bus,
        });
        let router = Arc::new(RoutingTaskProcessor::new(agentic, chat));
        let fleet = AgentFleet::new(scheduler, fleet_orch, router);
        fleet.run().await;
    });

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

    let client = orch_daemon::OrchDaemonClient::new(addr_str.clone());
    let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    let sub_client = client.clone();
    let sub = tokio::spawn(async move {
        let _ = sub_client.subscribe_events(tx).await;
    });

    // Chat-category submission: must take the one-phase-change fast path.
    let chat_submitted = client
        .submit_task(serde_json::json!({
            "description": "chat routing-proof probe (stub, zero cost)",
            "file_manifest": [],
            "priority": null,
            "session_id": "chat-routing-proof",
            "task_category": "Chat",
        }))
        .await
        .expect("submit_task (chat)");
    let chat_task_id = chat_submitted["task_id"].as_u64().expect("task_id");

    // Default-category (agentic) submission: must still take the full
    // six-phase path.
    let agentic_submitted = client
        .submit_task(serde_json::json!({
            "description": "agentic routing-proof probe (stub, zero cost)",
            "file_manifest": [],
            "priority": null,
            "session_id": "agentic-routing-proof",
        }))
        .await
        .expect("submit_task (agentic)");
    let agentic_task_id = agentic_submitted["task_id"].as_u64().expect("task_id");

    let mut chat_phase_changes: u32 = 0;
    let mut agentic_phase_changes: u32 = 0;
    let mut chat_completed = false;
    let mut agentic_completed = false;
    let deadline = tokio::time::Instant::now() + ROUND_TRIP_DEADLINE;
    while !(chat_completed && agentic_completed) {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!(
                "routing-proof round trip stalled: chat_completed={chat_completed} \
                 (phase_changes={chat_phase_changes}) agentic_completed={agentic_completed} \
                 (phase_changes={agentic_phase_changes}) within {ROUND_TRIP_DEADLINE:?}"
            );
        }
        let frame = tokio::time::timeout(remaining, rx.recv())
            .await
            .expect("routing-proof round trip stalled waiting for next event")
            .expect("event stream closed unexpectedly");
        let kind = frame["kind"]["type"].as_str().unwrap_or("");
        let frame_task = frame["kind"]["task_id"].as_u64();
        if frame_task == Some(chat_task_id) {
            match kind {
                "task_phase_changed" => chat_phase_changes += 1,
                "task_completed" => chat_completed = true,
                _ => {}
            }
        } else if frame_task == Some(agentic_task_id) {
            match kind {
                "task_phase_changed" => agentic_phase_changes += 1,
                "task_completed" => agentic_completed = true,
                _ => {}
            }
        }
    }

    assert_eq!(
        chat_phase_changes, 1,
        "chat-category task must take the single-call fast path (exactly one \
         TaskPhaseChanged), got {chat_phase_changes}"
    );
    assert_eq!(
        agentic_phase_changes, 6,
        "default-category task must still go through the full 6-phase \
         pipeline, got {agentic_phase_changes}"
    );

    fleet.abort();
    drop(rx);
    sub.abort();
    server.abort();
}
