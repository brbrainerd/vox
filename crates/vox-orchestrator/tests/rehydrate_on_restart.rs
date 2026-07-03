//! T1.4 RED tests: on daemon restart (`init_db` re-run against the same
//! durable DB), the journal wins — derived in-memory state is rebuilt/patched
//! from it rather than lost.
//!
//! These tests simulate "restart" the same way
//! `crates/vox-orchestrator/tests/task_lifecycle_durable_oplog.rs` proves
//! durability: they never construct a second `Orchestrator` process, but they
//! DO exercise the exact code path a real restart runs (`init_db` against a
//! DB that already has durable oplog rows from a prior in-process lifecycle),
//! which is what actually rehydrates state — a second `Orchestrator::new()`
//! sharing the same `Arc<VoxDb>` and re-running `init_db` is behaviorally
//! equivalent to a fresh process attaching to the same on-disk DB.

use vox_db::{DbConfig, VoxDb};
use vox_orchestrator::{FileAffinity, Orchestrator, OrchestratorConfig};

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig::for_testing()
}

/// RED test 1: a task submitted via the real `submit_task` path (not the
/// hopper) that never reaches TaskComplete/TaskFail is re-enqueued after a
/// simulated restart (`init_db` re-run against the same durable DB on a
/// fresh `Orchestrator`).
#[tokio::test]
async fn direct_submit_task_in_flight_is_rehydrated_after_restart() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Orchestrator::new(test_config());
    orch.init_db(db.clone()).await.expect("init_db");

    let task_id = orch
        .submit_task(
            "rehydrate-me direct submit",
            vec![FileAffinity::write("src/rehydrate_target.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");

    // Never completed or failed — simulate a crash right here.

    // "Restart": a brand-new Orchestrator process attaching to the same
    // durable DB and re-running init_db, exactly like vox-orchestrator-d's
    // boot path (`orch.init_db(db)` in vox_orchestrator_d.rs). A real daemon
    // boot spawns its agent pool before init_db's rehydration loops run (the
    // pre-existing hopper-inbox loop has this same ordering requirement) —
    // mirror that here rather than relying on default agent auto-bootstrap.
    let restarted = Orchestrator::new(test_config());
    restarted
        .spawn_agent("rehydrate-restart-agent")
        .expect("spawn agent before init_db, matching real daemon boot order");
    restarted.init_db(db.clone()).await.expect("init_db");

    let agent_ids = restarted.agent_ids();
    assert!(
        !agent_ids.is_empty(),
        "restarted orchestrator must have at least one agent to rehydrate onto"
    );

    let mut found = false;
    for agent_id in agent_ids {
        if let Some(queue) = restarted.agent_queue(agent_id) {
            let q = queue.read().unwrap();
            if q.tasks().iter().any(|t| t.id == task_id) {
                found = true;
                break;
            }
        }
    }
    assert!(
        found,
        "task {task_id} was submitted but never completed/failed before the \
         simulated crash; it must be re-enqueued (by id) on restart, not lost"
    );
}

/// RED test 2: a task that reached TaskComplete before the crash must NOT be
/// re-enqueued on restart (it already reached a terminal state; re-running
/// it would be exactly the "completed item re-executes on restart" bug this
/// whole plan exists to close).
#[tokio::test]
async fn direct_submit_task_completed_before_crash_is_not_rehydrated() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Orchestrator::new(test_config());
    orch.init_db(db.clone()).await.expect("init_db");

    let task_id = orch
        .submit_task(
            "completed before crash",
            vec![FileAffinity::write("src/completed_before_crash.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");

    let agent_id = *orch.agent_ids().first().expect("should have an agent");
    orch.agent_queue(agent_id)
        .expect("agent queue")
        .write()
        .unwrap()
        .dequeue();
    orch.complete_task_with_attestation(
        task_id,
        Some(vox_orchestrator::CompletionAttestation {
            checks_passed: vec!["human_review_approved".to_string()],
            ..Default::default()
        }),
    )
    .await
    .expect("complete should succeed");

    // "Restart".
    let restarted = Orchestrator::new(test_config());
    restarted.init_db(db.clone()).await.expect("init_db");

    for agent_id in restarted.agent_ids() {
        if let Some(queue) = restarted.agent_queue(agent_id) {
            let q = queue.read().unwrap();
            assert!(
                !q.tasks().iter().any(|t| t.id == task_id),
                "task {task_id} reached TaskComplete before the crash; it must \
                 NOT be re-enqueued on restart"
            );
        }
    }
}

/// RED test 3: a hopper-admitted item that was already completed (real
/// `HopperIntake::complete` called, per T1.1 production wiring) is NOT
/// re-executed after a simulated restart. Verifies the finding that T1.1's
/// real assign/complete wiring already closes this bug as a side effect:
/// `SqliteHopper::inbox()` only returns `ItemState::Inbox` rows, so a
/// completed item never reappears in the hopper-inbox rehydration loop.
#[tokio::test]
async fn completed_hopper_item_is_not_reexecuted_after_restart() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Orchestrator::new(test_config());
    orch.init_db(db.clone()).await.expect("init_db");
    if orch.agent_ids().is_empty() {
        let _ = orch
            .submit_task(
                "bootstrap agent",
                vec![FileAffinity::write("src/bootstrap_restart.rs")],
                None,
                None,
                None,
            )
            .await;
    }
    assert!(!orch.agent_ids().is_empty());

    let hopper = orch.hopper();
    let item = hopper
        .submit(
            "hopper item completed before crash".into(),
            vec![],
            vox_orchestrator::hopper::PriorityHint::Normal,
            vox_orchestrator::hopper::IntakeSource::Developer,
            None,
        )
        .await;

    // Wait for the production dispatcher to assign it.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let assigned = hopper.assigned().await;
        if assigned.iter().any(|i| i.item_id == item.item_id) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "dispatcher never assigned the hopper-admitted item"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let task_id = vox_orchestrator::TaskId(vox_orchestrator::orchestrator::dispatch::stable_hash(
        &item.item_id.0,
    ));
    let agent_id = *orch.agent_ids().first().expect("has an agent");
    orch.agent_queue(agent_id)
        .expect("agent queue")
        .write()
        .unwrap()
        .dequeue();
    orch.complete_task_with_attestation(
        task_id,
        Some(vox_orchestrator::CompletionAttestation {
            checks_passed: vec!["human_review_approved".to_string()],
            ..Default::default()
        }),
    )
    .await
    .expect("complete should succeed");

    let history = hopper.history().await;
    assert!(
        history.iter().any(|i| i.item_id == item.item_id
            && matches!(i.state, vox_orchestrator::hopper::ItemState::Done)),
        "hopper item must be Done before the simulated restart"
    );

    // "Restart".
    let restarted = Orchestrator::new(test_config());
    restarted.init_db(db.clone()).await.expect("init_db");

    // Completed item must not reappear in the inbox, and must not be
    // re-enqueued as a task on any agent queue.
    let restarted_hopper = restarted.hopper();
    let inbox_after_restart = restarted_hopper.inbox().await;
    assert!(
        !inbox_after_restart
            .iter()
            .any(|i| i.item_id == item.item_id),
        "completed hopper item must not reappear in the inbox after restart"
    );

    for agent_id in restarted.agent_ids() {
        if let Some(queue) = restarted.agent_queue(agent_id) {
            let q = queue.read().unwrap();
            assert!(
                !q.tasks().iter().any(|t| t.id == task_id),
                "completed hopper item's backing task must not be re-enqueued after restart"
            );
        }
    }
}
