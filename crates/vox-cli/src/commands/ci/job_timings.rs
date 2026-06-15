//! `vox ci job-timings` — measure and surface CI **job run time** and warn on
//! anything slower than the budget (default 10 min).
//!
//! "Run time" is `completed_at - started_at` from the GitHub Actions jobs API —
//! i.e. the time a job spent **executing on a runner**, excluding the queue /
//! startup wait (`started_at` is stamped when the runner picks the job up). Jobs
//! that never started (queued/skipped) are ignored, per spec.
//!
//! Designed to run both on-demand (`vox ci job-timings`) and automatically after
//! every CI run (see `.github/workflows/ci-timings.yml`, which passes
//! `--run-id` and `--annotate` so slow jobs surface as GitHub check annotations).

use anyhow::{Context, Result, anyhow};
use chrono::DateTime;
use serde::Deserialize;
use std::process::Command;

use super::constants::REPO_SLUG;

/// The budget: a CI job that *executes* longer than this is "too long" (10 min).
/// SSOT for the threshold — mirrored as `THRESHOLD_SECS` in `ci-timings.yml`.
pub const SLOW_JOB_THRESHOLD_SECS: i64 = 600;

#[derive(Debug, Deserialize)]
struct JobsResponse {
    jobs: Vec<JobRow>,
}

#[derive(Debug, Deserialize, Clone)]
struct JobRow {
    name: String,
    /// `None`/absent until the runner picks the job up.
    started_at: Option<String>,
    completed_at: Option<String>,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    run_id: Option<u64>,
}

/// A job's measured run time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobTiming {
    pub name: String,
    pub run_secs: i64,
    pub conclusion: String,
    pub run_id: u64,
}

// ---------------------------------------------------------------------------
// Pure logic (unit-tested)
// ---------------------------------------------------------------------------

/// Execution seconds between two RFC3339 stamps, or `None` if either is missing
/// or unparseable (a job that never started has no `started_at`).
pub fn run_seconds(started_at: Option<&str>, completed_at: Option<&str>) -> Option<i64> {
    let s = DateTime::parse_from_rfc3339(started_at?).ok()?;
    let c = DateTime::parse_from_rfc3339(completed_at?).ok()?;
    let secs = (c - s).num_seconds();
    (secs >= 0).then_some(secs)
}

/// True when a job's run time exceeds the budget.
pub fn is_slow(run_secs: i64, threshold_secs: i64) -> bool {
    run_secs > threshold_secs
}

fn fmt_dur(secs: i64) -> String {
    format!("{}m{:02}s", secs / 60, secs % 60)
}

// ---------------------------------------------------------------------------
// IO + reporting
// ---------------------------------------------------------------------------

