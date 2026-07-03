use tempfile::tempdir;
use vox_db::{AgentRunRow, DbConfig, VoxDb};

fn sample_run(run_id: &str, status: &str) -> AgentRunRow {
    AgentRunRow {
        run_id: run_id.to_string(),
        workflow_name: "demo".to_string(),
        command: Some("vox build".to_string()),
        repo: Some("vox".to_string()),
        worktree: Some("/wt".to_string()),
        model: Some("opus-4-8".to_string()),
        status: status.to_string(),
        planned_steps: 3,
        completed_steps: 0,
        cost_usd: 0.0,
        tokens_in: 0,
        tokens_out: 0,
        logs_ref: None,
        artifacts_json: "[]".to_string(),
        approval_ref: None,
        started_at_ms: 1000,
        updated_at_ms: 1000,
        completed_at_ms: None,
        last_error: None,
    }
}

/// B2 acceptance: an agent run is queryable by id, the latest upsert wins, it
/// appears in the recent list, and it survives a process restart (file reopen).
#[tokio::test]
async fn agent_runs_persist_query_and_survive_restart() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("vox.db").to_str().unwrap().to_string();

    {
        let db = VoxDb::connect(DbConfig::Local { path: path.clone() })
            .await
            .unwrap();
        db.agent_runs_upsert(&sample_run("R-1", "running"))
            .await
            .unwrap();

        // Re-upserting the same run id advances its lifecycle.
        let mut done = sample_run("R-1", "completed");
        done.completed_steps = 3;
        done.completed_at_ms = Some(2000);
        done.updated_at_ms = 2000;
        db.agent_runs_upsert(&done).await.unwrap();

        let got = db
            .agent_runs_get("R-1")
            .await
            .unwrap()
            .expect("run queryable by id");
        assert_eq!(got.status, "completed");
        assert_eq!(got.completed_steps, 3);

        let recent = db.agent_runs_recent(10).await.unwrap();
        assert!(recent.iter().any(|r| r.run_id == "R-1"));
    }

    // Reopen the same file — the run must survive restart and stay queryable.
    let db = VoxDb::connect(DbConfig::Local { path }).await.unwrap();
    let got = db
        .agent_runs_get("R-1")
        .await
        .unwrap()
        .expect("run survives restart");
    assert_eq!(got.status, "completed");
    assert_eq!(got.command.as_deref(), Some("vox build"));
}

/// T1.5 Part 1: `find_approval_id_for_run` joins an `ApprovalRequested` op-log
/// entry (persisted the way `OpLog::record_persisted`/`write_entry` write it —
/// see crates/vox-orchestrator-queue/src/oplog/persist.rs) by its `run_id`
/// field and returns the matching `approval_id`.
#[tokio::test]
async fn find_approval_id_for_run_joins_by_run_id() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("vox.db").to_str().unwrap().to_string();
    let db = VoxDb::connect(DbConfig::Local { path }).await.unwrap();

    // Shape matches `OperationKind::ApprovalRequested`'s serde_json encoding:
    // an externally-tagged enum variant carrying { approval_id, tool, run_id }.
    let kind_json = serde_json::json!({
        "ApprovalRequested": {
            "approval_id": "AP-000042",
            "tool": "vox_run_shell",
            "run_id": "77",
        }
    })
    .to_string();

    db.insert_convergence_op_log(
        1,
        "0000000000000000000000000000",
        "[]",
        &kind_json,
        "deadbeef",
        None,
        None,
        None,
        0,
        "00000000000000000000000000000000",
        1000,
        "Approval requested for vox_run_shell",
        None,
        None,
    )
    .await
    .unwrap();

    let found = db.find_approval_id_for_run("77").await;
    assert_eq!(found.as_deref(), Some("AP-000042"));

    // No matching run_id -> None (graceful, not an error).
    assert!(db.find_approval_id_for_run("no-such-run").await.is_none());
}

/// T1.5 Part 2: `find_task_root_summary_totals` joins a `TaskRootSummaryEvent`
/// (persisted the way `ResearchMetricsSink` writes it — session_id
/// `task:{task_id}`, metric_type `task.root_summary`, metadata_json = the
/// serialized event) by numeric task_id and returns its cost/token totals.
#[tokio::test]
async fn find_task_root_summary_totals_joins_by_task_id() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("vox.db").to_str().unwrap().to_string();
    let db = VoxDb::connect(DbConfig::Local { path }).await.unwrap();

    let metadata_json = serde_json::json!({
        "task_id": 99,
        "trace_id": "t-1",
        "repository_id": null,
        "outcome": "completed",
        "wall_time_ms": 4200,
        "total_input_tokens": 1500,
        "total_output_tokens": 300,
        "total_cost_usd": 0.0234,
        "child_call_count": 2,
        "max_span_depth": 1,
        "subagent_fanout": 0,
    })
    .to_string();

    db.append_research_metric(
        "task:99",
        "task.root_summary",
        Some(0.0234),
        Some(&metadata_json),
    )
    .await
    .unwrap();

    let (cost, tin, tout) = db
        .find_task_root_summary_totals("99")
        .await
        .expect("telemetry present");
    assert!((cost - 0.0234).abs() < 1e-9);
    assert_eq!(tin, 1500);
    assert_eq!(tout, 300);

    // No matching telemetry -> None (graceful degradation, not an error).
    assert!(
        db.find_task_root_summary_totals("no-such-task")
            .await
            .is_none()
    );
}
