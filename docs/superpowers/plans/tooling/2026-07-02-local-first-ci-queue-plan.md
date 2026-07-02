# Local-First CI Queue Implementation Plan (rev 2, adversarially audited)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the local runner fleet the default CI plane mechanically: a `vox ci queue` SSOT signal (queue state + failure signal), auto-clear of superseded/stale runs on the autoscaler tick, hooks that block remote check-watching, a concurrency guard, a hosted-fallback gate fix, and AI-first catalog registration.

**Architecture:** A run-centric `queue.rs` beside `runner_scale.rs` (reusing its `gh` plumbing). Classification is fail-open: only `push`/`pull_request` events are cancellable, the supersede key is `(workflow-path, repo, branch, event)`, stale-cancel is disabled when the fleet is down, re-runs/tags/waiting are exempt. Enforcement is a PreToolUse hook matching `Bash|PowerShell` (normalized substring + loop-heuristic match, exit 2) plus SessionStart snapshot injection. The snapshot also carries recent failures — the async failure signal the contract promises.

**Spec:** `docs/superpowers/specs/2026-07-02-local-first-ci-queue-design.md` (rev 2 — read it first; §2's exemption/classification rules and §5's clap exit-2 hazard are load-bearing).

**House rules:** never `cargo fmt --all` (use `cargo fmt -p <crate>`); never pipe cargo output to `head`/`grep`; workspace clippy must `--exclude vox-gui`; `.vox` files are Vox source; no new `.ps1`/`.sh`/`.py`.

**Task order is load-bearing:** the binary must be installed (Task 6 Step 1) before `.claude/settings.json` lands (Task 6 Step 4), or every agent shell call gets blocked by a stale-clap exit-2 collision.

---

### Task 1: `queue.rs` pure core — types, exemption, classification, advice

**Files:**
- Create: `crates/vox-cli/src/commands/ci/queue.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (add `pub mod queue;` in alphabetical position)
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs` (visibility only)

- [ ] **Step 1: Make shared helpers reachable**

In `runner_scale.rs`, change three private functions to `pub(crate)` (no body changes): `now_secs` (~line 231), `gh_json` (~line 270), `max_runners`. Add one helper next to `managed_containers`:

```rust
/// Count of managed runner containers currently running (queue snapshot summary).
pub(crate) fn managed_running_count() -> Result<u32> {
    Ok(managed_containers("running").len() as u32)
}
```

- [ ] **Step 2: Write the failing tests**

