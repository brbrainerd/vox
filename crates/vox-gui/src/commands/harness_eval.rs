//! Tauri commands backing the Vox Axis "Harness Health" GUI surface (chat harness continuous
//! eval design, 2026-08-02). Read-only: all writes to these tables happen via `vox harness eval
//! --live`/`publish` (CLI), never from the GUI.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use vox_db::VoxDb;

use crate::commands::gui_db_pool::{GuiDbPool, map_db_err};

fn pool_db(pool: &GuiDbPool) -> Result<Arc<VoxDb>, String> {
    pool.handle()
}

/// Per-category pass/fail rollup for one run — closes design spec §10.2's "per-task-category
/// breakdown... visible at a glance, not buried in an aggregate pass rate" requirement.
#[derive(Debug, Serialize)]
pub struct CategorySummaryDto {
    pub category: String,
    pub pass_count: i64,
    pub fail_count: i64,
}

/// One row of the GUI's recent-runs table.
#[derive(Debug, Serialize)]
pub struct HarnessEvalRunDto {
    pub run_id: String,
    pub git_sha: String,
    pub triggered_by: String,
    pub pass_count: i64,
    pub fail_count: i64,
    pub skip_count: i64,
    pub total_cost_usd: f64,
    pub started_at_ms: i64,
    pub category_breakdown: Vec<CategorySummaryDto>,
}

#[tauri::command]
pub async fn harness_eval_history(
    pool: State<'_, GuiDbPool>,
    limit: Option<usize>,
) -> Result<Vec<HarnessEvalRunDto>, String> {
    let db = pool_db(&pool)?;
    let runs = db
        .list_harness_eval_runs(limit.unwrap_or(50))
        .await
        .map_err(map_db_err)?;
    let run_ids: Vec<String> = runs.iter().map(|r| r.run_id.clone()).collect();
    let mut task_results_by_run = db
        .get_harness_eval_task_results_for_runs(&run_ids)
        .await
        .map_err(map_db_err)?;
    let mut out = Vec::with_capacity(runs.len());
    for r in runs {
        let task_results = task_results_by_run.remove(&r.run_id).unwrap_or_default();
        let mut by_category: std::collections::BTreeMap<String, (i64, i64)> =
            std::collections::BTreeMap::new();
        for t in &task_results {
            let entry = by_category.entry(t.category.clone()).or_default();
            if t.status == "pass" {
                entry.0 += 1;
            } else if t.status == "fail" {
                entry.1 += 1;
            }
        }
        out.push(HarnessEvalRunDto {
            run_id: r.run_id,
            git_sha: r.git_sha,
            triggered_by: r.triggered_by,
            pass_count: r.pass_count,
            fail_count: r.fail_count,
            skip_count: r.skip_count,
            total_cost_usd: r.total_cost_usd,
            started_at_ms: r.started_at_ms,
            category_breakdown: by_category
                .into_iter()
                .map(|(category, (pass_count, fail_count))| CategorySummaryDto {
                    category,
                    pass_count,
                    fail_count,
                })
                .collect(),
        });
    }
    Ok(out)
}

/// One flagged regression, DTO shape for the GUI's regression banner (design spec §10.2).
#[derive(Debug, Serialize)]
pub struct RegressionFlagDto {
    pub kind: String,
    pub previous_run_id: String,
    pub current_run_id: String,
    pub previous_git_sha: String,
    pub current_git_sha: String,
    pub changed_files: Vec<String>,
    pub flipped_task_ids: Vec<String>,
    pub detail: String,
}

