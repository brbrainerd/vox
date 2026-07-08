//! Host-side detection of self-hosted CI runner containers hard-killed by
//! their own memory cgroup limit — and reporting that evidence directly on
//! the PR/run that was affected.
//!
//! Two things this exists to work around (see the design doc for the
//! evidence): a runner container cannot read `dmesg` itself (no `CAP_SYSLOG`
//! by default, correctly so), and a job that gets OOM-killed cannot run its
//! own `if: always()` report step (the runner agent process dies with the
//! container — there is no "after" for that same job). So detection and
//! reporting both live here, on the host-side autoscaler tick
//! (`vox ci runner-scale`, invoked every 2 minutes), not inside the job.
//!
//! Design: docs/superpowers/specs/2026-07-07-ci-runner-memory-budget-and-oom-visibility-design.md
//!
//! **Currently implemented in this file:** dmesg-line parsing
//! (`parse_oom_events`), dedup-persistence primitives (`read_oom_seen`/
//! `write_oom_seen`/`new_events`/`append_seen`), container-name resolution
//! (`parse_container_names`/`fetch_recent_container_events`), GitHub job/run
//! correlation (`find_matching_job`/`find_run_for_runner`), and PR comment
//! composition/posting (`oom_comment_body`/`post_pr_comment`) — all pure or
//! thin IO. The top-level orchestration entrypoint that chains these
//! together and is called from the autoscaler tick still lands in a
//! follow-up task of the implementation plan; nothing in this module is
//! called from a live code path yet (the module itself isn't even wired into
//! `mod.rs` as `pub` — that happens once that orchestration entrypoint
//! exists).

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use regex::Regex;

use super::constants::REPO_SLUG;
use super::runner_scale::{MANAGED_PREFIX, gh_json, quiet_command};

/// One parsed `oom-kill:constraint=CONSTRAINT_MEMCG` kernel log line — the
/// single `dmesg` line that carries both the killed process name and the
/// container's full cgroup id together (`oom_memcg=/docker/<id>,...,task=<name>`).
///
/// `#[allow(dead_code)]`: not yet constructed outside `#[cfg(test)]` — the
/// dedup-persistence task later in the implementation plan is the first
/// caller. Remove this allow once that task lands and wires it up.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OomEvent {
    /// Raw matched line, used as the dedup key (content-based, not
    /// timestamp-based — avoids parsing dmesg's locale-dependent date format).
    pub raw_line: String,
    /// Killed process name, e.g. "rustdoc".
    pub process: String,
    /// Full 64-char docker container/cgroup id.
    pub cgroup_id: String,
}

/// `#[allow(dead_code)]`: only called from `parse_oom_events` today, which
/// is itself only called from tests until the orchestration task
/// (`scan_and_report_oom_events`) lands later in the implementation plan.
#[allow(dead_code)]
fn oom_line_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"oom_memcg=/docker/([0-9a-f]{64}).*?task=([^,]+)")
            .expect("static oom-kill regex must compile")
    })
}

/// Parse `dmesg` output for `oom-kill:constraint=CONSTRAINT_MEMCG` lines,
/// extracting the killed process name and the container's cgroup id from
/// each. Lines that don't match (the overwhelming majority of `dmesg`) are
/// skipped. Pure — no IO.
///
/// `#[allow(dead_code)]`: only called from tests today — the orchestration
/// task (`scan_and_report_oom_events`) later in the implementation plan is
/// the first non-test caller. Remove this allow once that task lands.
#[allow(dead_code)]
pub fn parse_oom_events(dmesg_text: &str) -> Vec<OomEvent> {
    let re = oom_line_regex();
    dmesg_text
        .lines()
        .filter(|l| l.contains("oom-kill:constraint=CONSTRAINT_MEMCG"))
        .filter_map(|line| {
            let caps = re.captures(line)?;
            Some(OomEvent {
                raw_line: line.to_string(),
                cgroup_id: caps.get(1)?.as_str().to_string(),
                process: caps.get(2)?.as_str().to_string(),
            })
        })
        .collect()
}

