//! Host-side detection of self-hosted CI runner containers that exit
//! unexpectedly while their assigned GitHub Actions job is still
//! `in_progress` — i.e. neither a normal ephemeral job-complete exit, nor a
//! memcg OOM-kill (see `oom_watch.rs` for that separate, higher-confidence
//! signal). Reports via the same PR-comment mechanism `oom_watch.rs` uses.
//!
//! Design: docs/superpowers/specs/2026-07-22-ci-runner-death-visibility-and-reap-hardening-design.md
//!
//! Top-level entrypoint: [`scan_and_report_unexpected_exits`], called from
//! the autoscaler's `--apply` tick in `runner_scale.rs`, after the OOM scan
//! (so an OOM-claimed container is never also reported here) and BEFORE
//! `run_scale`'s own exited-container cleanup (so `docker inspect` can still
//! read the container's exit code before it's pruned).

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;

use super::oom_watch::{JobRow, find_matching_job, post_pr_comment};
use super::runner_scale::quiet_command;

// --- seen-list persistence (mirrors oom_watch's seen-list exactly) ---------

fn unexpected_exit_seen_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-runner-unexpected-exit-seen.json")
}

fn read_seen() -> Vec<String> {
    std::fs::read_to_string(unexpected_exit_seen_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_seen(seen: &[String]) {
    let p = unexpected_exit_seen_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(seen) {
        Ok(s) => {
            if let Err(e) = std::fs::write(&p, s) {
                eprintln!(
                    "runner-scale: failed to write unexpected-exit seen-state to {}: {e:#} — a \
                     duplicate PR comment may follow next tick since this event couldn't be \
                     persisted as seen",
                    p.display()
                );
            }
        }
        Err(e) => {
            eprintln!(
                "runner-scale: failed to serialize unexpected-exit seen-state for {}: {e:#} — a \
                 duplicate PR comment may follow next tick since this event couldn't be \
                 persisted as seen",
                p.display()
            );
        }
    }
}

const SEEN_MAX: usize = 500;

/// Append newly-seen container names, capped to `SEEN_MAX` most-recent
/// entries. Pure. Mirrors `oom_watch::append_seen` exactly.
pub fn append_seen(mut seen: Vec<String>, newly_seen: &[String]) -> Vec<String> {
    seen.extend(newly_seen.iter().cloned());
    if seen.len() > SEEN_MAX {
        let drop = seen.len() - SEEN_MAX;
        seen.drain(0..drop);
    }
    seen
}

// --- running-container tracking (new state, not shared with oom_watch) ----

fn running_seen_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-runner-running-seen.json")
}

fn read_running_seen() -> HashSet<String> {
    std::fs::read_to_string(running_seen_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_running_seen(names: &HashSet<String>) {
    let p = running_seen_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(names) {
        Ok(s) => {
            if let Err(e) = std::fs::write(&p, s) {
                eprintln!(
                    "runner-scale: failed to write running-container snapshot to {}: {e:#} — \
                     next tick's running→exited diff may be computed against a stale snapshot",
                    p.display()
                );
            }
        }
        Err(e) => {
            eprintln!(
                "runner-scale: failed to serialize running-container snapshot for {}: {e:#} — \
                 next tick's running→exited diff may be computed against a stale snapshot",
                p.display()
            );
        }
    }
}

/// Container names present in `prev_running` but absent from
/// `curr_running` — i.e. they transitioned running→exited (or vanished)
/// since the prior tick. Pure.
pub fn newly_exited(prev_running: &HashSet<String>, curr_running: &HashSet<String>) -> Vec<String> {
    prev_running
        .iter()
        .filter(|name| !curr_running.contains(name.as_str()))
        .cloned()
        .collect()
}

// --- comment composition ----------------------------------------------------

/// Build the PR comment body for one detected unexpected exit. Pure.
///
/// Deliberately does NOT attempt a same-tick "here's why" diagnosis — the
/// exited-container scan runs before any reap-hardening decision is made in
/// the same tick (no correlating evidence can exist yet), and even if it
/// could, that wouldn't establish causality cleanly. Both known hypotheses
/// are stated neutrally instead.
pub fn unexpected_exit_comment_body(
    container_name: &str,
    job_name: &str,
    run_id: u64,
    exit_code: i64,
) -> String {
    format!(
        "**CI runner exited unexpectedly** — job `{job_name}` (run `{run_id}`) did not \
         complete or get cancelled normally: its runner container `{container_name}` exited \
         (code `{exit_code}`) while the job was still `in_progress`. This was not a memcg \
         OOM-kill (see the separate OOM-visibility check).\n\n\
         Two known causes: (1) GitHub's runners-API `busy` flag briefly lagging a runner that \
         just started a job, or (2) an external cause (WSL2 VM memory pressure, a Docker \
         daemon hiccup) unrelated to the autoscaler's own decisions. This detector cannot \
         distinguish between them from a single event; if this recurs, check whether it \
         correlates with autoscaler reap activity around the same timestamp.\n\n\
         Auto-detected by the host-side runner autoscaler (`vox ci runner-scale`)."
    )
}

// --- container exit-code lookup --------------------------------------------

/// `docker inspect --format '{{.State.ExitCode}}' <name>` — must be called
/// BEFORE `run_scale`'s own exited-container cleanup removes the container.
/// Returns `None` if the container is already gone (cleanup raced ahead) or
/// the exit code couldn't be parsed.
pub fn inspect_exit_code(container_name: &str) -> Option<i64> {
    let out = quiet_command("docker")
        .args(["inspect", "--format", "{{.State.ExitCode}}", container_name])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

// --- orchestration -----------------------------------------------------------

/// Scan for containers that transitioned running→exited since the last tick,
/// correlate each against a fresh jobs-API lookup, skip anything already
/// claimed this tick by the OOM scan, and post a comment for genuine
/// unexpected exits. Returns the count of successfully reported events.
///
/// `already_oom_claimed`: container names the OOM scan already reported this
/// same tick (passed in from `run_scale`, which calls both scanners in the
/// same tick) — never double-report the same death under two framings.
/// `job_rows`: the tick's shared jobs-API fetch — this function never
/// fetches its own copy.
/// Cap on how many fresh unexpected-exit events get correlated (and potentially posted) in
/// a single tick. Mirrors `oom_watch::OOM_FAN_OUT_MAX`: a storm of exits (e.g. many runners
/// dying at once from a Docker daemon restart) shouldn't fan out into dozens of sequential
/// `gh api` calls inside one already-busy, time-boxed tick (the host's Task Scheduler entry
/// kills the whole process past 2 minutes). Events past the cap are left unmarked-seen and
/// simply get picked up on a later tick — nothing is dropped, just spread out.
const UNEXPECTED_EXIT_FAN_OUT_MAX: usize = 5;

pub fn scan_and_report_unexpected_exits(
    curr_running: &HashSet<String>,
    already_oom_claimed: &HashSet<String>,
    job_rows: &[JobRow],
) -> Result<u32> {
    let prev_running = read_running_seen();
    let exited = newly_exited(&prev_running, curr_running);
    write_running_seen(curr_running);

    if exited.is_empty() {
        return Ok(0);
    }

    let seen = read_seen();
    let fresh: Vec<&String> = exited
        .iter()
        .filter(|name| !seen.iter().any(|s| s == *name))
        .filter(|name| !already_oom_claimed.contains(name.as_str()))
        .take(UNEXPECTED_EXIT_FAN_OUT_MAX)
        .collect();
    if fresh.is_empty() {
        return Ok(0);
    }

    let mut reported = 0u32;
    let mut seen_so_far = seen;
    for name in fresh {
        let Some(job) = find_matching_job(job_rows, name) else {
            eprintln!(
                "runner-scale: unexpected-exit on {name} — no PR-triggered in_progress job \
                 match found; likely already completed/cancelled normally, not reporting"
            );
            continue;
        };
        let exit_code = inspect_exit_code(name).unwrap_or(-1);
        let body = unexpected_exit_comment_body(name, &job.job_name, job.run_id, exit_code);
        match post_pr_comment(job.pr_number, &body, false) {
            Ok(()) => {
                reported += 1;
                seen_so_far = append_seen(seen_so_far, std::slice::from_ref(name));
                write_seen(&seen_so_far);
                println!(
                    "runner-scale: unexpected-exit reported for {name} (job={}, run={}, exit_code={exit_code})",
                    job.job_name, job.run_id
                );
            }
            Err(e) => {
                eprintln!("runner-scale: unexpected-exit comment post failed (will retry next tick): {e:#}")
            }
        }
    }
    Ok(reported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newly_exited_finds_names_present_before_but_not_now() {
        let prev: HashSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let curr: HashSet<String> = ["a", "c"].iter().map(|s| s.to_string()).collect();
        let mut result = newly_exited(&prev, &curr);
        result.sort();
        assert_eq!(result, vec!["b".to_string()]);
    }

    #[test]
    fn newly_exited_empty_when_nothing_changed() {
        let prev: HashSet<String> = ["a"].iter().map(|s| s.to_string()).collect();
        let curr: HashSet<String> = ["a"].iter().map(|s| s.to_string()).collect();
        assert!(newly_exited(&prev, &curr).is_empty());
    }

    #[test]
    fn append_seen_caps_to_max_dropping_oldest_first() {
        let seen: Vec<String> = (0..SEEN_MAX).map(|i| format!("old-{i}")).collect();
        let updated = append_seen(seen, &["new-1".to_string()]);
        assert_eq!(updated.len(), SEEN_MAX);
        assert!(!updated.contains(&"old-0".to_string()));
        assert!(updated.contains(&"new-1".to_string()));
    }

    #[test]
    fn comment_body_names_job_container_and_exit_code() {
        let body = unexpected_exit_comment_body("vox-runner-auto-abc-0", "docs-quality", 999, 143);
        assert!(body.contains("docs-quality"));
        assert!(body.contains("vox-runner-auto-abc-0"));
        assert!(body.contains("999"));
        assert!(body.contains("143"));
    }

    #[test]
    fn comment_body_states_both_hypotheses_neutrally() {
        let body = unexpected_exit_comment_body("vox-runner-auto-abc-0", "docs-quality", 999, 143);
        assert!(body.contains("busy") && body.contains("lag"));
        assert!(body.contains("external cause"));
        assert!(body.contains("cannot distinguish"));
    }

    #[test]
    fn full_pipeline_finds_correlates_and_composes_a_correct_comment() {
        let prev_running: HashSet<String> = ["vox-runner-auto-abc123-0", "vox-runner-auto-zzz-9"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let curr_running: HashSet<String> = ["vox-runner-auto-zzz-9"].iter().map(|s| s.to_string()).collect();
        let job_rows = vec![JobRow {
            runner_name: "vox-runner-auto-abc123-0".to_string(),
            job_name: "Lints (clippy + rustdoc)".to_string(),
            run_id: 28861698905,
            pr_number: 440,
        }];

        // 1. find: which container transitioned running -> exited.
        let exited = newly_exited(&prev_running, &curr_running);
        assert_eq!(exited, vec!["vox-runner-auto-abc123-0".to_string()]);

        // 2. correlate: container name -> job row.
        let matched = find_matching_job(&job_rows, &exited[0]).expect("job matched");
        assert_eq!(matched.job_name, "Lints (clippy + rustdoc)");

        // 3. compose: final comment body is correct.
        let body = unexpected_exit_comment_body(&exited[0], &matched.job_name, matched.run_id, 137);
        assert!(body.contains("Lints (clippy + rustdoc)"));
        assert!(body.contains("28861698905"));
        assert!(body.contains("vox-runner-auto-abc123-0"));
    }
}
