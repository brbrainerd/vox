//! T1.6 RED tests: `orchestrator::core::checkpoint::compact_now` produces a
//! real, durable `Checkpoint` marker in `agent_oplog`, prunes rows it covers,
//! and a restart bounds rehydration to the post-checkpoint tail instead of
//! re-scanning full history.

use vox_db::{DbConfig, VoxDb};
use vox_orchestrator::orchestrator::core::checkpoint::compact_now;
use vox_orchestrator::{FileAffinity, Orchestrator, OrchestratorConfig};

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig::for_testing()
}

/// RED test 1: seed enough completed tasks to build real history, checkpoint
/// via `compact_now` with a deterministic `up_to`, then verify: (a) a durable
/// Checkpoint entry exists with the correct op_id_lo/op_id_hi, (b) rows below
/// op_id_lo are pruned from `agent_oplog`, (c) the checkpoint blob round-trips
/// the open-task state.
#[tokio::test]
async fn compact_now_checkpoints_and_prunes_agent_oplog() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Orchestrator::new(test_config());
    orch.init_db(db.clone()).await.expect("init_db");
    let repo = vox_orchestrator::lineage::repository_id();

    // Submit and complete a batch of tasks (durable TaskSubmit + TaskComplete
    // pairs land in agent_oplog).
    let mut last_task_id = None;
    for i in 0..8 {
        let task_id = orch
            .submit_task(
                format!("compaction seed task {i}"),
                vec![FileAffinity::write(format!("src/compact_{i}.rs"))],
                None,
                None,
                None,
            )
            .await
            .expect("submit should succeed");
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
        last_task_id = Some(task_id);
    }
    assert!(last_task_id.is_some());

    let up_to = db
        .max_agent_oplog_id(&repo)
        .await
        .expect("max_agent_oplog_id")
        .expect("should have durable rows by now");

    let rows_before = db
        .list_oplog_entries(None, &repo, 10_000)
        .await
        .expect("list_oplog_entries");
    assert!(
        rows_before.len() >= 16,
        "expected at least 16 rows (8 submit + 8 complete), got {}",
        rows_before.len()
    );

    compact_now(&orch, up_to)
        .await
        .expect("compact_now should succeed");

    // (a) durable Checkpoint entry exists with correct op_id_lo/op_id_hi
    let checkpoint = db
        .latest_checkpoint_blob(&repo)
        .await
        .expect("latest_checkpoint_blob")
        .expect("checkpoint should exist");
    let (_blob_id, op_id_lo, op_id_hi, _blake3_hex, payload) = checkpoint;
    assert_eq!(op_id_lo, 0, "first checkpoint starts from 0");
    assert_eq!(op_id_hi, up_to, "checkpoint op_id_hi must match up_to");
    assert!(!payload.is_empty());

    // (b) rows below op_id_lo (i.e. <= up_to) are pruned from agent_oplog
    // Every row covered by the checkpoint (operation_id <= up_to) must be
    // pruned; only the Checkpoint marker itself (operation_id == up_to + 1,
    // deliberately outside the pruned range so it survives to be found on
    // the next boot) may remain.
    let rows_after = db
        .list_oplog_entries(None, &repo, 10_000)
        .await
        .expect("list_oplog_entries");
    let non_marker_rows: Vec<_> = rows_after
        .iter()
        .filter(|r| {
            let is_checkpoint = r
                .get(2)
                .and_then(|v| v.as_deref())
                .map(|k| k.contains("\"Checkpoint\""))
                .unwrap_or(false);
            !is_checkpoint
        })
        .collect();
    assert!(
        non_marker_rows.is_empty(),
        "all rows covered by the checkpoint (<= up_to={up_to}) must be pruned, got {} non-marker rows remaining",
        non_marker_rows.len()
    );
    assert_eq!(
        rows_after.len(),
        1,
        "exactly the Checkpoint marker itself should remain in agent_oplog"
    );

    // (c) restore fidelity: all 8 tasks completed, so open-task state should
    // be empty — checkpoint payload should decode to that.
    let state: serde_json::Value =
        serde_json::from_slice(&payload).expect("checkpoint payload should be valid JSON");
    let submitted = state
        .get("submitted")
        .and_then(|v| v.as_object())
        .expect("submitted field");
    assert!(
        submitted.is_empty(),
        "all 8 tasks completed before checkpoint; open-task state must be empty"
    );
}