// --- dedup persistence across ticks -----------------------------------

/// `#[allow(dead_code)]`: not called from anywhere yet, including tests —
/// the file-IO round-trip this and its two siblings below perform isn't unit
/// tested (matches this file's other thin, side-effecting IO wrappers, e.g.
/// `fetch_recent_container_events` once that lands). The orchestration task
/// (`scan_and_report_oom_events`) later in the implementation plan is the
/// first caller. Remove this allow once that task lands.
#[allow(dead_code)]
fn oom_seen_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-runner-oom-seen.json")
}

/// `#[allow(dead_code)]`: not called from anywhere yet, including tests —
/// see [`oom_seen_path`]. The orchestration task
/// (`scan_and_report_oom_events`) later in the implementation plan is the
/// first caller. Remove this allow once that task lands.
#[allow(dead_code)]
fn read_oom_seen() -> Vec<String> {
    std::fs::read_to_string(oom_seen_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// `#[allow(dead_code)]`: not called from anywhere yet, including tests —
/// see [`oom_seen_path`]. The orchestration task
/// (`scan_and_report_oom_events`) later in the implementation plan is the
/// first caller. Remove this allow once that task lands.
#[allow(dead_code)]
fn write_oom_seen(seen: &[String]) {
    let p = oom_seen_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(seen) {
        let _ = std::fs::write(p, s);
    }
}

/// Events from `events` whose raw line isn't already in `seen`. Pure.
///
/// `#[allow(dead_code)]`: only called from tests today — the orchestration
/// task (`scan_and_report_oom_events`) later in the implementation plan is
/// the first non-test caller. Remove this allow once that task lands.
#[allow(dead_code)]
pub fn new_events<'a>(events: &'a [OomEvent], seen: &[String]) -> Vec<&'a OomEvent> {
    events
        .iter()
        .filter(|e| !seen.iter().any(|s| s == &e.raw_line))
        .collect()
}

/// Cap on the seen-list so its state file never grows unbounded.
///
/// `#[allow(dead_code)]`: only referenced from tests today — the
/// orchestration task (`scan_and_report_oom_events`) later in the
/// implementation plan is the first non-test use. Remove this allow once
/// that task lands.
#[allow(dead_code)]
const OOM_SEEN_MAX: usize = 500;

/// Append newly-seen raw lines to the existing seen-list, capped to
/// [`OOM_SEEN_MAX`] most-recent entries (oldest dropped first). Pure.
///
/// `#[allow(dead_code)]`: only called from tests today — the orchestration
/// task (`scan_and_report_oom_events`) later in the implementation plan is
/// the first non-test caller. Remove this allow once that task lands.
#[allow(dead_code)]
pub fn append_seen(mut seen: Vec<String>, newly_seen: &[String]) -> Vec<String> {
    seen.extend(newly_seen.iter().cloned());
    if seen.len() > OOM_SEEN_MAX {
        let drop = seen.len() - OOM_SEEN_MAX;
        seen.drain(0..drop);
    }
    seen
}

// --- container name resolution ------------------------------------------

/// `#[allow(dead_code)]`: only called from `parse_container_names` today,
/// which is itself not yet called outside `#[cfg(test)]` — the orchestration
/// task (`scan_and_report_oom_events`) later in the implementation plan is
/// the first non-test caller. Remove this allow once that task lands.
#[allow(dead_code)]
fn container_event_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"container \S+ ([0-9a-f]{64}) \(.*?name=([^,)]+)")
            .expect("static container-event regex must compile")
    })
}

