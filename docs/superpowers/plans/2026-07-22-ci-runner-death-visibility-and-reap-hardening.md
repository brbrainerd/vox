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

`oom_watch::JobRow` (struct + all 4 fields) and `find_matching_job` are already `pub` — confirmed by fidelity review against the real file, no visibility change needed for this test. (Two OTHER `oom_watch.rs` items — `fetch_recent_job_rows` and `post_pr_comment`, both currently private — DO need widening to `pub(crate)`, but not until Task 3/4 actually call them; don't widen them here, it'd be dead scope creep for this task.)

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

Requires Tasks 1 and 2 landed.

**Critical correctness requirement (found by adversarial review of this plan before implementation began — read this before writing any code):** a scale-down candidate that gets BLOCKED by the new hardening (corroborated-busy, or not yet 2-tick-eligible) must still end up in the same tracked-idle state the idle-timeout path consumes afterward. The naive approach — strip every name in `reap_set` out of `idle_runners` unconditionally (via the pre-existing `idle_runners.retain(|(name, _)| !reap_set.contains(name));` line), THEN loop over `reap_set` deciding per-name whether to actually call `reap()` — silently drops a blocked candidate's `idle_since` from `new_state` regardless of whether it was actually reaped, because the `retain()` already removed it from `idle_runners` before the per-name decision is made. Next tick, that runner looks freshly-idle again (no entry in `prev`), gets selected as a scale-down candidate again, gets blocked again by the SAME 2-tick check (since it's "first tick idle" again from the state's perspective) — forever, as long as it keeps being an excess candidate. This would silently defeat scale-to-zero for exactly the runners this hardening targets, with no error, no log line indicating anything is wrong (the runner just never gets reaped, indistinguishable from "everything is fine"). The fix (Step 3 below) is to only strip a name from `idle_runners` when it's ACTUALLY reaped, not merely selected as a candidate — a blocked candidate stays in `idle_runners` and flows into the idle-timeout loop like any other still-idle runner.

- [ ] **Step 1: Read the file fresh**

Re-open `runner_scale.rs`. Re-confirm the exact current shape of:
- The scale-down block (`if total_keep > desired { ... }`, using `idle_runners`/`scale_down_reap_targets`/`reap`, and the `idle_runners.retain(...)` call that currently runs BEFORE the reap loop).
- The idle-timeout block (`for (name, idle_since) in &idle_runners { if should_reap_idle(...) { reap(...) } ... }`).
- Confirm `fetch_recent_job_rows` and `post_pr_comment` are still private in `oom_watch.rs` (per Task 1's fidelity-checked note) — this task widens `fetch_recent_job_rows` to `pub(crate)` (it needs it; `post_pr_comment` stays private for now, Task 4 widens that one when it's actually needed).

This task's own fetch below is intentionally NOT yet the single tick-wide shared fetch described in the design doc — Task 5 consolidates it later once the OOM/unexpected-exit call sites exist to share with. Implementing a working, self-contained version here first (TDD: correct in isolation, verified by the tests below) and then having a later task fold in the sharing is the incremental path; don't try to do both fetch-consolidation and reap-hardening-wiring in the same task.

- [ ] **Step 2: Fetch job rows once per tick, only when there's a reap candidate, and refresh the scale lock around it**

Immediately before the scale-down block, add:

```rust
    // Fetched once per tick and shared across both reap-hardening checks
    // below (scale-down and idle-timeout), only when there's at least one
    // idle-classified runner that might be reaped — an empty fleet or a
    // fleet with no idle runners this tick has nothing to corroborate,
    // so this avoids an unconditional extra `gh api` call every single tick.
    // (Task 5 later folds this into a single tick-wide fetch shared with the
    // OOM/unexpected-exit scanners — this is a self-contained first version.)
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
    // This fetch can be a real multi-call gh api fan-out (see
    // fetch_recent_job_rows's own doc comment) — refresh the scale lock's
    // heartbeat immediately after it, the same way the OOM-visibility step
    // already does around its own IO, so a slow tick here doesn't let a
    // concurrent invocation see the lock as stale and steal it.
    if let Some(lock) = _lock.as_ref() {
        lock.refresh(now_secs());
    }
```

- [ ] **Step 3: Apply both checks in the scale-down block — WITHOUT losing a blocked candidate's tracked state**

