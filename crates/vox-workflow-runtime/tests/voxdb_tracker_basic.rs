#![allow(missing_docs)]
//! Phase 3.1: `VoxDbTracker` implements `WorkflowTracker` correctly against a
//! real (in-memory) `VoxDb`. Proves the DB-backed tracker round-trips activity
//! results via the actual `workflow_activity_log` schema, not just the trait
//! shape.
//!
//! Refs ADR-019 v1 journal contract; ADR-021 generated-vs-interpreted parity.

use serde_json::json;
use std::sync::Arc;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::{VoxDbTracker, WorkflowTracker};

#[tokio::test]
async fn voxdb_tracker_records_and_loads_activity_result() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let mut tracker = VoxDbTracker::new(Arc::new(db), "run-3.1");

    tracker
        .on_activity_completed(
            "checkout",
            "charge_card",
            "charge_card_step_1",
            &json!("tx_42"),
        )
        .await
        .expect("record activity completed");

    let got = tracker
        .load_activity_result("checkout", "charge_card_step_1")
        .await
        .expect("load activity result");

    assert_eq!(got, Some(json!("tx_42")));
}

#[tokio::test]
async fn voxdb_tracker_reports_completed_activity() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let mut tracker = VoxDbTracker::new(Arc::new(db), "run-3.1b");

    tracker
        .on_activity_completed("wf", "a", "a_1", &json!(7))
        .await
        .expect("record activity completed");

    let completed = tracker
        .is_activity_completed("wf", "a_1")
        .await
        .expect("query is_activity_completed");
    assert!(
        completed,
        "activity should be reported completed after record"
    );

    let not_completed = tracker
        .is_activity_completed("wf", "never_recorded")
        .await
        .expect("query is_activity_completed");
    assert!(
        !not_completed,
        "unknown activity should be reported not-completed"
    );
}
