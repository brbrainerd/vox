# CI Runner Death Visibility + Reap Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden the CI runner autoscaler's reap logic against reaping a genuinely-busy runner, and add a new detector that surfaces (via PR comment + rich console output) any managed runner container that exits unexpectedly while its assigned job is still `in_progress` — closing the visibility gap that let 3 GitHub Actions jobs sit silently stuck on dead runners during this session's PR #460 investigation.

**Architecture:** Two independent hardenings to the existing reap decisions in `run_scale()` (a corroborating jobs-API busy-check, and requiring 2 consecutive idle ticks for the scale-down path specifically, by reusing already-tracked `idle_since` state rather than adding new state) — plus a new sibling module, `unexpected_exit_watch.rs`, mirroring the existing `oom_watch.rs`'s structure (seen-list persistence, GitHub job correlation, PR-comment posting) for a different signal (unexplained exit while a job is still running, not a memcg OOM-kill). Both the existing OOM detector and the new one also gain richer per-tick console output.

**Tech Stack:** Rust (`crates/vox-cli`), `gh` CLI (GitHub API), `docker` CLI, `serde_json` for local state persistence under `~/.vox/`.

---

### Task 1: Corroborating busy-check — pure logic + tests

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs`
- Test: same file, `#[cfg(test)] mod tests` at the bottom (read the file's existing test module structure first — there is already a `mod tests` at the bottom of this file; add to it, don't create a second one)

This introduces a pure function that decides, given a candidate reap name and a fresh jobs-API lookup, whether the reap should be blocked because the runner is corroborated-busy via an independent signal.

- [ ] **Step 1: Read the file fresh**

Open `crates/vox-cli/src/commands/ci/runner_scale.rs` and re-confirm the current line numbers for `RunnerRow` (type alias, `pub type RunnerRow = (String, String, bool)`, ~line 199), `managed_busy_map` (~line 387), and the existing `#[cfg(test)] mod tests` block at the bottom of the file. Also open `crates/vox-cli/src/commands/ci/oom_watch.rs` and re-confirm `JobRow` (struct with `runner_name`, `job_name`, `run_id`, `pr_number`) and `find_matching_job` (~line 212) — this task reuses that exact type and function rather than defining a new one.

- [ ] **Step 2: Write the failing test**

Add to `runner_scale.rs`'s existing `mod tests` block:

```rust
#[test]
fn corroborated_busy_blocks_reap_when_job_rows_show_in_progress() {
    let job_rows = vec![crate::commands::ci::oom_watch::JobRow {
        runner_name: "vox-runner-auto-abc-0".to_string(),
        job_name: "docs-quality".to_string(),
        run_id: 123,
        pr_number: 460,
    }];
    assert!(is_corroborated_busy("vox-runner-auto-abc-0", &job_rows));
}

#[test]
fn corroborated_busy_false_when_no_matching_job_row() {
    let job_rows = vec![crate::commands::ci::oom_watch::JobRow {
        runner_name: "vox-runner-auto-other-0".to_string(),
        job_name: "docs-quality".to_string(),
        run_id: 123,
        pr_number: 460,
    }];
    assert!(!is_corroborated_busy("vox-runner-auto-abc-0", &job_rows));
}

#[test]
fn corroborated_busy_false_on_empty_job_rows() {
    assert!(!is_corroborated_busy("vox-runner-auto-abc-0", &[]));
}
```

