//! `vox harness history`/`report` — CLI surfacing for persisted harness eval runs, plus
//! the regression-detection logic shared with the GUI's Harness Health surface (design spec
//! §10.3).

/// A detected regression between two consecutive runs. `flipped_task_ids` is populated only for
/// `RegressionKind::TaskFlippedToFail` (design spec §10.3's "specific task/selection rows that
/// changed" requirement) — empty for the aggregate-threshold kinds.
#[derive(Debug, Clone, PartialEq)]
pub struct RegressionFlag {
    pub kind: RegressionKind,
    pub previous_run_id: String,
    pub current_run_id: String,
    pub previous_git_sha: String,
    pub current_git_sha: String,
    pub changed_files: Vec<String>,
    pub flipped_task_ids: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionKind {
    /// A single task went from pass in the previous run to fail in the current run — flagged
    /// independent of the aggregate pass-rate threshold below, since an aggregate percentage can
    /// mask one task flipping fail while a different task happens to flip pass in the same run.
    TaskFlippedToFail,
    PassRateDrop,
    CostTierRatioDrop,
}

const PASS_RATE_DROP_THRESHOLD_PP: f64 = 10.0;
const COST_TIER_RATIO_DROP_THRESHOLD_PP: f64 = 15.0;

fn pass_rate(run: &vox_db::HarnessEvalRunRecord) -> f64 {
    let graded = run.task_count - run.skip_count;
    if graded <= 0 {
        return 100.0;
    }
    (run.pass_count as f64 / graded as f64) * 100.0
}

/// Task M3: unbiased pass@k estimator (Chen et al. 2021, "Evaluating Large Language Models
/// Trained on Code", eq. 1) for one task's `n` total samples and `c` passing ones. Using
/// `c/n >= threshold` instead would need `k` *independently drawn* reruns to be unbiased;
/// this needs only the single `n`-sample batch already recorded.
///
/// Returns `None` when `k > n` — not enough samples were drawn to estimate at that `k`.
fn pass_at_k(n: i64, c: i64, k: i64) -> Option<f64> {
    if k > n || n <= 0 || k <= 0 {
        return None;
    }
    if n - c < k {
        // Fewer than k failures exist, so every size-k sample includes at least one pass.
        return Some(1.0);
    }
    let mut prob_all_fail = 1.0;
    for i in 0..k {
        prob_all_fail *= (n - c - i) as f64 / (n - i) as f64;
    }
    Some(1.0 - prob_all_fail)
}

/// Task M3: mean pass@k across every task in a run for which `k <= total_samples` — tasks
/// sampled fewer than `k` times are excluded rather than treated as 0, since "not enough
/// samples to know" and "known to always fail" are different facts. `None` when no task in
/// the run has enough samples for this `k`.
fn mean_pass_at_k(task_results: &[vox_db::HarnessEvalTaskResultRecord], k: i64) -> Option<f64> {
    let scores: Vec<f64> = task_results
        .iter()
        .filter_map(|t| pass_at_k(t.total_samples, t.pass_samples, k))
        .collect();
    if scores.is_empty() {
        return None;
    }
    Some(scores.iter().sum::<f64>() / scores.len() as f64)
}

fn free_cheap_ratio(events: &[vox_db::ModelSelectionEventRecord]) -> f64 {
    let non_privacy_forced: Vec<_> = events.iter().filter(|e| !e.was_privacy_gated).collect();
    if non_privacy_forced.is_empty() {
        return 100.0;
    }
    let free_or_cheap = non_privacy_forced
        .iter()
        .filter(|e| e.cost_tier == "free" || e.cost_tier == "cheap")
        .count();
    (free_or_cheap as f64 / non_privacy_forced.len() as f64) * 100.0
}

/// Compare two consecutive runs, their selection events, AND their per-task result lists,
/// returning any regressions detected. Pure function — no DB access — so it's fully unit-testable
/// against fixture data (design spec §12). Aggregate-only comparison (the two runs' pass_count/
/// task_count alone) cannot answer "which task regressed" — that's why `previous_task_results`/
/// `current_task_results` are required inputs, not optional ones.
pub fn detect_regressions(
    previous: &vox_db::HarnessEvalRunRecord,
    current: &vox_db::HarnessEvalRunRecord,
    previous_task_results: &[vox_db::HarnessEvalTaskResultRecord],
    current_task_results: &[vox_db::HarnessEvalTaskResultRecord],
    previous_events: &[vox_db::ModelSelectionEventRecord],
    current_events: &[vox_db::ModelSelectionEventRecord],
    changed_files: &[String],
) -> Vec<RegressionFlag> {
    let mut flags = Vec::new();

    let prev_status_by_task: std::collections::HashMap<&str, &str> = previous_task_results
        .iter()
        .map(|t| (t.task_id.as_str(), t.status.as_str()))
        .collect();
    let flipped_task_ids: Vec<String> = current_task_results
        .iter()
        .filter(|t| {
            t.status == "fail" && prev_status_by_task.get(t.task_id.as_str()) == Some(&"pass")
        })
        .map(|t| t.task_id.clone())
        .collect();
    if !flipped_task_ids.is_empty() {
        flags.push(RegressionFlag {
            kind: RegressionKind::TaskFlippedToFail,
            previous_run_id: previous.run_id.clone(),
            current_run_id: current.run_id.clone(),
            previous_git_sha: previous.git_sha.clone(),
            current_git_sha: current.git_sha.clone(),
            changed_files: changed_files.to_vec(),
            detail: format!(
                "{} task(s) flipped from pass to fail: {}",
                flipped_task_ids.len(),
                flipped_task_ids.join(", ")
            ),
            flipped_task_ids,
        });
    }

    let prev_pass_rate = pass_rate(previous);
    let cur_pass_rate = pass_rate(current);
    if prev_pass_rate - cur_pass_rate > PASS_RATE_DROP_THRESHOLD_PP {
        flags.push(RegressionFlag {
            kind: RegressionKind::PassRateDrop,
            previous_run_id: previous.run_id.clone(),
            current_run_id: current.run_id.clone(),
            previous_git_sha: previous.git_sha.clone(),
            current_git_sha: current.git_sha.clone(),
            changed_files: changed_files.to_vec(),
            flipped_task_ids: vec![],
            detail: format!("pass rate dropped from {prev_pass_rate:.1}% to {cur_pass_rate:.1}%"),
        });
    }

    let prev_ratio = free_cheap_ratio(previous_events);
    let cur_ratio = free_cheap_ratio(current_events);
    if prev_ratio - cur_ratio > COST_TIER_RATIO_DROP_THRESHOLD_PP {
        flags.push(RegressionFlag {
            kind: RegressionKind::CostTierRatioDrop,
            previous_run_id: previous.run_id.clone(),
            current_run_id: current.run_id.clone(),
            previous_git_sha: previous.git_sha.clone(),
            current_git_sha: current.git_sha.clone(),
            changed_files: changed_files.to_vec(),
            flipped_task_ids: vec![],
            detail: format!(
                "free/cheap model-selection ratio dropped from {prev_ratio:.1}% to {cur_ratio:.1}%"
            ),
        });
    }

    flags
}

use clap::Parser;

/// Same `git_sha` validation `ingest_runs` (Task 6) applies at write time — re-checked here as a
/// defense-in-depth boundary, since this function is the one that actually constructs a `git`
/// subprocess call from a stored value. A well-formed value from `ingest_runs` will always pass;
/// this only ever rejects something that slipped through some other write path.
fn is_valid_git_sha(s: &str) -> bool {
    (7..=40).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[derive(Parser)]
pub struct HistoryArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// Filter to runs where this task category has a result.
    #[arg(long)]
    pub category: Option<String>,
}

pub async fn run_history(args: HistoryArgs) -> anyhow::Result<()> {
    let db = vox_db::open_project_db().await?;
    super::publish::sync_from_jsonl(
        &db,
        std::path::Path::new("docs/harness-eval-history/runs.jsonl"),
    )
    .await?;

    let mut runs = db.list_harness_eval_runs(args.limit).await?;
    if let Some(category) = &args.category {
        let mut kept = Vec::new();
        for run in runs {
            let task_results = db.get_harness_eval_task_results(&run.run_id).await?;
            if task_results.iter().any(|t| &t.category == category) {
                kept.push(run);
            }
        }
        runs = kept;
    }
    if runs.is_empty() {
        println!("no harness eval runs recorded yet");
        return Ok(());
    }
    println!(
        "{:<24} {:<12} {:>6} {:>6} {:>6} {:>10} {:>12}",
        "run_id", "git_sha", "pass", "fail", "skip", "cost_usd", "free/cheap%"
    );
    for run in &runs {
        let events = db.get_model_selection_events(&run.run_id).await?;
        let free_cheap_pct = free_cheap_ratio(&events);
        println!(
            "{:<24} {:<12} {:>6} {:>6} {:>6} {:>10.4} {:>11.1}%",
            run.run_id,
            run.git_sha,
            run.pass_count,
            run.fail_count,
            run.skip_count,
            run.total_cost_usd,
            free_cheap_pct
        );
    }
    Ok(())
}

#[derive(Parser)]
pub struct ReportArgs {
    /// Compare against runs since this run_id (exclusive) instead of just the two most recent.
    /// Currently used only to widen which "current" run is reported on; full multi-run trend
    /// summaries across the range are a natural follow-up, not implemented in this task.
    #[arg(long)]
    pub since: Option<String>,
}

pub async fn run_report(args: ReportArgs) -> anyhow::Result<()> {
    let db = vox_db::open_project_db().await?;
    super::publish::sync_from_jsonl(
        &db,
        std::path::Path::new("docs/harness-eval-history/runs.jsonl"),
    )
    .await?;

    let limit = if args.since.is_some() { 50 } else { 2 };
    let runs = db.list_harness_eval_runs(limit).await?;
    let runs: Vec<_> = if let Some(since) = &args.since {
        let since_run = runs.iter().find(|r| &r.run_id == since).cloned();
        runs.into_iter()
            .take_while(|r| &r.run_id != since)
            .chain(since_run)
            .collect()
    } else {
        runs
    };
    if runs.len() < 2 {
        println!(
            "need at least 2 runs to compare; only {} recorded",
            runs.len()
        );
        return Ok(());
    }
    let (current, previous) = (&runs[0], &runs[runs.len() - 1]);
    if !is_valid_git_sha(&previous.git_sha) || !is_valid_git_sha(&current.git_sha) {
        anyhow::bail!(
            "refusing to shell out to git diff with a malformed git_sha (previous={:?}, current={:?})",
            previous.git_sha,
            current.git_sha
        );
    }
    let previous_task_results = db.get_harness_eval_task_results(&previous.run_id).await?;
    let current_task_results = db.get_harness_eval_task_results(&current.run_id).await?;
    let current_events = db.get_model_selection_events(&current.run_id).await?;
    let previous_events = db.get_model_selection_events(&previous.run_id).await?;
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

    let flags = detect_regressions(
        previous,
        current,
        &previous_task_results,
        &current_task_results,
        &previous_events,
        &current_events,
        &changed_files,
    );
    match (
        mean_pass_at_k(&current_task_results, 1),
        mean_pass_at_k(&current_task_results, 3),
    ) {
        (Some(p1), Some(p3)) => println!("pass@1: {:.1}%  pass@3: {:.1}%", p1 * 100.0, p3 * 100.0),
        (Some(p1), None) => println!(
            "pass@1: {:.1}%  pass@3: insufficient data (no task sampled >= 3x)",
            p1 * 100.0
        ),
        _ => {}
    }
    if flags.is_empty() {
        println!(
            "no regressions detected between {} and {}",
            previous.run_id, current.run_id
        );
    } else {
        for flag in &flags {
            println!(
                "REGRESSION [{:?}]: {} (git {}..{}, {} file(s) changed)",
                flag.kind,
                flag.detail,
                flag.previous_git_sha,
                flag.current_git_sha,
                flag.changed_files.len()
            );
            for f in &flag.changed_files {
                println!("    {f}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(run_id: &str, pass_count: i64, task_count: i64) -> vox_db::HarnessEvalRunRecord {
        vox_db::HarnessEvalRunRecord {
            run_id: run_id.to_string(),
            triggered_by: "ci-nightly".to_string(),
            git_sha: format!("sha-{run_id}"),
            git_branch: "main".to_string(),
            changed_files: vec![],
            config_version: None,
            samples_per_task: 3,
            task_count,
            pass_count,
            fail_count: task_count - pass_count,
            skip_count: 0,
            total_cost_usd: 0.01,
            started_at_ms: 1000,
            finished_at_ms: 2000,
        }
    }

    fn event(
        model_id: &str,
        cost_tier: &str,
        privacy_gated: bool,
    ) -> vox_db::ModelSelectionEventRecord {
        vox_db::ModelSelectionEventRecord {
            run_id: "r".to_string(),
            task_id: "t".to_string(),
            model_id: model_id.to_string(),
            cost_tier: cost_tier.to_string(),
            selection_reason: "test".to_string(),
            was_privacy_gated: privacy_gated,
            recorded_at_ms: 1000,
        }
    }

    fn task_result(task_id: &str, status: &str) -> vox_db::HarnessEvalTaskResultRecord {
        vox_db::HarnessEvalTaskResultRecord {
            run_id: "r".to_string(),
            task_id: task_id.to_string(),
            category: "chat".to_string(),
            checker_kind: "deterministic".to_string(),
            status: status.to_string(),
            pass_samples: if status == "pass" { 3 } else { 0 },
            total_samples: 3,
            latency_p50_ms: Some(200),
            cost_usd: Some(0.0001),
            failure_detail: None,
            recorded_at_ms: 1000,
        }
    }

    #[test]
    fn pass_at_k_returns_none_when_k_exceeds_samples_drawn() {
        assert_eq!(pass_at_k(1, 1, 3), None);
        assert_eq!(pass_at_k(2, 0, 3), None);
    }

    #[test]
    fn pass_at_k_matches_naive_ratio_when_k_equals_1() {
        // pass@1 with n samples and c passes reduces to the plain success ratio.
        assert_eq!(pass_at_k(4, 2, 1), Some(0.5));
        assert_eq!(pass_at_k(5, 5, 1), Some(1.0));
        assert_eq!(pass_at_k(5, 0, 1), Some(0.0));
    }

    #[test]
    fn pass_at_k_is_one_when_failures_are_fewer_than_k() {
        // Only 1 failure exists among 5 samples, so every 3-sample draw includes a pass.
        assert_eq!(pass_at_k(5, 4, 3), Some(1.0));
    }

    #[test]
    fn pass_at_k_of_all_failures_is_zero() {
        assert_eq!(pass_at_k(5, 0, 3), Some(0.0));
    }

    #[test]
    fn mean_pass_at_k_excludes_undersampled_tasks_rather_than_scoring_them_zero() {
        let results = vec![
            {
                let mut t = task_result("t1", "pass");
                t.total_samples = 1;
                t.pass_samples = 1;
                t
            },
            {
                let mut t = task_result("t2", "pass");
                t.total_samples = 3;
                t.pass_samples = 3;
                t
            },
        ];
        // pass@3 only has one task with >= 3 samples (t2, always passes) -- t1 must be
        // excluded, not counted as a 0.
        assert_eq!(mean_pass_at_k(&results, 3), Some(1.0));
    }

    #[test]
    fn mean_pass_at_k_is_none_when_no_task_has_enough_samples() {
        let results = vec![{
            let mut t = task_result("t1", "pass");
            t.total_samples = 1;
            t.pass_samples = 1;
            t
        }];
        assert_eq!(mean_pass_at_k(&results, 3), None);
    }

    #[test]
    fn no_regression_when_pass_rate_and_ratio_are_stable() {
        let prev = run("r1", 9, 10);
        let cur = run("r2", 9, 10);
        let prev_events = vec![event("m1", "free", false); 5];
        let cur_events = vec![event("m1", "free", false); 5];
        let flags = detect_regressions(&prev, &cur, &[], &[], &prev_events, &cur_events, &[]);
        assert!(flags.is_empty());
    }

    #[test]
    fn pass_rate_drop_beyond_threshold_is_flagged() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 5, 10); // 100% -> 50%, a 50pp drop
        let flags =
            detect_regressions(&prev, &cur, &[], &[], &[], &[], &["src/foo.rs".to_string()]);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].kind, RegressionKind::PassRateDrop);
        assert_eq!(flags[0].changed_files, vec!["src/foo.rs".to_string()]);
    }