/// Parse `docker events` text into an id→name map, restricted to managed
/// runner containers (`MANAGED_PREFIX`). Covers already-destroyed containers
/// too (the whole point — an OOM-killed runner is gone by the time the next
/// tick polls), since `docker events` is a historical log, not a live query.
/// Pure — no IO.
///
/// `#[allow(dead_code)]`: only called from tests today — the orchestration
/// task (`scan_and_report_oom_events`) later in the implementation plan is
/// the first non-test caller. Remove this allow once that task lands.
#[allow(dead_code)]
pub fn parse_container_names(events_text: &str) -> HashMap<String, String> {
    let re = container_event_regex();
    let mut map = HashMap::new();
    for line in events_text.lines() {
        let Some(caps) = re.captures(line) else {
            continue;
        };
        let (Some(id), Some(name)) = (caps.get(1), caps.get(2)) else {
            continue;
        };
        let name = name.as_str();
        if name.starts_with(MANAGED_PREFIX) {
            map.insert(id.as_str().to_string(), name.to_string());
        }
    }
    map
}

/// Window (seconds) of `docker events` history to fetch when resolving a
/// cgroup id to a container name. Comfortably covers the 2-minute autoscaler
/// tick cadence with margin for a slow tick.
///
/// `#[allow(dead_code)]`: only referenced from [`fetch_recent_container_events`]
/// today, which is itself not yet called outside `#[cfg(test)]` — the
/// orchestration task (`scan_and_report_oom_events`) later in the
/// implementation plan is the first non-test caller. Remove this allow once
/// that task lands.
#[allow(dead_code)]
const OOM_EVENTS_WINDOW_SECS: i64 = 600;

/// Fetch `docker events` for the last [`OOM_EVENTS_WINDOW_SECS`], bounded by
/// `--since`/`--until` (both unix seconds) so this returns immediately rather
/// than streaming.
///
/// `#[allow(dead_code)]`: not yet called outside `#[cfg(test)]` — the
/// orchestration task (`scan_and_report_oom_events`) later in the
/// implementation plan is the first caller. Remove this allow once that task
/// lands.
#[allow(dead_code)]
fn fetch_recent_container_events(now: i64) -> Result<String> {
    let since = (now - OOM_EVENTS_WINDOW_SECS).to_string();
    let until = now.to_string();
    let out = quiet_command("docker")
        .args([
            "events",
            "--since",
            &since,
            "--until",
            &until,
            "--filter",
            "type=container",
        ])
        .output()
        .context("run docker events")?;
    if !out.status.success() {
        return Err(anyhow!(
            "docker events failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

// --- GitHub job/run correlation ------------------------------------------

/// One (runner_name, job_name) row from a workflow run's jobs list. Pure —
/// testable without a live `gh` call, mirroring how `runner_scale::runner_rows`
/// separates the tab-parsing shape from the `gh api` call that produces it.
pub fn find_matching_job<'a>(
    job_rows: &'a [(String, String)],
    runner_name: &str,
) -> Option<&'a str> {
    job_rows
        .iter()
        .find(|(rn, _)| rn == runner_name)
        .map(|(_, name)| name.as_str())
}

/// A workflow run this OOM event corresponds to: run id, originating PR
/// number, and the job name that was executing on the killed runner.
///
/// `#[allow(dead_code)]`: not yet constructed outside `#[cfg(test)]`'s
/// callers — the orchestration task (`scan_and_report_oom_events`) later in
/// the implementation plan is the first non-test caller. Remove this allow
/// once that task lands.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerJobMatch {
    pub run_id: u64,
    pub pr_number: u64,
    pub job_name: String,
}

/// Cap on recent runs inspected per status when correlating a runner name to
/// a job — mirrors `runner_scale::DEMAND_RUNS_PER_STATUS`.
///
/// `#[allow(dead_code)]`: only referenced from [`find_run_for_runner`]
/// today, which is itself not yet called outside `#[cfg(test)]` — the
/// orchestration task (`scan_and_report_oom_events`) later in the
/// implementation plan is the first non-test caller. Remove this allow once
/// that task lands.
#[allow(dead_code)]
const CORRELATE_RUNS_PER_STATUS: u32 = 20;

/// Find which job (if any) was assigned to `runner_name`, scanning recent
/// in_progress then completed runs. GitHub Actions job objects expose a
/// `runner_name` field once a job is assigned to a runner.
///
/// `#[allow(dead_code)]`: not yet called outside `#[cfg(test)]` — the
/// orchestration task (`scan_and_report_oom_events`) later in the
/// implementation plan is the first caller. Remove this allow once that task
/// lands.
#[allow(dead_code)]
fn find_run_for_runner(runner_name: &str) -> Result<Option<RunnerJobMatch>> {
    for status in ["in_progress", "completed"] {
        let runs = gh_json(&[
            "api",
            &format!(
                "repos/{REPO_SLUG}/actions/runs?status={status}&per_page={CORRELATE_RUNS_PER_STATUS}"
            ),
            "--jq",
            r#".workflow_runs[]|[.id, (.pull_requests[0].number // 0)]|@tsv"#,
        ])?;
        for line in runs.lines() {
            let mut parts = line.split('\t');
            let (Some(run_id_str), Some(pr_str)) = (parts.next(), parts.next()) else {
                continue;
            };
            let (Ok(run_id), Ok(pr_number)) = (run_id_str.parse::<u64>(), pr_str.parse::<u64>())
            else {
                continue;
            };
            if pr_number == 0 {
                continue; // not a PR-triggered run — no PR to comment on
            }
            let job_raw = gh_json(&[
                "api",
                &format!("repos/{REPO_SLUG}/actions/runs/{run_id}/jobs?per_page=100"),
                "--jq",
                r#".jobs[]|select(.runner_name != null)|[.runner_name, .name]|@tsv"#,
            ])?;
            let job_rows: Vec<(String, String)> = job_raw
                .lines()
                .filter_map(|l| {
                    let mut p = l.split('\t');
                    Some((p.next()?.to_string(), p.next()?.to_string()))
                })
                .collect();
            if let Some(job_name) = find_matching_job(&job_rows, runner_name) {
                return Ok(Some(RunnerJobMatch {
                    run_id,
                    pr_number,
                    job_name: job_name.to_string(),
                }));
            }
        }
    }
    Ok(None)
}

// --- PR comment composition and posting ----------------------------------

fn backtick_run_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"`{3,}").expect("static backtick-run regex must compile"))
}

