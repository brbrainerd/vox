# CI/CD Runner Audit Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the single-box self-hosted CI gate fit its real 6-runner ceiling, fix the one confirmed config bug that disables sccache, close the zombie-job cancellation gap, and make queue-wait time measurable — without losing any test coverage.

**Architecture:** Measure-first (Phase 0 adds the missing queue-wait + cache observability), then confirmed quick wins (Phase A: the `CARGO_INCREMENTAL` one-liner, demand-telemetry fix, safe workflow relocations), then the structural fix (Phase B: tier non-required smokes off the `merge_group` critical path so the self-hosted fan-out fits 6 runners in one wave, bake apt deps, add a conservative zombie force-cancel). Phase C (elastic retreat) is design-only and gated on Phase 0 data — no tasks here.

**Tech Stack:** Rust (`vox-cli`), GitHub Actions YAML, Docker (WSL2 self-hosted runners), `gh` CLI, `cargo nextest`, `actionlint`.

**Source spec:** `docs/superpowers/specs/2026-06-30-ci-cd-runner-audit-remediation-design.md`

---

## File Structure

**Rust (unit-testable seams):**
- `crates/vox-cli/src/commands/ci/job_timings.rs` — add `created_at` to `JobRow`, add pure `queue_wait_seconds`, exclude cancelled rows. (Phase 0)
- `crates/vox-cli/src/commands/ci/runner_scale.rs` — extract a pure demand-accumulation fn with pagination + error propagation; log S3 reachability into the scale event. (Phase 0/A)
- `crates/vox-cli/src/commands/ci/` — new `workflow_lint.rs` (or extend an existing `ci` guard module) with a pure `sccache_requires_incremental` check. (Phase A)

**Config (verified by actionlint / grep gates / observation — no unit seam):**
- `.github/workflows/ci.yml` — `CARGO_INCREMENTAL`; `sccache --show-stats`; tier non-required smokes off `merge_group`; remove 5 apt steps.
- `.github/workflows/cross-platform-check.yml` — move self-hosted Linux leg to `ubuntu-latest`.
- `.github/workflows/gitleaks.yml`, `link_checker.yml`, `os-compat-report.yml`, `mobile-e2e-ios.yml` — runner + trigger changes.
- `.github/workflows/ci-health-watchdog.yml`, `.github/actions/ci-health-assess/action.yml`, `.github/workflows/ci-health-watchdog-test.yml` — zombie force-cancel + fixture.
- `.github/workflows/ci-timings.yml` — queue-wait column; exclude cancelled.
- `infra/ci-runner/Dockerfile`, `infra/ci-runner/entrypoint.sh` — bake apt stack; sccache shim guard.
- `scripts/ci/voxcirunnerscale.task.xml` — only if PT10M is adopted (gated on the heartbeat task).

**Conventions:** per-crate fmt only (`cargo fmt -p vox-cli`, never `cargo fmt --all` — Windows arg-limit). Run Rust tests with `cargo nextest run -p vox-cli`. Validate YAML with `actionlint .github/workflows/<file>.yml`.

---

## Phase 0 — Measure first

### Task 1: Queue-wait metric (pure fn + JobRow field)

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/job_timings.rs` (`JobRow` ~:29-39, beside `run_seconds` ~:56)
- Test: same file's `#[cfg(test)]` module

- [ ] **Step 1: Write the failing test**

Add to the test module in `job_timings.rs`:

```rust
#[test]
fn queue_wait_is_started_minus_created() {
    // 10:00:00 created, 10:00:30 started => 30s queued
    assert_eq!(
        queue_wait_seconds(Some("2026-06-30T10:00:00Z"), Some("2026-06-30T10:00:30Z")),
        Some(30)
    );
}

#[test]
fn queue_wait_none_when_either_timestamp_missing() {
    assert_eq!(queue_wait_seconds(None, Some("2026-06-30T10:00:30Z")), None);
    assert_eq!(queue_wait_seconds(Some("2026-06-30T10:00:00Z"), None), None);
}

#[test]
fn queue_wait_never_negative() {
    // clock skew: started before created => clamp to 0, not a negative wait
    assert_eq!(
        queue_wait_seconds(Some("2026-06-30T10:00:30Z"), Some("2026-06-30T10:00:00Z")),
        Some(0)
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p vox-cli queue_wait`
Expected: FAIL — `queue_wait_seconds` not found.