fn gh_json(path: &str) -> Result<String> {
    // vox-arch-check: allow git-exec
    let out = Command::new("gh")
        .args(["api", path, "--paginate"])
        .output()
        .context("run gh (is the GitHub CLI installed and authenticated?)")?;
    if !out.status.success() {
        return Err(anyhow!(
            "gh api {path} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Parse one or more concatenated `{jobs: [...]}` pages (gh `--paginate` streams them).
fn parse_jobs(raw: &str) -> Vec<JobRow> {
    let mut out = Vec::new();
    let de = serde_json::Deserializer::from_str(raw).into_iter::<JobsResponse>();
    for page in de.flatten() {
        out.extend(page.jobs);
    }
    out
}

fn timings_from_rows(rows: &[JobRow]) -> Vec<JobTiming> {
    let mut t: Vec<JobTiming> = rows
        .iter()
        .filter_map(|j| {
            run_seconds(j.started_at.as_deref(), j.completed_at.as_deref()).map(|secs| JobTiming {
                name: j.name.clone(),
                run_secs: secs,
                conclusion: j.conclusion.clone().unwrap_or_else(|| "—".into()),
                run_id: j.run_id.unwrap_or(0),
            })
        })
        .collect();
    t.sort_by_key(|j| std::cmp::Reverse(j.run_secs));
    t
}

fn recent_completed_run_ids(limit: u32) -> Result<Vec<u64>> {
    let raw = gh_json(&format!(
        "repos/{REPO_SLUG}/actions/runs?status=completed&per_page={limit}"
    ))?;
    #[derive(Deserialize)]
    struct Runs {
        workflow_runs: Vec<RunId>,
    }
    #[derive(Deserialize)]
    struct RunId {
        id: u64,
    }
    let mut ids = Vec::new();
    for page in serde_json::Deserializer::from_str(&raw)
        .into_iter::<Runs>()
        .flatten()
    {
        ids.extend(page.workflow_runs.into_iter().map(|r| r.id));
    }
    ids.truncate(limit as usize);
    Ok(ids)
}

fn fetch_timings(run_id: Option<u64>, limit: u32) -> Result<Vec<JobTiming>> {
    let ids = match run_id {
        Some(id) => vec![id],
        None => recent_completed_run_ids(limit)?,
    };
    let mut rows = Vec::new();
    for id in ids {
        let raw = gh_json(&format!("repos/{REPO_SLUG}/actions/runs/{id}/jobs"))?;
        rows.extend(parse_jobs(&raw));
    }
    Ok(timings_from_rows(&rows))
}

/// `vox ci job-timings`.
pub fn run(
    run_id: Option<u64>,
    threshold_mins: Option<i64>,
    limit: u32,
    json: bool,
    annotate: bool,
    strict: bool,
) -> Result<()> {
    let threshold = threshold_mins.map_or(SLOW_JOB_THRESHOLD_SECS, |m| m * 60);
    let timings = fetch_timings(run_id, limit)?;
    let slow: Vec<&JobTiming> = timings
        .iter()
        .filter(|t| is_slow(t.run_secs, threshold))
        .collect();

    if json {
        let rows: Vec<_> = timings
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "run_secs": t.run_secs,
                    "slow": is_slow(t.run_secs, threshold),
                    "conclusion": t.conclusion,
                    "run_id": t.run_id,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!(
            "job-timings: {} job(s) measured, budget {}m — {} over budget",
            timings.len(),
            threshold / 60,
            slow.len()
        );
        for t in timings.iter().take(20) {
            let flag = if is_slow(t.run_secs, threshold) {
                "  ⚠ OVER"
            } else {
                ""
            };
            println!(
                "  {:>8}  {} [{}]{flag}",
                fmt_dur(t.run_secs),
                t.name,
                t.conclusion
            );
        }
    }

    // GitHub check annotations so slow jobs surface in the PR UI.
    if annotate {
        for t in &slow {
            println!(
                "::warning title=Slow CI job::'{}' ran {} (> {}m budget) — consider moving it to merge-only or splitting it.",
                t.name,
                fmt_dur(t.run_secs),
                threshold / 60
            );
        }
    }

    if !slow.is_empty() {
        eprintln!(
            "warning: {} CI job(s) exceeded the {}-minute run-time budget.",
            slow.len(),
            threshold / 60
        );
        if strict {
            return Err(anyhow!(
                "{} job(s) over the {}-minute budget (strict mode)",
                slow.len(),
                threshold / 60
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_seconds_is_execution_time_excluding_queue() {
        // started→completed spans 12 minutes of EXECUTION.
        let s = run_seconds(Some("2026-06-06T01:00:00Z"), Some("2026-06-06T01:12:00Z"));
        assert_eq!(s, Some(720));
    }

    #[test]
    fn run_seconds_none_when_not_started() {
        // A queued/skipped job has no started_at → ignored.
        assert_eq!(run_seconds(None, Some("2026-06-06T01:12:00Z")), None);
        assert_eq!(run_seconds(Some("2026-06-06T01:00:00Z"), None), None);
        assert_eq!(run_seconds(None, None), None);
    }

    #[test]
    fn run_seconds_rejects_negative_and_garbage() {
        assert_eq!(
            run_seconds(Some("2026-06-06T01:12:00Z"), Some("2026-06-06T01:00:00Z")),
            None
        );
        assert_eq!(
            run_seconds(Some("not-a-date"), Some("2026-06-06T01:00:00Z")),
            None
        );
    }

    #[test]
    fn slow_is_strictly_over_budget() {
        assert!(!is_slow(600, 600)); // exactly 10m is not "over"
        assert!(is_slow(601, 600));
        assert!(!is_slow(120, 600));
    }

    #[test]
    fn timings_sorted_desc_and_skip_unstarted() {
        let rows = vec![
            JobRow {
                name: "fast".into(),
                started_at: Some("2026-06-06T01:00:00Z".into()),
                completed_at: Some("2026-06-06T01:02:00Z".into()),
                conclusion: Some("success".into()),
                run_id: Some(1),
            },
            JobRow {
                name: "slow".into(),
                started_at: Some("2026-06-06T01:00:00Z".into()),
                completed_at: Some("2026-06-06T01:25:00Z".into()),
                conclusion: Some("success".into()),
                run_id: Some(1),
            },
            JobRow {
                name: "queued".into(),
                started_at: None,
                completed_at: None,
                conclusion: None,
                run_id: Some(1),
            },
        ];
        let t = timings_from_rows(&rows);
        assert_eq!(t.len(), 2); // queued job dropped
        assert_eq!(t[0].name, "slow"); // sorted desc
        assert_eq!(t[0].run_secs, 1500);
    }
}
