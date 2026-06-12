use vox_gamify::discovery::ledger;
use vox_gamify::discovery::Recall;

#[tokio::test]
async fn record_seen_then_used_accumulates() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.unwrap();
    ledger::record(&db, "u1", "vox.scientia.review", Recall::Seen, 2_000, 1_000)
        .await
        .unwrap();
    ledger::record(&db, "u1", "vox.scientia.review", Recall::Used, 5_000, 0)
        .await
        .unwrap();
    let row = ledger::get(&db, "u1", "vox.scientia.review")
        .await
        .unwrap()
        .expect("row exists");
    assert_eq!(row.seen_count, 1);
    assert_eq!(row.used_count, 1);
    assert_eq!(row.dwell_ms_total, 1_000);
    assert!(row.fsrs_due_ms > 5_000);
}

#[tokio::test]
async fn due_query_returns_overdue_items() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.unwrap();
    ledger::record(&db, "u1", "vox.populi.status", Recall::Seen, 1, 0)
        .await
        .unwrap();
    // Far-future "now" makes the seeded item overdue.
    let due = ledger::due_action_ids(&db, "u1", i64::MAX, 10).await.unwrap();
    assert!(due.contains(&"vox.populi.status".to_string()));
}