Create `queue.rs` with the types/signatures from Step 4 stubbed out and this test module (write tests first; they fail to compile until Step 4):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: u64, wf: &str, br: &str, repo: &str, ev: &str, started: i64, st: &str, attempt: u32) -> String {
        format!("{id}\t{wf}\t{br}\t{repo}\t{ev}\t{started}\t{st}\t{attempt}")
    }

    fn run(id: u64, br: &str, ev: &str, st: &str, started: i64, now: i64) -> QueueRun {
        parse_run_line(&line(id, "ci.yml", br, "vox-foundation/vox", ev, started, st, 1), now).unwrap()
    }

    #[test]
    fn parse_run_line_roundtrip() {
        let r = run(42, "feat/x", "push", "queued", 1000, 1600);
        assert_eq!((r.id, r.age_secs, r.run_attempt), (42, 600, 1));
        assert_eq!(r.repo, "vox-foundation/vox");
        assert!(!r.exempt);
        assert!(parse_run_line("garbage", 0).is_none());
        assert!(parse_run_line("1\tci.yml\tonly-three", 0).is_none());
    }

    #[test]
    fn exemption_event_allowlist_fails_open() {
        // Only push/pull_request are cancellable; unknown events are exempt.
        for ev in ["merge_group", "schedule", "workflow_dispatch", "workflow_run", "dynamic", "some_future_event"] {
            assert!(is_exempt("feat/x", ev, 1, "queued"), "{ev} must be exempt");
        }
        assert!(!is_exempt("feat/x", "push", 1, "queued"));
        assert!(!is_exempt("feat/x", "pull_request", 1, "queued"));
    }

    #[test]
    fn exemption_branch_tag_attempt_waiting() {
        assert!(is_exempt("main", "push", 1, "queued"));
        assert!(is_exempt("null", "push", 1, "queued"));          // API null head_branch
        assert!(is_exempt("v0.6.0", "push", 1, "queued"));        // tag push (release-binaries, live-verified)
        assert!(!is_exempt("very-cool-branch", "push", 1, "queued")); // 'v' prefix alone is not a tag
        assert!(!is_exempt("v-experiment", "push", 1, "queued"));
        assert!(is_exempt("feat/x", "push", 2, "queued"));        // re-run = explicit human request
        assert!(is_exempt("feat/x", "push", 1, "waiting"));       // deployment approval gate
    }

    #[test]
    fn superseded_key_includes_repo_and_event() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "feat/x", "push", "queued", 1000, now),
            run(2, "feat/x", "push", "queued", 3000, now), // same key, newer -> 1 superseded
            // push/PR siblings for the same commit (mobile-eas-build shape): must NOT cancel each other
            run(3, "feat/x", "pull_request", "queued", 3001, now),
            // fork collision: same branch name, different repo -> independent
            parse_run_line(&line(4, "ci.yml", "patch-1", "forkA/vox", "pull_request", 1000, "queued", 1), now).unwrap(),
            parse_run_line(&line(5, "ci.yml", "patch-1", "forkB/vox", "pull_request", 3000, "queued", 1), now).unwrap(),
        ];
        classify_runs(&mut runs, 3600 * 24, true);
        assert_eq!(runs[0].class, RunClass::Superseded);
        assert_eq!(runs[1].class, RunClass::Active);
        assert_eq!(runs[2].class, RunClass::Active, "event is in the key");
        assert_eq!(runs[3].class, RunClass::Active, "repo is in the key");
        assert_eq!(runs[4].class, RunClass::Active);
    }

    #[test]
    fn superseded_ties_and_exempt_never() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "feat/x", "push", "queued", 2000, now),
            run(2, "feat/x", "push", "queued", 2000, now), // equal started: keep both
            run(3, "main", "push", "queued", 1000, now),
            run(4, "main", "push", "queued", 9000, now),
        ];
        classify_runs(&mut runs, 3600 * 24, true);
        assert!(runs.iter().all(|r| r.class == RunClass::Active));
        assert!(runs[2].exempt && runs[3].exempt);
    }

    #[test]
    fn stale_ttl_and_fleet_gate() {
        let now = 10_000;
        let ttl = 2700;
        let mk = |id, st: &str, started| run(id, &format!("b{id}"), "push", st, started, now);
        let mut runs = vec![
            mk(1, "queued", now - ttl),      // exactly TTL: not stale
            mk(2, "queued", now - ttl - 1),  // past TTL: stale
            mk(3, "pending", now - ttl - 1), // pending counts (concurrency-blocked)
            mk(4, "in_progress", 0),         // in_progress: never stale
        ];
        classify_runs(&mut runs, ttl, true);
        assert_eq!(runs[0].class, RunClass::Active);
        assert_eq!(runs[1].class, RunClass::Stale);
        assert_eq!(runs[2].class, RunClass::Stale);
        assert_eq!(runs[3].class, RunClass::Active);
        // Fleet down: stale sweep disabled entirely (outage != abandonment).
        let mut runs2 = vec![mk(5, "queued", 0)];
        classify_runs(&mut runs2, ttl, false);
        assert_eq!(runs2[0].class, RunClass::Active);
    }

    #[test]
    fn advice_phrasings() {
        assert!(advice_for(3, 4, 0, 0, 2, false).contains("healthy"));
        let clearable = advice_for(14, 4, 9, 3, 2, false);
        assert!(clearable.contains("vox ci queue --clear"));
        assert!(clearable.contains("9 superseded") && clearable.contains("3 stale"));
        let outage = advice_for(9, 4, 0, 0, 0, false);
        assert!(outage.contains("outage") && outage.contains("runner-status"));
        let backlog = advice_for(9, 4, 0, 0, 2, false);
        assert!(backlog.contains("real demand"));
        let deg = advice_for(0, 4, 0, 0, 0, true);
        assert!(deg.contains("degraded") && deg.contains("local gates"));
    }

    #[test]
    fn failure_advice_leads() {
        let f = FailedRun {
            id: 123, workflow: "ci.yml".into(), branch: "feat/x".into(),
            conclusion: "failure".into(), head_sha: "abc".into(),
            completed_epoch: 0, url: "https://g/123".into(),
        };
        let a = failure_advice(&f);
        assert!(a.contains("123") && a.contains("--log-failed") && a.contains("do not push blind retries"));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-cli commands::ci::queue`
Expected: compile error — types/functions not defined.

- [ ] **Step 4: Implement the core**

```rust
//! `vox ci queue` — run-centric CI queue snapshot, classification, clearing,
//! and the async failure signal.
//!
//! SSOT for the local-first CI contract
//! (docs/superpowers/specs/2026-07-02-local-first-ci-queue-design.md): agents
//! verify with local gates and never watch remote checks; this command is the
//! sanctioned way to read (`--json`/`--brief`) or clear (`--clear`) the queue,
//! and its snapshot carries recent run failures back to future sessions.
//! `--hook-guard` is the PreToolUse enforcement mode. Run-level only — the
//! autoscaler's job-label demand counting stays in `runner_scale.rs`.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use super::constants::REPO_SLUG;
use super::runner_scale::{gh_json, now_secs};

pub const DEFAULT_TTL_MINS: i64 = 45;
/// Blast-radius + POST-burst bound per sweep; remainder clears on later ticks.
pub const MAX_CANCELS_PER_SWEEP: usize = 50;
/// `--from-snapshot` refuses older snapshots (steady state is ~2 min via the tick).
const SNAPSHOT_STALE_SECS: i64 = 600;
const FAILURE_WINDOW_SECS: i64 = 86_400;
const FAILURE_CAP: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunClass { Active, Superseded, Stale }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueRun {
    pub id: u64,
    /// Workflow file path (".github/workflows/x.yml") — display `name` can
    /// contain tabs and collide; `.path` cannot.
    pub workflow: String,
    /// "null" when the API reports no head branch — never supersedable.
    pub branch: String,
    /// head_repository.full_name — fork disambiguation in the supersede key.
    pub repo: String,
    pub event: String,
    /// queued | in_progress | pending | waiting
    pub status: String,
    pub run_attempt: u32,
    /// run_started_at when present (re-runs reset it), else created_at.
    pub started_epoch: i64,
    pub age_secs: i64,
    pub class: RunClass,
    pub exempt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedRun {
    pub id: u64,
    pub workflow: String,
    pub branch: String,
    /// failure | timed_out | startup_failure. `cancelled` is EXCLUDED so the
    /// auto-clear's own cancellations never echo back as failures.
    pub conclusion: String,
    pub head_sha: String,
    pub completed_epoch: i64,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub generated_at: i64,
    pub degraded: bool,
    /// queued + pending (both "not yet running").
    pub queued: u32,
    pub in_progress: u32,
    pub superseded: u32,
    pub stale: u32,
    pub fleet_alive: u32,
    pub fleet_max: u32,
    /// THE machine-readable signal: always present, says what to do next.
    pub advice: String,
    /// Async failure signal: last 24h, newest-first, cap 20, main included.
    pub failures: Vec<FailedRun>,
    /// Ids cancelled by the previous sweep — force-cancel escalation state.
    pub cancelled_last_sweep: Vec<u64>,
    pub runs: Vec<QueueRun>,
}

/// Release workflows trigger on `tags: v*`; their runs report the tag as
/// head_branch with event=push (live-verified: release-binaries → "v0.6.0").
fn is_tag_like(branch: &str) -> bool {
    let mut c = branch.chars();
    c.next() == Some('v') && c.next().is_some_and(|c| c.is_ascii_digit())
}

/// Fail-open exemption: a run is cancellable only when EVERY arm below passes.
/// Unknown events, re-runs, tag pushes, approval-gated runs are all exempt.
pub fn is_exempt(branch: &str, event: &str, run_attempt: u32, status: &str) -> bool {
    let cancellable_event = matches!(event, "push" | "pull_request");
    !cancellable_event
        || branch == "main"
        || branch == "null"
        || (event == "push" && is_tag_like(branch))
        || run_attempt > 1
        || status == "waiting"
}

/// One tab-separated line from JQ_RUN_LINE:
/// id \t path \t branch \t repo \t event \t started_epoch \t status \t run_attempt
pub fn parse_run_line(line: &str, now: i64) -> Option<QueueRun> {
    let mut p = line.split('\t');
    let id = p.next()?.trim().parse().ok()?;
    let workflow = p.next()?.trim().to_string();
    let branch = p.next()?.trim().to_string();
    let repo = p.next()?.trim().to_string();
    let event = p.next()?.trim().to_string();
    let started_epoch: i64 = p.next()?.trim().parse().ok()?;
    let status = p.next()?.trim().to_string();
    let run_attempt: u32 = p.next()?.trim().parse().ok()?;
    let exempt = is_exempt(&branch, &event, run_attempt, &status);
    Some(QueueRun {
        id, workflow, branch, repo, event, status, run_attempt, started_epoch,
        age_secs: now.saturating_sub(started_epoch),
        class: RunClass::Active,
        exempt,
    })
}

/// Superseded: strictly newer non-exempt run with the same
/// (workflow, repo, branch, event) key. Stale: queued/pending past TTL —
/// only while the fleet is alive (`stale_enabled`); a deep queue with zero
/// runners is an outage, and cancelling it would kill the async safety net
/// AND reset the health watchdog's queue_age signal.
pub fn classify_runs(runs: &mut [QueueRun], ttl_secs: i64, stale_enabled: bool) {
    for i in 0..runs.len() {
        if runs[i].exempt {
            runs[i].class = RunClass::Active;
            continue;
        }
        let newer = runs.iter().any(|o| {
            !o.exempt
                && o.id != runs[i].id
                && o.workflow == runs[i].workflow
                && o.repo == runs[i].repo
                && o.branch == runs[i].branch
                && o.event == runs[i].event
                && o.started_epoch > runs[i].started_epoch
        });
        runs[i].class = if newer {
            RunClass::Superseded
        } else if stale_enabled
            && matches!(runs[i].status.as_str(), "queued" | "pending")
            && runs[i].age_secs > ttl_secs
        {
            RunClass::Stale
        } else {
            RunClass::Active
        };
    }
}

/// Global advice (no branch context — the snapshot is written by the tick).
/// Branch-failure advice is layered at render time via `failure_advice`.
pub fn advice_for(
    active_queued: u32,
    capacity: u32,
    superseded: u32,
    stale: u32,
    fleet_alive: u32,
    degraded: bool,
) -> String {
    if degraded {
        return "degraded: gh unreachable or partial data; do not retry-loop — \
                proceed on local gates (`vox ci pre-push --complete`) and try `vox ci queue` later"
            .to_string();
    }
    if superseded + stale > 0 {
        return format!(
            "queued {active_queued} vs capacity {capacity}: run 'vox ci queue --clear' \
             (would cancel {superseded} superseded + {stale} stale)"
        );
    }
    if active_queued > capacity && fleet_alive == 0 {
        return format!(
            "queue backlog: {active_queued} active > capacity {capacity} with fleet at 0 — \
             outage, not backlog; stale sweep disabled; check 'vox ci runner-status'"
        );
    }
    if active_queued > capacity {
        return format!(
            "queue backlog: {active_queued} active queued > capacity {capacity}; \
             nothing clearable — this is real demand, do not add speculative pushes"
        );
    }
    format!("queue healthy: {active_queued} active queued ≤ capacity {capacity}")
}

/// The failure half of the signal — used when the CURRENT branch has a red run.
pub fn failure_advice(f: &FailedRun) -> String {
    format!(
        "CI FAILED for this branch (run {}): read {} or 'gh run view {} --log-failed', \
         fix locally, re-run local gates — do not push blind retries",
        f.id, f.url, f.id
    )
}
```

- [ ] **Step 5: Register the module and run tests**

Add `pub mod queue;` to `crates/vox-cli/src/commands/ci/mod.rs`.
Run: `cargo test -p vox-cli commands::ci::queue` — Expected: 8 tests PASS.
Run: `cargo clippy -p vox-cli --all-targets -- -D warnings` — Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/queue.rs crates/vox-cli/src/commands/ci/mod.rs crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): vox ci queue core — fail-open exemption, audited supersede key, advice signal"
```

---

### Task 2: fetch, failure signal, snapshot file, rendering, CLI wiring

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/queue.rs` (append)
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (new `Queue` variant after `RunnerStatus`, ~line 838)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (dispatch near line 555)

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module:

```rust
    fn failed(id: u64, br: &str, completed: i64) -> FailedRun {
        parse_failed_line(
            &format!("{id}\tci.yml\t{br}\tfailure\tabc123\t{completed}\thttps://g/{id}"),
        )
        .unwrap()
    }

    #[test]
    fn parse_failed_line_roundtrip() {
        let f = failed(7, "feat/x", 5000);
        assert_eq!((f.id, f.completed_epoch), (7, 5000));
        assert!(parse_failed_line("garbage").is_none());
    }

    #[test]
    fn failures_window_and_cap() {
        let now = 200_000;
        let mut fs: Vec<FailedRun> = (0..30).map(|i| failed(i, "b", now - 100)).collect();
        fs.push(failed(99, "old", now - FAILURE_WINDOW_SECS - 1));
        let kept = filter_failures(fs, now);
        assert_eq!(kept.len(), FAILURE_CAP);
        assert!(kept.iter().all(|f| f.id != 99));
    }

    #[test]
    fn snapshot_roundtrip_and_brief() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "feat/x", "push", "queued", 1000, now),
            run(2, "feat/x", "push", "queued", 3000, now),
        ];
        classify_runs(&mut runs, 2700, true);
        let fails = vec![failed(9, "feat/x", now - 60), failed(10, "main", now - 120)];
        let snap = build_snapshot(runs, fails, 2, 4, false, now, vec![]);
        assert_eq!((snap.queued, snap.superseded), (2, 1));
        let back: QueueSnapshot =
            serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        assert_eq!(back.advice, snap.advice);

        let brief = render_brief(&back, Some("feat/x"), now);
        assert!(brief.contains("FAILED on feat/x: ci.yml #9"));
        assert!(brief.contains("FAILED on main: ci.yml #10"));
        assert!(brief.contains("do not push blind retries")); // failure advice leads
        assert!(brief.lines().count() <= 7);

        let clean = build_snapshot(vec![], vec![], 2, 4, false, now, vec![]);
        let brief2 = render_brief(&clean, Some("feat/x"), now);
        assert!(!brief2.contains("FAILED"));
        assert!(brief2.contains("advice:"));
    }

    #[test]
    fn snapshot_staleness() {
        assert!(!snapshot_is_stale(1000, 1000 + SNAPSHOT_STALE_SECS));
        assert!(snapshot_is_stale(1000, 1000 + SNAPSHOT_STALE_SECS + 1));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-cli commands::ci::queue`
Expected: compile error — `parse_failed_line`, `filter_failures`, `build_snapshot`, `render_brief`, `snapshot_is_stale` not defined.

- [ ] **Step 3: Implement fetch + snapshot + renders**

Append to `queue.rs`:

```rust
/// jq projections. `fromdateiso8601` is proven in this repo (ci-timings.yml:44
/// uses it inside `gh api --jq` in production). `.path` not `.name` (tab-safe).
const JQ_RUN_LINE: &str = ".workflow_runs[]|\"\\(.id)\\t\\(.path)\\t\\(.head_branch)\\t\\(.head_repository.full_name)\\t\\(.event)\\t\\((.run_started_at // .created_at)|fromdateiso8601)\\t\\(.status)\\t\\(.run_attempt)\"";
const JQ_FAIL_LINE: &str = ".workflow_runs[]|select(.conclusion==\"failure\" or .conclusion==\"timed_out\" or .conclusion==\"startup_failure\")|\"\\(.id)\\t\\(.path)\\t\\(.head_branch)\\t\\(.conclusion)\\t\\(.head_sha)\\t\\(.updated_at|fromdateiso8601)\\t\\(.html_url)\"";

/// The four live-ish statuses. `pending` = concurrency-group blocked (real in
/// this repo, live-probed); `waiting` = deployment approval (fetched for
/// visibility, exempt from cancellation via is_exempt).
const FETCH_STATUSES: &[&str] = &["queued", "in_progress", "pending", "waiting"];
/// Newest-first API + manual page loop (gh --paginate cannot be capped):
/// 5 pages × 100 bounds the flood case without going blind to the stale tail.
const MAX_PAGES: u32 = 5;

fn fetch_status_runs(status: &str, now: i64) -> Result<Vec<QueueRun>> {
    let mut out = Vec::new();
    for page in 1..=MAX_PAGES {
        let raw = gh_json(&[
            "api",
            &format!("repos/{REPO_SLUG}/actions/runs?status={status}&per_page=100&page={page}"),
            "--jq",
            JQ_RUN_LINE,
        ])?;
        let mut n = 0u32;
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            if let Some(r) = parse_run_line(line, now) {
                out.push(r);
            }
            n += 1;
        }
        if n < 100 {
            break;
        }
    }
    Ok(out)
}

