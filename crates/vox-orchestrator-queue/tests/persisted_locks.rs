//! P0-T1c: file-lock map round-trips through vox-db (write-through + hydration).

use std::path::Path;
use vox_db::{DbConfig, VoxDb};
use vox_orchestrator_queue::locks::{FileLockManager, LockKind};
use vox_orchestrator_types::AgentId;

#[tokio::test]
async fn acquire_then_replay_from_db() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let mgr = FileLockManager::with_db(db.clone(), "node-A", "repo-1");

    mgr.try_acquire_persisted(Path::new("src/main.rs"), AgentId(1), LockKind::Exclusive)
        .await
        .expect("acquire");

    // Rebuild a fresh manager from DB only.
    drop(mgr);
    let mgr2 = FileLockManager::with_db(db, "node-A", "repo-1");
    mgr2.hydrate_from_db().await.expect("hydrate");

    assert!(
        mgr2.is_locked(Path::new("src/main.rs")),
        "lock not replayed after hydration"
    );
    let (holder, kind) = mgr2
        .holder(Path::new("src/main.rs"))
        .expect("holder missing");
    assert_eq!(holder, AgentId(1));
    assert_eq!(kind, LockKind::Exclusive);
}

#[tokio::test]
async fn release_propagates_to_db() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let mgr = FileLockManager::with_db(db.clone(), "node-A", "repo-1");

    mgr.try_acquire_persisted(Path::new("src/lib.rs"), AgentId(1), LockKind::Exclusive)
        .await
        .unwrap();
    mgr.release_persisted(Path::new("src/lib.rs"), AgentId(1))
        .await;

    let rows = db.mesh_locks_for_repo("repo-1").await.unwrap();
    assert!(
        rows.is_empty(),
        "expected no rows after release; got {rows:?}"
    );
}
