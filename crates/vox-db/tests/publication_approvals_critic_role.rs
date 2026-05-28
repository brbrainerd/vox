//! Phase D wiring — `publication_approvals.approver_role` column +
//! role-aware counters used by the critic-gate evaluator.

use vox_db::{DbConfig, VoxDb};

#[tokio::test]
async fn approvals_table_has_phase_d_columns() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let rows = db
        .query_all(
            "SELECT sql FROM sqlite_master \
             WHERE type='table' AND name='publication_approvals'",
            (),
        )
        .await
        .expect("query");
    let sql: String = rows.first().expect("row").get(0).expect("sql column");
    for col in ["approver_role", "critic_fingerprint_json", "critic_report_uri"] {
        assert!(sql.contains(col), "DDL missing column `{col}`; got:\n{sql}");
    }
}

#[tokio::test]
async fn human_approvals_default_to_role_human() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    db.record_publication_approval_for_digest("pub-1", "digest-abc", "alice")
        .await
        .expect("record human");
    let (humans, critics) = db
        .count_publication_approvers_by_role("pub-1", "digest-abc")
        .await
        .expect("count");
    assert_eq!(humans, 1);
    assert_eq!(critics, 0);
}

#[tokio::test]
async fn critic_approval_recorded_with_fingerprint_and_role() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    db.record_publication_critic_approval_for_digest(
        "pub-2",
        "digest-xyz",
        "critic-001",
        r#"{"provider":"anthropic","model_id":"claude-3-5-sonnet"}"#,
        Some("file:///tmp/report.json"),
    )
    .await
    .expect("record critic");
    let (humans, critics) = db
        .count_publication_approvers_by_role("pub-2", "digest-xyz")
        .await
        .expect("count");
    assert_eq!(humans, 0);
    assert_eq!(critics, 1);

    let rows = db
        .list_publication_approvals_for_digest("pub-2", "digest-xyz")
        .await
        .expect("list");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_critic());
    assert_eq!(rows[0].approver, "critic-001");
    assert_eq!(
        rows[0].critic_report_uri.as_deref(),
        Some("file:///tmp/report.json")
    );
    assert!(rows[0].critic_fingerprint_json.is_some());
}

#[tokio::test]
async fn mixed_human_and_critic_count_independently() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    db.record_publication_approval_for_digest("pub-3", "d", "alice")
        .await
        .unwrap();
    db.record_publication_critic_approval_for_digest(
        "pub-3",
        "d",
        "critic-X",
        r#"{"provider":"acme","model_id":"m-1"}"#,
        None,
    )
    .await
    .unwrap();
    let (humans, critics) = db
        .count_publication_approvers_by_role("pub-3", "d")
        .await
        .unwrap();
    assert_eq!(humans, 1);
    assert_eq!(critics, 1);
    // Total distinct count includes both rows.
    let total = db
        .count_publication_approvers_for_digest("pub-3", "d")
        .await
        .unwrap();
    assert_eq!(total, 2);
}

#[tokio::test]
async fn empty_digest_returns_zero_counts_not_error() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    let (humans, critics) = db
        .count_publication_approvers_by_role("nonexistent", "nada")
        .await
        .expect("count must not error on empty");
    assert_eq!(humans, 0);
    assert_eq!(critics, 0);
}

#[tokio::test]
async fn list_orders_by_approved_at_ascending() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
    db.record_publication_approval_for_digest("pub-4", "d2", "alice")
        .await
        .unwrap();
    // Small sleep to ensure distinct ts; mesh-locks tests use this pattern.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    db.record_publication_approval_for_digest("pub-4", "d2", "bob")
        .await
        .unwrap();
    let rows = db
        .list_publication_approvals_for_digest("pub-4", "d2")
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].approved_at_ms <= rows[1].approved_at_ms);
}