pub fn fetch_all_runs(now: i64) -> Result<Vec<QueueRun>> {
    let mut runs = Vec::new();
    for status in FETCH_STATUSES {
        runs.extend(fetch_status_runs(status, now)?);
    }
    Ok(runs)
}

pub fn parse_failed_line(line: &str) -> Option<FailedRun> {
    let mut p = line.split('\t');
    Some(FailedRun {
        id: p.next()?.trim().parse().ok()?,
        workflow: p.next()?.trim().to_string(),
        branch: p.next()?.trim().to_string(),
        conclusion: p.next()?.trim().to_string(),
        head_sha: p.next()?.trim().to_string(),
        completed_epoch: p.next()?.trim().parse().ok()?,
        url: p.next()?.trim().to_string(),
    })
}

pub fn filter_failures(mut failures: Vec<FailedRun>, now: i64) -> Vec<FailedRun> {
    failures.retain(|f| now.saturating_sub(f.completed_epoch) <= FAILURE_WINDOW_SECS);
    failures.sort_by_key(|f| std::cmp::Reverse(f.completed_epoch));
    failures.truncate(FAILURE_CAP);
    failures
}

pub fn fetch_recent_failures(now: i64) -> Result<Vec<FailedRun>> {
    let raw = gh_json(&[
        "api",
        &format!("repos/{REPO_SLUG}/actions/runs?status=completed&per_page=50"),
        "--jq",
        JQ_FAIL_LINE,
    ])?;
    Ok(filter_failures(
        raw.lines().filter(|l| !l.trim().is_empty()).filter_map(parse_failed_line).collect(),
        now,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn build_snapshot(
    runs: Vec<QueueRun>,
    failures: Vec<FailedRun>,
    fleet_alive: u32,
    fleet_max: u32,
    degraded: bool,
    now: i64,
    cancelled_last_sweep: Vec<u64>,
) -> QueueSnapshot {
    let queued = runs.iter().filter(|r| matches!(r.status.as_str(), "queued" | "pending")).count() as u32;
    let in_progress = runs.iter().filter(|r| r.status == "in_progress").count() as u32;
    let superseded = runs.iter().filter(|r| r.class == RunClass::Superseded).count() as u32;
    let stale = runs.iter().filter(|r| r.class == RunClass::Stale).count() as u32;
    let active_queued = runs
        .iter()
        .filter(|r| matches!(r.status.as_str(), "queued" | "pending") && r.class == RunClass::Active)
        .count() as u32;
    let advice = advice_for(active_queued, fleet_max, superseded, stale, fleet_alive, degraded);
    QueueSnapshot {
        generated_at: now, degraded, queued, in_progress, superseded, stale,
        fleet_alive, fleet_max, advice, failures, cancelled_last_sweep, runs,
    }
}

fn snapshot_path() -> PathBuf {
    crate::fs_utils::user_home_dir().join(".vox").join("ci-queue-snapshot.json")
}

/// Atomic write (temp + rename): parallel agent sessions and the autoscaler
/// tick race on this file; a torn read must be impossible.
pub fn write_snapshot(snap: &QueueSnapshot) -> Result<()> {
    let p = snapshot_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(snap)?)
        .with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, &p).with_context(|| format!("rename into {}", p.display()))
}

pub fn read_snapshot() -> Option<QueueSnapshot> {
    std::fs::read_to_string(snapshot_path()).ok().and_then(|s| serde_json::from_str(&s).ok())
}

pub fn snapshot_is_stale(generated_at: i64, now: i64) -> bool {
    now.saturating_sub(generated_at) > SNAPSHOT_STALE_SECS
}

fn current_branch() -> Option<String> {
    let out = crate::fs_utils::quiet_command("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn failed_line(prefix: &str, f: &FailedRun, now: i64) -> String {
    format!(
        "FAILED on {prefix}: {} #{} ({}, {}m ago) -> {}",
        f.workflow, f.id, f.conclusion,
        now.saturating_sub(f.completed_epoch) / 60, f.url
    )
}

/// ≤7 lines; SessionStart hook stdout, injected into agent context.
/// Branch failure overrides the displayed advice (the failure IS the signal).
pub fn render_brief(snap: &QueueSnapshot, branch: Option<&str>, now: i64) -> String {
    let mut lines = vec![
        "CI queue (local-first: local gates = verdict for what they cover; never watch remote checks):".to_string(),
        format!(
            "queued {} / in-progress {} (superseded {}, stale {}); fleet {}/{}",
            snap.queued, snap.in_progress, snap.superseded, snap.stale,
            snap.fleet_alive, snap.fleet_max
        ),
    ];
    let branch_fail = branch.and_then(|b| snap.failures.iter().find(|f| f.branch == b));
    if let Some(f) = branch_fail {
        lines.push(failed_line(&f.branch.clone(), f, now));
    }
    if let Some(f) = snap.failures.iter().find(|f| f.branch == "main") {
        lines.push(failed_line("main", f, now));
    }
    lines.push(format!(
        "advice: {}",
        branch_fail.map(failure_advice).unwrap_or_else(|| snap.advice.clone())
    ));
    lines.push("commands: `vox ci queue --json` | `vox ci queue --clear`".to_string());
    lines.join("\n")
}

fn render_table(snap: &QueueSnapshot, now: i64) -> String {
    let mut out = String::from(
        "RUN_ID      AGE_MIN  CLASS       STATUS       EVENT             BRANCH                    WORKFLOW\n",
    );
    for r in &snap.runs {
        let class = match (r.exempt, r.class) {
            (true, _) => "exempt",
            (_, RunClass::Active) => "active",
            (_, RunClass::Superseded) => "superseded",
            (_, RunClass::Stale) => "stale",
        };
        out.push_str(&format!(
            "{:<11} {:>7} {:<11} {:<12} {:<17} {:<25} {}\n",
            r.id, r.age_secs / 60, class, r.status, r.event, r.branch, r.workflow
        ));
    }
    if !snap.failures.is_empty() {
        out.push_str("\nFAILED (24h):\n");
        for f in &snap.failures {
            out.push_str(&format!("  {}\n", failed_line(&f.branch.clone(), f, now)));
        }
    }
    out.push_str(&format!("\nadvice: {}\n", snap.advice));
    out
}

/// Managed fleet counts (alive containers, configured max). Best-effort —
/// (0, max) when docker is down, which conservatively disables stale-cancel.
fn fleet_counts() -> (u32, u32) {
    let alive = super::runner_scale::managed_running_count().unwrap_or(0);
    (alive, super::runner_scale::max_runners())
}
```

**Note:** `quiet_command` — this repo's no-console-window spawn helper. If it lives elsewhere than `crate::fs_utils` (grep `fn quiet_command`), use its actual path; `runner_scale.rs` already calls one for `gh`/`docker`.

- [ ] **Step 4: Implement `run` + entry points**

```rust
pub struct QueueArgs {
    pub json: bool,
    pub brief: bool,
    pub from_snapshot: bool,
    pub clear: bool,
    pub dry_run: bool,
    pub ttl_mins: Option<i64>,
    pub hook_guard: bool,
}

/// Live snapshot: fetch runs + failures, classify (stale gated on fleet
/// health), persist atomically.
pub fn live_snapshot(ttl_mins: i64, now: i64, cancelled_last_sweep: Vec<u64>) -> Result<QueueSnapshot> {
    let mut runs = fetch_all_runs(now)?;
    let (alive, max) = fleet_counts();
    classify_runs(&mut runs, ttl_mins * 60, alive > 0);
    let failures = fetch_recent_failures(now).unwrap_or_default();
    let snap = build_snapshot(runs, failures, alive, max, false, now, cancelled_last_sweep);
    write_snapshot(&snap)?;
    Ok(snap)
}

pub fn run(args: QueueArgs) -> Result<()> {
    if args.hook_guard {
        return hook_guard_main(); // Task 4
    }
    if args.clear && args.from_snapshot {
        return Err(anyhow!(
            "--clear requires live data; refusing to cancel from a snapshot up to 10 min old"
        ));
    }
    let now = now_secs();
    let ttl = args.ttl_mins.unwrap_or(DEFAULT_TTL_MINS);

    let snap = if args.from_snapshot {
        match read_snapshot() {
            Some(s) if !snapshot_is_stale(s.generated_at, now) => s,
            _ => {
                println!("queue snapshot unavailable/stale — run `vox ci queue` for live state");
                return Ok(());
            }
        }
    } else {
        match live_snapshot(ttl, now, Vec::new()) {
            Ok(s) => s,
            Err(e) if args.clear => return Err(e).context("--clear needs live gh data"),
            Err(e) => {
                eprintln!("queue: gh query failed: {e:#}");
                build_snapshot(Vec::new(), Vec::new(), 0, 0, true, now, Vec::new())
            }
        }
    };

    if args.clear {
        return clear_runs(&snap, args.dry_run); // Task 3
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&snap)?);
    } else if args.brief {
        println!("{}", render_brief(&snap, current_branch().as_deref(), now));
    } else {
        println!("{}", render_table(&snap, now));
    }
    Ok(())
}

