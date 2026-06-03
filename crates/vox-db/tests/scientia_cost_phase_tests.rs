//! Producer-level coverage for the Scientia cost-category split: phase-tagged
//! cost rows written via `insert_scientia_cost_telemetry` are grouped by
//! `pipeline_phase` by `scientia_cost_by_phase`, and untagged rows are excluded.

use tempfile::tempdir;
use vox_db::{DbConfig, VoxDb};

const WINDOW_START: i64 = 0;
const WINDOW_END: i64 = i64::MAX;

#[tokio::test]
async fn cost_by_phase_groups_tagged_rows_and_ignores_untagged() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("vox.db").to_str().unwrap().to_string();
    let db = VoxDb::connect(DbConfig::Local { path }).await.unwrap();

    // Two extraction rows (accumulate), one critic row.
    db.insert_scientia_cost_telemetry(
        "extractor", "pub-1", "vox-scientia", "extraction",
        Some("anthropic"), Some("mock"), None, None, 1.25, None,
    )
    .await
    .unwrap();
    db.insert_scientia_cost_telemetry(
        "extractor", "pub-2", "vox-scientia", "extraction",
        Some("anthropic"), Some("mock"), None, None, 0.75, None,
    )
    .await
    .unwrap();
    db.insert_scientia_cost_telemetry(
        "critic", "pub-1", "vox-scientia", "critic",
        Some("openai"), Some("gpt"), None, None, 0.50, None,
    )
    .await
    .unwrap();

    // An untagged generic telemetry cost row must NOT be attributed to a phase.
    db.insert_telemetry_flat_raw(
        "agent", "sess", None, "vox", "cost",
        None, Some("m"), Some("anthropic"), None, None, None, Some(9.99), None,
    )
    .await
    .unwrap();

    let rows = db.scientia_cost_by_phase(WINDOW_START, WINDOW_END).await.unwrap();

    let extraction = rows.iter().find(|r| r.phase == "extraction").unwrap();
    assert!((extraction.total_usd - 2.0).abs() < 1e-9, "got {}", extraction.total_usd);
    let critic = rows.iter().find(|r| r.phase == "critic").unwrap();
    assert!((critic.total_usd - 0.50).abs() < 1e-9);

    // Untagged row excluded; novelty/scholarly never written → absent.
    assert!(rows.iter().all(|r| r.phase != "novelty" && r.phase != "scholarly"));
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn cost_by_phase_empty_db_is_empty() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("vox.db").to_str().unwrap().to_string();
    let db = VoxDb::connect(DbConfig::Local { path }).await.unwrap();
    let rows = db.scientia_cost_by_phase(WINDOW_START, WINDOW_END).await.unwrap();
    assert!(rows.is_empty());
}
