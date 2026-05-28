#![allow(missing_docs)]
//! External integration tests for `interpret_workflow_durable` and WorkflowTracker.

use serde_json::Value;
use std::sync::{Arc, Mutex};
use vox_compiler::hir::HirModule;
use vox_db::{DbConfig, VoxDb};
use vox_workflow_runtime::{WorkflowTracker, interpret_workflow_durable};

// ── RecordingTracker ─────────────────────────────────────────────────────────

#[derive(Default)]
struct RecordingTracker {
    events: Arc<Mutex<Vec<String>>>,
}

impl RecordingTracker {
    fn push(&self, s: impl Into<String>) {
        self.events.lock().unwrap().push(s.into());
    }
}

impl WorkflowTracker for RecordingTracker {
    // The trait uses RPIT (`impl Future + Send`) so implementors keep the same
    // signature; rewriting as `async fn` is not equivalent here.
    #[allow(clippy::manual_async_fn)]
    fn is_activity_completed(
        &self,
        _wf: &str,
        _activity_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<bool>> + Send {
        async move { Ok(false) }
    }

    fn on_workflow_started(
        &mut self,
        wf: &str,
        steps: usize,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        self.push(format!("workflow_started:{wf}:{steps}"));
        async { Ok(()) }
    }

    fn on_activity_started(
        &mut self,
        _wf: &str,
        name: &str,
        _id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        self.push(format!("activity_started:{name}"));
        async { Ok(()) }
    }

    fn on_activity_completed(
        &mut self,
        _wf: &str,
        name: &str,
        _id: &str,
        _result: &Value,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        self.push(format!("activity_completed:{name}"));
        async { Ok(()) }
    }

    fn on_workflow_completed(
        &mut self,
        wf: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        self.push(format!("workflow_completed:{wf}"));
        async { Ok(()) }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// Documents that `interpret_workflow_durable` (and the underlying planner)
/// immediately returns an error now that `HirWorkflow` / `HirModule::workflows`
/// have been removed from vox-compiler. All higher-level workflow tests have
/// moved to vox-orchestrator, which owns durable execution.
#[tokio::test]
async fn workflow_not_found_returns_error_stub() {
    let hir = HirModule::default();
    let mut tracker = RecordingTracker::default();
    let err = interpret_workflow_durable(&hir, "any_workflow", &mut tracker).await;
    assert!(
        err.is_err(),
        "planner should error when workflow HIR is unavailable"
    );
    let msg = err.unwrap_err().to_string();
    assert!(
        msg.contains("not found"),
        "error should mention workflow not found: {msg}"
    );
}

#[tokio::test]
async fn workflow_activity_log_schema_contains_replay_contract_columns() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let rows = db
        .query_all("PRAGMA table_info(workflow_activity_log)", ())
        .await
        .expect("table info");

    let mut columns = std::collections::HashSet::new();
    for row in rows {
        let name: String = row.get(1).expect("name column");
        columns.insert(name);
    }

    for required in [
        "run_id",
        "workflow_name",
        "activity_name",
        "activity_id",
        "status",
        "result_json",
        "recorded_at_ms",
    ] {
        assert!(
            columns.contains(required),
            "workflow_activity_log missing required replay column `{required}`"
        );
    }
}

#[tokio::test]
async fn workflow_run_log_schema_contains_lease_columns() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let rows = db
        .query_all("PRAGMA table_info(workflow_run_log)", ())
        .await
        .expect("table info");
    let mut columns = std::collections::HashSet::new();
    for row in rows {
        let name: String = row.get(1).expect("name column");
        columns.insert(name);
    }
    for required in [
        "run_id",
        "workflow_name",
        "status",
        "planned_steps",
        "completed_steps",
        "plan_session_id",
        "plan_node_id",
        "plan_version",
        "lease_owner",
        "lease_until_ms",
        "started_at_ms",
        "updated_at_ms",
    ] {
        assert!(
            columns.contains(required),
            "workflow_run_log missing required column `{required}`"
        );
    }
}

#[tokio::test]
async fn workflow_signal_log_schema_contains_required_columns() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let rows = db
        .query_all("PRAGMA table_info(workflow_signal_log)", ())
        .await
        .expect("table info");
    let mut columns = std::collections::HashSet::new();
    for row in rows {
        let name: String = row.get(1).expect("name column");
        columns.insert(name);
    }
    for required in [
        "id",
        "run_id",
        "signal_key",
        "payload_json",
        "recorded_at_ms",
        "consumed_at_ms",
    ] {
        assert!(
            columns.contains(required),
            "workflow_signal_log missing required column `{required}`"
        );
    }
}

#[tokio::test]
async fn workflow_activity_attempt_log_schema_contains_required_columns() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let rows = db
        .query_all("PRAGMA table_info(workflow_activity_attempt_log)", ())
        .await
        .expect("table info");
    let mut columns = std::collections::HashSet::new();
    for row in rows {
        let name: String = row.get(1).expect("name column");
        columns.insert(name);
    }
    for required in [
        "run_id",
        "workflow_name",
        "activity_id",
        "attempt_no",
        "status",
        "worker_owner",
        "lease_until_ms",
        "error",
        "recorded_at_ms",
    ] {
        assert!(
            columns.contains(required),
            "workflow_activity_attempt_log missing required column `{required}`"
        );
    }
}