// Compile-ordering stubs — replaced in Tasks 3 and 4; nothing ships before then.
fn clear_runs(_snap: &QueueSnapshot, _dry_run: bool) -> Result<()> {
    Err(anyhow!("--clear lands in Task 3"))
}
fn hook_guard_main() -> Result<()> {
    Err(anyhow!("--hook-guard lands in Task 4"))
}
```

- [ ] **Step 5: Wire the clap variant + dispatch**

`cmd_enums.rs`, after `RunnerStatus` (~line 838):

```rust
    /// Run-centric CI queue snapshot: classifies runs active/superseded/stale, carries the
    /// async failure signal, emits machine-readable `advice`, and clears cancellable backlog.
    /// The SSOT queue interaction for agents under the local-first CI contract.
    #[command(name = "queue")]
    Queue {
        /// Emit the full QueueSnapshot as JSON.
        #[arg(long)]
        json: bool,
        /// ≤7-line summary incl. FAILED lines (SessionStart hook uses this).
        #[arg(long)]
        brief: bool,
        /// Read ~/.vox/ci-queue-snapshot.json (no network; refuses >10 min old).
        #[arg(long)]
        from_snapshot: bool,
        /// Cancel superseded + stale runs (live data only; exempt-aware; ≤50/sweep).
        #[arg(long)]
        clear: bool,
        /// With --clear: print the cancellation plan without cancelling.
        #[arg(long)]
        dry_run: bool,
        /// Stale TTL in minutes for queued/pending runs (default 45).
        #[arg(long)]
        ttl_mins: Option<i64>,
        /// PreToolUse hook mode: read hook JSON on stdin; exit 2 on banned remote-watch commands.
        #[arg(long)]
        hook_guard: bool,
    },
```

`run_body.rs`, next to the `RunnerScale` arm (~line 555):

```rust
        CiCmd::Queue { json, brief, from_snapshot, clear, dry_run, ttl_mins, hook_guard } => {
            super::queue::run(super::queue::QueueArgs {
                json, brief, from_snapshot, clear, dry_run, ttl_mins, hook_guard,
            })
        }
```

- [ ] **Step 6: Tests + clippy + live smoke**

Run: `cargo test -p vox-cli commands::ci::queue` — Expected: 12 tests PASS.
Run: `cargo clippy -p vox-cli --all-targets -- -D warnings` — Expected: clean.
Run: `cargo run -p vox-cli -- ci queue --brief` — Expected: brief with real counts (and FAILED lines if the last 24h had red runs); `~/.vox/ci-queue-snapshot.json` created.
Run: `cargo run -p vox-cli -- ci queue --from-snapshot --brief` — Expected: same, instant, no network.
Run: `cargo run -p vox-cli -- ci queue --clear --from-snapshot` — Expected: hard error.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/ci/queue.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs
git commit -m "feat(ci): vox ci queue — paginated fetch, failure signal, atomic snapshot, renders"
```

---

### Task 3: `--clear` — cancel superseded + stale (capped)

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/queue.rs` (replace the `clear_runs` stub)

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn clear_plan_selects_only_cancellable_and_caps() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "feat/x", "push", "queued", 1000, now), // superseded by 2
            run(2, "feat/x", "push", "queued", 3000, now), // active
            run(3, "feat/y", "push", "queued", 1, now),    // stale
            run(4, "main", "push", "queued", 1, now),      // exempt
        ];
        classify_runs(&mut runs, 2700, true);
        let snap = build_snapshot(runs, vec![], 2, 4, false, now, vec![]);
        let ids: Vec<u64> = clear_plan(&snap).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 3]);

        // Cap: 60 stale runs -> plan holds MAX_CANCELS_PER_SWEEP.
        let mut many: Vec<QueueRun> = (0..60).map(|i| run(i, &format!("b{i}"), "push", "queued", 1, now)).collect();
        classify_runs(&mut many, 2700, true);
        let snap2 = build_snapshot(many, vec![], 2, 4, false, now, vec![]);
        assert_eq!(clear_plan(&snap2).len(), MAX_CANCELS_PER_SWEEP);
    }
```

- [ ] **Step 2: Run to verify failure, then implement**

Run: `cargo test -p vox-cli commands::ci::queue::tests::clear_plan_selects_only_cancellable_and_caps` — Expected: compile error.

Replace the stub:

```rust
/// Runs `--clear` cancels: non-exempt, non-Active, in a cancellable status,
/// capped per sweep (blast-radius + POST-burst bound).
pub fn clear_plan(snap: &QueueSnapshot) -> Vec<&QueueRun> {
    let mut v: Vec<&QueueRun> = snap
        .runs
        .iter()
        .filter(|r| {
            !r.exempt
                && r.class != RunClass::Active
                && matches!(r.status.as_str(), "queued" | "in_progress" | "pending")
        })
        .collect();
    v.truncate(MAX_CANCELS_PER_SWEEP);
    v
}

fn cancel_run(id: u64, force: bool) -> Result<String> {
    let tail = if force { "force-cancel" } else { "cancel" };
    gh_json(&["api", "-X", "POST", &format!("repos/{REPO_SLUG}/actions/runs/{id}/{tail}")])
}

/// Best-effort sweep: a 409 means the run completed meanwhile — the race
/// resolved itself; log and continue, never abort.
fn clear_runs(snap: &QueueSnapshot, dry_run: bool) -> Result<()> {
    let plan = clear_plan(snap);
    if plan.is_empty() {
        println!("queue clear: nothing cancellable ({})", snap.advice);
        return Ok(());
    }
    let mut cancelled_ids = Vec::new();
    let mut failed = 0u32;
    for r in &plan {
        let tag = format!("{} ({} / {} / {:?})", r.id, r.workflow, r.branch, r.class);
        if dry_run {
            println!("would cancel {tag}");
            continue;
        }
        match cancel_run(r.id, false) {
            Ok(_) => {
                println!("cancelled {tag}");
                cancelled_ids.push(r.id);
            }
            Err(e) => {
                eprintln!("cancel {tag} failed (continuing): {e:#}");
                failed += 1;
            }
        }
    }
    if !dry_run {
        println!("queue clear: cancelled {}, failed {failed}, of {} planned", cancelled_ids.len(), plan.len());
        let now = now_secs();
        if let Ok(s) = live_snapshot(DEFAULT_TTL_MINS, now, cancelled_ids) {
            println!("post-clear: {}", s.advice);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Verify + commit**

Run: `cargo test -p vox-cli commands::ci::queue` — Expected: 13 PASS.
Run: `cargo clippy -p vox-cli --all-targets -- -D warnings` — Expected: clean.
Run: `cargo run -p vox-cli -- ci queue --clear --dry-run` — Expected: plan or "nothing cancellable", NO cancellations.

```bash
git add crates/vox-cli/src/commands/ci/queue.rs
git commit -m "feat(ci): vox ci queue --clear — capped, exempt-aware cancellation"
```

---

### Task 4: `--hook-guard` — PreToolUse enforcement

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/queue.rs` (replace the `hook_guard_main` stub)

