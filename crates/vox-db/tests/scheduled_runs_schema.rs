//! Phase 4.1: scheduled_runs table exists with the expected columns + index.

use vox_db::{DbConfig, VoxDb};

async fn pragma_columns(db: &VoxDb, table: &str) -> Vec<String> {
    let sql = format!("PRAGMA table_info({table})");
    let rows: Vec<turso::Row> = db.query_all(&sql, ()).await.expect("pragma");
    let mut out = Vec::new();
    for r in rows {
        let name: String = r.get(1).expect("col name");
        out.push(name);
    }
    out
}

#[tokio::test]
async fn scheduled_runs_table_has_expected_columns() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("connect");
    let cols = pragma_columns(&db, "scheduled_runs").await;
    for required in [
        "function_name",
        "interval_ms",
        "next_due_at_ms",
        "last_run_id",
        "last_started_at_ms",
        "last_completed_at_ms",
    ] {
        assert!(
            cols.iter().any(|c| c == required),
            "scheduled_runs.{required} missing: {cols:?}"
        );
    }
}

#[tokio::test]
async fn scheduled_runs_next_due_index_exists() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("connect");
    let rows: Vec<turso::Row> = db
        .query_all(
            "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_scheduled_runs_next_due'",
            (),
        )
        .await
        .expect("query");
    assert_eq!(
        rows.len(),
        1,
        "idx_scheduled_runs_next_due must exist (rows: {})",
        rows.len()
    );
}