- [ ] **Step 3: Add the field and the pure function**

In `JobRow` add (mirror the existing `started_at`/`completed_at` `Option<String>` fields):

```rust
    #[serde(default)]
    pub created_at: Option<String>,
```

Add beside `run_seconds` (reuse the same ISO-8601 parse helper `run_seconds` already uses; this example uses `time::OffsetDateTime` — match whatever `run_seconds` imports):

```rust
/// Seconds a job waited in queue before a runner picked it up
/// (`started_at - created_at`). `None` if either timestamp is absent.
/// Clamped at 0 to absorb clock skew. This is the fleet-starvation metric;
/// `run_seconds` measures execution, not wait.
pub fn queue_wait_seconds(created_at: Option<&str>, started_at: Option<&str>) -> Option<i64> {
    let created = parse_iso8601(created_at?)?;
    let started = parse_iso8601(started_at?)?;
    Some((started - created).whole_seconds().max(0))
}
```

If `run_seconds` parses inline rather than via a `parse_iso8601` helper, extract that parse into `fn parse_iso8601(s: &str) -> Option<OffsetDateTime>` and call it from both.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p vox-cli queue_wait`
Expected: PASS (3 tests).

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/job_timings.rs
git commit -m "feat(ci): measure queue-wait time (started_at - created_at)"
```

### Task 2: Exclude cancelled runs from the timing dataset

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/job_timings.rs` (the row→timing mapping ~:101-115)
- Test: same file

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn cancelled_jobs_excluded_from_timings() {
    let rows = vec![
        JobRow { conclusion: Some("success".into()),   started_at: Some("2026-06-30T10:00:00Z".into()), completed_at: Some("2026-06-30T10:05:00Z".into()), created_at: Some("2026-06-30T09:59:00Z".into()), ..Default::default() },
        JobRow { conclusion: Some("cancelled".into()), started_at: Some("2026-06-30T10:00:00Z".into()), completed_at: Some("2026-06-30T10:01:00Z".into()), created_at: Some("2026-06-30T09:59:00Z".into()), ..Default::default() },
    ];
    let timings = timings_from_rows(&rows);
    assert_eq!(timings.len(), 1, "cancelled run must not pollute the dataset");
}
```

