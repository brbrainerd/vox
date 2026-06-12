use tempfile::tempdir;
use vox_db::{DbConfig, VoxDb};

#[tokio::test]
async fn probe_sqlite_capabilities_returns_snapshot() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    let snap = db.probe_sqlite_capabilities().await.unwrap();
    assert!(!snap.journal_mode.is_empty());
}

#[tokio::test]
async fn sqlite_capabilities_snapshot_is_idempotent() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    let a = db.sqlite_capabilities_snapshot().await.unwrap();
    let b = db.sqlite_capabilities_snapshot().await.unwrap();
    assert_eq!(a.journal_mode, b.journal_mode);
    assert_eq!(a.foreign_keys_on, b.foreign_keys_on);
    assert_eq!(a.fts5_reported, b.fts5_reported);
}

#[tokio::test]
async fn test_db_memory_smoke() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    assert_eq!(
        db.schema_version().await.unwrap(),
        vox_db::schema::BASELINE_VERSION
    );

    let hash = db.store("test", b"hello").await.unwrap();
    assert!(!hash.is_empty());
}

#[tokio::test]
async fn test_db_local_file_persistence() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("vox.db");
    let path_str = db_path.to_str().unwrap().to_string();
    let hash;

    {
        let db = VoxDb::connect(DbConfig::Local {
            path: path_str.clone(),
        })
        .await
        .unwrap();
        hash = db.store("perm", b"data").await.unwrap();
    }

    // Reopen and check if it still works
    let db = VoxDb::connect(DbConfig::Local { path: path_str })
        .await
        .unwrap();
    let obj = db.get(&hash).await.unwrap();
    assert_eq!(obj, b"data");
}

#[tokio::test]
async fn test_discovery_state_table_exists() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    // Inserting a row proves the table + columns exist after baseline init.
    db.connection()
        .execute(
            "INSERT INTO discovery_state \
             (user_id, action_id, seen_count, used_count, last_seen_ms, last_used_ms, \
              dwell_ms_total, fsrs_stability, fsrs_difficulty, fsrs_due_ms) \
             VALUES ('u1','vox.scientia.review',1,0,10,0,0,0.0,0.0,0)",
            (),
        )
        .await
        .unwrap();
    let mut rows = db
        .connection()
        .query(
            "SELECT seen_count FROM discovery_state WHERE user_id='u1' AND action_id='vox.scientia.review'",
            (),
        )
        .await
        .unwrap();
    let n = rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn test_db_circuit_breaker() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    let breaker = db.breaker();
    assert_eq!(breaker.state(), vox_db::CircuitState::Closed);

    // We can't easily trigger a real failure in memory without mock,
    // but we can check if it exists and is closed.
}

#[tokio::test]
async fn test_db_transaction_success() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();

    db.transaction(async {
        db.save_memory(vox_db::MemoryParams {
            agent_id: "tx_agent",
            session_id: "sess_1",
            memory_type: "observation",
            content: "tx_data",
            metadata: None,
            importance: 1.0,
            vcs_snapshot_id: None,
        })
        .await?;
        Ok(())
    })
    .await
    .unwrap();

    let recalled = db.recall_memory("tx_agent", None, 10, None).await.unwrap();
    assert_eq!(recalled.len(), 1);
}
