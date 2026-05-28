//! journal::execute: run-and-record on first call, replay-from-journal on resume.
//! Uses test_support feature for in-memory state; production path uses VoxDbTracker
//! (wired in Phase 3).

#![cfg(feature = "test-support")]

use std::sync::Mutex;
use vox_workflow_runtime::journal;

// Serialize all tests in this file — the journal in-memory state is global,
// and parallel tests would race on reset()/seed_completed().
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn execute_runs_body_and_records_journal_entry() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    journal::test_support::reset();
    let result: Result<i64, anyhow::Error> = journal::execute("activity-1", async move {
        Ok(42i64)
    })
    .await;
    assert_eq!(result.unwrap(), 42);
    let recorded = journal::test_support::recorded_for("activity-1");
    assert_eq!(recorded.len(), 1, "expected one journal entry for activity-1");
    assert_eq!(recorded[0], serde_json::json!(42));
}

#[tokio::test]
async fn execute_replays_from_journal_on_resume() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    journal::test_support::reset();
    journal::test_support::seed_completed("activity-2", serde_json::json!(99i64));

    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counter_clone = counter.clone();
    let result: Result<i64, anyhow::Error> = journal::execute("activity-2", async move {
        counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(7i64)
    })
    .await;

    assert_eq!(result.unwrap(), 99, "replay should return seeded value not fresh body");
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0, "body must NOT execute on replay");
}

#[tokio::test]
async fn execute_propagates_errors_from_body() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    journal::test_support::reset();
    let result: Result<i64, anyhow::Error> = journal::execute("activity-3", async move {
        Err(anyhow::anyhow!("simulated activity failure"))
    })
    .await;
    assert!(result.is_err(), "error from body must propagate");
    let recorded = journal::test_support::recorded_for("activity-3");
    assert_eq!(recorded.len(), 0, "failed activities must NOT be recorded as completed");
}
