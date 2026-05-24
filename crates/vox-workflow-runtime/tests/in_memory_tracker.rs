#![allow(missing_docs)]
//! In-memory `WorkflowTracker` impl for unit tests. Avoids requiring vox-db in
//! the e2e codegen_roundtrip test (Task 2.2). Phase 3.1 verifies that
//! `VoxDbTracker` implements the same trait against the SQL-backed store.

use serde_json::json;
use vox_workflow_runtime::workflow::tracker::{InMemoryTracker, WorkflowTracker};

#[tokio::test]
async fn records_and_replays_activity() {
    let mut t = InMemoryTracker::default();
    t.on_activity_completed("workflow1", "act", "a1", &json!(7))
        .await
        .expect("record");
    let got = t
        .load_activity_result("workflow1", "a1")
        .await
        .expect("lookup");
    assert_eq!(got, Some(json!(7)));
}

#[tokio::test]
async fn returns_none_for_missing_activity() {
    let t = InMemoryTracker::default();
    let got = t
        .load_activity_result("workflow1", "missing")
        .await
        .expect("lookup");
    assert_eq!(got, None);
}

#[tokio::test]
async fn different_workflows_have_separate_namespaces() {
    let mut t = InMemoryTracker::default();
    t.on_activity_completed("wfA", "act", "a1", &json!(1))
        .await
        .expect("record");
    t.on_activity_completed("wfB", "act", "a1", &json!(2))
        .await
        .expect("record");
    assert_eq!(
        t.load_activity_result("wfA", "a1").await.unwrap(),
        Some(json!(1))
    );
    assert_eq!(
        t.load_activity_result("wfB", "a1").await.unwrap(),
        Some(json!(2))
    );
}

#[tokio::test]
async fn is_activity_completed_reflects_recorded_state() {
    let mut t = InMemoryTracker::default();
    assert!(
        !t.is_activity_completed("wf", "a1").await.unwrap(),
        "no record yet"
    );
    t.on_activity_completed("wf", "act", "a1", &json!("ok"))
        .await
        .unwrap();
    assert!(
        t.is_activity_completed("wf", "a1").await.unwrap(),
        "now completed"
    );
}
