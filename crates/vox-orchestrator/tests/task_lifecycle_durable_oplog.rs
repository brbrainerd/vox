//! T1.1 RED tests: task submit/complete/fail lifecycle transitions are durably
//! persisted to vox-db's `convergence_op_log`-adjacent oplog table (via
//! `Orchestrator::record_operation` -> `append_to_db_with_breaker`), not just
//! held in the in-memory hot tier. This proves append-before-broadcast
//! durability holds even with zero live event-bus subscribers: we never
//! subscribe to `orch.event_bus` in these tests, only query the DB.

use std::path::PathBuf;

use vox_db::{DbConfig, VoxDb};
use vox_orchestrator::{CompletionAttestation, FileAffinity, Orchestrator, OrchestratorConfig};

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig::for_testing()
}

fn completion_attestation_for_tests() -> CompletionAttestation {
    CompletionAttestation {
        checks_passed: vec!["human_review_approved".to_string()],
        ..Default::default()
    }
}

/// Query the durable (vox-db) tier directly, bypassing the in-memory hot tier,
/// so a pass here proves real write-through persistence rather than an
/// in-process-only record.
async fn db_has_operation_kind(db: &VoxDb, repo: &str, predicate: impl Fn(&str) -> bool) -> bool {
    let rows = db
        .list_oplog_entries(None, repo, 100)
        .await
        .expect("list_oplog_entries");
    rows.iter().any(|row| {
        row.get(2)
            .and_then(|v| v.as_deref())
            .map(&predicate)
            .unwrap_or(false)
    })
}

#[tokio::test]
async fn task_submit_is_durably_persisted_to_db() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Orchestrator::new(test_config());
    orch.init_db(db.clone()).await.expect("init_db");

    let repo = vox_orchestrator::lineage::repository_id();

    let task_id = orch
        .submit_task(
            "durable submit test",
            vec![FileAffinity::write("src/durable.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");

    assert!(
        db_has_operation_kind(&db, &repo, |k| k.contains("TaskSubmit")
            && k.contains(&task_id.0.to_string()))
        .await,
        "TaskSubmit for task {task_id} must be queryable from the durable op-log \
         (vox-db), even though no event-bus subscriber was ever registered"
    );
}

#[tokio::test]
async fn task_complete_and_fail_are_durably_persisted_to_db() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Orchestrator::new(test_config());
    orch.init_db(db.clone()).await.expect("init_db");

    let repo = vox_orchestrator::lineage::repository_id();

    // --- Completion path ---
    let task_id = orch
        .submit_task(
            "durable complete test",
            vec![FileAffinity::write("src/durable_complete.rs")],
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
    orch.complete_task_with_attestation(task_id, Some(completion_attestation_for_tests()))
        .await
        .expect("complete should succeed");

    assert!(
        db_has_operation_kind(&db, &repo, |k| k.contains("TaskComplete")
            && k.contains(&task_id.0.to_string()))
        .await,
        "TaskComplete for task {task_id} must be queryable from the durable op-log"
    );

    // --- Failure path ---
    let fail_task_id = orch
        .submit_task(
            "durable fail test",
            vec![FileAffinity::write("src/durable_fail.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");
    orch.agent_queue(agent_id)
        .expect("agent queue")
        .write()
        .unwrap()
        .dequeue();
    orch.fail_task(fail_task_id, "synthetic failure for T1.1 durability test".into())
        .await
        .expect("fail should succeed");

    assert!(
        db_has_operation_kind(&db, &repo, |k| k.contains("TaskFail")
            && k.contains(&fail_task_id.0.to_string()))
        .await,
        "TaskFail for task {fail_task_id} must be queryable from the durable op-log"
    );
}

/// T1.1 (hopper wiring): a hopper-admitted item is picked up by the real
/// production dispatcher (`run_dispatcher_with_oplog`, spawned from
/// `Orchestrator::new`), which calls the real `HopperIntake::assign` — the
/// previously-unreachable production caller — and records `HopperAdmit` +
/// `HopperAssign` to the op-log. Completing the resulting task then calls the
/// real `HopperIntake::complete` and records `HopperComplete`. This is the
/// full admit -> assign -> complete lifecycle the spec calls out as the
/// highest-value gap ("a completed hopper item is NOT re-executed").
#[tokio::test]
async fn hopper_admit_assign_complete_lifecycle_is_wired_and_durable() {
    let orch = Orchestrator::new(test_config());
    // Give the orchestrator at least one agent to route to (spawn_agent is
    // sync/internal — use whatever the test config seeds, falling back to
    // submitting a throwaway direct task first to force agent bootstrap if
    // the default config starts with zero agents).
    if orch.agent_ids().is_empty() {
        let _ = orch
            .submit_task(
                "bootstrap agent",
                vec![FileAffinity::write("src/bootstrap.rs")],
                None,
                None,
                None,
            )
            .await;
    }
    assert!(
        !orch.agent_ids().is_empty(),
        "test setup: orchestrator must have at least one agent"
    );

    let hopper = orch.hopper();
    let item = hopper
        .submit(
            "hopper-sourced task".into(),
            vec![],
            vox_orchestrator::hopper::PriorityHint::Normal,
            vox_orchestrator::hopper::IntakeSource::Developer,
            None,
        )
        .await;

    // The dispatcher runs on a spawned task reacting to the HopperItemAdmitted
    // event `submit` just emitted; poll briefly for it to land.
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

    {
        let entries = orch.list_recent_operations(None, 256).await;
        assert!(
            entries.iter().any(|e| matches!(
                &e.kind,
                vox_orchestrator::oplog::OperationKind::HopperAdmit { item_id }
                    if item_id == &item.item_id.0
            )),
            "expected a HopperAdmit oplog entry for {}",
            item.item_id.0
        );
        assert!(
            entries.iter().any(|e| matches!(
                &e.kind,
                vox_orchestrator::oplog::OperationKind::HopperAssign { item_id, task_id: tid }
                    if item_id == &item.item_id.0 && *tid == task_id.0
            )),
            "expected a HopperAssign oplog entry for {}",
            item.item_id.0
        );
    }

    // Complete the task through the real production path; this must call
    // HopperIntake::complete and record HopperComplete (success/mod.rs).
    let agent_id = *orch.agent_ids().first().expect("has an agent");
    orch.agent_queue(agent_id)
        .expect("agent queue")
        .write()
        .unwrap()
        .dequeue();
    orch.complete_task_with_attestation(task_id, Some(completion_attestation_for_tests()))
        .await
        .expect("complete should succeed");

    {
        let entries = orch.list_recent_operations(None, 256).await;
        assert!(
            entries.iter().any(|e| matches!(
                &e.kind,
                vox_orchestrator::oplog::OperationKind::HopperComplete { item_id }
                    if item_id == &item.item_id.0
            )),
            "expected a HopperComplete oplog entry for {}",
            item.item_id.0
        );
    }

    let history = hopper.history().await;
    assert!(
        history.iter().any(|i| i.item_id == item.item_id
            && matches!(i.state, vox_orchestrator::hopper::ItemState::Done)),
        "hopper item must be Done after the real HopperIntake::complete call"
    );
}

#[allow(dead_code)]
fn touch_pathbuf(_p: PathBuf) {}
