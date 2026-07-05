//! P3-T1 acceptance: op-log persists to vox-db and survives reopen.

use vox_db::VoxDb;
use vox_orchestrator_queue::oplog::{OpLog, OperationKind};
use vox_orchestrator_types::AgentId;

#[tokio::test]
async fn record_persists_to_vox_db_and_survives_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("vox.sqlite");
    let db = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();
    let mut log = OpLog::with_db(db.clone(), 10_000);

    let id = log
        .record_persisted(
            AgentId(1),
            OperationKind::FileEdit {
                paths: vec!["a.rs".into()],
            },
            "edit a.rs",
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("record_persisted");

    // Drop the first log; state lives only in vox-db now.
    drop(log);

    // Reopen on the same db and warm-load.
    let mut log2 = OpLog::with_db(db.clone(), 10_000);
    log2.warm_load_recent(100).await.unwrap();

    assert_eq!(log2.lookup(id).map(|e| e.id), Some(id));
}

#[tokio::test]
async fn warm_load_respects_kind_json() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("vox2.sqlite");
    let db = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();
    let mut log = OpLog::with_db(db.clone(), 10_000);

    log.record_persisted(
        AgentId(2),
        OperationKind::TaskSubmit { task_id: 42 },
        "submit task 42",
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record_persisted");

    drop(log);

    let mut log2 = OpLog::with_db(db.clone(), 10_000);
    log2.warm_load_recent(100).await.unwrap();

    let entry = log2
        .history()
        .into_iter()
        .find(|e| matches!(e.kind, OperationKind::TaskSubmit { task_id: 42 }));
    assert!(entry.is_some(), "TaskSubmit entry should survive reopen");
}

/// T1.3 RED: `OperationId` generation must survive a simulated daemon
/// restart. Persist a few ops through one `OpLog`, drop it (as if the
/// process died), then construct a *fresh* `OpLog::with_db_seeded` against
/// the same DB and record a new op — its `OperationId` must be strictly
/// greater than every id assigned before the "restart", not reset to 1.
#[tokio::test]
async fn operation_id_survives_simulated_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("vox3.sqlite");
    let db = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    let mut log = OpLog::with_db(db.clone(), 10_000);
    let mut last_before_restart = None;
    for i in 0..5 {
        let id = log
            .record_persisted(
                AgentId(1),
                OperationKind::Custom {
                    label: format!("pre-restart-{i}"),
                },
                format!("op {i}"),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("record_persisted");
        last_before_restart = Some(id);
    }
    let last_before_restart = last_before_restart.expect("recorded at least one op");

    // Simulate a process restart: drop the in-memory OpLog entirely (state
    // only survives in vox-db), then construct a fresh one against the same
    // DB using the seeded constructor.
    drop(log);

    let mut log2 = OpLog::with_db_seeded(db.clone(), 10_000)
        .await
        .expect("with_db_seeded");
    let id_after_restart = log2
        .record_persisted(
            AgentId(1),
            OperationKind::Custom {
                label: "post-restart".into(),
            },
            "op after restart",
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("record_persisted after restart");

    assert!(
        id_after_restart.0 > last_before_restart.0,
        "post-restart OperationId {id_after_restart} must be strictly greater than \
         pre-restart OperationId {last_before_restart} (generator must not reset to 1)"
    );
}

/// A brand-new DB (no prior rows) must still start `OperationId` at 1 via
/// `with_db_seeded` — the seeding must be a no-op when there is no history.
#[tokio::test]
async fn operation_id_starts_at_one_on_fresh_db() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("vox4.sqlite");
    let db = VoxDb::open(db_path.to_str().unwrap()).await.unwrap();

    let mut log = OpLog::with_db_seeded(db, 10_000)
        .await
        .expect("with_db_seeded on fresh db");
    let id = log
        .record_persisted(
            AgentId(1),
            OperationKind::Custom {
                label: "first-ever".into(),
            },
            "first op on fresh db",
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("record_persisted");
    assert_eq!(id.0, 1, "fresh DB must still start OperationId at 1");
}