Find the scale-down block. It currently looks like (read the real current version — this is the shape from before this task's changes):

```rust
        let reap_set: HashSet<String> = to_reap.into_iter().collect();
        idle_runners.retain(|(name, _)| !reap_set.contains(name));
        for name in reap_set {
            reap(&name, dry_run, "scale-down above desired");
            reaped_scale_down += 1;
        }
```

Replace it with:

```rust
        let reap_set: HashSet<String> = to_reap.into_iter().collect();
        let mut actually_reaped: HashSet<String> = HashSet::new();
        for name in &reap_set {
            let corroborated_busy = job_rows_for_reap_check
                .as_deref()
                .is_some_and(|rows| is_corroborated_busy(name, rows));
            let two_tick_eligible = eligible_for_scale_down_reap(name, &prev);
            if corroborated_busy || !two_tick_eligible {
                println!(
                    "runner-scale: scale-down reap of {name} blocked (corroborated_busy={corroborated_busy}, \
                     two_tick_eligible={two_tick_eligible}) — leaving it idle-tracked rather than reaping \
                     or silently dropping its state"
                );
                continue;
            }
            reap(name, dry_run, "scale-down above desired");
            reaped_scale_down += 1;
            actually_reaped.insert(name.clone());
        }
        // Only strip names that were ACTUALLY reaped -- a blocked candidate
        // (see the println! above) stays in idle_runners and flows into the
        // idle-timeout loop below like any other still-idle runner, so its
        // idle_since is persisted into new_state instead of being silently
        // dropped. See this task's header note for why this ordering matters.
        idle_runners.retain(|(name, _)| !actually_reaped.contains(name));
```

- [ ] **Step 4: Apply the corroborating check in the idle-timeout block**

Find the idle-timeout loop (`for (name, idle_since) in &idle_runners { if should_reap_idle(*idle_since, now, reap_secs) { reap(name, dry_run, "idle > reap grace (never assigned)"); reaped += 1; } ... }`). This path already has the multi-minute grace, so it does NOT need the 2-tick check (Task 2 was scoped specifically to the scale-down gap) — but it should still get the corroborating busy-check as defense-in-depth, since a runner idle for the full grace period could still theoretically be mid-registration-lag on a very slow job pickup. Note this loop now also receives any candidate Step 3 blocked from scale-down (per Step 3's fix) — that's intended, it just means such a runner gets ANOTHER chance to be correctly classified here, using the same corroboration signal, rather than being silently dropped:

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

- [ ] **Step 5: Write the integration test that specifically covers the bug this task's header describes**

There is no existing test harness in this file that mocks `gh`/`docker` IO for `run_scale` as a whole — but the specific regression Step 3 fixes (a blocked candidate's state surviving into `new_state`) is testable as PURE LOGIC if the scale-down block's core decision is extracted into a small testable helper, rather than left fully inline. Add this helper near `scale_down_reap_targets`:

```rust
/// Given this tick's scale-down candidates and the two hardening signals,
/// returns (names to actually reap, names blocked and therefore still
/// idle-tracked). Pure — the actual `reap()` IO call and `println!` stay in
/// `run_scale` itself, this function only makes the decision. Extracted
/// specifically so the "a blocked candidate must not be lost" invariant
/// (see Task 3's plan header) is unit-testable without mocking IO.
pub fn partition_scale_down_candidates(
    reap_set: &HashSet<String>,
    job_rows: Option<&[super::oom_watch::JobRow]>,
    prev: &HashMap<String, i64>,
) -> (HashSet<String>, HashSet<String>) {
    let mut to_reap = HashSet::new();
    let mut blocked = HashSet::new();
    for name in reap_set {
        let corroborated_busy = job_rows.is_some_and(|rows| is_corroborated_busy(name, rows));
        let two_tick_eligible = eligible_for_scale_down_reap(name, prev);
        if corroborated_busy || !two_tick_eligible {
            blocked.insert(name.clone());
        } else {
            to_reap.insert(name.clone());
        }
    }
    (to_reap, blocked)
}
```

Then simplify Step 3's inline block to call this helper instead of repeating its logic:

```rust
        let reap_set: HashSet<String> = to_reap.into_iter().collect();
        let (actually_reaped, blocked) =
            partition_scale_down_candidates(&reap_set, job_rows_for_reap_check.as_deref(), &prev);
        for name in &blocked {
            println!(
                "runner-scale: scale-down reap of {name} blocked (corroborated busy or not yet \
                 2-tick eligible) — leaving it idle-tracked rather than reaping or silently \
                 dropping its state"
            );
        }
        for name in &actually_reaped {
            reap(name, dry_run, "scale-down above desired");
            reaped_scale_down += 1;
        }
        idle_runners.retain(|(name, _)| !actually_reaped.contains(name));
```

Add the test (in the existing `mod tests` block):

```rust
#[test]
fn blocked_scale_down_candidate_is_not_reaped_and_not_stripped_from_idle_tracking() {
    // This is the exact regression adversarial review found in this plan
    // before implementation: a candidate blocked by the hardening must NOT
    // be silently dropped from the set that flows into idle-timeout tracking.
    let mut reap_set = HashSet::new();
    reap_set.insert("vox-runner-auto-blocked-0".to_string());
    reap_set.insert("vox-runner-auto-clean-0".to_string());

    // "blocked-0" is corroborated-busy via a fresh job row; "clean-0" has no
    // matching job row and IS 2-tick-eligible, so it should actually reap.
    let job_rows = vec![super::oom_watch::JobRow {
        runner_name: "vox-runner-auto-blocked-0".to_string(),
        job_name: "docs-quality".to_string(),
        run_id: 1,
        pr_number: 460,
    }];
    let mut prev = HashMap::new();
    prev.insert("vox-runner-auto-clean-0".to_string(), 1_000);

    let (to_reap, blocked) =
        partition_scale_down_candidates(&reap_set, Some(&job_rows), &prev);

    assert!(to_reap.contains("vox-runner-auto-clean-0"));
    assert!(!to_reap.contains("vox-runner-auto-blocked-0"));
    assert!(blocked.contains("vox-runner-auto-blocked-0"));
    assert!(!blocked.contains("vox-runner-auto-clean-0"));
    // The critical assertion: the blocked name must be accounted for
    // SOMEWHERE (to_reap ∪ blocked = reap_set), never silently dropped.
    let accounted: HashSet<String> = to_reap.union(&blocked).cloned().collect();
    assert_eq!(&accounted, &reap_set);
}
```

- [ ] **Step 6: Build and run the full test suite for this crate**

Run: `cargo build -p vox-cli 2>&1 | tail -30`
Expected: compiles clean.

Run: `cargo test -p vox-cli --lib 2>&1 | tail -60`
Expected: all existing tests plus Task 1/2's new tests plus this task's `blocked_scale_down_candidate_is_not_reaped_and_not_stripped_from_idle_tracking` test still pass (no regressions from the wiring change).

- [ ] **Step 7: Manual dry-run verification**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- ci runner-scale` (dry-run, default — no `--apply`)
Expected: exits cleanly, prints the normal `runner-scale: dry_run=true ...` summary line with no panics. If any runner is currently idle-classified, confirm the new corroboration logic runs without erroring (even if `gh`/`docker` aren't fully set up in the environment this is run from, the `Err(e) => ... None` fallback in Step 2 must degrade gracefully, not panic — verify this specifically if `gh`/`docker` aren't available here).

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): wire reap-hardening (corroborating busy-check + 2-tick scale-down gate) into run_scale, without losing blocked candidates' idle-tracked state"
```

---

### Task 4: New `unexpected_exit_watch.rs` module — detection, correlation, comment composition + tests

**Files:**
- Create: `crates/vox-cli/src/commands/ci/unexpected_exit_watch.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (register the new module — read this file fresh first to see how `oom_watch` is declared, e.g. `mod oom_watch;` or `pub mod oom_watch;`, and mirror that exact declaration style for the new module)

This mirrors `oom_watch.rs`'s structure directly: a seen-list to avoid duplicate reports, GitHub job correlation, PR-comment composition, and a tick entrypoint — but the detection signal is "container name that was running last tick and is exited now, whose assigned job is still `in_progress`" instead of a `dmesg` OOM line.

- [ ] **Step 1: Read the reference file fresh**

Re-open `crates/vox-cli/src/commands/ci/oom_watch.rs` in full. This task's module should mirror: its module-level doc comment style, `OomEvent`-equivalent struct, `oom_seen_path`/`read_oom_seen`/`write_oom_seen`-equivalent persistence trio, `new_events`/`append_seen`-equivalent pure filtering, `JobRow`/`find_matching_job` (REUSED directly from `oom_watch.rs`, not redefined — `JobRow` and `find_matching_job` are already `pub`, confirmed by fidelity review, so no visibility change needed for those two; import `super::oom_watch::{JobRow, find_matching_job}`), `oom_comment_body`-equivalent pure comment composer, `post_pr_comment`-equivalent poster (also reuse `oom_watch::post_pr_comment` directly rather than redefining — this one IS currently private, widen it to `pub(crate)` in `oom_watch.rs` in this task), and `scan_and_report_oom_events`-equivalent tick entrypoint. `fetch_recent_job_rows` is also currently private but was already widened to `pub(crate)` by Task 3 — confirm that widening is present rather than re-doing it.

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
/// Deliberately does NOT attempt a same-tick "here's why" diagnosis —
/// adversarial review of this plan found that impossible to do honestly:
/// the exited-container scan runs before any reap-hardening decision is made
/// in the same tick (no near-miss evidence can exist yet), and even if it
/// could, a BLOCKED reap by construction did not execute, so it can't be why
/// a container died — that would be backwards causality. Both known
/// hypotheses are stated neutrally instead.
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
/// `job_rows`: the tick's shared jobs-API fetch (see Task 5, which fetches
/// this ONCE per apply-tick and threads it to every consumer that needs it —
/// this function never fetches its own copy).
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
        // Both known causes are named, and neither is claimed as more likely
        // than the other from a single event -- see this function's doc
        // comment for why a same-tick diagnosis isn't attempted.
        assert!(body.contains("busy") && body.contains("lag"));
        assert!(body.contains("external cause"));
        assert!(body.contains("cannot distinguish"));
    }
}
```

- [ ] **Step 3: Register the module**

Open `crates/vox-cli/src/commands/ci/mod.rs`, find the `mod oom_watch;` (or `pub mod oom_watch;`) declaration, and add an identically-styled declaration for the new module (e.g. `mod unexpected_exit_watch;` — match whichever visibility `oom_watch`'s own declaration actually uses).

- [ ] **Step 4: Widen `post_pr_comment`'s visibility**

`JobRow`/`find_matching_job` are already `pub`; `fetch_recent_job_rows` was already widened to `pub(crate)` by Task 3 (confirm it's present). The one remaining item this task needs is `post_pr_comment`, currently private in `oom_watch.rs` — widen it to `pub(crate)` there.

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

### Task 5: Wire the new detector into `run_scale`, consolidate to a single shared job-rows fetch, correct ordering

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs`
- Modify: `crates/vox-cli/src/commands/ci/oom_watch.rs`

Requires Tasks 3 and 4 landed. Two constraints, both load-bearing:

1. **Ordering**: the new detector's exited-container scan must run BEFORE `run_scale`'s existing step 1 (`for name in managed_containers("exited") { deregister(&name); docker rm -f ... }`, ~line 852-859) — once that cleanup runs, `docker inspect` can no longer read the exited container's exit code.
2. **Single shared fetch**: adversarial review found the naive design (each of OOM-visibility, unexpected-exit-visibility, and Task 3's reap-hardening independently calling `fetch_recent_job_rows()`) could fan out to ~3 independent multi-call `gh api` sequences in a single tick — worst case coinciding exactly during an incident, the worst time for rate-limit exposure to spike. This task consolidates all three onto ONE fetch, done once near the top of the `apply` tick, threaded to every consumer.

- [ ] **Step 1: Read the file fresh**

Re-open `runner_scale.rs`. Re-confirm the exact current position of: the OOM-visibility call (§0.5, ~line 826-834, apply-only, including the `_lock.as_ref()` refresh calls immediately before/after it), step 1's exited-container cleanup loop (~line 852-859), and Task 3's `job_rows_for_reap_check` fetch (added earlier in this same plan's Task 3, immediately before the scale-down block). Also re-confirm what `managed_containers("running")` returns (a `Vec<String>` of names) and where it's first computed in this tick (currently at ~line 862, AFTER step 1's cleanup — this task needs a running-containers snapshot BEFORE cleanup too; `managed_containers` is read-only `docker ps -a --filter ...`, safe to call twice in one tick).

- [ ] **Step 2: Change `scan_and_report_oom_events` to accept job rows as a parameter, and return which containers it claimed**

Read `oom_watch::scan_and_report_oom_events`'s current signature and body. Change its signature from `pub fn scan_and_report_oom_events(now: i64) -> Result<u32>` to `pub fn scan_and_report_oom_events(now: i64, job_rows: &[JobRow]) -> Result<(u32, HashSet<String>)>` — remove its internal `fetch_recent_job_rows()?` call (the caller now provides the rows), and thread a `HashSet<String>` of successfully-reported container names through its existing `for event in fresh { ... match post_pr_comment(...) { Ok(()) => { reported += 1; ... } ... } }` loop alongside the existing `reported` count (`container_name` is already an available local in that loop — insert it into the set in the same branch that increments `reported`). Update any of `oom_watch.rs`'s own tests that call `scan_and_report_oom_events` directly to pass a `job_rows` argument and match the new return shape (most of that file's tests call the smaller pure functions instead, so this is likely a small change — verify by reading the test file fresh).

- [ ] **Step 3: Replace the OOM-visibility block with a shared-fetch version, and insert the unexpected-exit scan right after it**

Replace the existing OOM-visibility block (`if apply { let oom_reported = ...; ... }`) with:

```rust
    // 0.5/0.6. Shared jobs-API fetch, done ONCE per apply-tick and threaded
    // to every consumer that needs it this tick (OOM-visibility,
    // unexpected-exit-visibility, and Task 3's reap-hardening later in this
    // same tick) -- consolidated here (rather than each consumer fetching
    // its own copy) specifically to avoid fanning out to multiple
    // independent multi-call gh api sequences in one tick, which adversarial
    // review flagged as a real rate-limit risk concentrated exactly during
    // incident conditions (a dying runner tends to trigger several of these
    // checks in the same tick).
    let mut oom_claimed_names: HashSet<String> = HashSet::new();
    let mut unexpected_exit_reported = 0u32;
    let mut shared_job_rows: Option<Vec<super::oom_watch::JobRow>> = None;
    if apply {
        match super::oom_watch::fetch_recent_job_rows() {
            Ok(rows) => shared_job_rows = Some(rows),
            Err(e) => eprintln!(
                "runner-scale: shared jobs-API fetch failed this tick (OOM-visibility, \
                 unexpected-exit-visibility, and reap-hardening all degrade to their fallback \
                 behavior this tick): {e:#}"
            ),
        }
        // A real multi-call gh api sequence just ran -- refresh the lock's
        // heartbeat immediately, same as every other IO-heavy step in this
        // tick already does, so a slow fetch here can't let a concurrent
        // invocation see the lock as stale and steal it.
        if let Some(lock) = _lock.as_ref() {
            lock.refresh(now_secs());
        }
    }

    if apply && let Some(rows) = shared_job_rows.as_deref() {
        match super::oom_watch::scan_and_report_oom_events(now, rows) {
            Ok((oom_reported, claimed)) => {
                oom_claimed_names = claimed;
                if oom_reported > 0 {
                    println!("runner-scale: reported {oom_reported} OOM-killed job(s) this tick");
                }
            }
            Err(e) => eprintln!("runner-scale: OOM-visibility scan skipped (degraded): {e:#}"),
        }

        // Unexpected-exit visibility: detect any managed container that
        // transitioned running->exited since the last tick while its
        // assigned job was still in_progress (not a normal ephemeral
        // job-complete exit, not already claimed by the OOM scan above).
        // MUST run before step 1's cleanup below removes exited containers
        // -- docker inspect can't read an exit code from a pruned container.
        let curr_running: HashSet<String> = managed_containers("running").into_iter().collect();
        match super::unexpected_exit_watch::scan_and_report_unexpected_exits(
            &curr_running,
            &oom_claimed_names,
            rows,
        ) {
            Ok(n) => unexpected_exit_reported = n,
            Err(e) => eprintln!("runner-scale: unexpected-exit scan skipped (degraded): {e:#}"),
        }
    }
```

(The `if apply && let Some(rows) = ... {` construct is a let-chain — if this crate's Rust edition/MSRV doesn't support let-chains yet, verify via `cargo build -p vox-cli` in Step 6 and fall back to nested `if apply { if let Some(rows) = ... { ... } }` if it doesn't compile; either is fine, this is a style choice not a design requirement.)

- [ ] **Step 4: Reuse the shared fetch in Task 3's reap-hardening instead of its own separate fetch**

Task 3 added its own `job_rows_for_reap_check` fetch immediately before the scale-down block. Re-open that code (search for `job_rows_for_reap_check` in `runner_scale.rs`) and replace its independent `fetch_recent_job_rows()` call with a reference to `shared_job_rows` (already fetched earlier in this same tick per Step 3 above):

```rust
    // Reuses the tick's single shared jobs-API fetch (see the top of this
    // tick's apply block) rather than fetching its own copy -- this is a
    // deliberate simplification versus fetching again this-many-seconds
    // later in the tick for a marginally fresher snapshot: one consistent
    // snapshot for the whole tick is simpler to reason about and closes the
    // multi-fetch rate-limit concern entirely, at the cost of the
    // corroboration check working from a very-slightly-older snapshot than
    // a dedicated second fetch would give it. Given the corroborating check
    // was already documented (design doc §1a) as narrowing, not closing,
    // the staleness window, this cost is acceptable.
    //
    // NOTE: shared_job_rows is only populated when apply=true (see Step 3
    // above) -- in a dry-run tick, the corroborating-busy check below always
    // sees None and degrades to using only the 2-tick eligibility check,
    // which still runs and is still meaningful for a dry-run preview. This
    // is a deliberate, disclosed dry-run behavior difference, not a bug.
    let job_rows_for_reap_check: Option<&[super::oom_watch::JobRow]> = shared_job_rows.as_deref();
```

Remove the old standalone fetch block (the one Task 3 added with its own `match super::oom_watch::fetch_recent_job_rows() { ... }` and its own `if let Some(lock) = _lock.as_ref() { lock.refresh(now_secs()); }` call) entirely — both the fetch and its lock-refresh are now covered by Step 3's shared version, which runs earlier in the tick. Update `partition_scale_down_candidates`'s call site (from Task 3 Step 5) to pass `job_rows_for_reap_check` (now `Option<&[JobRow]>` directly, matching its existing parameter type) rather than `job_rows_for_reap_check.as_deref()` (the `.as_deref()` is no longer needed since the type is already `Option<&[...]>` — check the exact type Task 3 left it as and adjust whichever side needs the conversion; the net function signature `partition_scale_down_candidates(reap_set: &HashSet<String>, job_rows: Option<&[JobRow]>, prev: &HashMap<String, i64>)` from Task 3 doesn't change, only how its second argument gets constructed here does).

- [ ] **Step 5: Fold the new count into the tick's summary line**

Find the final `println!("runner-scale: dry_run={dry_run} queued_jobs={demand} ...")` summary (~line 988). Add `unexpected_exits_reported={unexpected_exit_reported}` to it, matching this line's existing comma-free space-separated `key=value` style.

- [ ] **Step 6: Build and test**

Run: `cargo build -p vox-cli 2>&1 | tail -40`
Expected: compiles clean. If the let-chain syntax from Step 3 doesn't compile on this crate's Rust edition, fall back to nested `if let` as noted there.

Run: `cargo test -p vox-cli --lib 2>&1 | tail -60`
Expected: all tests (existing + Tasks 1, 2, 3, 4's new ones) still pass — including Task 3's `blocked_scale_down_candidate_is_not_reaped_and_not_stripped_from_idle_tracking` test, which must still pass unchanged since `partition_scale_down_candidates`'s own signature didn't change in this task, only its caller did.

- [ ] **Step 7: Manual dry-run verification**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- ci runner-scale` (dry-run, default — no `--apply`)
Expected: exits cleanly, `unexpected_exits_reported=0` in the summary (dry-run never calls the apply-gated block from Step 3, so this is always 0 in dry-run — expected, not a bug). No panics.

If you have a working `gh`/`docker` setup and understand it will mutate the real fleet, also run `VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- ci runner-scale --apply` and confirm the same summary line format with real counts, no panics. Skip this second run if unsure — Step 6's build+test coverage plus the dry-run pass above are sufficient to land this task; real-fleet verification can happen once this ships.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli/src/commands/ci/runner_scale.rs crates/vox-cli/src/commands/ci/oom_watch.rs
git commit -m "feat(ci): wire unexpected_exit_watch into run_scale with a single shared jobs-API fetch per tick"
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

**Spec coverage** — §1a (corroborating busy-check) → Tasks 1, 3, consolidated onto the shared fetch in Task 5. §1b (2-consecutive-tick requirement) → Task 2, 3 (implemented by reusing the existing `idle_since`/`read_state()` mechanism rather than adding new state, scoped to the scale-down path specifically since the idle-timeout path already has an equivalent-or-stronger grace period — a deliberate, disclosed refinement of the spec's original framing: the spec's own intent, "require 2 consecutive idle ticks before eligible for reap," is satisfied for the path that actually lacked it). §2 (new detector: detection/skip-if-OOM-claimed/reporting) → Task 4, 5. §3 (rich console output, both detectors) → Task 4 (new detector's own output, built in from the start) + Task 6 (OOM detector's output, added). §4 (`ScaleLock` heartbeat coverage, added to the design during adversarial review) → Task 3 Step 2 and Task 5 Step 3, both refresh the lock immediately after their respective fetches.

**Adversarial-review corrections applied to this plan before implementation began** (both a fidelity audit against the real codebase and a design-soundness critique — findings folded in above, not left as a separate addendum):
- **Critical**: the original Task 3 wiring would have silently and permanently prevented scale-to-zero for any runner ever selected as a scale-down candidate and then blocked by the new hardening — the pre-existing `idle_runners.retain(...)` call stripped every candidate regardless of whether it was actually reaped, losing its `idle_since` state and causing it to be re-selected-and-re-blocked forever. Fixed: only actually-reaped names are stripped from `idle_runners`; blocked candidates fall through to the idle-timeout loop like any other still-idle runner (Task 3 Step 3), and a dedicated integration-style test (Task 3 Step 5, `blocked_scale_down_candidate_is_not_reaped_and_not_stripped_from_idle_tracking`) specifically guards against this regression recurring, since it's a wiring bug that no pure-function unit test would have caught in isolation.
- **High**: the original "near-miss" diagnostic correlation (comment body naming a likely cause based on same-tick reap-hardening activity) was both structurally unreachable given the real tick ordering and causally backwards even when reachable (a *blocked* reap, by construction, did not execute, so it cannot explain a death). Removed entirely; the comment body now states both known hypotheses neutrally (Task 4's `unexpected_exit_comment_body`, no `near_miss_blocked` parameter).
- **Medium-High**: the original design had each of three consumers (OOM scan, unexpected-exit scan, reap-hardening) independently calling `fetch_recent_job_rows()`, risking up to ~3 independent multi-call `gh api` fan-outs in a single tick, worst case concentrated exactly during incident conditions. Consolidated to one shared fetch per apply-tick, threaded to all three consumers (Task 5 Steps 3-4).
- **Medium**: the original design's `ScaleLock` heartbeat wasn't refreshed around the new IO this design adds, widening the exact stale-lock double-execution race this session's own investigation encountered. Fixed: explicit `lock.refresh(now_secs())` calls added immediately after both the shared fetch (Task 5 Step 3) and (prior to consolidation) Task 3's original fetch.
- **Medium**: the design's language around the corroborating busy-check was softened from "closes" to "narrows" the GitHub API staleness window, since the check itself has its own (smaller, bounded) staleness window rather than eliminating the problem outright.
- **Medium**: added the integration-style test described above specifically because the two most severe findings (Critical, High) were both wiring/integration bugs that this plan's original pure-function-only test coverage would not have caught, despite touching a subsystem with a documented real-incident history.
- **Minor (fidelity)**: corrected an overhedged premise in the original plan's Task 1/3/4 — `JobRow`/`find_matching_job`/`scan_and_report_oom_events` are already `pub` in `oom_watch.rs` (no widening needed); only `fetch_recent_job_rows` and `post_pr_comment` were actually private and needed widening to `pub(crate)`, which Tasks 3 and 4 now state precisely rather than as an open "check and widen if needed" for all four.

**Type consistency** — `JobRow`/`find_matching_job`/`fetch_recent_job_rows`/`post_pr_comment` are defined once in `oom_watch.rs` and reused by name (not redefined) in `runner_scale.rs` (Task 3, 5) and `unexpected_exit_watch.rs` (Task 4). `is_corroborated_busy` (Task 1), `eligible_for_scale_down_reap` (Task 2), and `partition_scale_down_candidates` (Task 3) are each defined once and called by the same names throughout. `scan_and_report_oom_events`'s signature change (`fn(now: i64) -> Result<u32>` → `fn(now: i64, job_rows: &[JobRow]) -> Result<(u32, HashSet<String>)>`, Task 5 Step 2) is applied consistently at its one call site (Task 5 Step 3) in the same task that changes the signature. `scan_and_report_unexpected_exits`'s signature (Task 4: `curr_running, already_oom_claimed, job_rows: &[JobRow]`, no `near_miss_names`) matches its one call site (Task 5 Step 3) exactly.