- [ ] **Step 1: Write the failing pattern tests**

```rust
    #[test]
    fn hook_guard_patterns() {
        // Banned.
        assert!(hook_guard_matches("gh pr checks 431 --watch"));
        assert!(hook_guard_matches("gh pr checks 431"));        // one-shot too (contract: snapshot is the channel)
        assert!(hook_guard_matches("gh  pr   checks"));         // whitespace collapse
        assert!(hook_guard_matches("GH RUN WATCH 12345"));      // case
        assert!(hook_guard_matches("gh api repos/o/r/commits/abc/check-runs"));
        assert!(hook_guard_matches("gh api repos/o/r/check_runs --paginate"));
        assert!(hook_guard_matches("vox ci watch-run --sha abc"));
        assert!(hook_guard_matches("cargo run -p vox-cli -- ci watch-run"));
        // Loop heuristic: hand-rolled watchers from allowed one-shots.
        assert!(hook_guard_matches("while true; do gh run list --branch x; sleep 15; done"));
        assert!(hook_guard_matches("for i in $(seq 40); do gh pr view 4 --json statusCheckRollup; sleep 30; done"));
        assert!(hook_guard_matches("until false; do gh api repos/o/r/actions/runs; sleep 9; done"));
        // Alias evasion.
        assert!(hook_guard_matches("gh alias set pc 'pr checks'"));
        // Allowed: one-shot reads, failure logs, our own commands.
        assert!(!hook_guard_matches("gh run list --status queued"));
        assert!(!hook_guard_matches("gh run view 12345 --log-failed"));
        assert!(!hook_guard_matches("gh run view 12345 --log && pnpm vitest --watch")); // rev-1 FP, arm dropped
        assert!(!hook_guard_matches("gh pr view 431 --json statusCheckRollup"));        // one-shot
        assert!(!hook_guard_matches("gh pr view 431"));
        assert!(!hook_guard_matches("vox ci queue --json"));
        assert!(!hook_guard_matches("cargo test && sleep 5 && gh run list"));           // sleep without loop keyword
        assert!(!hook_guard_matches("git push"));
    }
```

- [ ] **Step 2: Run to verify failure, then implement**

Run: `cargo test -p vox-cli commands::ci::queue::tests::hook_guard_patterns` — Expected: compile error.

Replace the stub:

```rust
/// Normalized substring match on the command an agent is about to run.
/// Normalization (lowercase + whitespace collapse) kills the `gh  pr  checks`
/// evasion class. `gh run view --watch` is deliberately NOT an arm: the flag
/// does not exist (`-w` is `--web`), and matching it only produced false
/// positives on compound commands. Known collateral: a banned phrase inside a
/// quoted string still blocks — acceptable; the deny message names the
/// sanctioned alternatives.
pub fn hook_guard_matches(cmd: &str) -> bool {
    let c: String = cmd.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    let has = |s: &str| c.contains(s);
    has("gh pr checks")
        || has("gh run watch")
        || (has("gh api") && (has("check-runs") || has("check_runs")))
        || has("ci watch-run")
        // Hand-rolled watch loop from allowed primitives.
        || ((has("while ") || has("until ") || has("for "))
            && has("sleep")
            && (has("gh pr") || has("gh run") || has("gh api")))
        // Alias evasion.
        || (has("gh alias set") && (has("pr checks") || has("run watch")))
}

const HOOK_GUARD_DENY: &str = "Local-first CI: remote check-watching is disabled.\n\
- Verdict: run local gates (`vox ci pre-push --complete`); green = done, push and move on.\n\
- Queue + failures: `vox ci queue --json` (the `advice` field tells you what to do).\n\
- Read one failure's logs: `gh run list --branch <b>` then `gh run view <id> --log-failed` (allowed).\n\
- Clear backlog: `vox ci queue --clear`.";

/// PreToolUse mode: read the Claude Code hook JSON from stdin, extract
/// `tool_input.command` (Bash and PowerShell tools both use `command`), exit 2
/// (block; stderr fed to the model) on a banned pattern. Everything else —
/// including unparseable input — exits 0: fail-open on infrastructure,
/// fail-closed only on the banned patterns. Purely local, no network.
///
/// `VOX_HOOK_GUARD_DISABLE=1` in the HOOK PROCESS env (session-level export,
/// not settable from inside a guarded command string) short-circuits to allow
/// — for maintainer sessions working on the guard itself.
fn hook_guard_main() -> Result<()> {
    if std::env::var("VOX_HOOK_GUARD_DISABLE").as_deref() == Ok("1") {
        return Ok(());
    }
    let mut input = String::new();
    use std::io::Read;
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return Ok(());
    }
    let cmd = serde_json::from_str::<serde_json::Value>(&input)
        .ok()
        .and_then(|v| {
            v.get("tool_input")
                .and_then(|t| t.get("command"))
                .and_then(|c| c.as_str())
                .map(str::to_string)
        });
    if let Some(cmd) = cmd {
        if hook_guard_matches(&cmd) {
            eprintln!("{HOOK_GUARD_DENY}");
            std::process::exit(2);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Verify + end-to-end + commit**

Run: `cargo test -p vox-cli commands::ci::queue` — Expected: 14 PASS.
Run: `cargo clippy -p vox-cli --all-targets -- -D warnings` — Expected: clean.

End-to-end (note: after Task 6 lands, this exact command would itself be
denied by the hook because its string contains a banned phrase — that is the
known self-referential collateral; use `VOX_HOOK_GUARD_DISABLE=1` sessions or
non-Bash tools for guard maintenance):

```bash
echo '{"tool_input":{"command":"gh pr checks 431 --watch"}}' | cargo run -p vox-cli -- ci queue --hook-guard; echo "exit=$?"
```

Expected: deny message on stderr, `exit=2`.

```bash
echo '{"tool_input":{"command":"git push"}}' | cargo run -p vox-cli -- ci queue --hook-guard; echo "exit=$?"
```

Expected: silent, `exit=0`.

```bash
git add crates/vox-cli/src/commands/ci/queue.rs
git commit -m "feat(ci): vox ci queue --hook-guard — normalized patterns, loop heuristic, env escape"
```

---

### Task 5: autoscaler tick — auto-clear, force-cancel escalation, ledger

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/queue.rs` (tick entry point)
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs` (`run_scale` ~line 758; `scale_event_json` ~line 699 + call site + its tests)

- [ ] **Step 1: Add the tick entry point**

```rust
/// Autoscaler-tick entry: clear cancellable runs (apply mode), escalate to
/// force-cancel any run still in_progress that the PREVIOUS sweep cancelled
/// (shielded by always()/post steps — same two-tick pattern as
/// zombies_for_force_cancel), then persist the snapshot. Returns ACTUAL
/// (cleared_superseded, cleared_stale) — 0/0 on dry-run, which logs the
/// clearable counts to stdout instead so the ledger never claims un-done work.
pub fn auto_clear_and_snapshot(dry_run: bool, now: i64) -> Result<(u32, u32)> {
    let prev_cancelled = read_snapshot().map(|s| s.cancelled_last_sweep).unwrap_or_default();
    let mut runs = fetch_all_runs(now)?;
    let (alive, max) = fleet_counts();
    classify_runs(&mut runs, DEFAULT_TTL_MINS * 60, alive > 0);
    let failures = fetch_recent_failures(now).unwrap_or_default();
    let snap = build_snapshot(runs, failures, alive, max, false, now, Vec::new());

    let plan = clear_plan(&snap);
    let mut sup = 0u32;
    let mut stale = 0u32;
    let mut cancelled_ids = Vec::new();

    if dry_run {
        if !plan.is_empty() {
            println!("runner-scale (dry-run): {} clearable runs (not cancelled)", plan.len());
        }
    } else {
        for r in &plan {
            // Escalate if the previous sweep already cancelled this id.
            let force = r.status == "in_progress" && prev_cancelled.contains(&r.id);
            if cancel_run(r.id, force).is_err() {
                continue; // 409: completed meanwhile — next tick self-corrects
            }
            cancelled_ids.push(r.id);
            match r.class {
                RunClass::Superseded => sup += 1,
                RunClass::Stale => stale += 1,
                RunClass::Active => {}
            }
        }
    }
    let final_snap = QueueSnapshot { cancelled_last_sweep: cancelled_ids, ..snap };
    write_snapshot(&final_snap)?;
    Ok((sup, stale))
}
```

- [ ] **Step 2: Call it at the top of `run_scale`**

In `runner_scale.rs::run_scale`, immediately after the `let _lock = …;` block:

```rust
    // 0. Local-first CI: auto-clear superseded/stale runs and refresh the
    //    queue snapshot every tick (stale sweep self-disables at fleet 0).
    let (cleared_superseded, cleared_stale) =
        super::queue::auto_clear_and_snapshot(dry_run, now).unwrap_or_else(|e| {
            eprintln!("runner-scale: queue auto-clear skipped (degraded): {e:#}");
            (0, 0)
        });
```

- [ ] **Step 3: Extend the scale-event ledger**

Add two params to `scale_event_json` (keep `#[allow(clippy::too_many_arguments)]`): `cleared_superseded: u32, cleared_stale: u32`; extend the format string with `,"cleared_superseded":{cleared_superseded},"cleared_stale":{cleared_stale}` before the closing brace. Update the single call site in `run_scale` and any `#[cfg(test)]` assertions on the JSON shape (search `scale_event_json` in runner_scale's test module).

