//! Phase A — schema acceptance for `scientia_finding_candidates`.
//!
//! The table is the persistence target for SCIENTIA signal producers
//! (`vox-scientia-producers`). It mirrors the shape of `scientia_discoveries`
//! but is keyed by candidate-id (UNIQUE) and indexed by class + repository +
//! `(producer_name, signal_fingerprint)` for upsert-style dedup.

use vox_db::{DbConfig, VoxDb};

#[tokio::test]
async fn scientia_finding_candidates_table_exists_after_baseline() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let rows = db
        .query_all(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='scientia_finding_candidates'",
            (),
        )
        .await
        .expect("query");
    let count: i64 = rows.first().expect("row").get(0).expect("count");
    assert_eq!(count, 1, "scientia_finding_candidates table missing");
}

#[tokio::test]
async fn scientia_finding_candidates_class_index_exists() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let rows = db
        .query_all(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='index' AND name='idx_scientia_finding_candidates_class'",
            (),
        )
        .await
        .expect("query");
    let count: i64 = rows.first().expect("row").get(0).expect("count");
    assert_eq!(count, 1, "class index missing");
}

#[tokio::test]
async fn scientia_finding_candidates_fingerprint_unique_index_exists() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let rows = db
        .query_all(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='index' AND name='idx_scientia_finding_candidates_fingerprint'",
            (),
        )
        .await
        .expect("query");
    let count: i64 = rows.first().expect("row").get(0).expect("count");
    assert_eq!(count, 1, "(producer_name, signal_fingerprint) unique index missing");
}

#[tokio::test]
async fn scientia_finding_candidates_table_has_expected_columns() {
    // Turso does not support CHECK constraints, so enum validity is enforced
    // at the Rust layer (see `vox_db::FindingCandidateClass`). We still assert
    // the column list is what downstream store ops expect.
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let rows = db
        .query_all(
            "SELECT sql FROM sqlite_master \
             WHERE type='table' AND name='scientia_finding_candidates'",
            (),
        )
        .await
        .expect("query");
    let sql: String = rows.first().expect("row").get(0).expect("sql column");
    for col in [
        "candidate_id",
        "candidate_class",
        "internal_signals_json",
        "producer_name",
        "signal_fingerprint",
        "created_at_ms",
        "updated_at_ms",
    ] {
        assert!(sql.contains(col), "DDL missing column `{col}`; got:\n{sql}");
    }
}