/// Same validation `vox-cli`'s `ingest_runs`/`report.rs` apply — this command constructs its own
/// `git diff` subprocess call independently, so it needs its own defense-in-depth check too, not
/// just a shared library function it might forget to call.
fn is_valid_git_sha(s: &str) -> bool {
    (7..=40).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Compares the two most recent runs and returns any detected regressions (empty if none, or if
/// fewer than 2 runs exist yet). Reuses `vox-cli`'s pure `detect_regressions` function directly
/// — no duplicated logic between the CLI's `report` command and this GUI command.
#[tauri::command]
pub async fn harness_eval_regressions(
    pool: State<'_, GuiDbPool>,
) -> Result<Vec<RegressionFlagDto>, String> {
    let db = pool_db(&pool)?;
    let runs = db.list_harness_eval_runs(2).await.map_err(map_db_err)?;
    if runs.len() < 2 {
        return Ok(vec![]);
    }
    let (current, previous) = (&runs[0], &runs[1]);
    if !is_valid_git_sha(&previous.git_sha) || !is_valid_git_sha(&current.git_sha) {
        return Err(format!(
            "refusing to shell out to git diff with a malformed git_sha (previous={:?}, current={:?})",
            previous.git_sha, current.git_sha
        ));
    }
    let previous_task_results = db
        .get_harness_eval_task_results(&previous.run_id)
        .await
        .map_err(map_db_err)?;
    let current_task_results = db
        .get_harness_eval_task_results(&current.run_id)
        .await
        .map_err(map_db_err)?;
    let current_events = db
        .get_model_selection_events(&current.run_id)
        .await
        .map_err(map_db_err)?;
    let previous_events = db
        .get_model_selection_events(&previous.run_id)
        .await
        .map_err(map_db_err)?;
    let repo_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let changed_files: Vec<String> = vox_git::read_cmd::read_only(
        &repo_root,
        &[
            "diff",
            "--name-only",
            &format!("{}..{}", previous.git_sha, current.git_sha),
            "--",
        ],
    )
    .ok()
    .map(|s| s.lines().map(str::to_string).collect())
    .unwrap_or_default();

    let flags = vox_cli::commands::harness::report::detect_regressions(
        previous,
        current,
        &previous_task_results,
        &current_task_results,
        &previous_events,
        &current_events,
        &changed_files,
    );
    Ok(flags
        .into_iter()
        .map(|f| RegressionFlagDto {
            kind: format!("{:?}", f.kind),
            previous_run_id: f.previous_run_id,
            current_run_id: f.current_run_id,
            previous_git_sha: f.previous_git_sha,
            current_git_sha: f.current_git_sha,
            changed_files: f.changed_files,
            flipped_task_ids: f.flipped_task_ids,
            detail: f.detail,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Manager;

    /// Calls the REAL `#[tauri::command]` function through `tauri::test::mock_app()` + a real
    /// in-memory `GuiDbPool` — the pattern already established in `crates/vox-gui/src/commands/
    /// chat.rs`'s own tests. A test that manually re-derives the DTO shape inline (as an earlier
    /// draft of this file did) never actually calls the production function and cannot catch a
    /// bug in it; this does.
    #[tokio::test]
    async fn harness_eval_history_returns_persisted_runs_via_the_real_command() {
        let app = tauri::test::mock_app();
        let pool = GuiDbPool::connect_memory().await.expect("memory pool");
        let db = pool.handle().expect("db handle");
        db.record_harness_eval_run(&vox_db::HarnessEvalRunRecord {
            run_id: "r1".to_string(),
            triggered_by: "local".to_string(),
            git_sha: "abc1234".to_string(),
            git_branch: "main".to_string(),
            changed_files: vec![],
            config_version: None,
            samples_per_task: 3,
            task_count: 10,
            pass_count: 9,
            fail_count: 1,
            skip_count: 0,
            total_cost_usd: 0.05,
            started_at_ms: 1000,
            finished_at_ms: 2000,
        })
        .await
        .expect("record run");
        app.manage(pool);

        let state = app.state::<GuiDbPool>();
        let result = harness_eval_history(state, None)
            .await
            .expect("history call");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].run_id, "r1");
        assert_eq!(result[0].pass_count, 9);
    }

    #[tokio::test]
    async fn harness_eval_regressions_returns_empty_with_fewer_than_two_runs() {
        let app = tauri::test::mock_app();
        let pool = GuiDbPool::connect_memory().await.expect("memory pool");
        app.manage(pool);

        let state = app.state::<GuiDbPool>();
        let result = harness_eval_regressions(state)
            .await
            .expect("regressions call");
        assert!(
            result.is_empty(),
            "fewer than 2 runs must return no regressions, not error"
        );
    }

    #[test]
    fn is_valid_git_sha_rejects_a_dash_prefixed_value() {
        assert!(!is_valid_git_sha("--output=/tmp/evil"));
        assert!(is_valid_git_sha("abc1234"));
    }
}