- [ ] **Step 4: Verify + commit**

Run: `cargo test -p vox-cli commands::ci::runner_scale` and `cargo test -p vox-cli commands::ci::queue` — Expected: PASS.
Run: `cargo clippy -p vox-cli --all-targets -- -D warnings` — Expected: clean.
Run: `cargo run -p vox-cli -- ci runner-scale` (dry-run, safe) — Expected: no cancellations; snapshot mtime refreshed; "clearable" line if backlog exists.

```bash
git add crates/vox-cli/src/commands/ci/queue.rs crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): autoscaler tick auto-clears queue + force-cancel escalation + ledger fields"
```

---

### Task 6: install binary, doctor diag, THEN the hooks commit

**Order inside this task is the whole point** (spec §5 hazard: a stale
`vox.exe` exits 2 on the unknown subcommand — clap's usage-error code is the
hook block code — denying every agent shell call).

**Files:**
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_health.rs`
- Create: `.claude/settings.json`

- [ ] **Step 1: Install the new binary and verify the round-trip from PATH**

Run: `cargo install --path crates/vox-cli --locked` (if the exe is locked by a
running process, use the repo's rename trick: stop or rename the running
`vox.exe` first — known gotcha).
Then verify the INSTALLED binary (not `cargo run`):

```bash
echo '{"tool_input":{"command":"gh pr checks"}}' | vox ci queue --hook-guard; echo "exit=$?"
```

Expected: deny message containing "Local-first CI", `exit=2`. If you see a
clap "unrecognized subcommand" error instead, PATH still resolves an old
binary — fix that before proceeding. **Do not start Step 4 until this passes.**

- [ ] **Step 2: Add the doctor diag (pure classifier + test first)**

In `build_health.rs` (same file as the T14 sccache-shim diag, commit
`7d480de698` — mirror its check-emission pattern with a `[diag id=..]` tag):

```rust
/// Discriminates a healthy hook-guard from the stale-binary clap collision:
/// clap usage errors also exit 2, but never carry the deny marker.
pub(crate) fn hook_guard_verdict(exit_code: i32, stderr: &str) -> Option<&'static str> {
    match (exit_code, stderr.contains("Local-first CI")) {
        (2, true) => None, // healthy: banned command blocked with the real deny
        (2, false) => Some(
            "exit 2 without deny marker — a stale vox binary on PATH is turning every \
             agent shell call into a block (clap usage error). Reinstall: \
             cargo install --path crates/vox-cli --locked",
        ),
        (0, _) => Some("banned command was NOT blocked — hook-guard inert (old binary or disabled)"),
        _ => Some("unexpected hook-guard exit code"),
    }
}
```

Test (in the file's existing `#[cfg(test)]` module):

```rust
    #[test]
    fn hook_guard_verdicts() {
        assert!(hook_guard_verdict(2, "Local-first CI: remote check-watching is disabled.").is_none());
        assert!(hook_guard_verdict(2, "error: unrecognized subcommand 'queue'").unwrap().contains("stale"));
        assert!(hook_guard_verdict(0, "").unwrap().contains("NOT blocked"));
    }
```

Wire a check that pipes `{"tool_input":{"command":"gh pr checks"}}` into
`vox ci queue --hook-guard` (the PATH binary, via the same quiet-spawn helper
the sccache check uses) and reports `hook_guard_verdict` — only when
`.claude/settings.json` exists, so the diag is silent pre-rollout.

Run: `cargo test -p vox-cli hook_guard_verdicts` — Expected: PASS.

- [ ] **Step 3: Commit the doctor diag**

```bash
git add crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_health.rs
git commit -m "feat(doctor): hook-guard round-trip diag — catches the stale-binary clap-exit-2 collision"
```

- [ ] **Step 4: Create `.claude/settings.json`** (only now)

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash|PowerShell",
        "hooks": [
          {
            "type": "command",
            "command": "vox ci queue --hook-guard"
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "vox ci queue --brief --from-snapshot"
          }
        ]
      }
    ]
  }
}
```

The matcher must be `Bash|PowerShell` — this harness exposes a PowerShell
tool whose input schema also uses `command`; a Bash-only matcher leaves the
most-used exec path on this machine unguarded (audit finding B1).

- [ ] **Step 5: Verify hook behavior in a fresh session, then commit**

From a NEW Claude Code session in the repo: (a) the SessionStart brief appears
in context; (b) a Bash `gh pr checks` attempt is denied with the local-first
message; (c) `vox doctor` reports the hook-guard diag healthy.

```bash
git add .claude/settings.json
git commit -m "feat(hooks): block remote check-watching (Bash+PowerShell) + inject queue/failure state at session start"
```

---

### Task 7: concurrency sweep + `workflow-concurrency-guard` + SSOT protection

**Files:**
- Modify: `.github/workflows/ci-health-watchdog-test.yml` (the only push/PR-triggered workflow missing `concurrency`)
- Create: `docs/src/ci/concurrency-exceptions.md`
- Create: `crates/vox-cli-ci/src/workflow_concurrency_guard.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs`, `crates/vox-cli/src/commands/ci/cmd_enums.rs` (~line 345), `run_body.rs` (~line 299), `pre_push.rs` (~lines 520 + 1081), `crates/vox-cli/src/commands/ci/constants.rs` (DOCS_SSOT_FILES)

- [ ] **Step 1: Sweep — add to `ci-health-watchdog-test.yml`** (after `on:`, before `permissions:`)

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
```

- [ ] **Step 2: Create the exceptions doc** (`docs/src/ci/concurrency-exceptions.md`)

```markdown
---
title: "Workflow concurrency exceptions"
description: "Registered exceptions for workflows that intentionally omit a concurrency group (cancel-in-progress would be wrong)."
category: "CI & Quality"
last_updated: "2026-07-02"
training_eligible: true

schema_type: "TechArticle"
---

# Workflow concurrency exceptions

`vox ci workflow-concurrency-guard` requires every workflow triggered by `push`
or `pull_request` to declare a top-level `concurrency:` group with
`cancel-in-progress: true`, so superseded runs die at the source instead of
flooding the fleet. A workflow may be listed here (backticked filename +
reason) when cancel-in-progress would be incorrect. This is also the
conceptual "never cancel" set behind the queue clearer's tag-push exemption
(`vox ci queue`, spec §2).

- `release-binaries.yml` — tag-push only; a release build must never be cancelled by a later tag.
- `release-gui.yml` — tag-push only; same as above.
- `release-installers.yml` — tag-push only; same as above.
- `scorecard.yml` — pushes to `main` only; supply-chain scorecard runs should complete, and main runs are exempt from queue clearing anyway.
```

- [ ] **Step 3: Write the failing guard tests** (create `workflow_concurrency_guard.rs` tests-first)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_detection_handles_yaml_11_on_key() {
        // serde_yaml (YAML 1.1) parses a bare `on:` key as Bool(true).
        let pr: serde_yaml::Value =
            serde_yaml::from_str("on:\n  pull_request:\n    paths: ['x']\njobs: {}").unwrap();
        assert!(needs_concurrency(&pr));
        let push: serde_yaml::Value =
            serde_yaml::from_str("on:\n  push:\n    tags: ['v*']\njobs: {}").unwrap();
        assert!(needs_concurrency(&push)); // tag-push still "needs" — exceptions doc carries it
        let sched: serde_yaml::Value =
            serde_yaml::from_str("on:\n  schedule:\n    - cron: '0 0 * * *'\njobs: {}").unwrap();
        assert!(!needs_concurrency(&sched));
        let wf_run: serde_yaml::Value =
            serde_yaml::from_str("on:\n  workflow_run:\n    types: [completed]\njobs: {}").unwrap();
        assert!(!needs_concurrency(&wf_run));
        let scalar: serde_yaml::Value = serde_yaml::from_str("on: push\njobs: {}").unwrap();
        assert!(needs_concurrency(&scalar));
        let seq: serde_yaml::Value =
            serde_yaml::from_str("on: [push, workflow_dispatch]\njobs: {}").unwrap();
        assert!(needs_concurrency(&seq));
    }

    #[test]
    fn concurrency_presence() {
        let with: serde_yaml::Value = serde_yaml::from_str(
            "on: push\nconcurrency:\n  group: g\n  cancel-in-progress: true\njobs: {}",
        )
        .unwrap();
        assert!(has_concurrency(&with));
        let without: serde_yaml::Value = serde_yaml::from_str("on: push\njobs: {}").unwrap();
        assert!(!has_concurrency(&without));
    }

    #[test]
    fn exception_matching_is_backticked_filename() {
        let doc = "- `release-binaries.yml` — tag-push only.";
        assert!(is_excepted(doc, "release-binaries.yml"));
        assert!(!is_excepted(doc, "ci.yml"));
    }
}
```

Add `pub mod workflow_concurrency_guard;` to `crates/vox-cli-ci/src/lib.rs`
(alphabetical). Run: `cargo test -p vox-cli-ci workflow_concurrency` —
Expected: compile error (functions missing).

- [ ] **Step 4: Implement the guard** (above the tests)

```rust
//! `vox ci workflow-concurrency-guard` — require a top-level `concurrency:` block on
//! every workflow triggered by `push` or `pull_request`, so superseded runs are
//! cancelled at the source (flood prevention for the local runner fleet).
//!
//! Advisory by default; `--strict` fails. Exceptions: backticked filenames in
//! `docs/src/ci/concurrency-exceptions.md` (pattern mirrors runner_policy_check.rs).

