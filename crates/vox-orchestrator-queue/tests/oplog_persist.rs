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
