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
    let path = dir
        .path()
        .join("vox.db")
        .to_str()
        .unwrap()
        .to_string();

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