    #[test]
    fn small_pass_rate_drop_under_threshold_is_not_flagged() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 9, 10); // 100% -> 90%, a 10pp drop, not > threshold
        let flags = detect_regressions(&prev, &cur, &[], &[], &[], &[], &[]);
        assert!(flags.is_empty());
    }

    #[test]
    fn cost_tier_ratio_drop_beyond_threshold_is_flagged() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 10, 10);
        let prev_events = vec![event("m1", "free", false); 10];
        let cur_events = vec![event("m1", "premium", false); 10]; // 100% -> 0% free/cheap
        let flags = detect_regressions(&prev, &cur, &[], &[], &prev_events, &cur_events, &[]);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].kind, RegressionKind::CostTierRatioDrop);
    }

    #[test]
    fn privacy_gated_events_are_excluded_from_the_ratio_calculation() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 10, 10);
        // All events are privacy-gated (forced local); tier drift among them must not count.
        let prev_events = vec![event("local-1", "free", true); 5];
        let cur_events = vec![event("local-1", "premium", true); 5];
        let flags = detect_regressions(&prev, &cur, &[], &[], &prev_events, &cur_events, &[]);
        assert!(
            flags.is_empty(),
            "privacy-forced selections must not affect the free/cheap ratio regression check"
        );
    }

    #[test]
    fn both_regressions_can_be_flagged_simultaneously() {
        let prev = run("r1", 10, 10);
        let cur = run("r2", 5, 10);
        let prev_events = vec![event("m1", "free", false); 10];
        let cur_events = vec![event("m1", "premium", false); 10];
        let flags = detect_regressions(&prev, &cur, &[], &[], &prev_events, &cur_events, &[]);
        assert_eq!(flags.len(), 2);
    }

    #[test]
    fn single_task_flip_to_fail_is_flagged_even_when_aggregate_pass_rate_is_unchanged() {
        // Two tasks each run; one flips pass->fail while a different one flips fail->pass in
        // the same run — aggregate pass_count stays identical (9/10 both runs), so the
        // aggregate PassRateDrop check alone would see nothing. TaskFlippedToFail must still
        // catch the real regression on task-a.
        let prev = run("r1", 9, 10);
        let cur = run("r2", 9, 10);
        let prev_results = vec![task_result("task-a", "pass"), task_result("task-b", "fail")];
        let cur_results = vec![task_result("task-a", "fail"), task_result("task-b", "pass")];
        let flags = detect_regressions(&prev, &cur, &prev_results, &cur_results, &[], &[], &[]);
        let flip_flags: Vec<_> = flags
            .iter()
            .filter(|f| f.kind == RegressionKind::TaskFlippedToFail)
            .collect();
        assert_eq!(flip_flags.len(), 1);
        assert_eq!(flip_flags[0].flipped_task_ids, vec!["task-a".to_string()]);
    }
}