/// Neutralize every run of 3+ backticks so an embedded raw dmesg line can
/// never break out of the surrounding markdown code fence in
/// [`oom_comment_body`]. dmesg content is kernel-generated, not attacker
/// input, but a process name containing backticks (however unlikely)
/// shouldn't be able to garble the comment. Pure.
///
/// A single non-overlapping `str::replace("```", ...)` is not sufficient
/// here: replacing the first three backticks of a 4+ backtick run leaves the
/// remaining backtick(s) free to rejoin the inserted characters into a fresh
/// ` ``` ` (e.g. `"````".replace("```", "`\u{200b}``")` still contains
/// `"```"`). Instead, match each maximal run of 3+ backticks with a regex and
/// insert a zero-width space after every backtick in the matched run, so no
/// 3 consecutive backticks can ever survive.
///
/// `#[allow(dead_code)]`: only called from `oom_comment_body` and tests today
/// — the orchestration task (`scan_and_report_oom_events`) later in the
/// implementation plan is the first non-test caller. Remove this allow once
/// that task lands.
#[allow(dead_code)]
fn escape_for_code_fence(s: &str) -> String {
    backtick_run_regex()
        .replace_all(s, |caps: &regex::Captures| {
            caps[0]
                .chars()
                .map(|c| format!("{c}\u{200b}"))
                .collect::<String>()
        })
        .into_owned()
}