use std::path::Path;

use anyhow::{Context, Result, anyhow};

const EXCEPTIONS_DOC: &str = "docs/src/ci/concurrency-exceptions.md";

/// True when the workflow's triggers include `push` or `pull_request`.
/// serde_yaml (YAML 1.1) parses the bare `on:` key as `Bool(true)`.
fn needs_concurrency(doc: &serde_yaml::Value) -> bool {
    let Some(map) = doc.as_mapping() else { return false };
    let triggers = map
        .get(serde_yaml::Value::String("on".into()))
        .or_else(|| map.get(serde_yaml::Value::Bool(true)));
    let Some(triggers) = triggers else { return false };
    let hit = |s: &str| s == "push" || s == "pull_request";
    match triggers {
        serde_yaml::Value::String(s) => hit(s),
        serde_yaml::Value::Sequence(seq) => seq.iter().any(|v| v.as_str().is_some_and(hit)),
        serde_yaml::Value::Mapping(m) => m.keys().any(|k| k.as_str().is_some_and(hit)),
        _ => false,
    }
}

fn has_concurrency(doc: &serde_yaml::Value) -> bool {
    doc.as_mapping()
        .map(|m| m.contains_key(serde_yaml::Value::String("concurrency".into())))
        .unwrap_or(false)
}

fn is_excepted(exceptions_text: &str, file_name: &str) -> bool {
    exceptions_text.contains(&format!("`{file_name}`"))
}