(If `JobRow` has no `Default`, derive it: `#[derive(Default, ...)]` on the struct, or construct the two rows with whatever fields the struct requires.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p vox-cli cancelled_jobs_excluded`
Expected: FAIL — `len()` is 2.

- [ ] **Step 3: Add the filter**

In `timings_from_rows` (the mapping at ~:101), skip cancelled before computing the delta:

```rust
    if row.conclusion.as_deref() == Some("cancelled") {
        continue; // concurrency-cancelled runs have truncated durations — exclude
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p vox-cli cancelled_jobs_excluded`
Expected: PASS.

- [ ] **Step 5: Mirror in the workflow jq and commit**

In `.github/workflows/ci-timings.yml`, add `and (.conclusion != "cancelled")` to the `.jobs[] | select(...)` filter (~:42-43).

```bash
actionlint .github/workflows/ci-timings.yml
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/job_timings.rs .github/workflows/ci-timings.yml
git commit -m "fix(ci): exclude cancelled runs from timing dataset"
```

### Task 3: Surface sccache stats + S3-cache reachability

**Files:**
- Modify: `.github/workflows/ci.yml` (the `setup` build step, after the `cargo build -p vox-cli` at ~:106)
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs` (`scale_event_json` ~:654; reachability already computed at ~:522)
- Test: `runner_scale.rs` test module

- [ ] **Step 1: Write the failing test for the scale-event field**

```rust
#[test]
fn scale_event_json_includes_s3_reachable() {
    let json = scale_event_json(/* existing args */, /* s3_reachable: */ false);
    assert!(json.contains("\"s3_cache_reachable\":false"));
}
```

Match `scale_event_json`'s real signature — add an `s3_reachable: bool` parameter.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p vox-cli scale_event_json_includes_s3`
Expected: FAIL — arity / missing field.

- [ ] **Step 3: Thread the flag in**

Add `s3_cache_reachable` to the JSON object built in `scale_event_json`, and pass the already-computed value from the caller at ~:522.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p vox-cli scale_event_json_includes_s3`
Expected: PASS. Also run the existing `scale_event_json` test to confirm no regression: `cargo nextest run -p vox-cli scale_event`.

- [ ] **Step 5: Add the sccache stats step + commit**

In `ci.yml` after the build step (~:106), add:

```yaml
      - name: sccache stats (gate cache hit rate)
        if: always()
        run: sccache --show-stats
```

```bash
actionlint .github/workflows/ci.yml
cargo fmt -p vox-cli
git add .github/workflows/ci.yml crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): surface sccache hit rate + S3-cache reachability"
```

### Task 4: Capture the baseline (observation, no code)

**Files:** none (record numbers into the spec's "Phase 0 baseline" or a tracking issue).

- [ ] **Step 1: After Tasks 1-3 are merged, run one full `merge_group` gate and record:**
  - merge-gate wall-clock (queue→green),
  - per-job queue-wait (from Task 1),
  - sccache hit rate (from Task 3 — expected LOW, pre-W1),
  - count of self-hosted jobs queued at merge time vs the 6-runner ceiling.

- [ ] **Step 2: Write the four numbers into the spec's Success-criteria baseline section and commit that doc edit.** These gate Phase B (step "≤ 6 in one wave") and Phase C.

---

## Phase A — Confirmed quick wins

### Task 5: W1 — enable sccache on the gate + regression guard

**Files:**
- Modify: `.github/workflows/ci.yml:25-29` (env block)
- Create: `crates/vox-cli/src/commands/ci/workflow_lint.rs` (pure check + wire into the existing `vox ci` guard surface)
- Test: `workflow_lint.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn sccache_workflow_must_set_incremental_zero() {
    let bad = "env:\n  RUSTC_WRAPPER: sccache\n";
    let good = "env:\n  RUSTC_WRAPPER: sccache\n  CARGO_INCREMENTAL: \"0\"\n";
    assert!(sccache_requires_incremental(bad).is_err(), "sccache without CARGO_INCREMENTAL=0 must fail");
    assert!(sccache_requires_incremental(good).is_ok());
}

#[test]
fn non_sccache_workflow_is_unaffected() {
    assert!(sccache_requires_incremental("env:\n  FOO: bar\n").is_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p vox-cli sccache_`
Expected: FAIL — `sccache_requires_incremental` not found.

- [ ] **Step 3: Implement the pure check**

```rust
/// Any workflow that activates sccache (`RUSTC_WRAPPER: sccache`) must also set
/// `CARGO_INCREMENTAL: "0"`, or sccache silently caches nothing (incremental
/// artifacts are not cacheable). Returns Err naming the offending file content.
pub fn sccache_requires_incremental(workflow_text: &str) -> Result<(), String> {
    let uses_sccache = workflow_text.contains("RUSTC_WRAPPER: sccache");
    let sets_incremental = workflow_text.contains("CARGO_INCREMENTAL");
    if uses_sccache && !sets_incremental {
        return Err("workflow sets RUSTC_WRAPPER: sccache but not CARGO_INCREMENTAL — sccache will 0%-hit".into());
    }
    Ok(())
}
```

Wire it into the existing `vox ci` guard command that scans `.github/workflows/*.yml` (follow the pattern of whatever guard already iterates workflow files; if none, call it from the `guards-fast` lane's `vox ci` entry).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p vox-cli sccache_`
Expected: PASS.

- [ ] **Step 5: Apply the actual one-line fix + commit**

In `ci.yml:25-29` env block add the line:

```yaml
  CARGO_INCREMENTAL: "0"
```

Run the guard against the repo to confirm it now passes (and would have failed before): `cargo run -p vox-cli -- ci <guard-subcommand>`.

```bash
actionlint .github/workflows/ci.yml
cargo fmt -p vox-cli
git add .github/workflows/ci.yml crates/vox-cli/src/commands/ci/workflow_lint.rs
git commit -m "fix(ci): set CARGO_INCREMENTAL=0 on gate so sccache caches (+guard)"
```

- [ ] **Step 6: Verify the win against baseline**

Re-run the gate; confirm `sccache --show-stats` (Task 3) now shows a non-zero hit rate on the second run.

### Task 6: B4 — demand telemetry pagination + error propagation

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs` (`query_queued_job_demand` ~:273-299; `DEMAND_RUNS_PER_STATUS` :77)
- Test: `runner_scale.rs`

- [ ] **Step 1: Write the failing test (pure accumulation seam)**

```rust
#[test]
fn accumulate_demand_counts_beyond_first_page_and_stops_at_max() {
    // 30 matching label-lines, max = u32::MAX (telemetry path) => full 30 counted
    let lines: Vec<String> = (0..30).map(|_| "self-hosted,linux,x64".into()).collect();
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    assert_eq!(accumulate_demand(refs.iter().copied(), "self-hosted,linux,x64", u32::MAX), 30);
    // spawn path: max = 6 => early-exit at 6
    let refs2: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    assert_eq!(accumulate_demand(refs2.iter().copied(), "self-hosted,linux,x64", 6), 6);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p vox-cli accumulate_demand`
Expected: FAIL — `accumulate_demand` not found.

- [ ] **Step 3: Extract the pure accumulator and use it**

```rust
/// Count label-lines matching the runner's labels, stopping once `max` is hit
/// (the spawn path passes the runner cap; the telemetry path passes u32::MAX).
pub fn accumulate_demand<'a>(
    label_lines: impl Iterator<Item = &'a str>,
    runner_labels: &str,
    max: u32,
) -> u32 {
    let mut total = 0;
    for line in label_lines {
        if count_matching_queued_jobs(line, runner_labels) > 0 {
            total += 1;
            if total >= max {
                return max;
            }
        }
    }
    total
}
```

In `query_queued_job_demand`: (a) add `--paginate` to the runs list call (~:279) so the telemetry path sees the full backlog; (b) replace `.unwrap_or_default()` (~:291) with error propagation — on a `gh` error return `Err` (or log + retry) rather than counting zero; (c) feed the per-run label lines through `accumulate_demand`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p vox-cli accumulate_demand`
Expected: PASS. Confirm existing `desired_runner_count` / `count_matching_queued_jobs` tests still pass: `cargo nextest run -p vox-cli runner_scale`.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "fix(ci): paginate demand telemetry + propagate gh errors (B4, telemetry-scope)"
```

### Task 7: B2 hardening — bound the gh fan-out (no PT10M)

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs` (fan-out in `query_queued_job_demand`)

> Do **not** raise `ExecutionTimeLimit` here — that is hazardous without a lock heartbeat (see spec Risks). This task only caps the per-tick `gh` cost so PT2M is comfortably met.

- [ ] **Step 1: Cap concurrency conservatively (2-4) with the error-propagation from Task 6 already in place.** If introducing concurrency, bound it explicitly; otherwise keep sequential but ensure the early-exit (`total >= max`) is hit on the spawn path so the worst-case 42-call fan-out only occurs on the telemetry path.

- [ ] **Step 2: Add/extend a unit test** asserting the spawn-path call (`max = 6`) cannot exceed `1 + 6` per-run calls (early-exit), pinning the bound. Run: `cargo nextest run -p vox-cli runner_scale`.

- [ ] **Step 3: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "perf(ci): bound autoscaler gh fan-out so PT2M tick budget holds"
```

### Task 8: Re-enable the autoscaler (ops verification)

**Files:** `scripts/ci/voxcirunnerscale.task.xml` (already `<Enabled>true</Enabled>`); operational.

- [ ] **Step 1:** With Tasks 6-7 merged, re-register/enable the Task Scheduler job on the runner host.
- [ ] **Step 2:** Watch `ci-runner-history.jsonl` for 3-5 ticks; confirm ticks complete within PT2M (no hard-kill) and runners spawn/reap cleanly. If a tick still exceeds PT2M, do NOT raise the limit — reduce the fan-out further (Task 7) first.
- [ ] **Step 3:** Record the observation in the tracking issue (no commit unless the `.task.xml` changed).

### Task 9: Safe bloat — relocate gitleaks + link_checker, drop push triggers

**Files:**
- Modify: `.github/workflows/gitleaks.yml` (`runs-on` ~:28), `link_checker.yml` (`runs-on` ~:23 + `push:` trigger ~:3-9), `os-compat-report.yml` (`push:` ~:8-13), `mobile-e2e-ios.yml` (`push:` ~:4-6)

- [ ] **Step 1:** In `gitleaks.yml` and `link_checker.yml`, change `runs-on: [self-hosted, linux, x64]` → `runs-on: ubuntu-latest`.
- [ ] **Step 2:** Remove the `push:` trigger block from `link_checker.yml`, `os-compat-report.yml`, `mobile-e2e-ios.yml` (each retains `schedule` + `workflow_dispatch`).
- [ ] **Step 3:** Validate and commit.

```bash
actionlint .github/workflows/gitleaks.yml .github/workflows/link_checker.yml .github/workflows/os-compat-report.yml .github/workflows/mobile-e2e-ios.yml
git add .github/workflows/gitleaks.yml .github/workflows/link_checker.yml .github/workflows/os-compat-report.yml .github/workflows/mobile-e2e-ios.yml
git commit -m "ci: move gitleaks+link_checker to hosted, drop redundant push triggers"
```

> Do NOT touch `compile-matrix.yml`, `cr-l-gates.yml`, `cr-l8-corpus-feedback.yml`, or the `ci-health-*` workflows — verified to gate distinct surfaces (spec Rejected).

---

## Phase B — Structural

### Task 10: Tier non-required smokes off the merge_group critical path

**Files:** `.github/workflows/ci.yml` (jobs: `visualizer-ingest-smoke` ~:1331, `web-vite-build-smoke` ~:1351, `vox-vscode-extension` ~:1377, `docker-vox-image-smoke` ~:1413, `vox-browser-cdp-smoke` ~:1429, `gui-playwright-smoke` ~:1469, `all-features-matrix` ~:1584)

> These are NOT in `ci-summary.needs` (`ci.yml:1305`), so they do not gate the merge — but on `merge_group` they fire and consume runners, serializing the gate. Goal: they should run post-merge / scheduled, not on the `merge_group` event.

- [ ] **Step 1:** For each of the 7 jobs, change the `if:` so it does NOT run on `merge_group`. Current pattern `if: github.event_name != 'pull_request' || contains(...labels..., 'full-ci')` is TRUE on `merge_group`. Replace with an explicit post-merge condition, e.g.:

```yaml
    if: github.event_name == 'push' || contains(github.event.pull_request.labels.*.name, 'full-ci')
```

(or gate behind `workflow_run` after the merge lands — match the repo's existing post-merge pattern). Confirm none of the 7 appears in `ci-summary`'s `needs` (it must not, or this would drop a required gate).

- [ ] **Step 2:** Verify with actionlint and a dry trigger-trace: `actionlint .github/workflows/ci.yml`. Grep to confirm `ci-summary` `needs:` still lists exactly `[guards-fast, lints, compiler-gates, tests, audits]`:

Run: `grep -n "needs: \[guards-fast" .github/workflows/ci.yml`
Expected: the unchanged 5-job needs line.

- [ ] **Step 3:** After merge, re-measure (Task 4 method): count self-hosted jobs queued on a `merge_group` event.
Expected: ≤ 6 (fits one wave). Record vs baseline.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: tier non-required smokes off merge_group so gate fits 6-runner ceiling"
```

### Task 11: Move cross-platform-check's self-hosted Linux leg to hosted

**Files:** `.github/workflows/cross-platform-check.yml` (matrix `os` ~:38)

- [ ] **Step 1:** Change the Linux matrix entry from `[self-hosted, linux, x64]` to `ubuntu-latest` (the Windows/macOS legs already use hosted runners). Add `CARGO_INCREMENTAL: "0"` is already present (`:45`) — leave it.
- [ ] **Step 2:** Validate and commit.

```bash
actionlint .github/workflows/cross-platform-check.yml
git add .github/workflows/cross-platform-check.yml
git commit -m "ci: move cross-platform Linux leg to hosted, free a self-hosted slot at merge"
```

### Task 12: W6 — bake the GTK/webkit stack into the runner image

**Files:** `infra/ci-runner/Dockerfile` (~:38-42), `.github/workflows/ci.yml` (5 apt steps at ~:105,626,774,876,1217)

- [ ] **Step 1:** Add to the Dockerfile's `apt-get install` line: `libdbus-1-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev`.
- [ ] **Step 2:** Delete the 5 per-job `apt-get update && apt-get install ...` steps in `ci.yml`.
- [ ] **Step 3:** Add a regression guard. Write a failing arch-check/forbidden-pattern assertion (follow the repo's `arch-check` forbidden_pattern format) that `apt-get install` of `libgtk|libwebkit|libsoup` must not appear in `.github/workflows/ci.yml`. Run it: expect FAIL before Step 2's deletions, PASS after.
- [ ] **Step 4:** Rebuild the runner image (publish-ci-runner path) and smoke one gate run to confirm the heavy jobs still find the libs.
- [ ] **Step 5: Commit**

```bash
actionlint .github/workflows/ci.yml
git add infra/ci-runner/Dockerfile .github/workflows/ci.yml
git commit -m "ci: bake GTK/webkit stack into runner image, drop 5 per-job apt installs"
```

### Task 13: CANCEL_GAP — conservative zombie force-cancel

**Files:** `.github/workflows/ci-health-watchdog.yml` (reap loop ~:77-89), `.github/actions/ci-health-assess/action.yml` (zombie selector ~:29), `.github/workflows/ci-health-watchdog-test.yml` (fixture)

> Hazard guard (PR #334): single-sample offline+busy is insufficient. Require ≥2 consecutive offline ticks past a grace window, keep the 422-defer inner guard, log the decision.

- [ ] **Step 1: Add the healthy-fleet fixture test FIRST.** In `ci-health-watchdog-test.yml`, add a second matrix leg / job whose stubbed `gh` returns 2 runners (`status=online`, `busy=false`, names `vox-runner-auto-*`) and an empty/recent `run list`. Assert: `online>=1`, `zombies=0`, `problems` empty, and the reap step runs (does not error). This pins the already-correct healthy path (refutes the v1 B1 claim) and is the harness for Step 3.

Run: trigger `ci-health-watchdog-test.yml` via `workflow_dispatch`; expected PASS.

- [ ] **Step 2: Add an offline-busy fixture leg** whose stub returns a `vox-runner-auto-*` runner `status=offline, busy=true` with an `in_progress` `run_id`, presented across two polls. Assert (will FAIL until Step 3): the reap path emits a `gh api -X POST .../actions/runs/<run_id>/cancel` after the second tick, and a within-window 422 is deferred (no cancel).

- [ ] **Step 3: Implement the escalation** in `ci-health-watchdog.yml` reap loop: track offline-busy runners across ticks (persist a small state file or use the assess output), and when one is offline-busy for ≥2 consecutive ticks past the grace window, POST the run cancel, then DELETE the runner; on a still-cancellable 422 within the window, defer (unchanged). Echo the decision (`::notice::force-cancel run <id> on zombie runner <name>`).

- [ ] **Step 4:** Re-run both fixtures; expected PASS.

- [ ] **Step 5: Commit**

```bash
actionlint .github/workflows/ci-health-watchdog.yml .github/workflows/ci-health-watchdog-test.yml
git add .github/workflows/ci-health-watchdog.yml .github/actions/ci-health-assess/action.yml .github/workflows/ci-health-watchdog-test.yml
git commit -m "feat(ci): conservative zombie-job force-cancel (2-tick grace, 422-defer guard)"
```

### Task 14: Runner-image sccache shim guard

**Files:** `infra/ci-runner/Dockerfile` (after the sccache install ~:57-60), `infra/ci-runner/entrypoint.sh` (~:28)

- [ ] **Step 1:** In the Dockerfile after `ENV RUSTC_WRAPPER=sccache`, add a build-time smoke:

```dockerfile
RUN sccache --version && sccache --start-server && sccache --show-stats
```

- [ ] **Step 2:** In `entrypoint.sh`, add a sanity check that aborts if sccache is shadowed:

```bash
case "$(command -v sccache)" in
  /usr/local/cargo/bin/sccache) : ;;
  *) echo "FATAL: sccache resolves outside /usr/local/cargo/bin — possible fake shim" >&2; exit 1 ;;
esac
```

- [ ] **Step 3:** Rebuild the image; confirm it builds and the smoke passes.
- [ ] **Step 4: Commit**

```bash
git add infra/ci-runner/Dockerfile infra/ci-runner/entrypoint.sh
git commit -m "ci: guard against fake/shadowed sccache shim in runner image"
```

---

## Phase C — Elastic retreat (design-only, NO tasks)

Not implemented. Build only if the post-A+B measurement (Task 4 method, re-run) still shows queue-wait or merge wall-clock above the threshold agreed at the C gate. Items, in priority order: (a) hosted backlog-overflow path; (b) wire `CostCircuitBreaker` (`cost_defense.rs`) into demand-based scaling with the failing-integration-test seam noted in the spec; (c) second self-hosted host. See spec §"Phase C".

---

## Self-Review

**Spec coverage:** Phase 0 (queue-wait T1, exclude-cancelled T2, cache obs T3, baseline T4); Phase A (W1 T5, B4 T6, B2 hardening T7, autoscaler re-enable T8, safe bloat T9); Phase B (tier smokes T10, cross-platform leg T11, W6 T12, zombie cancel T13, sccache shim T14). W2 folded into T3. Rejected items explicitly fenced (T9 note). Phase C design-only. ✅ All spec sections map to a task.

**Placeholder scan:** No TBD/TODO; every code step shows code; YAML/config steps show exact lines + actionlint/grep verification (these are spec-marked "no unit seam"). The two Rust spots that depend on exact existing signatures (`scale_event_json` args in T3, `parse_iso8601` extraction in T1) include a "match the real signature" instruction with the concrete shape shown. ✅

**Type consistency:** `queue_wait_seconds(Option<&str>, Option<&str>) -> Option<i64>`, `accumulate_demand(impl Iterator<&str>, &str, u32) -> u32`, `sccache_requires_incremental(&str) -> Result<(),String>`, `JobRow.created_at: Option<String>` — names used identically across tasks. ✅