/// RED test 2 (restart bounding): seed history, checkpoint, seed MORE history
/// after the checkpoint (an in-flight task that never completes), simulate a
/// restart, and assert (a) the in-flight task submitted after the checkpoint
/// is still correctly rehydrated (proving the tail replay picks it up) and
/// (b) the pre-checkpoint completed tasks are NOT re-enqueued (proving the
/// checkpoint's restored state, not a full rescan, is what's authoritative —
/// a full rescan would also get this right, but only the bounded path does it
/// without re-reading pruned rows, which is exactly what (c) proves directly).
#[tokio::test]
async fn restart_rehydrates_from_checkpoint_plus_bounded_tail_only() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let db = std::sync::Arc::new(db);

    let orch = Orchestrator::new(test_config());
    orch.init_db(db.clone()).await.expect("init_db");
    let repo = vox_orchestrator::lineage::repository_id();

    // Pre-checkpoint: submit and complete some tasks.
    for i in 0..5 {
        let task_id = orch
            .submit_task(
                format!("pre-checkpoint task {i}"),
                vec![FileAffinity::write(format!("src/pre_{i}.rs"))],
                None,
                None,
                None,
            )
            .await
            .expect("submit should succeed");
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
    }

    let checkpoint_up_to = db
        .max_agent_oplog_id(&repo)
        .await
        .expect("max_agent_oplog_id")
        .expect("should have rows");
    compact_now(&orch, checkpoint_up_to)
        .await
        .expect("compact_now");

    // Confirm the pre-checkpoint rows are gone from the warm tier (only the
    // Checkpoint marker itself should remain).
    let rows_after_checkpoint = db
        .list_oplog_entries(None, &repo, 10_000)
        .await
        .expect("list_oplog_entries");
    assert_eq!(
        rows_after_checkpoint.len(),
        1,
        "pre-checkpoint rows must be pruned; only the Checkpoint marker should remain"
    );

    // Post-checkpoint: submit a task that never completes (in-flight at crash time).
    let in_flight_task_id = orch
        .submit_task(
            "post-checkpoint in-flight task",
            vec![FileAffinity::write("src/post_checkpoint.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");

    // "Restart": a brand-new Orchestrator attaching to the same durable DB.
    let restarted = Orchestrator::new(test_config());
    restarted
        .spawn_agent("restart-agent")
        .expect("spawn agent before init_db");
    restarted.init_db(db.clone()).await.expect("init_db");

    // (a) the post-checkpoint in-flight task is rehydrated (tail replay works).
    let mut found_in_flight = false;
    for agent_id in restarted.agent_ids() {
        if let Some(queue) = restarted.agent_queue(agent_id) {
            let q = queue.read().unwrap();
            if q.tasks().iter().any(|t| t.id == in_flight_task_id) {
                found_in_flight = true;
            }
        }
    }
    assert!(
        found_in_flight,
        "task {in_flight_task_id} was submitted after the checkpoint and never completed; \
         it must be rehydrated via the post-checkpoint tail scan"
    );

    // (b) none of the pre-checkpoint completed tasks reappear (they were
    // correctly folded out of the checkpoint's open-task state, and their
    // raw rows are gone from agent_oplog entirely — so even a buggy full
    // rescan couldn't resurrect them; this directly proves the pruning in
    // test 1 didn't lose live state).
    let total_tasks_on_restart: usize = restarted
        .agent_ids()
        .iter()
        .filter_map(|id| restarted.agent_queue(*id))
        .map(|q| q.read().unwrap().tasks().len())
        .sum();
    assert_eq!(
        total_tasks_on_restart, 1,
        "only the single post-checkpoint in-flight task should be rehydrated, got {total_tasks_on_restart}"
    );
}
