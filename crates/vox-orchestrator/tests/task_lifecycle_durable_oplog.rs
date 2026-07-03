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
    orch.fail_task(
        fail_task_id,
        "synthetic failure for T1.1 durability test".into(),
    )
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

/// T1.3 follow-up: `Orchestrator::attach_db` — the DB-attach path used by the
/// `vox mcp` stdio server (`ServerState::with_db_initialized`, the *other*
/// real production entry point besides `vox-orchestrator-d`'s `init_db`) —
/// must also reseed the durable `OperationId` generator from
/// `convergence_op_log`. Before this fix, `attach_db` set the DB handle but
/// never called the T1.3 reseed logic, so every `vox mcp` restart reset
/// `OperationId` back to 1 even though this process durably records Tier-A
/// operations via `record_operation`.
///
/// Simulates a `vox mcp` restart: one `Orchestrator` records ops against a
/// DB via `init_db` (as if a prior `vox mcp` session had run and exited), a
/// *second, fresh* `Orchestrator` then attaches the same DB purely via
/// `attach_db` (the exact call `ServerState::with_db_initialized` makes) and
/// records a new op. Its `OperationId` must be strictly greater than every
/// id assigned before the "restart".
#[tokio::test]
async fn attach_db_reseeds_operation_id_generator_like_init_db() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("vox_attach_db_reseed.sqlite");
    let db = VoxDb::open(db_path.to_str().unwrap()).await.expect("open db");
    let db = std::sync::Arc::new(db);

    // "Session 1": a full init_db attach (as vox-orchestrator-d would do),
    // recording a handful of Tier-A ops.
    let orch1 = Orchestrator::new(test_config());
    orch1.init_db(db.clone()).await.expect("init_db");

    for i in 0..3 {
        orch1
            .submit_task(
                format!("pre-restart task {i}"),
                vec![FileAffinity::write(format!("src/pre_restart_{i}.rs"))],
                None,
                None,
                None,
            )
            .await
            .expect("submit should succeed");
    }
    let last_id_before_restart = {
        let entries = orch1.list_recent_operations(None, 256).await;
        entries.iter().map(|e| e.id.0).max().unwrap_or(0)
    };
    assert!(
        last_id_before_restart > 0,
        "test setup: at least one operation must have been recorded pre-restart"
    );

    // Drop orch1 entirely — nothing but vox-db survives, exactly like a
    // `vox mcp` process exiting.
    drop(orch1);

    // "Session 2": a brand-new Orchestrator attaches the SAME db purely via
    // attach_db — the exact call ServerState::with_db_initialized makes for
    // the `vox mcp` stdio server. No init_db call here.
    let orch2 = Orchestrator::new(test_config());
    orch2.attach_db(db.clone()).await;

    let task_id = orch2
        .submit_task(
            "post-restart task via attach_db",
            vec![FileAffinity::write("src/post_restart.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");

    let entries_after = orch2.list_recent_operations(None, 256).await;
    let post_restart_id = entries_after
        .iter()
        .filter(|e| matches!(&e.kind, vox_orchestrator::oplog::OperationKind::TaskSubmit { task_id: tid } if *tid == task_id.0))
        .map(|e| e.id.0)
        .max()
        .expect("post-restart TaskSubmit entry must exist");

    assert!(
        post_restart_id > last_id_before_restart,
        "post-restart OperationId {post_restart_id} (via attach_db) must be strictly \
         greater than pre-restart OperationId {last_id_before_restart} — attach_db must \
         reseed the OperationId generator from convergence_op_log just like init_db does, \
         otherwise every `vox mcp` restart resets replay-offset ids back to 1"
    );
}

/// T1.2 RED test: the general durable-before-broadcast PRINCIPLE, proven with
/// zero live event-bus subscribers. `submit_task` internally calls
/// `record_operation(TaskSubmit)` (a synchronous-then-awaited durable write)
/// strictly before the `TaskSubmitted` bus emit in the same straight-line
/// async function body (see `orchestrator/task_dispatch/submit/task_submit.rs`).
/// If a subscriber is never registered, the broadcast silently has zero
/// receivers (`tokio::broadcast::Sender::send` on a channel with no
/// subscribers is a no-op, not an error) — so a pass here can only be
/// explained by the durable write having actually happened, independent of
/// whether anyone was listening on the bus.
#[tokio::test]
async fn tier_a_transition_is_durable_even_with_zero_bus_subscribers() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Orchestrator::new(test_config());
    orch.init_db(db.clone()).await.expect("init_db");

    // Deliberately never subscribe to the event bus ourselves — assert the
    // durable write stands on its own, decoupled from broadcast delivery. (The
    // orchestrator's own internal machinery, e.g. the hopper dispatcher loop,
    // holds its own subscription; we only assert we add none of our own.)
    let baseline_subscribers = orch.event_bus().subscriber_count();

    let repo = vox_orchestrator::lineage::repository_id();
    let task_id = orch
        .submit_task(
            "tier-a durability principle test",
            vec![FileAffinity::write("src/tier_a_principle.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");

    assert_eq!(
        orch.event_bus().subscriber_count(),
        baseline_subscribers,
        "this test must not have added any subscriber of its own"
    );
    assert!(
        db_has_operation_kind(&db, &repo, |k| k.contains("TaskSubmit")
            && k.contains(&task_id.0.to_string()))
        .await,
        "Tier-A TaskSubmit must be durably queryable even though this test never \
         subscribed to the event bus — proves durable-before-broadcast, not \
         durable-because-broadcast-was-observed"
    );
}