pub fn run(repo_root: &Path, strict: bool) -> Result<()> {
    let exceptions_path = repo_root.join(EXCEPTIONS_DOC);
    let exceptions_text = std::fs::read_to_string(&exceptions_path)
        .with_context(|| format!("read {}", exceptions_path.display()))?;
    let wf_dir = repo_root.join(".github").join("workflows");
    let mut entries: Vec<_> = std::fs::read_dir(&wf_dir)
        .with_context(|| format!("read {}", wf_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yml" || x == "yaml"))
        .collect();
    entries.sort();
    let mut violations = Vec::new();
    for path in entries {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        if needs_concurrency(&doc) && !has_concurrency(&doc) && !is_excepted(&exceptions_text, &name) {
            violations.push(name);
        }
    }
    if violations.is_empty() {
        println!("workflow-concurrency-guard OK ({EXCEPTIONS_DOC} consulted)");
        return Ok(());
    }
    let msg = format!(
        "workflow-concurrency-guard: {} workflow(s) with push/pull_request triggers lack a \
         top-level `concurrency:` block and are not registered in {EXCEPTIONS_DOC}:\n  {}\n\
         Fix: add `concurrency: {{ group: ${{{{ github.workflow }}}}-${{{{ github.ref }}}}, \
         cancel-in-progress: true }}` or register an exception with a reason.",
        violations.len(),
        violations.join("\n  ")
    );
    if strict { Err(anyhow!(msg)) } else { eprintln!("WARN {msg}"); Ok(()) }
}
```

Run: `cargo test -p vox-cli-ci workflow_concurrency` — Expected: 3 PASS.

- [ ] **Step 5: Wire CLI + pre-push + DOCS_SSOT_FILES**

`cmd_enums.rs` after `RunnerPolicyCheck` (~line 345):

```rust
    /// Require a `concurrency:` block on push/PR-triggered workflows (flood prevention);
    /// exceptions registered in docs/src/ci/concurrency-exceptions.md.
    #[command(name = "workflow-concurrency-guard")]
    WorkflowConcurrencyGuard {
        /// Fail (exit 1) instead of advisory warn.
        #[arg(long)]
        strict: bool,
    },
```

`run_body.rs` next to the `RunnerPolicyCheck` arm (~line 299):

```rust
        CiCmd::WorkflowConcurrencyGuard { strict } => {
            vox_cli_ci::workflow_concurrency_guard::run(&root, strict)
        }
```

`pre_push.rs`: after the `runner-policy-check` OwnedStep (~line 520):

```rust
        OwnedStep {
            label: "vox ci workflow-concurrency-guard".into(),
            scope: None,
            run: Box::new(step_workflow_concurrency_guard),
        },
```

and near `step_runner_policy_check` (~line 1081):

```rust
fn step_workflow_concurrency_guard(root: &Path) -> Result<()> {
    // In-process (same as runner-policy-check) — avoids Windows nested `current_exe()`
    // spawning a stale `vox.exe`. Strict: the tree is already clean + exceptions exist.
    vox_cli_ci::workflow_concurrency_guard::run(root, true)
}
```

`constants.rs` DOCS_SSOT_FILES: add two lines (protects the guard's input doc
and the contract doc from deletion, mirroring `github-hosted-exceptions.md`):

```rust
    "docs/src/ci/concurrency-exceptions.md",
    "docs/src/ci/local-first-ci.md",
```

(The second file is created in Task 9 — create both docs before running the
docs-ssot gate, or add this constants line in Task 9 instead if executing
strictly sequentially. Keep it here only if Task 9 lands in the same push.)

- [ ] **Step 6: Verify against the real tree + commit**

Run: `cargo run -p vox-cli -- ci workflow-concurrency-guard --strict`
Expected: `workflow-concurrency-guard OK`. If it flags another workflow, the
trigger audit missed one — add the block or an exception row per its trigger
shape.
Run: `cargo clippy -p vox-cli -p vox-cli-ci --all-targets -- -D warnings` — Expected: clean.

```bash
git add .github/workflows/ci-health-watchdog-test.yml docs/src/ci/concurrency-exceptions.md crates/vox-cli-ci/src/workflow_concurrency_guard.rs crates/vox-cli-ci/src/lib.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs crates/vox-cli/src/commands/ci/pre_push.rs crates/vox-cli/src/commands/ci/constants.rs
git commit -m "feat(ci): workflow-concurrency-guard + sweep — flood prevention unregressable"
```

---

### Task 8: gate the hosted-fallback Windows smoke (audit F3)

**Files:**
- Modify: `.github/workflows/ci-fallback-hosted.yml` (`gui-windows-build-smoke` job, ~line 131)
- Modify: `docs/src/ci/github-hosted-exceptions.md` (the ci-fallback row)

- [ ] **Step 1: Add the missing gate**

`gui-windows-build-smoke` currently has NO `if:` — it runs a full Tauri
Windows build on `windows-latest` on **every PR synchronize**, contradicting
the workflow's own "outage valve / nightly mirror" header. Give it the same
condition the `gate` job already has (~line 46):

```yaml
    if: github.event_name != 'pull_request' || contains(github.event.pull_request.labels.*.name, 'fleet-down')
```

Hosted Windows smoke now runs on schedule, dispatch, and labeled outages only.

- [ ] **Step 2: True up the docs**

Update the `ci-fallback-hosted.yml` row in
`docs/src/ci/github-hosted-exceptions.md` and the workflow header comment to
state the actual behavior: "all jobs (incl. the Windows GUI smoke) run only on
nightly schedule, manual dispatch, or PRs labeled `fleet-down`."

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci-fallback-hosted.yml docs/src/ci/github-hosted-exceptions.md
git commit -m "fix(ci): gate hosted Windows smoke on fleet-down — no more per-push hosted builds"
```

---

### Task 9: the written contract — AGENTS.md + docs page

**Files:**
- Modify: `AGENTS.md` (new section after "## Local CI Gate Tiers (SSOT)" ~line 302; add the guard row to the tier table ~line 339)
- Create: `docs/src/ci/local-first-ci.md`
- Modify: `docs/src/ci/runner-autoscaling.md` (one cross-link line in the intro)

- [ ] **Step 1: AGENTS.md section** (tier-accurate wording — audit F5)

```markdown
## Local-First CI Verification Contract (Required, SSOT)

The local runner fleet is the CI plane. For agents:

- **Local gates green = the verdict for what they cover.** Run
  `vox ci pre-push --complete` (or `--full` when code/tests changed). Green =
  push and move on — never wait on remote checks. (The default fast tier omits
  clippy and all tests; do not treat fast-tier green as the verdict for code
  changes.)
- **Fleet CI is authoritative for the rest** (rustdoc, deny/audit, compiler
  gates, integration/docker/browser/GUI smokes, coverage/architecture budgets,
  all-features/mutation/cross-platform/mobile lanes). Its verdicts arrive
  asynchronously via the queue snapshot's `failures` field — surfaced at
  SessionStart and by `vox ci queue`. A red there is new information to fix
  locally, never a reason to re-push and watch.
- **Remote check-watching is blocked** for agent sessions (PreToolUse hook →
  `vox ci queue --hook-guard`): `gh pr checks`, `gh run watch`,
  check-runs polling, `vox ci watch-run`, and hand-rolled gh+sleep loops.
  Reading one failure's logs stays allowed: `gh run list --branch <b>` then
  `gh run view <id> --log-failed`.
- **Queue interactions:** `vox ci queue --json` to read (the `advice` field
  says what to do); `vox ci queue --clear` to cancel superseded + stale runs.
  Cancellable = push/pull_request events only, first attempt, non-main,
  non-tag; stale-clearing self-disables when the fleet is down.
- **No new hosted jobs / unguarded workflows:** hosted `runs-on` needs a row in
  `docs/src/ci/github-hosted-exceptions.md` (`vox ci runner-policy-check`);
  push/PR workflows need `concurrency:` or a row in
  `docs/src/ci/concurrency-exceptions.md` (`vox ci workflow-concurrency-guard`).

Details: `docs/src/ci/local-first-ci.md`.
```

Also add `vox ci workflow-concurrency-guard` to the fast-tier step list in the
"Local CI Gate Tiers (SSOT)" table.

- [ ] **Step 2: Create `docs/src/ci/local-first-ci.md`**

```markdown
---
title: "Local-first CI: queue signal, failure signal, and agent contract"
description: "The vox ci queue SSOT signal, superseded/stale auto-clearing, the async failure signal, and the hooks that keep agents on the local runner fleet."
category: "CI & Quality"
last_updated: "2026-07-02"
training_eligible: true

schema_type: "TechArticle"
---

# Local-first CI: queue signal, failure signal, and agent contract

The local runner fleet is the CI plane; GitHub Actions remains the job queue it
consumes. This page documents the machinery that keeps every harness call and
every agent on that plane mechanically.

## The contract

Local gates green (`vox ci pre-push --complete`, or `--full` when code/tests
changed) is the verdict for what they cover: push and move on. Fleet CI is
authoritative for everything without a local equivalent; its verdicts arrive
asynchronously through the failure signal below — never through an agent
sitting in a watch loop. Remote check-watching (`gh pr checks`,
`gh run watch`, check-runs polling, `vox ci watch-run`, hand-rolled gh+sleep
loops) is blocked for agent sessions by the PreToolUse hook in
`.claude/settings.json`. Reading a specific failure's logs stays allowed:
`gh run list --branch <b>`, then `gh run view <id> --log-failed`.

## `vox ci queue`

Run-centric queue snapshot. A run is cancellable only when ALL of: event is
`push` or `pull_request` (all other events — merge_group, schedule,
workflow_dispatch, workflow_run, anything unknown — are exempt by default);
branch is not `main`, not a `v<digit>…` tag, not null; first attempt
(re-runs are explicit human requests); not `waiting` (deployment approval).

- **superseded** — a strictly newer run exists for the same
  (workflow-path, head-repo, branch, event); only the newest survives.
- **stale** — `queued`/`pending` past the TTL (default 45 min,
  `--ttl-mins`) — only while the fleet has live runners. A deep queue at
  fleet zero is an outage, not abandonment; the stale sweep self-disables.

Flags: `--json`, `--brief` (SessionStart injection), `--from-snapshot`
(no network; reads `~/.vox/ci-queue-snapshot.json`, ≤2 min stale in steady
state, hard cap 10 min), `--clear [--dry-run]` (live data only; ≤50
cancellations per sweep), `--hook-guard` (PreToolUse mode;
`VOX_HOOK_GUARD_DISABLE=1` session env is the maintainer escape).

## The failure signal

Every snapshot also records completed runs from the last 24 h with conclusion
`failure`/`timed_out`/`startup_failure` (cap 20, `cancelled` excluded so
auto-clear's own work never echoes as failure). The SessionStart brief and
`vox ci queue` surface FAILED lines for the current branch and for main, and
the `advice` field leads with the fix path. This is the mechanism behind
"failures come back as a signal".

## Auto-heal

Every `vox ci runner-scale` tick (~2 min) auto-clears per the rules above,
escalates to `force-cancel` any run still in_progress one tick after being
cancelled (shielded post-steps), rewrites the snapshot atomically, and logs
`cleared_superseded`/`cleared_stale` (actual cancellations only) to the
scale-event ledger.

## Flood prevention at the source

Push/PR-triggered workflows must declare
`concurrency: { group: workflow-ref, cancel-in-progress: true }` — enforced
strictly in pre-push by `vox ci workflow-concurrency-guard`, with exceptions
in [concurrency-exceptions](concurrency-exceptions.md). The hosted fallback's
Windows smoke runs only on schedule/dispatch/`fleet-down`-labeled PRs.

## If every shell call is suddenly blocked

A stale `vox` binary on PATH exits 2 (clap usage error) on the unknown
`queue` subcommand — the same exit code the hook uses to block. Fix:
`cargo install --path crates/vox-cli --locked` (rename/stop a locked
`vox.exe` first), or temporarily remove the PreToolUse hook. `vox doctor`
detects this state.

## Deferred roadmap

Local verdict ledger (`vox ci verdict <sha>`), a local orchestration plane
bypassing the Actions queue, and hosted-job migration
(`vox ci runner-policy-check --strict` flip) — see the design spec
`docs/superpowers/specs/2026-07-02-local-first-ci-queue-design.md`.
```

- [ ] **Step 3: Cross-link + verify docs gates**

Add to the intro of `docs/src/ci/runner-autoscaling.md`:

```markdown
Queue clearing, the agent-facing queue signal, and the async failure signal
are documented in [local-first-ci](local-first-ci.md).
```

Run: `cargo run -p vox-cli -- ci check-links` — Expected: OK. Do NOT touch
`SUMMARY.md` (gitignored; generated from frontmatter at Astro build time).

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md docs/src/ci/local-first-ci.md docs/src/ci/runner-autoscaling.md
git commit -m "docs(ci): local-first CI contract — tier-accurate wording + failure signal"
```

---

### Task 10: AI-first catalog registration (spec §7)

**Files:**
- Modify: `contracts/operations/catalog.v1.yaml` (two rows, alphabetical among `ci.*` ids)
- Modify: `docs/src/reference/cli.md` (two rows — `ref_cli_required: true` demands them)
- Regenerated by tooling: `contracts/cli/command-registry.yaml`, `docs/src/reference/cli-command-surface.generated.md` (NEVER hand-edit these)

- [ ] **Step 1: Add the catalog rows** (mirror the `ci.build-bench` row shape exactly — same null fields):

```yaml
- id: ci.queue
  title: Ci Queue
  description: CLI operation `vox ci queue`
  description_human: Run-centric CI queue snapshot with active/superseded/stale classification, machine-readable advice + async failure signal, and capped clearing of cancellable backlog. SSOT queue interaction under the local-first CI contract.
  product_lane: platform
  intent_tags: []
  side_effect_class: null
  scope_kind: null
  reversible: null
  requires_repo: null
  preferred_for_models: null
  human_takeover_friendly: null
  mens_planner_visible: null
  canonical_name: null
  latin_aliases: null
  mcp: null
  cli:
    path:
    - ci
    - queue
    status: active
    latin_ns: null
    handler_rust: null
    feature_gate: null
    catalog_group: null
    ref_cli_required: true
    reachability_required: false
- id: ci.workflow-concurrency-guard
  title: Ci Workflow-concurrency-guard
  description: CLI operation `vox ci workflow-concurrency-guard`
  description_human: Require a concurrency group on push/PR-triggered workflows (flood prevention for the local runner fleet); exceptions in docs/src/ci/concurrency-exceptions.md.
  product_lane: platform
  intent_tags: []
  side_effect_class: null
  scope_kind: null
  reversible: null
  requires_repo: null
  preferred_for_models: null
  human_takeover_friendly: null
  mens_planner_visible: null
  canonical_name: null
  latin_aliases: null
  mcp: null
  cli:
    path:
    - ci
    - workflow-concurrency-guard
    status: active
    latin_ns: null
    handler_rust: null
    feature_gate: null
    catalog_group: null
    ref_cli_required: true
    reachability_required: false
```

- [ ] **Step 2: Regenerate the projection chain + document**

```bash
cargo run -p vox-cli -- ci operations-sync --target cli --write
cargo run -p vox-cli -- ci command-sync --write
```

Then add both commands to `docs/src/reference/cli.md` (one row each, matching
the surrounding `vox ci` entries' format — `ref_cli_required: true` makes
`command-compliance` demand this).

- [ ] **Step 3: Verify the compliance gates**

Run: `cargo run -p vox-cli -- ci command-compliance` — Expected: PASS.
If it flags missing metadata for the new rows, follow its error text — the
guard's messages name the exact field and file.

- [ ] **Step 4: Commit**

```bash
git add contracts/operations/catalog.v1.yaml contracts/cli/command-registry.yaml docs/src/reference/cli-command-surface.generated.md docs/src/reference/cli.md
git commit -m "feat(ci): register vox ci queue + workflow-concurrency-guard in the operations catalog"
```

---

### Task 11: full-gate verification + push

- [ ] **Step 1: Full local gates**

Run: `cargo run -p vox-cli -- ci pre-push --complete`
Expected: all steps PASS, including the two new guards. (Locked-`vox.exe`
gotcha: stop the main-dir `vox.exe` first if the hook build fails.)

- [ ] **Step 2: Workspace clippy (house invocation)**

Run: `cargo clippy --workspace --all-targets --exclude vox-gui -- -D warnings`
Expected: clean.

- [ ] **Step 3: Push and confirm — the new-contract way**

```bash
git push -u origin claude/interesting-leavitt-246363
```

Verify with `gh pr view` (push "error" output can be spurious). Then per the
contract this branch just shipped: do NOT watch the remote checks — local
gates passed; any CI red will arrive in the next session's brief via the
failure signal.