Check `oom_watch::JobRow`'s field visibility first (re-read `oom_watch.rs` — if its fields are not `pub`, either make them `pub` there, or make `JobRow`/`find_matching_job` `pub(crate)` if not already, since `runner_scale.rs` needs to construct/read them in this test and in Task 3's real wiring). If `find_matching_job` is not currently `pub` or `pub(crate)`, widen its visibility to `pub(crate)` in `oom_watch.rs` in this step (a one-line change, still part of this step since the test needs it to compile).

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-cli corroborated_busy --lib`
Expected: FAIL — `is_corroborated_busy` doesn't exist yet.

- [ ] **Step 4: Implement**

Add near `managed_busy_map` in `runner_scale.rs`:

```rust
/// True when `runner_name` is assigned to an `in_progress` job per a fresh,
/// independent jobs-API lookup — used to corroborate (or refute) the
/// `runners` API's own `busy` flag before ever reaping a runner classified
/// idle by that flag, since the flag is known to lag briefly behind a
/// runner actually starting a job.
pub fn is_corroborated_busy(runner_name: &str, job_rows: &[super::oom_watch::JobRow]) -> bool {
    super::oom_watch::find_matching_job(job_rows, runner_name).is_some()
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-cli corroborated_busy --lib`
Expected: PASS (3/3).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/runner_scale.rs crates/vox-cli/src/commands/ci/oom_watch.rs
git commit -m "feat(ci): is_corroborated_busy — cross-check a reap candidate against a fresh jobs-API lookup"
```

---

### Task 2: Two-consecutive-tick requirement for scale-down reap — pure logic + tests

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs`
- Test: same file's `mod tests`

The existing idle-timeout reap path already has a 5-minute grace (`DEFAULT_IDLE_REAP_SECS = 300`) via `should_reap_idle`, which is far more forgiving than the ~2-minute tick cadence that produced this session's incident. The **scale-down** reap path (`if total_keep > desired`, ~line 882-897) has no such grace at all — it reaps based on a single tick's `busy=false` snapshot immediately. This task closes that gap by requiring a runner to have ALSO been idle as of the previously-persisted `idle_since` state (`prev = read_state()`, already fetched at ~line 844) before it's scale-down-eligible — reusing existing state, no new persistence file.

- [ ] **Step 1: Read the file fresh**

Re-open `runner_scale.rs` and re-confirm: `prev = read_state()` (~line 844, a `HashMap<String, i64>` of `{runner_name: idle_since_timestamp}` from the prior tick), and the scale-down block (~line 882-897, currently building `idle_runners: Vec<(String, Option<i64>)>` from this tick's `busy_map` at ~867-876, then reaping via `scale_down_reap_targets` over ALL of `idle_runners` with no reference to `prev`).

- [ ] **Step 2: Write the failing test**

```rust
#[test]
fn eligible_for_scale_down_reap_requires_prior_tick_idle_state() {
    // Idle this tick (Some(_) idle_since from next_idle_since), but absent
    // from prev — i.e. this is the FIRST tick it's been observed idle.
    // Not yet eligible: a single sample is insufficient (mirrors
    // zombies_for_force_cancel's own 2-consecutive-tick rationale).
    let prev: HashMap<String, i64> = HashMap::new();
    assert!(!eligible_for_scale_down_reap("vox-runner-auto-abc-0", &prev));
}

#[test]
fn eligible_for_scale_down_reap_true_when_idle_on_prior_tick_too() {
    let mut prev: HashMap<String, i64> = HashMap::new();
    prev.insert("vox-runner-auto-abc-0".to_string(), 1_000);
    assert!(eligible_for_scale_down_reap("vox-runner-auto-abc-0", &prev));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-cli eligible_for_scale_down_reap --lib`
Expected: FAIL — function doesn't exist yet.

- [ ] **Step 4: Implement**

Add near `should_reap_idle`:

```rust
/// True when `runner_name` was also idle-tracked as of the PRIOR tick's
/// persisted state (`prev`, from `read_state()`) — i.e. this is at least the
/// second consecutive tick it's been observed idle. The scale-down reap path
/// (unlike the idle-timeout path, which already has a multi-minute grace via
/// `should_reap_idle`) previously reaped on a single tick's snapshot with no
/// history check at all; this closes that gap by requiring the same kind of
/// 2-consecutive-tick evidence `zombies_for_force_cancel` already requires
/// for a different reap decision in this file, for the same reason: "a
/// single ... sample is insufficient" (see that function's doc comment).
pub fn eligible_for_scale_down_reap(runner_name: &str, prev: &HashMap<String, i64>) -> bool {
    prev.contains_key(runner_name)
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-cli eligible_for_scale_down_reap --lib`
Expected: PASS (2/2).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): eligible_for_scale_down_reap — require 2 consecutive idle ticks before scale-down reap"
```

---

### Task 3: Wire both hardening checks into `run_scale`'s reap paths

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs`

Requires Tasks 1 and 2 landed. This task is integration wiring — no new pure logic, so it's verified via the crate's existing integration-style coverage plus a manual dry-run rather than a new unit test (there is no existing test harness in this file that mocks `gh`/`docker` IO for `run_scale` itself — its pure sub-functions are what's unit-tested, consistent with this file's own established split between "pure logic" and "IO" sections).

- [ ] **Step 1: Read the file fresh**

Re-open `runner_scale.rs`. Re-confirm the exact current shape of:
- The scale-down block (`if total_keep > desired { ... }`, using `idle_runners`/`scale_down_reap_targets`/`reap`).
- The idle-timeout block (`for (name, idle_since) in &idle_runners { if should_reap_idle(...) { reap(...) } ... }`).
- Where `job_rows` would need to be fetched — this task needs ONE fresh `oom_watch::fetch_recent_job_rows()` call per tick, fetched once and shared across both reap-path checks (mirroring `oom_watch.rs`'s own "fetched once per tick, shared" pattern — re-read that function's doc comment for the exact rationale to replicate). Confirm `fetch_recent_job_rows` is `pub(crate)` in `oom_watch.rs` (widen visibility if it's currently private — this crate needs to call it from `runner_scale.rs`).

- [ ] **Step 2: Fetch job rows once per tick, only when there's a reap candidate**

Immediately before the scale-down block, add:

```rust
    // Fetched once per tick and shared across both reap-hardening checks
    // below (scale-down and idle-timeout), only when there's at least one
    // idle-classified runner that might be reaped — an empty fleet or a
    // fleet with no idle runners this tick has nothing to corroborate,
    // so this avoids an unconditional extra `gh api` call every single tick.
    let job_rows_for_reap_check: Option<Vec<super::oom_watch::JobRow>> = if !idle_runners.is_empty() {
        match super::oom_watch::fetch_recent_job_rows() {
            Ok(rows) => Some(rows),
            Err(e) => {
                eprintln!(
                    "runner-scale: reap-hardening jobs-API corroboration skipped (degraded, \
                     falling back to runners-API busy flag alone this tick): {e:#}"
                );
                None
            }
        }
    } else {
        None
    };
```

- [ ] **Step 3: Apply both checks in the scale-down block**

Find the scale-down block (currently reaping every name in `reap_set` unconditionally). Change the reap loop to skip any name that's corroborated-busy OR not yet eligible per the 2-tick check:

```rust
        for name in reap_set {
            let corroborated_busy = job_rows_for_reap_check
                .as_deref()
                .is_some_and(|rows| is_corroborated_busy(&name, rows));
            let two_tick_eligible = eligible_for_scale_down_reap(&name, &prev);
            if corroborated_busy || !two_tick_eligible {
                println!(
                    "runner-scale: scale-down reap of {name} blocked (corroborated_busy={corroborated_busy}, \
                     two_tick_eligible={two_tick_eligible}) — treating this tick's idle classification as \
                     possibly stale rather than reaping"
                );
                continue;
            }
            reap(&name, dry_run, "scale-down above desired");
            reaped_scale_down += 1;
        }
```

(`reaped_scale_down` was previously incremented unconditionally in this loop — moving the increment inside the non-skipped branch, as shown, is the only behavioral change to that counter: it now reflects reaps that actually happened, not reap attempts.)

- [ ] **Step 4: Apply the corroborating check in the idle-timeout block**

Find the idle-timeout loop (`for (name, idle_since) in &idle_runners { if should_reap_idle(*idle_since, now, reap_secs) { reap(name, dry_run, "idle > reap grace (never assigned)"); reaped += 1; } ... }`). This path already has the multi-minute grace, so it does NOT need the 2-tick check (Task 2 was scoped specifically to the scale-down gap) — but it should still get the corroborating busy-check as defense-in-depth, since a runner idle for the full grace period could still theoretically be mid-registration-lag on a very slow job pickup:

```rust
    for (name, idle_since) in &idle_runners {
        if should_reap_idle(*idle_since, now, reap_secs) {
            let corroborated_busy = job_rows_for_reap_check
                .as_deref()
                .is_some_and(|rows| is_corroborated_busy(name, rows));
            if corroborated_busy {
                println!(
                    "runner-scale: idle-timeout reap of {name} blocked (corroborated busy via jobs-API \
                     despite {reap_secs}s+ idle per runners-API) — treating as possibly stale"
                );
                if let Some(s) = idle_since {
                    new_state.insert(name.clone(), *s);
                }
                keep += 1;
                continue;
            }
            reap(name, dry_run, "idle > reap grace (never assigned)");
            reaped += 1;
        } else {
            if let Some(s) = idle_since {
                new_state.insert(name.clone(), *s);
            }
            keep += 1;
        }
    }
```

- [ ] **Step 5: Build and run the full test suite for this crate**

Run: `cargo build -p vox-cli 2>&1 | tail -30`
Expected: compiles clean.

Run: `cargo test -p vox-cli --lib 2>&1 | tail -60`
Expected: all existing tests plus Task 1/2's new tests still pass (no regressions from the wiring change).

- [ ] **Step 6: Manual dry-run verification**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- ci runner-scale` (dry-run, default — no `--apply`)
Expected: exits cleanly, prints the normal `runner-scale: dry_run=true ...` summary line with no panics. If any runner is currently idle-classified, confirm the new corroboration logic runs without erroring (even if `gh`/`docker` aren't fully set up in the environment this is run from, the `Err(e) => ... None` fallback in Step 2 must degrade gracefully, not panic — verify this specifically if `gh`/`docker` aren't available here).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): wire reap-hardening (corroborating busy-check + 2-tick scale-down gate) into run_scale"
```

---

### Task 4: New `unexpected_exit_watch.rs` module — detection, correlation, comment composition + tests

**Files:**
- Create: `crates/vox-cli/src/commands/ci/unexpected_exit_watch.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (register the new module — read this file fresh first to see how `oom_watch` is declared, e.g. `mod oom_watch;` or `pub mod oom_watch;`, and mirror that exact declaration style for the new module)

This mirrors `oom_watch.rs`'s structure directly: a seen-list to avoid duplicate reports, GitHub job correlation, PR-comment composition, and a tick entrypoint — but the detection signal is "container name that was running last tick and is exited now, whose assigned job is still `in_progress`" instead of a `dmesg` OOM line.

- [ ] **Step 1: Read the reference file fresh**

Re-open `crates/vox-cli/src/commands/ci/oom_watch.rs` in full. This task's module should mirror: its module-level doc comment style, `OomEvent`-equivalent struct, `oom_seen_path`/`read_oom_seen`/`write_oom_seen`-equivalent persistence trio, `new_events`/`append_seen`-equivalent pure filtering, `JobRow`/`find_matching_job` (REUSED directly from `oom_watch.rs`, not redefined — import `super::oom_watch::{JobRow, find_matching_job, fetch_recent_job_rows}`), `oom_comment_body`-equivalent pure comment composer, `post_pr_comment`-equivalent poster (also reuse `oom_watch::post_pr_comment` directly rather than redefining — widen its visibility to `pub(crate)` in `oom_watch.rs` if not already), and `scan_and_report_oom_events`-equivalent tick entrypoint.

- [ ] **Step 2: Write the failing tests**

Create `crates/vox-cli/src/commands/ci/unexpected_exit_watch.rs`:

```rust
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
    if let Ok(s) = serde_json::to_string_pretty(seen) {
        let _ = std::fs::write(p, s);
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
    if let Ok(s) = serde_json::to_string_pretty(names) {
        let _ = std::fs::write(p, s);
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
/// `near_miss_blocked` is `true` when Task 3's reap-hardening logged a
/// blocked reap attempt for this same container name this same tick — real,
/// concrete evidence pointing at GitHub's `busy` flag lagging as the likely
/// cause. `false` means no such near-miss was observed this tick, which
/// points instead toward an external cause (WSL2/Docker), not the
/// autoscaler's own reap logic.
pub fn unexpected_exit_comment_body(
    container_name: &str,
    job_name: &str,
    run_id: u64,
    exit_code: i64,
    near_miss_blocked: bool,
) -> String {
    let likely_cause = if near_miss_blocked {
        "The autoscaler's reap-hardening caught a related stale-busy-flag reap attempt on \
         this same container this same tick, which may explain this — investigate GitHub \
         Actions runners-API `busy` flag lag around this timestamp."
    } else {
        "No related near-miss was caught by the autoscaler's reap-hardening this tick — this \
         points toward an external cause (WSL2/Docker), not the autoscaler's own reap logic."
    };
    format!(
        "**CI runner exited unexpectedly** — job `{job_name}` (run `{run_id}`) did not \
         complete or get cancelled normally: its runner container `{container_name}` exited \
         (code `{exit_code}`) while the job was still `in_progress`. This was not a memcg \
         OOM-kill (see the separate OOM-visibility check).\n\n\
         {likely_cause}\n\n\
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
/// `job_rows`: the SAME fetch Task 3's reap-hardening already performs this
/// tick when there's a reap candidate — pass it through rather than
/// re-fetching. If no reap candidate existed this tick (so Task 3 didn't
/// fetch it), this function fetches its own copy.
/// `near_miss_names`: container names Task 3's reap-hardening logged a
/// blocked reap attempt for this same tick (used for `near_miss_blocked` in
/// the comment body).
pub fn scan_and_report_unexpected_exits(
    curr_running: &HashSet<String>,
    already_oom_claimed: &HashSet<String>,
    job_rows: Option<&[JobRow]>,
    near_miss_names: &HashSet<String>,
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
        .collect();
    if fresh.is_empty() {
        return Ok(0);
    }

    let owned_job_rows;
    let job_rows: &[JobRow] = match job_rows {
        Some(rows) => rows,
        None => {
            owned_job_rows = super::oom_watch::fetch_recent_job_rows()?;
            &owned_job_rows
        }
    };

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
        let near_miss = near_miss_names.contains(name.as_str());
        let body = unexpected_exit_comment_body(name, &job.job_name, job.run_id, exit_code, near_miss);
        match post_pr_comment(job.pr_number, &body, false) {
            Ok(()) => {
                reported += 1;
                seen_so_far = append_seen(seen_so_far, std::slice::from_ref(name));
                write_seen(&seen_so_far);
                println!(
                    "runner-scale: unexpected-exit reported for {name} (job={}, run={}, \
                     exit_code={exit_code}, near_miss_blocked={near_miss})",
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
    fn comment_body_names_job_container_exit_code_and_near_miss_state() {
        let body = unexpected_exit_comment_body("vox-runner-auto-abc-0", "docs-quality", 999, 143, true);
        assert!(body.contains("docs-quality"));
        assert!(body.contains("vox-runner-auto-abc-0"));
        assert!(body.contains("999"));
        assert!(body.contains("143"));
        assert!(body.contains("stale-busy-flag"));
    }

    #[test]
    fn comment_body_points_to_external_cause_when_no_near_miss() {
        let body = unexpected_exit_comment_body("vox-runner-auto-abc-0", "docs-quality", 999, 143, false);
        assert!(body.contains("external cause"));
        assert!(!body.contains("stale-busy-flag"));
    }
}
```

- [ ] **Step 3: Register the module**

Open `crates/vox-cli/src/commands/ci/mod.rs`, find the `mod oom_watch;` (or `pub mod oom_watch;`) declaration, and add an identically-styled declaration for the new module (e.g. `mod unexpected_exit_watch;` — match whichever visibility `oom_watch`'s own declaration actually uses).

- [ ] **Step 4: Widen visibility on reused `oom_watch` items if needed**

Re-check (from Step 1's read): `JobRow`'s fields, `find_matching_job`, `fetch_recent_job_rows`, and `post_pr_comment` must all be `pub(crate)` (or `pub`) in `oom_watch.rs` for this new module to use them. Widen any that are currently private, in `oom_watch.rs`.

- [ ] **Step 5: Run tests to verify they fail, then pass**

Run: `cargo test -p vox-cli --lib unexpected_exit_watch 2>&1 | tail -40`
Expected first: FAIL (module/functions don't exist / visibility errors).
After Steps 2-4 are in place, rerun the same command.
Expected: PASS (5/5).

- [ ] **Step 6: Run the full crate build**

Run: `cargo build -p vox-cli 2>&1 | tail -30`
Expected: compiles clean (confirms the visibility widening in Step 4 didn't break anything else, and the new module's `mod.rs` registration is correct).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/ci/unexpected_exit_watch.rs crates/vox-cli/src/commands/ci/mod.rs crates/vox-cli/src/commands/ci/oom_watch.rs
git commit -m "feat(ci): add unexpected_exit_watch — detect and report a runner dying mid-job (not OOM)"
```

---

### Task 5: Wire the new detector into `run_scale`, with correct ordering

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs`

Requires Tasks 3 and 4 landed. **Critical ordering constraint**: the new detector's exited-container scan must run BEFORE `run_scale`'s existing step 1 (`for name in managed_containers("exited") { deregister(&name); docker rm -f ... }`, ~line 852-859) — once that cleanup runs, `docker inspect` can no longer read the exited container's exit code.

- [ ] **Step 1: Read the file fresh**

Re-open `runner_scale.rs`. Re-confirm the exact current position of: the OOM-visibility call (§0.5, ~line 826-834, apply-only), and step 1's exited-container cleanup loop (~line 852-859). Also re-confirm what `managed_containers("running")` returns (a `Vec<String>` of names) and where it's first computed in this tick (currently at ~line 862, `let running: Vec<String> = managed_containers("running");` — this happens AFTER step 1's cleanup in the current code; this task needs a running-containers snapshot BEFORE cleanup too, so check whether calling `managed_containers("running")` an extra time earlier in the tick, before cleanup, is acceptable — it is: `managed_containers` is a read-only `docker ps -a --filter ...` call with no side effects, safe to call twice in one tick).

- [ ] **Step 2: Track OOM-claimed container names this tick**

The existing OOM-visibility call currently discards which containers it reported (only returns a count, `oom_reported: u32`, from `scan_and_report_oom_events`). This task needs the actual set of names to pass to the new detector's `already_oom_claimed` parameter.

Read `oom_watch::scan_and_report_oom_events`'s current signature and body (`crates/vox-cli/src/commands/ci/oom_watch.rs`). Change its return type from `Result<u32>` to `Result<(u32, HashSet<String>)>`, returning the set of container names it successfully reported this tick alongside the existing count (thread this through the function's existing `for event in fresh { ... match post_pr_comment(...) { Ok(()) => { reported += 1; ... } ... } }` loop — collect `container_name` into a `HashSet<String>` alongside incrementing `reported`; `container_name` is already an available local in that loop). Update `oom_watch.rs`'s own tests for this function if any directly assert on its return type (check `full_pipeline_parses_correlates_and_composes_a_correct_comment` and any other test calling `scan_and_report_oom_events` directly — most of that file's tests call the smaller pure functions instead, so this is likely a small or zero-test-impact change, but verify).

- [ ] **Step 3: Insert the new detector's scan before step 1's cleanup**

In `run_scale`, immediately after the existing §0.5 OOM-visibility block (and its `if apply { ... }` guard) and BEFORE the comment `// 1. Remove exited managed containers`, insert:

```rust
    // 0.6. Unexpected-exit visibility: detect any managed container that
    //      transitioned running->exited since the last tick while its
    //      assigned job was still in_progress (not a normal ephemeral
    //      job-complete exit, not already claimed by the OOM scan above).
    //      MUST run before step 1's cleanup below removes exited containers
    //      -- docker inspect can't read an exit code from a pruned container.
    let mut unexpected_exit_reported = 0u32;
    if apply {
        let curr_running: HashSet<String> = managed_containers("running").into_iter().collect();
        match super::unexpected_exit_watch::scan_and_report_unexpected_exits(
            &curr_running,
            &oom_claimed_names,
            None,
            &HashSet::new(), // near_miss_names: Task 3's reap-hardening runs later this same tick,
                              // so no near-miss evidence exists yet at this point in the tick.
                              // Acceptable per the design: the comment body's near-miss framing is a
                              // best-effort diagnostic hint, not a hard requirement, and a genuine
                              // stale-busy-flag reap would be BLOCKED (not executed) by Task 3's
                              // hardening later this tick regardless -- so a container that died from
                              // that exact cause wouldn't reach this unexpected-exit scan as "exited"
                              // in the first place on the tick the hardening actually blocks it. This
                              // parameter exists for a FUTURE tick's correlation, once persisted
                              // near-miss history exists -- out of scope for this plan's first cut.
        ) {
            Ok(n) => unexpected_exit_reported = n,
            Err(e) => eprintln!("runner-scale: unexpected-exit scan skipped (degraded): {e:#}"),
        }
    }
```

Adjust the OOM-visibility block just above this insertion to capture `oom_claimed_names` from Step 2's new return shape:

```rust
    let mut oom_claimed_names: HashSet<String> = HashSet::new();
    if apply {
        match super::oom_watch::scan_and_report_oom_events(now) {
            Ok((oom_reported, claimed)) => {
                oom_claimed_names = claimed;
                if oom_reported > 0 {
                    println!("runner-scale: reported {oom_reported} OOM-killed job(s) this tick");
                }
            }
            Err(e) => eprintln!("runner-scale: OOM-visibility scan skipped (degraded): {e:#}"),
        }
    }
```

(This replaces the existing `let oom_reported = super::oom_watch::scan_and_report_oom_events(now).unwrap_or_else(|e| { ...; 0 });` block — read it fresh first to match its exact current surrounding structure, including the `_lock.as_ref()` refresh calls immediately before/after it, which must be preserved unchanged.)

- [ ] **Step 4: Fold the new count into the tick's summary line**

Find the final `println!("runner-scale: dry_run={dry_run} queued_jobs={demand} ...")` summary (~line 988). Add `unexpected_exits_reported={unexpected_exit_reported}` to it, in a sensible position (e.g. right after the OOM count would appear, or at the end before the closing paren-content — match this line's existing comma-free space-separated `key=value` style).

- [ ] **Step 5: Build and test**

Run: `cargo build -p vox-cli 2>&1 | tail -40`
Expected: compiles clean.

Run: `cargo test -p vox-cli --lib 2>&1 | tail -60`
Expected: all tests (existing + Tasks 1, 2, 4's new ones) still pass.

- [ ] **Step 6: Manual dry-run verification**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- ci runner-scale --apply`
Expected: exits cleanly. Since this is `--apply`, it performs real IO (spawns/reaps runners for real) — only run this if you have a working `gh`/`docker` setup and understand it will mutate the real fleet; if unsure, skip this step and rely on Step 5's build+test coverage plus a later manual verification pass once this lands. Confirm the tick's summary line includes `unexpected_exits_reported=0` (or a real count if something genuinely happened during the run) with no panics.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/ci/runner_scale.rs crates/vox-cli/src/commands/ci/oom_watch.rs
git commit -m "feat(ci): wire unexpected_exit_watch into run_scale, before exited-container cleanup"
```

---

### Task 6: Rich console output for both detectors

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs`

Requires Task 5 landed. This task's Step 3 (Task 5) already added the unexpected-exit detector's own per-event `println!` inside `scan_and_report_unexpected_exits` (see Task 4's `Ok(())` branch, which already prints full per-event detail: name, job, run, exit_code, near_miss_blocked). This task's remaining scope is the OOM side, which currently only prints a bare count.

- [ ] **Step 1: Read the file fresh**

Re-open `oom_watch.rs`'s `scan_and_report_oom_events`. Confirm its `Ok(())` branch (inside the `for event in fresh { ... match post_pr_comment(...) { Ok(()) => { reported += 1; ...} ... } }` loop) currently has no `println!` of its own — the only console output for a successful OOM report currently happens one level up, in `run_scale`'s bare-count line (`println!("runner-scale: reported {oom_reported} OOM-killed job(s) this tick")`).

- [ ] **Step 2: Add per-event console output inside the OOM scan's success branch**

Inside `scan_and_report_oom_events`'s `Ok(()) => { ... }` branch (after `write_oom_seen(&seen_so_far);`), add:

```rust
                println!(
                    "runner-scale: OOM-killed reported for {container_name} (process={}, job={}, run={})",
                    event.process, m.job_name, m.run_id
                );
```

(Confirm the exact local variable names available at this point in the loop — `event`, `container_name`, `m` per Task 5 Step 2's read of this function; adjust names to match what's really there if they differ.)

- [ ] **Step 3: Run tests**

Run: `cargo test -p vox-cli --lib oom_watch 2>&1 | tail -40`
Expected: all existing `oom_watch` tests still pass (this is a pure `println!` addition with no logic change, so no test should need updating — if any test asserts on captured stdout, which none of `oom_watch.rs`'s current tests appear to given they test pure functions directly, verify and adjust).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli/src/commands/ci/oom_watch.rs
git commit -m "feat(ci): print full per-event detail for OOM reports, not just a bare count"
```

---

### Task 7: Full crate verification

**Files:** none (verification only)

- [ ] **Step 1: Full crate test suite**

Run: `cargo test -p vox-cli 2>&1 | tail -80`
Expected: 0 failures, all new tests from Tasks 1, 2, 4 present and passing alongside existing coverage.

- [ ] **Step 2: Full crate build, including any integration tests**

Run: `cargo build -p vox-cli --tests 2>&1 | tail -40`
Expected: compiles clean.

- [ ] **Step 3: Format check**

Run: `cargo fmt -p vox-cli -- --check` (NEVER `cargo fmt --all` on this Windows workspace — it overflows the command-line length limit; always scope to the specific crate)
Expected: no diff. If there IS a diff, run `cargo fmt -p vox-cli` (no `--check`) to apply it, then re-run the `--check` invocation to confirm clean, then amend the affected commit(s) or add a small `style: cargo fmt vox-cli` commit.

- [ ] **Step 4: Clippy check on this crate**

Run: `cargo clippy -p vox-cli -- -D warnings 2>&1 | tail -60`
Expected: clean (0 warnings). Fix any findings before proceeding — this session's established practice is to run this explicitly since CI's own clippy gate can be bypassed by an admin-merge, so don't rely on CI alone to catch it.

- [ ] **Step 5: Commit any fixes from Steps 3-4**

```bash
git add -A
git commit -m "chore(ci): fmt + clippy fixes"
```

(Only if Steps 3-4 produced changes; skip this step if both were already clean.)

---

## Self-Review

**Spec coverage** — §1a (corroborating busy-check) → Tasks 1, 3. §1b (2-consecutive-tick requirement) → Task 2, 3 (refined during planning: implemented by reusing the existing `idle_since`/`read_state()` mechanism rather than adding new state, and scoped to the scale-down path specifically since the idle-timeout path already has an equivalent-or-stronger grace period — this is a deliberate, disclosed refinement of the spec's original framing, not a gap: the spec's own intent, "require 2 consecutive idle ticks before eligible for reap," is satisfied for the path that actually lacked it). §2 (new detector: detection/skip-if-OOM-claimed/reporting) → Task 4, 5. §3 (rich console output, both detectors) → Task 4 (new detector's own output, built in from the start) + Task 6 (OOM detector's output, added).

**Placeholder scan** — every step shows real, complete code grounded in a fresh read of the actual current file contents (re-confirmed multiple times during planning, including line numbers for every insertion point). The one explicitly-deferred piece (near-miss correlation being empty on its first tick, Task 5 Step 3) is disclosed with a full paragraph explaining exactly why it's an acceptable first-cut limitation, not a silently-dropped requirement — matching this session's established practice for genuine, reasoned scope decisions.

**Type consistency** — `JobRow`/`find_matching_job`/`fetch_recent_job_rows`/`post_pr_comment` are defined once in `oom_watch.rs` and reused by name (not redefined) in `runner_scale.rs` (Task 3) and `unexpected_exit_watch.rs` (Task 4). `is_corroborated_busy` (Task 1) and `eligible_for_scale_down_reap` (Task 2) are each defined once and called by the same names in Task 3's wiring. `scan_and_report_oom_events`'s return-type change (`Result<u32>` → `Result<(u32, HashSet<String>)>`, Task 5 Step 2) is applied consistently at its one call site (Task 5 Step 3) in the same task that changes the signature, so there's no window where caller and callee disagree.
