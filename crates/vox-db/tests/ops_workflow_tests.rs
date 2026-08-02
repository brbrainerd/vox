#![allow(missing_docs)]
//! Integration tests for vox-package workflow execution storage.

use vox_db::LogExecutionParams;
use vox_db::VoxDb;

async fn db() -> VoxDb {
    VoxDb::open_memory().await.expect("open_memory")
}

// workflow_executions is quarantined (DORMANT, no confirmed caller outside
// vox-db even via wrapper-call detection — Task 4, VoxDB audit condensation
// plan) — off by default, see crates/vox-db/src/schema/domains/quarantine.rs.
// These 5 tests were never caught by the test-census scan because their
// bodies call start_workflow_execution/finish_workflow_execution/
// get_workflow_execution, not the literal table name — same class of blind
// spot the wrapper-call detection pass exists for, on the test side.
// execution_log's tests below (is_activity_completed_*) are unaffected —
// that table stays LIVE (real caller found via wrapper-call detection).
#[tokio::test]
#[cfg(feature = "quarantine")]
async fn start_workflow_sets_status_running() {
    let cs: VoxDb = db().await;
    cs.start_workflow_execution("wf-1", 3).await.expect("start");
    let row = cs
        .get_workflow_execution("wf-1")
        .await
        .expect("get")
        .expect("present");
    assert_eq!(row.status, "running");
    assert_eq!(row.step_count, 3);
    assert_eq!(row.error_count, 0);
    assert!(row.finished_at.is_none());
}

#[tokio::test]
#[cfg(feature = "quarantine")]
async fn finish_workflow_sets_status_ok() {
    let cs: VoxDb = db().await;
    cs.start_workflow_execution("wf-2", 2).await.expect("start");
    cs.finish_workflow_execution("wf-2", "ok", 0)
        .await
        .expect("finish");
    let row = cs
        .get_workflow_execution("wf-2")
        .await
        .expect("get")
        .expect("present");
    assert_eq!(row.status, "ok");
    assert!(row.finished_at.is_some());
}

#[tokio::test]
#[cfg(feature = "quarantine")]
async fn finish_workflow_sets_error_count() {
    let cs: VoxDb = db().await;
    cs.start_workflow_execution("wf-3", 5).await.expect("start");
    cs.finish_workflow_execution("wf-3", "error", 2)
        .await
        .expect("finish");
    let row = cs
        .get_workflow_execution("wf-3")
        .await
        .expect("get")
        .expect("present");
    assert_eq!(row.status, "error");
    assert_eq!(row.error_count, 2);
}

#[tokio::test]
#[cfg(feature = "quarantine")]
async fn get_workflow_execution_returns_none_for_unknown() {
    let cs: VoxDb = db().await;
    let r = cs
        .get_workflow_execution("wf-does-not-exist")
        .await
        .expect("get");
    assert!(r.is_none());
}

#[tokio::test]
#[cfg(feature = "quarantine")]
async fn start_workflow_is_idempotent_upsert() {
    let cs: VoxDb = db().await;
    cs.start_workflow_execution("wf-idem", 1)
        .await
        .expect("first");
    cs.start_workflow_execution("wf-idem", 4)
        .await
        .expect("second"); // step_count update
    let row = cs
        .get_workflow_execution("wf-idem")
        .await
        .expect("get")
        .expect("present");
    assert_eq!(row.step_count, 4);
}

#[tokio::test]
async fn is_activity_completed_false_when_no_log() {
    let cs: VoxDb = db().await;
    let done = cs
        .is_activity_completed("wf-new", "fetch_user")
        .await
        .expect("check");
    assert!(!done);
}

fn exec_log<'a>(wf: &'a str, activity: &'a str) -> LogExecutionParams<'a> {
    LogExecutionParams {
        workflow_id: wf,
        agent_id: None,
        skill_id: None,
        activity_name: activity,
        status: "ok",
        attempt: 1,
        duration_ms: 50,
        output_size: 0,
        input: None,
        output: None,
        error: None,
        options: None,
    }
}

#[tokio::test]
async fn log_execution_then_is_activity_completed_true() {
    let cs: VoxDb = db().await;
    cs.log_execution(&exec_log("wf-done", "send_email"))
        .await
        .expect("log");
    let done = cs
        .is_activity_completed("wf-done", "send_email")
        .await
        .expect("check");
    assert!(done, "activity should be marked completed");
}

#[tokio::test]
async fn is_activity_completed_only_for_ok_status() {
    let cs: VoxDb = db().await;
    let mut p = exec_log("wf-fail", "fetch");
    p.status = "error";
    cs.log_execution(&p).await.expect("log_error");
    let done = cs
        .is_activity_completed("wf-fail", "fetch")
        .await
        .expect("check");
    assert!(!done, "error status must not count as completed");
}
