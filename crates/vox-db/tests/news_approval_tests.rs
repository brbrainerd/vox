use vox_db::{DbConfig, VoxDb};

// news_publish_approvals (the legacy, non-digest-bound table) is quarantined
// (DORMANT, Task 4, VoxDB audit condensation plan) — off by default, see
// crates/vox-db/src/schema/domains/quarantine.rs. record_news_approval_for_digest
// and its news_publish_approvals_v2-backed siblings below are unaffected.
#[tokio::test]
#[cfg(feature = "quarantine")]
async fn news_publish_approvals_require_two_distinct_approvers() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();

    db.record_news_approval("2026-03-25-example", "alice")
        .await
        .unwrap();
    assert_eq!(
        db.count_news_approvers("2026-03-25-example").await.unwrap(),
        1
    );
    assert!(
        !db.has_dual_news_approval("2026-03-25-example")
            .await
            .unwrap()
    );

    db.record_news_approval("2026-03-25-example", "bob")
        .await
        .unwrap();
    assert!(
        db.has_dual_news_approval("2026-03-25-example")
            .await
            .unwrap()
    );

    // Same approver again does not increase distinct count
    db.record_news_approval("2026-03-25-example", "alice")
        .await
        .unwrap();
    assert_eq!(
        db.count_news_approvers("2026-03-25-example").await.unwrap(),
        2
    );
}

#[tokio::test]
async fn digest_bound_approvals_require_two_distinct_for_same_digest() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    let id = "2026-03-25-example";
    let d1 = "abc123";
    let d2 = "def456";

    db.record_news_approval_for_digest(id, d1, "alice")
        .await
        .unwrap();
    assert_eq!(db.count_news_approvers_for_digest(id, d1).await.unwrap(), 1);
    assert!(!db.has_dual_news_approval_for_digest(id, d1).await.unwrap());

    db.record_news_approval_for_digest(id, d2, "bob")
        .await
        .unwrap();
    assert_eq!(db.count_news_approvers_for_digest(id, d1).await.unwrap(), 1);
    assert!(!db.has_dual_news_approval_for_digest(id, d1).await.unwrap());

    db.record_news_approval_for_digest(id, d1, "bob")
        .await
        .unwrap();
    assert!(db.has_dual_news_approval_for_digest(id, d1).await.unwrap());
}

// Exercises has_dual_news_approval's fallback to the legacy news_publish_approvals
// table, quarantined (DORMANT, Task 4) — off by default, see domains/quarantine.rs.
#[tokio::test]
#[cfg(feature = "quarantine")]
async fn digest_approval_fallback_uses_legacy_table_for_migration_window() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    let id = "2026-03-25-example";
    db.record_news_approval(id, "alice").await.unwrap();
    db.record_news_approval(id, "bob").await.unwrap();

    let ok = db
        .has_dual_news_approval_with_fallback(id, "digest-not-yet-approved")
        .await
        .unwrap();
    assert!(ok);
}

#[tokio::test]
async fn mark_news_published_column_order_matches_github_twitter_oc() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    db.mark_news_published("x", "dig-x", Some("gh"), Some("tw"), Some("oc"))
        .await
        .unwrap();
    let rows = db
        .query_all(
            "SELECT github_release_id, twitter_tweet_id, opencollective_update_id, content_sha3_256 FROM published_news WHERE news_id = 'x'",
            (),
        )
        .await
        .unwrap();
    let r = rows.first().unwrap();
    let gh: String = r.get(0).unwrap();
    let tw: String = r.get(1).unwrap();
    let oc: String = r.get(2).unwrap();
    let dig: String = r.get(3).unwrap();
    assert_eq!(gh, "gh");
    assert_eq!(tw, "tw");
    assert_eq!(oc, "oc");
    assert_eq!(dig, "dig-x");
}
