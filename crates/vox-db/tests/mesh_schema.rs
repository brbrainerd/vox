//! Phase-0 schema acceptance: vcs_lock and lock_leader tables exist after a
//! fresh baseline apply, and the typed mesh_locks accessors round-trip correctly.
//! (P0-T1, P0-T2)

use vox_db::{DbConfig, VoxDb};

// ── P0-T1a: table existence ───────────────────────────────────────────────────

#[tokio::test]
async fn vcs_lock_table_exists_after_baseline() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let rows = db
        .query_all(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='vcs_lock'",
            (),
        )
        .await
        .expect("query");
    let count: i64 = rows.first().expect("row").get(0).expect("count");
    assert_eq!(count, 1, "vcs_lock table missing");
}

#[tokio::test]
async fn lock_leader_table_exists_after_baseline() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let rows = db
        .query_all(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='lock_leader'",
            (),
        )
        .await
        .expect("query");
    let count: i64 = rows.first().expect("row").get(0).expect("count");
    assert_eq!(count, 1, "lock_leader table missing");
}

// ── P0-T1b: typed accessor round-trips ───────────────────────────────────────

use vox_db::mesh_locks::{LockKindRow, VcsLockRow};

#[tokio::test]
async fn upsert_then_load_vcs_lock_roundtrips() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let row = VcsLockRow {
        path: "src/main.rs".into(),
        kind: LockKindRow::Exclusive,
        holder: "1".into(),
        holder_node_id: "node-A".into(),
        repository_id: "repo-1".into(),
        acquired_at: 1_000,
        expires_at: 60_000,
        lease_id: None,
        fence_token: 1,
    };
    db.mesh_locks_upsert(&row).await.expect("upsert");
    let loaded = db.mesh_locks_for_repo("repo-1").await.expect("load");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].path, "src/main.rs");
    assert_eq!(loaded[0].kind, LockKindRow::Exclusive);
    assert_eq!(loaded[0].fence_token, 1);
}

#[tokio::test]
async fn release_vcs_lock_only_when_holder_matches() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let row = VcsLockRow {
        path: "src/lib.rs".into(),
        kind: LockKindRow::Exclusive,
        holder: "1".into(),
        holder_node_id: "node-A".into(),
        repository_id: "repo-1".into(),
        acquired_at: 1_000,
        expires_at: 60_000,
        lease_id: None,
        fence_token: 0,
    };
    db.mesh_locks_upsert(&row).await.unwrap();
    // Wrong holder: no-op.
    let removed = db.mesh_locks_release("src/lib.rs", "node-B").await.unwrap();
    assert_eq!(removed, 0);
    // Right holder: removes.
    let removed = db.mesh_locks_release("src/lib.rs", "node-A").await.unwrap();
    assert_eq!(removed, 1);
}