/// Build the PR comment body for one detected OOM kill. Pure — testable
/// without a live `gh` call.
///
/// `#[allow(dead_code)]`: only called from tests today — the orchestration
/// task (`scan_and_report_oom_events`) later in the implementation plan is
/// the first non-test caller. Remove this allow once that task lands.
#[allow(dead_code)]
pub fn oom_comment_body(event: &OomEvent, job_name: &str, run_id: u64) -> String {
    format!(
        "**CI runner OOM-killed** — job `{job_name}` (run `{run_id}`) did not fail \
         normally: its runner container's process `{}` was killed by the kernel's \
         per-container memory cgroup limit, not a real `timeout-minutes` cutoff or an \
         external cancellation.\n\n\
         Evidence (`dmesg`):\n```\n{}\n```\n\n\
         Auto-detected by the host-side runner autoscaler (`vox ci runner-scale`) — \
         no action needed unless this recurs after a `MEM_PER_RUNNER` bump.",
        event.process,
        escape_for_code_fence(&event.raw_line)
    )
}

/// Post `body` as a comment on PR `pr_number`. No-op (prints instead) when
/// `dry_run` — mirrors this command's existing `--apply`-gated mutation
/// pattern (`reap`, `deregister` etc. in `runner_scale.rs`).
#[allow(dead_code)]
fn post_pr_comment(pr_number: u64, body: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("[dry-run] would comment on PR #{pr_number}:\n{body}");
        return Ok(());
    }
    gh_json(&[
        "api",
        "-X",
        "POST",
        &format!("repos/{REPO_SLUG}/issues/{pr_number}/comments"),
        "-f",
        &format!("body={body}"),
    ])?;
    Ok(())
}

// --- orchestration ---------------------------------------------------------

/// Cap on how many fresh OOM events get correlated (and potentially posted) in a
/// single tick. An OOM storm shouldn't fan out into dozens of sequential `gh api`
/// calls inside one already-busy, time-boxed tick (the host's Task Scheduler entry
/// kills the whole process past 2 minutes). Events past the cap are left
/// unmarked-seen and simply get picked up on a later tick — nothing is dropped,
/// just spread out.
const OOM_FAN_OUT_MAX: usize = 5;

/// Scan for new OOM-kill events since the last tick, correlate each to the PR/run
/// that was executing on the killed container, and post a comment there.
/// Best-effort per event: an event is marked "seen" (so it's never retried) **only**
/// after a successful post — a transient correlation failure (job not visible in the
/// API yet, `gh` briefly unavailable) leaves it unmarked so it's retried next tick,
/// rather than permanently and silently dropped. Persisted immediately after each
/// successful post rather than batched at the end, so a process kill mid-tick (the
/// host's Task Scheduler entry force-stops a tick past 2 minutes) can't cause the
/// next tick to re-post a duplicate for an event that already succeeded. Returns the
/// count of successfully reported events.
pub fn scan_and_report_oom_events(now: i64) -> Result<u32> {
    let dmesg_out = quiet_command("wsl")
        .args(["-e", "dmesg", "-T"])
        .output()
        .context("run dmesg via wsl (is WSL2 available on this host?)")?;
    let dmesg_text = String::from_utf8_lossy(&dmesg_out.stdout);
    let events = parse_oom_events(&dmesg_text);

    // Drift detection: if dmesg's line format ever changes (kernel/cgroup-driver
    // upgrade), the regex in parse_oom_events could silently stop matching. Compare
    // against a plain substring count so a format drift is visible in the log
    // instead of the scan just quietly finding nothing forever.
    let constraint_lines = dmesg_text
        .lines()
        .filter(|l| l.contains("oom-kill:constraint=CONSTRAINT_MEMCG"))
        .count();
    if constraint_lines > events.len() {
        eprintln!(
            "runner-scale: OOM-visibility saw {constraint_lines} oom-kill:constraint= \
             line(s) in dmesg but only parsed {} into events — dmesg's line format may \
             have drifted; check oom_line_regex against a fresh sample",
            events.len()
        );
    }

    let seen = read_oom_seen();
    let fresh: Vec<&OomEvent> = new_events(&events, &seen)
        .into_iter()
        .take(OOM_FAN_OUT_MAX)
        .collect();
    if fresh.is_empty() {
        return Ok(0);
    }

    // The cgroup id dmesg reports (oom_memcg=/docker/<id>) and the container id
    // `docker events` reports are the same full 64-character docker container id —
    // both come from the same underlying container, just observed by two different
    // subsystems (the kernel cgroup controller and the Docker daemon's event log).
    let events_text = fetch_recent_container_events(now)?;
    let names = parse_container_names(&events_text);

    let mut reported = 0u32;
    let mut seen_so_far = seen;
    for event in fresh {
        let Some(container_name) = names.get(&event.cgroup_id) else {
            eprintln!(
                "runner-scale: OOM event on cgroup {} (process {}) — no matching \
                 managed container name found in the last {OOM_EVENTS_WINDOW_SECS}s of \
                 docker events; will retry next tick",
                event.cgroup_id, event.process
            );
            continue;
        };
        // Correlation trusts a bare runner_name match with no kill-timestamp gate:
        // this fleet's runners are strictly ephemeral (one container takes exactly
        // one dispatched job, then self-deregisters and exits — see the module doc
        // at the top of runner_scale.rs), so a container name is never reused across
        // two different jobs. There is no second job it could be misattributed to.
        match find_run_for_runner(container_name) {
            Ok(Some(m)) => {
                let body = oom_comment_body(event, &m.job_name, m.run_id);
                match post_pr_comment(m.pr_number, &body, false) {
                    Ok(()) => {
                        reported += 1;
                        // Persist immediately: only ever mark an event seen right
                        // after its post actually succeeded, and do it before moving
                        // on to the next event so a mid-loop process kill can't
                        // leave a successfully-posted event unrecorded.
                        seen_so_far = append_seen(seen_so_far, &[event.raw_line.clone()]);
                        write_oom_seen(&seen_so_far);
                    }
                    Err(e) => eprintln!(
                        "runner-scale: OOM comment post failed (will retry next tick): {e:#}"
                    ),
                }
            }
            Ok(None) => {
                eprintln!(
                    "runner-scale: OOM on {container_name} — no PR-triggered job match found \
                     in recent runs; will retry next tick"
                );
            }
            Err(e) => {
                eprintln!(
                    "runner-scale: OOM job correlation failed (will retry next tick): {e:#}"
                );
            }
        }
    }

    Ok(reported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_captured_oom_line() {
        // Verbatim (container id truncated for readability, but kept a valid
        // 64-char hex string) shape of a real line captured via
        // `wsl -e dmesg -T` during the 2026-07-07 investigation.
        let line = "[Tue Jul  7 07:53:04 2026] oom-kill:constraint=CONSTRAINT_MEMCG,\
                     nodemask=(null),cpuset=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
                     mems_allowed=0,oom_memcg=/docker/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
                     task_memcg=/docker/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
                     task=rustdoc,pid=15612,uid=0";
        let events = parse_oom_events(line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].process, "rustdoc");
        assert_eq!(
            events[0].cgroup_id,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(events[0].raw_line, line);
    }

    #[test]
    fn ignores_unrelated_dmesg_lines() {
        let text = "[1.0] some unrelated boot message\n\
                     [2.0] another line entirely\n";
        assert!(parse_oom_events(text).is_empty());
    }

    #[test]
    fn parses_multiple_events_in_order_and_skips_a_malformed_decoy() {
        // The decoy line on [3.0] deliberately contains the
        // "oom-kill:constraint=CONSTRAINT_MEMCG" substring the initial
        // `.filter()` checks for, but its cgroup id is truncated (not a
        // valid 64-char hex string), so it fails `re.captures` and must be
        // dropped by the `filter_map`'s `?` short-circuit -- proving the
        // noise-skipping is enforced by the regex capture, not merely by
        // lines never containing the substring to begin with.
        let text = format!(
            "[1.0] noise\n\
             [2.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/{a},task_memcg=/docker/{a},task=cargo,pid=1,uid=0\n\
             [3.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/deadbeef,task_memcg=/docker/deadbeef,task=truncated,pid=9,uid=0\n\
             [4.0] more noise\n\
             [5.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/{b},task_memcg=/docker/{b},task=rustc,pid=2,uid=0\n",
            a = "a".repeat(64),
            b = "b".repeat(64),
        );
        let events = parse_oom_events(&text);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].process, "cargo");
        assert_eq!(events[1].process, "rustc");
    }

    #[test]
    fn new_events_filters_out_already_seen_lines() {
        let events = vec![
            OomEvent {
                raw_line: "line-a".to_string(),
                process: "cargo".to_string(),
                cgroup_id: "a".repeat(64),
            },
            OomEvent {
                raw_line: "line-b".to_string(),
                process: "rustc".to_string(),
                cgroup_id: "b".repeat(64),
            },
        ];
        let seen = vec!["line-a".to_string()];
        let fresh = new_events(&events, &seen);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].raw_line, "line-b");
    }

    #[test]
    fn new_events_returns_all_when_seen_is_empty() {
        let events = vec![OomEvent {
            raw_line: "line-a".to_string(),
            process: "cargo".to_string(),
            cgroup_id: "a".repeat(64),
        }];
        assert_eq!(new_events(&events, &[]).len(), 1);
    }

    #[test]
    fn append_seen_caps_to_max_dropping_oldest_first() {
        let seen: Vec<String> = (0..OOM_SEEN_MAX).map(|i| format!("old-{i}")).collect();
        let newly = vec!["new-1".to_string(), "new-2".to_string()];
        let updated = append_seen(seen, &newly);
        assert_eq!(updated.len(), OOM_SEEN_MAX);
        // The two oldest entries were dropped to make room.
        assert!(!updated.contains(&"old-0".to_string()));
        assert!(!updated.contains(&"old-1".to_string()));
        assert!(updated.contains(&"old-2".to_string()));
        // The new entries are present.
        assert!(updated.contains(&"new-1".to_string()));
        assert!(updated.contains(&"new-2".to_string()));
    }

    #[test]
    fn append_seen_under_cap_keeps_everything() {
        let seen = vec!["a".to_string()];
        let updated = append_seen(seen, &["b".to_string()]);
        assert_eq!(updated, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn parses_container_names_from_real_docker_events_format() {
        // Verbatim shape of real docker events output captured during the
        // 2026-07-07 investigation (ids shortened-then-padded to stay valid
        // 64-char hex for the fixture).
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let text = format!(
            "2026-07-07T08:14:08.590272062-04:00 container kill {a} (image=vox-ci-runner-local:latest, name=vox-runner-auto-6a4cebb2-0)\n\
             2026-07-07T08:14:08.766164548-04:00 container die {a} (exitCode=137, name=vox-runner-auto-6a4cebb2-0)\n\
             2026-07-07T08:14:10.722839758-04:00 container start {b} (name=vox-runner-auto-6a4ced91-0)\n\
             2026-07-07T08:06:39.404509619-04:00 container exec_die cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc (name=vox_clickhouse)\n"
        );
        let names = parse_container_names(&text);
        assert_eq!(
            names.get(a.as_str()),
            Some(&"vox-runner-auto-6a4cebb2-0".to_string())
        );
        assert_eq!(
            names.get(b.as_str()),
            Some(&"vox-runner-auto-6a4ced91-0".to_string())
        );
        // Non-managed containers (no MANAGED_PREFIX) must be filtered out --
        // vox_clickhouse is real host traffic we don't care about here.
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn parse_container_names_empty_on_no_matches() {
        assert!(parse_container_names("no container events here\n").is_empty());
    }

    #[test]
    fn find_matching_job_returns_the_matching_row() {
        let rows = vec![
            (
                "vox-runner-auto-aaa-0".to_string(),
                "Lints (clippy + rustdoc)".to_string(),
            ),
            ("vox-runner-auto-bbb-0".to_string(), "Audits".to_string()),
        ];
        assert_eq!(
            find_matching_job(&rows, "vox-runner-auto-bbb-0"),
            Some("Audits")
        );
    }

    #[test]
    fn find_matching_job_none_when_no_row_matches() {
        let rows = vec![("vox-runner-auto-aaa-0".to_string(), "Lints".to_string())];
        assert_eq!(find_matching_job(&rows, "vox-runner-auto-zzz-9"), None);
    }

    #[test]
    fn oom_comment_body_includes_job_process_and_raw_evidence() {
        let event = OomEvent {
            raw_line: "the raw dmesg line".to_string(),
            process: "rustdoc".to_string(),
            cgroup_id: "a".repeat(64),
        };
        let body = oom_comment_body(&event, "Lints (clippy + rustdoc)", 28861698905);
        assert!(body.contains("Lints (clippy + rustdoc)"));
        assert!(body.contains("28861698905"));
        assert!(body.contains("rustdoc"));
        assert!(body.contains("the raw dmesg line"));
        assert!(body.contains("OOM"));
    }

    #[test]
    fn full_pipeline_parses_correlates_and_composes_a_correct_comment() {
        let cgroup = "d".repeat(64);
        let dmesg_text = format!(
            "[1.0] noise\n\
             [2.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/{cgroup},\
             task_memcg=/docker/{cgroup},task=rustdoc,pid=99,uid=0\n"
        );
        let events_text = format!(
            "2026-07-07T07:53:04.000000000-04:00 container die {cgroup} \
             (exitCode=137, name=vox-runner-auto-abc123-0)\n"
        );
        let job_rows = vec![(
            "vox-runner-auto-abc123-0".to_string(),
            "Lints (clippy + rustdoc)".to_string(),
        )];

        // 1. parse: one fresh event, never seen before.
        let events = parse_oom_events(&dmesg_text);
        assert_eq!(events.len(), 1);
        let fresh = new_events(&events, &[]);
        assert_eq!(fresh.len(), 1);

        // 2. resolve: cgroup id -> container name.
        let names = parse_container_names(&events_text);
        let container_name = names.get(&fresh[0].cgroup_id).expect("name resolved");
        assert_eq!(container_name, "vox-runner-auto-abc123-0");

        // 3. correlate: container name -> job name.
        let job_name = find_matching_job(&job_rows, container_name).expect("job matched");
        assert_eq!(job_name, "Lints (clippy + rustdoc)");

        // 4. compose: final comment body is correct.
        let body = oom_comment_body(fresh[0], job_name, 28861698905);
        assert!(body.contains("Lints (clippy + rustdoc)"));
        assert!(body.contains("rustdoc"));
        assert!(body.contains("28861698905"));
        assert!(body.contains(fresh[0].raw_line.as_str()));
    }

    #[test]
    fn escape_for_code_fence_neutralizes_triple_backticks() {
        let raw = "before ``` after";
        let escaped = escape_for_code_fence(raw);
        assert!(!escaped.contains("```"));
        assert!(escaped.contains("before"));
        assert!(escaped.contains("after"));
    }

    #[test]
    fn escape_for_code_fence_neutralizes_longer_backtick_runs() {
        // Non-overlapping single-pass replacement of "```" would leave a
        // dangling ``` in 4+ backtick runs (e.g. "````".replace("```", ...)
        // matches once at index 0, leaving the 4th backtick to rejoin the
        // inserted "``" into a fresh "```"). Cover 4, 5, and 6+ backticks so
        // the fix (loop-until-stable or a `` `{3,}` `` regex) is verified
        // against the case where the bug actually manifests.
        for raw in ["````", "`````", "``````", "```````"] {
            let escaped = escape_for_code_fence(raw);
            assert!(
                !escaped.contains("```"),
                "escape_for_code_fence({raw:?}) still contains a triple-backtick run: {escaped:?}"
            );
        }
    }

    #[test]
    fn escape_for_code_fence_neutralizes_backtick_run_embedded_in_text() {
        let raw = "before ````` after";
        let escaped = escape_for_code_fence(raw);
        assert!(!escaped.contains("```"));
        assert!(escaped.contains("before"));
        assert!(escaped.contains("after"));
    }
}
