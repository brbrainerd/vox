# CI/CD Runner Audit Remediation Implementation Plan (v2, plan-verified)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the single-box self-hosted CI gate fit its real 6-runner ceiling, fix the one confirmed config bug that disables sccache, close the zombie-job cancellation gap, and make queue-wait time measurable — leaving behind machine-checkable guards, without losing any test coverage.

**Architecture:** Measure-first (Phase 0), then confirmed quick wins (Phase A), then the structural fix (Phase B: tier non-required smokes off `merge_group` so the *required-needs lane* fits 6 runners in one wave). Phase C (elastic retreat) is design-only. Every fix lands a `vox ci` / `vox doctor` / arch-check guard so it can't silently regress.

**Tech Stack:** Rust (`vox-cli`, `vox-arch-check`), GitHub Actions YAML, Docker (WSL2 self-hosted runners), `gh` CLI, `cargo nextest`, `actionlint`, `chrono`.

**Source spec:** `docs/superpowers/specs/2026-06-30-ci-cd-runner-audit-remediation-design.md`

> **v2 note.** Every code claim in v1 of this plan was adversarially re-verified against source. Corrected: datetime crate is **chrono** not `time`; `JobRow` needs a `Default` derive; `count_matching_queued_jobs` counts a whole multi-line blob (per-run), so the demand seam is per-run not per-line; `scale_event_json` has 12 positional args and its only caller is `run_scale` at :860 (calling `s3_cache_reachable()` there adds an 800ms probe); the W1 guard has no existing `vox ci` lane (use arch-check + a scoped pure fn); the 7 smoke jobs split into TWO `if:` families; the zombie cron is stateless (use a repo variable). See per-task code.

---

## Task DAG (parallel reads, serialized writes per shared file)

Reads (file inspection, `actionlint` dry-runs, grep checks) parallelize freely. **Writes serialize only within a shared file.** Lanes:

- **Lane R** — `job_timings.rs`: T1 → T2 (serial; same file)
- **Lane S** — `runner_scale.rs`: T3 → T6 → T7 (serial; same file)
- **Lane G** — guards (`layers.toml` + a vox-cli guard module): T5 (W1 guard) ∥ T15 (fan-out guard) — independent of R/S
- **Lane Y** — disjoint YAML: T9 ∥ T11 (different files)
- **Lane C** — shared `ci.yml`/Docker writes: T10 → T12 → T14 (T12/T14 both touch the Dockerfile; T10/T12 both touch ci.yml) ; T13 (ci-health-* files) parallel to Lane C
- **Gates:** T4 (baseline) after R+S land; **T8 (autoscaler re-enable) moved to the END of Phase B** (after T10/T11 reduce fan-out, else its PT2M/wave verification is stale).

---

## Phase 0 — Measure first

### Task 1: Queue-wait metric (pure fn + JobRow field)

**Files:** Modify `crates/vox-cli/src/commands/ci/job_timings.rs` (`JobRow` L29-39; beside `run_seconds` L56; existing test literals L275-296). Test: same file.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn queue_wait_is_started_minus_created() {
    assert_eq!(queue_wait_seconds(Some("2026-06-30T10:00:00Z"), Some("2026-06-30T10:00:30Z")), Some(30));
}
#[test]
fn queue_wait_none_when_either_missing() {
    assert_eq!(queue_wait_seconds(None, Some("2026-06-30T10:00:30Z")), None);
    assert_eq!(queue_wait_seconds(Some("2026-06-30T10:00:00Z"), None), None);
}
#[test]
fn queue_wait_never_negative() {
    assert_eq!(queue_wait_seconds(Some("2026-06-30T10:00:30Z"), Some("2026-06-30T10:00:00Z")), Some(0));
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo nextest run -p vox-cli queue_wait`
Expected: FAIL — `queue_wait_seconds` not found.

- [ ] **Step 3: Add the field (derive Default) + chrono fn + fix existing literals**

Change the `JobRow` derive to add `Default`, and add `created_at`:

```rust
#[derive(Debug, Deserialize, Clone, Default)]
struct JobRow {
    #[serde(default)]
    name: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    run_id: Option<u64>,
}
```

Add beside `run_seconds` (chrono — same as `run_seconds`; there is **no** `parse_iso8601` helper, do not invent one):

```rust
/// Seconds a job waited in queue before a runner picked it up
/// (`started_at - created_at`). `None` if either timestamp is absent/unparseable.
/// Clamped at 0 to absorb clock skew. (`run_seconds` measures execution; this measures wait.)
pub fn queue_wait_seconds(created_at: Option<&str>, started_at: Option<&str>) -> Option<i64> {
    let created = chrono::DateTime::parse_from_rfc3339(created_at?).ok()?;
    let started = chrono::DateTime::parse_from_rfc3339(started_at?).ok()?;
    Some((started - created).num_seconds().max(0))
}
```

The 3 existing `JobRow { ... }` literals at L275-296 enumerate every field; add `created_at: None,` to each (or rewrite with `..Default::default()`), else they won't compile.

- [ ] **Step 4: Run — expect PASS**

Run: `cargo nextest run -p vox-cli job_timings`
Expected: PASS (queue_wait tests + existing tests still green).

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/job_timings.rs
git commit -m "feat(ci): measure queue-wait time (started_at - created_at)"
```

### Task 2: Exclude cancelled runs from the timing dataset (+ the promised queue-wait column)

**Files:** Modify `job_timings.rs` (`timings_from_rows` L101-115 — iterator, NOT a for-loop); `.github/workflows/ci-timings.yml` (jq L41-44, summary L59-61). Test: `job_timings.rs`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn cancelled_jobs_excluded_from_timings() {
    let rows = vec![
        JobRow { conclusion: Some("success".into()),   started_at: Some("2026-06-30T10:00:00Z".into()), completed_at: Some("2026-06-30T10:05:00Z".into()), ..Default::default() },
        JobRow { conclusion: Some("cancelled".into()), started_at: Some("2026-06-30T10:00:00Z".into()), completed_at: Some("2026-06-30T10:01:00Z".into()), ..Default::default() },
    ];
    assert_eq!(timings_from_rows(&rows).len(), 1, "cancelled run must not pollute the dataset");
}
```

- [ ] **Step 2: Run — expect FAIL** (`len()` is 2). Run: `cargo nextest run -p vox-cli cancelled_jobs_excluded`

- [ ] **Step 3: Add the filter in the iterator chain** (there is no `for`/`continue` loop — it's `.iter().filter_map(...)`):

```rust
let mut t: Vec<JobTiming> = rows
    .iter()
    .filter(|j| j.conclusion.as_deref() != Some("cancelled")) // <-- add this line
    .filter_map(|j| run_seconds(j.started_at.as_deref(), j.completed_at.as_deref())
        .map(|secs| JobTiming { /* existing field construction unchanged */ }))
    .collect();
```

Add an SSOT-anchor comment above it: `// SSOT: mirrored in .github/workflows/ci-timings.yml jq (keep the "cancelled" literal in sync)`.

- [ ] **Step 4: Run — expect PASS.** Run: `cargo nextest run -p vox-cli cancelled_jobs_excluded`

- [ ] **Step 5: Edit ci-timings.yml — exclude cancelled AND add the queue-wait column the plan promised**

The exact current jq (L41-44):

```
  --jq '.jobs[] | select(.started_at != null and .completed_at != null)
        | {name, conclusion, secs: ((.completed_at|fromdateiso8601) - (.started_at|fromdateiso8601))}' \
```

becomes (the `.../actions/runs/<id>/jobs` endpoint returns `created_at` per job — verified live):

```
  --jq '.jobs[] | select(.started_at != null and .completed_at != null and .conclusion != "cancelled")
        | {name, conclusion,
           secs: ((.completed_at|fromdateiso8601) - (.started_at|fromdateiso8601)),
           queue_secs: ((if .created_at != null then ((.started_at|fromdateiso8601) - (.created_at|fromdateiso8601)) else 0 end) | if . < 0 then 0 else . end)}' \
```

Extend the step-summary table (L59-61) to add the queue-wait column:

```
echo "| run-time | queue-wait | job | conclusion |"
echo "|---:|---:|---|---|"
jq -r ".[] | \"| \((.secs/60)|floor)m\((.secs%60)|floor)s | \((.queue_secs)|floor)s | \(.name) | \(.conclusion) |\"" jobs.json
```

- [ ] **Step 6: Validate and commit**

```bash
actionlint .github/workflows/ci-timings.yml
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/job_timings.rs .github/workflows/ci-timings.yml
git commit -m "fix(ci): exclude cancelled runs + surface queue-wait in timings report"
```

### Task 3: Surface sccache stats + S3-cache reachability in the scale event

**Files:** `.github/workflows/ci.yml` (after the `setup` build, ~L106); `crates/vox-cli/src/commands/ci/runner_scale.rs` (`scale_event_json` L654-671 — **12 positional args**; sole caller `run_scale` L860; existing test L1253; `s3_cache_reachable()` L402). Test: `runner_scale.rs`.

- [ ] **Step 1: Write the failing test (13th arg)**

```rust
#[test]
fn scale_event_json_includes_s3_reachable() {
    let json = scale_event_json(1_000_000, false, 3, 2, 3, 1, 0, 0, 0, 1, 6, 1, /* s3_cache_reachable */ false);
    assert!(json.contains("\"s3_cache_reachable\":false"));
}
```

- [ ] **Step 2: Run — expect FAIL (arity).** Run: `cargo nextest run -p vox-cli scale_event_json_includes_s3`

- [ ] **Step 3a:** Add a trailing `s3_cache_reachable: bool` as the **last** positional param of `scale_event_json` (L654-671); append `,"s3_cache_reachable":{s3_cache_reachable}` to the `format!` literal before the closing `}}`.
- [ ] **Step 3b:** At the **sole caller** `run_scale` (L860) — `s3_cache_reachable` is NOT in scope here (the :522 call is inside `spawn_one`). Compute it **once** at the top of `run_scale` and bind a local `let s3_reachable = s3_cache_reachable();` (one 800ms TCP probe per tick — acceptable within PT2M; reuse the same local for the `spawn_one` path instead of the inline :522 call). Pass `s3_reachable` as the final arg to `scale_event_json(...)`.
- [ ] **Step 3c (REQUIRED):** Update the existing test `scale_event_json_has_all_decision_fields` (L1253) — add a trailing `false,` 13th arg, else the crate won't compile.

- [ ] **Step 4: Run — expect PASS.** Run: `cargo nextest run -p vox-cli scale_event`

- [ ] **Step 5: Add the sccache stats step + commit**

In `ci.yml` after the build step (~L106):

```yaml
      - name: sccache stats (gate cache hit rate)
        if: always()
        run: sccache --show-stats
```

```bash
actionlint .github/workflows/ci.yml
cargo fmt -p vox-cli
git add .github/workflows/ci.yml crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): surface sccache hit rate + S3-cache reachability in scale events"
```

### Task 4: Capture the baseline (observation, no code)

- [ ] After T1-T3 merge, run one full `merge_group` gate and record: merge-gate wall-clock, per-job **queue-wait** (T1), sccache hit rate (T3 — expect LOW pre-W1), and self-hosted jobs queued at merge time **bucketed by required label set** (general `linux,x64` vs `browser`-labelled). Write the numbers into the spec's success-criteria baseline and commit that doc edit. These gate Phase B and Phase C.

---

## Phase A — Confirmed quick wins

### Task 5: W1 — enable sccache on the gate + machine guard (all workflows)

**Files:** `.github/workflows/ci.yml:25-29`; a pure guard in an existing vox-cli ci module (NOT a new `workflow_lint.rs` wired into a nonexistent lane); `docs/src/architecture/layers.toml`. Test: colocated.

- [ ] **Step 1: The one-line fix (verified correct).** In `ci.yml:25-29` env block, after `RUSTC_WRAPPER: sccache` add:

```yaml
  CARGO_INCREMENTAL: "0"
```

- [ ] **Step 2: Write the failing guard test** (scoped to the **top-level env block only** — split before `\njobs:` — so the deliberate step-level `RUSTC_WRAPPER: ""` opt-out in `toolchain-lint-wave` is not flagged; iterate **all** workflows, not just ci.yml):

```rust
fn top_env_pins_incremental(workflow_text: &str) -> bool {
    let header = workflow_text.split("\njobs:").next().unwrap_or(workflow_text);
    let on = header.contains("\n  RUSTC_WRAPPER: sccache");
    let pinned = header.contains("\n  CARGO_INCREMENTAL: \"0\"");
    !on || pinned
}

#[test]
fn all_sccache_workflows_pin_incremental_zero() {
    use std::fs;
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows");
    let mut bad = vec![];
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map(|x| x == "yml").unwrap_or(false) {
            let txt = fs::read_to_string(&p).unwrap();
            if !top_env_pins_incremental(&txt) { bad.push(p.display().to_string()); }
        }
    }
    assert!(bad.is_empty(), "sccache-on workflows missing top-level CARGO_INCREMENTAL=0: {bad:?}");
}
```

- [ ] **Step 3: Run — expect PASS after Step 1** (and confirm it would have FAILED before Step 1 by reverting the one line locally and re-running). Run: `cargo nextest run -p vox-cli all_sccache_workflows_pin_incremental`

- [ ] **Step 4: Verify the win.** Re-run the gate; confirm `sccache --show-stats` (T3) now shows a non-zero hit rate on the second run. (Per PV4: confirm empirically rather than asserting 0%→100%.)

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-cli
git add .github/workflows/ci.yml crates/vox-cli/src/commands/ci/<module>.rs
git commit -m "fix(ci): set CARGO_INCREMENTAL=0 on gate so sccache caches (+all-workflow guard)"
```

### Task 6: B4 — demand telemetry pagination + error propagation (per-run seam)

**Files:** `runner_scale.rs` (`query_queued_job_demand` L273-299; `count_matching_queued_jobs` L162-173 — **takes a whole blob, returns a count**; `DEMAND_RUNS_PER_STATUS` L77). Test: `runner_scale.rs`.

- [ ] **Step 1: Write the failing test (MUST include a multi-job blob)** — this exposes the per-line bug v1 had:

```rust
#[test]
fn accumulate_demand_sums_multi_job_runs_and_stops_at_max() {
    let three = "self-hosted,linux,x64\nself-hosted,linux,x64\nself-hosted,linux,x64"; // one run, 3 queued jobs
    let one = "self-hosted,linux,x64";
    let blobs = [three, one, one];
    assert_eq!(accumulate_demand(blobs.iter().copied(), "self-hosted,linux,x64", u32::MAX), 5); // telemetry: full
    assert_eq!(accumulate_demand(blobs.iter().copied(), "self-hosted,linux,x64", 4), 4);        // spawn: early-exit
}
```

- [ ] **Step 2: Run — expect FAIL** (`accumulate_demand` not found). Run: `cargo nextest run -p vox-cli accumulate_demand`

- [ ] **Step 3: Add the per-RUN accumulator (reuse `count_matching_queued_jobs`, do NOT re-count per-line):**

```rust
/// Sum queued-job demand across runs, stopping once `max` is reached. Each item
/// is one run's jq blob (one queued job per line); spawn path passes the runner
/// cap, telemetry path passes u32::MAX. Preserves count_matching_queued_jobs's
/// per-blob count semantics (a single run can hold N matching jobs).
pub fn accumulate_demand<'a>(run_blobs: impl Iterator<Item = &'a str>, runner_labels: &str, max: u32) -> u32 {
    let mut total = 0u32;
    for blob in run_blobs {
        total = total.saturating_add(count_matching_queued_jobs(blob, runner_labels));
        if total >= max { return max; }
    }
    total
}
```

In `query_queued_job_demand`: (a) replace the `.unwrap_or_default()` at L291 with `?` so a `gh` error propagates (on the spawn path, an `Err` that aborts the tick is preferable to silently counting 0 and under-provisioning); (b) feed each run's blob through `accumulate_demand`; (c) **pagination caveat:** `--paginate` on the runs-list call (L279) has no effect while `per_page={DEMAND_RUNS_PER_STATUS}` caps it at 20 — to actually see the full backlog on the telemetry (`u32::MAX`) path you must also raise/remove the `DEMAND_RUNS_PER_STATUS` cap for that path; otherwise drop the "sees full backlog" claim and keep the cap.

- [ ] **Step 4: Run — expect PASS;** confirm existing `count_matching_queued_jobs` / `desired_runner_count` tests still green. Run: `cargo nextest run -p vox-cli runner_scale`

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "fix(ci): per-run demand accumulation + gh error propagation (B4 telemetry)"
```

### Task 7: B2 hardening — bound the gh fan-out (no PT10M)

**Files:** `runner_scale.rs`.

> Do NOT raise `ExecutionTimeLimit` (PT10M needs a lock heartbeat first — spec Risks). Cap fan-out only.

- [ ] **Step 1:** With T6's error-propagation in place, ensure the spawn path's early-exit (`total >= max`, L293, max=6) bounds it to `1 + 6` per-run calls; if introducing concurrency for the telemetry path, bound it to 2-4 with jitter (secondary-rate-limit guard).
- [ ] **Step 2: Unit test** the spawn-path bound (max=6 ⇒ ≤7 per-run calls). Run: `cargo nextest run -p vox-cli runner_scale`
- [ ] **Step 3: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "perf(ci): bound autoscaler gh fan-out so PT2M tick budget holds"
```

### Task 9: Safe bloat — relocate gitleaks + link_checker, drop push triggers

**Files:** `.github/workflows/gitleaks.yml` (`runs-on` ~L28), `link_checker.yml` (`runs-on` ~L23 + `push:` ~L3-9), `os-compat-report.yml` (`push:` ~L8-13), `mobile-e2e-ios.yml` (`push:` ~L4-6). (Lane Y — parallel to T11.)

- [ ] **Step 1:** `gitleaks.yml` + `link_checker.yml`: `runs-on: [self-hosted, linux, x64]` → `runs-on: ubuntu-latest`.
- [ ] **Step 2:** Remove the `push:` trigger block from `link_checker.yml`, `os-compat-report.yml`, `mobile-e2e-ios.yml` (each keeps `schedule` + `workflow_dispatch`).
- [ ] **Step 3:** Validate + commit.

```bash
actionlint .github/workflows/gitleaks.yml .github/workflows/link_checker.yml .github/workflows/os-compat-report.yml .github/workflows/mobile-e2e-ios.yml
git add .github/workflows/gitleaks.yml .github/workflows/link_checker.yml .github/workflows/os-compat-report.yml .github/workflows/mobile-e2e-ios.yml
git commit -m "ci: move gitleaks+link_checker to hosted, drop redundant push triggers"
```

> Do NOT touch `compile-matrix.yml`, `cr-l-gates.yml`, `cr-l8-corpus-feedback.yml`, or the `ci-health-*` workflows — verified to gate distinct surfaces (spec Rejected).

---

## Phase B — Structural

### Task 10: Tier non-required smokes off merge_group — PER JOB (two groups)

**Files:** `.github/workflows/ci.yml`. The 7 jobs split into TWO `if:` families — do NOT apply one blanket replacement.

- [ ] **Step 1: Confirm safety (verified):** `ci-summary` `needs: [guards-fast, lints, compiler-gates, tests, audits]` (L1305); none of the 7 appear in any `needs:`. Tiering them off drops no required gate.

Run: `grep -n "needs: \[guards-fast" .github/workflows/ci.yml`
Expected: L1305 unchanged.

- [ ] **Step 2: GROUP B (event/label-gated) — exclude merge_group.** For `docker-vox-image-smoke` (L1413), `vox-browser-cdp-smoke` (L1429), `gui-playwright-smoke` (L1469), `all-features-matrix` (L1584), replace
  `if: github.event_name != 'pull_request' || contains(github.event.pull_request.labels.*.name, 'full-ci')`
  with
  `if: (github.event_name == 'push' && github.ref == 'refs/heads/main') || contains(github.event.pull_request.labels.*.name, 'full-ci')`

- [ ] **Step 3: GROUP A (path-affects-gated) — add a merge_group exclusion, PRESERVE affects gating.** These run on merge_group only because setup forces `full=true` (L75). Do NOT use the blanket if (it would delete their PR-time `affects_web`/`affects_gui` coverage). Keep their `needs: setup`.
  - `visualizer-ingest-smoke` (L1331): `if: github.event_name != 'merge_group' && (needs.setup.outputs.full == 'true' || needs.setup.outputs.affects_web == 'true')`
  - `web-vite-build-smoke` (L1352): same as above (`affects_web`)
  - `vox-vscode-extension` (L1378): `if: github.event_name != 'merge_group' && (needs.setup.outputs.full == 'true' || needs.setup.outputs.affects_gui == 'true')`

- [ ] **Step 4: Validate + re-measure.** `actionlint .github/workflows/ci.yml`; confirm L1305 unchanged; re-run the Task-4 measurement and confirm the **required-needs lane** (`setup` + 5 needs = 6 general-label jobs) fits one wave and the 7 smokes (incl. the 24-leg matrix) no longer fire on merge_group. Bucket by label set (the browser-labelled jobs are gone from merge_group anyway).

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: tier non-required smokes off merge_group (per-job, preserve PR affects gating)"
```

### Task 11: Move cross-platform-check's Linux leg to hosted (+ flip sccache backend)

**Files:** `.github/workflows/cross-platform-check.yml` (matrix include L38-40). (Lane Y — parallel to T9.)

- [ ] **Step 1:** Change the Linux matrix entry (L38) from `- os: [self-hosted, linux, x64]` to `- os: ubuntu-latest`, AND change its `sccache_gha: "false"` (L40) to `sccache_gha: "true"` — the self-hosted `/cache/sccache` volume does not exist on ubuntu-latest and `actions/cache@v5` (L62-70) does not cache `SCCACHE_DIR`, so without the GHA backend the leg would 0%-hit. Leave `CARGO_INCREMENTAL: "0"` (L45) and the `runner.os == 'Linux'` gate (L89).
- [ ] **Step 2: Validate + commit.**

```bash
actionlint .github/workflows/cross-platform-check.yml
git add .github/workflows/cross-platform-check.yml
git commit -m "ci: move cross-platform Linux leg to hosted (GHA sccache), free a self-hosted slot"
```

### Task 12: W6 — bake the GTK/webkit stack into the runner image (+ arch-check guard)

**Files:** `infra/ci-runner/Dockerfile` (apt block L38-42; `libglib2.0-dev` already on L41); `.github/workflows/ci.yml` (5 apt steps L105,626,774,876,1217); `docs/src/architecture/layers.toml`.

- [ ] **Step 1:** Append to the Dockerfile apt list (L41) ONLY the not-yet-present libs (do NOT re-add `libglib2.0-dev`): `libdbus-1-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev`.
- [ ] **Step 2:** Delete the 5 per-job `apt-get update && apt-get install ...` steps in `ci.yml` (L105,626,774,876,1217).
- [ ] **Step 3: Rebuild the runner image** (publish-ci-runner path) and smoke one gate run to confirm the heavy jobs still find the libs.
- [ ] **Step 4: Add the durable arch-check guard** — in `docs/src/architecture/layers.toml` after the existing `[[forbidden_pattern]]` blocks (~L342):

```toml
[[forbidden_pattern]]
name = "no-apt-gtk-in-ci"
# W6: the GTK/webkit stack must be baked into infra/ci-runner/Dockerfile, not
# re-installed per CI job. The single `forbidden_pattern = "error"` gate fails CI
# on any new occurrence. Land this in the SAME commit as the deletions above.
pattern   = 'apt-get install[^\n]*(libgtk-3-dev|libwebkit2gtk|libsoup-3)'
file_glob = ".github/workflows/*.yml"
```

Add a tempdir fixture test mirroring `crates/vox-arch-check/src/forbidden_patterns.rs:229` with a `.github/workflows/x.yml` fixture to pin that the walker descends into `.github` (it should — `.github` is not in `WALK_PRUNE_DEFAULT_DIR_NAMES`). Run the arch-check gate: FAIL before Step 2, PASS after.

- [ ] **Step 5: Commit (deletions + guard together — the gate is `error`):**

```bash
actionlint .github/workflows/ci.yml
git add infra/ci-runner/Dockerfile .github/workflows/ci.yml docs/src/architecture/layers.toml
git commit -m "ci: bake GTK/webkit into runner image, drop 5 apt steps, add arch-check guard (W6)"
```

### Task 13: CANCEL_GAP — conservative zombie force-cancel (repo-variable state)

**Files:** `.github/workflows/ci-health-watchdog.yml` (reap loop L77-89; uses repo var `CI_HEALTH_STATE` L66,74 — the durable cross-tick channel); `.github/actions/ci-health-assess/action.yml` (single-sample zombie selector L29); `.github/workflows/ci-health-watchdog-test.yml`; pure intersection fn in `runner_scale.rs`.

> The watchdog runs on **stateless** `ubuntu-latest` cron (`*/10`); a local state file does NOT persist across ticks, and `~/.vox/ci-runner-history.jsonl` is self-hosted-local (unreachable here, watchdog.yml:129). Consecutiveness must come from a **GitHub repo variable** (the mechanism already used for `CI_HEALTH_STATE`).

- [ ] **Step 1: Pure intersection fn (TDD seam) + test.** In `runner_scale.rs`:

```rust
/// Runners offline-busy in BOTH the previous and current tick (>=2 consecutive
/// ticks) — the only ones eligible for force-cancel.
pub fn zombies_for_force_cancel(prev_ids: &[u64], curr_offline_busy: &[u64]) -> Vec<u64> {
    let prev: std::collections::HashSet<u64> = prev_ids.iter().copied().collect();
    curr_offline_busy.iter().copied().filter(|id| prev.contains(id)).collect()
}
```

```rust
#[test]
fn force_cancel_only_two_tick_offline_busy() {
    assert_eq!(zombies_for_force_cancel(&[1,2], &[2,3]), vec![2]); // 2 seen both ticks
    assert_eq!(zombies_for_force_cancel(&[], &[2,3]), Vec::<u64>::new()); // first sighting: grace
}
```

Run: `cargo nextest run -p vox-cli force_cancel_only_two_tick` → PASS.

- [ ] **Step 2: Seed the new repo variable** `CI_HEALTH_ZOMBIE_IDS` (empty) as a prereq alongside `CI_HEALTH_STATE` (document in the watchdog header).

- [ ] **Step 3: Wire the watchdog** (before the reap loop): read prior offline-busy IDs from the variable, intersect with the current tick's offline-busy managed set, force-cancel only the intersection, carry the current set forward. Keep the 422-defer inner guard. Log decisions.

```yaml
      - name: Force-cancel runners offline-busy >=2 consecutive ticks
        if: ${{ env.DRY_RUN != 'true' }}
        run: |
          set -euo pipefail
          runners=$(gh api "repos/$REPO/actions/runners" --paginate)
          mapfile -t curr < <(jq -r '.runners[]|select(.status=="offline" and .busy==true and (.name|startswith("vox-runner-auto-")))|.id' <<<"$runners")
          curr_csv=$(IFS=,; echo "${curr[*]:-}")
          prev_csv=$(gh variable get CI_HEALTH_ZOMBIE_IDS 2>/dev/null || echo "")
          for id in "${curr[@]:-}"; do
            [ -z "$id" ] && continue
            case ",$prev_csv," in
              *",$id,"*)
                run_id=$(gh run list --status in_progress --json databaseId --jq '.[0].databaseId' || true)
                if [ -n "$run_id" ]; then
                  gh api -X POST "repos/$REPO/actions/runs/$run_id/cancel" \
                    && echo "::notice::force-cancel run $run_id on zombie runner $id"
                fi
                gh api -X DELETE "repos/$REPO/actions/runners/$id" 2>/dev/null \
                  || echo "::warning::runner $id 422 mid-job — defer (inner guard)" ;;
              *) echo "::notice::runner $id offline-busy 1 tick — grace, deferring" ;;
            esac
          done
          gh variable set CI_HEALTH_ZOMBIE_IDS --body "${curr_csv:-}"
```

(The `GH_TOKEN` already has Actions:write + Administration, covering run-cancel + runner DELETE.)

- [ ] **Step 4: Fixture** in `ci-health-watchdog-test.yml` — a single dispatch is ONE tick, so simulate consecutiveness by SEEDING prior state: positive leg stubs `gh variable get CI_HEALTH_ZOMBIE_IDS` to return the zombie id + stubs the runners list offline-busy → assert the cancel POST fires; negative leg stubs the variable empty → assert NO cancel (grace notice). Also add the healthy-fleet leg (online≥1, zombies=0, problems empty) that refutes the v1 B1 claim.
- [ ] **Step 5: Commit**

```bash
actionlint .github/workflows/ci-health-watchdog.yml .github/workflows/ci-health-watchdog-test.yml
cargo fmt -p vox-cli
git add .github/workflows/ci-health-watchdog.yml .github/actions/ci-health-assess/action.yml .github/workflows/ci-health-watchdog-test.yml crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): conservative zombie force-cancel (2-tick repo-var state, 422-defer)"
```

### Task 14: Runner-image sccache shim guard (+ vox doctor diagnosis)

**Files:** `infra/ci-runner/Dockerfile` (sccache install L57; `ENV RUSTC_WRAPPER=sccache` L60; `CARGO_HOME=/usr/local/cargo` L29); `infra/ci-runner/entrypoint.sh` (`set -euo pipefail` L10); `crates/vox-cli/.../build_health.rs` (`sccache_guard` ~L246; `KNOWN_DIAGNOSIS_IDS` L17-30).

- [ ] **Step 1:** Insert a build-time smoke immediately AFTER `ENV RUSTC_WRAPPER=sccache` (L60):

```dockerfile
RUN sccache --version && sccache --start-server && sccache --show-stats
```

- [ ] **Step 2:** In `entrypoint.sh` after `set -euo pipefail` (L10), add the shim guard (binary lands at `/usr/local/cargo/bin/sccache` via `CARGO_HOME` L29):

```bash
case "$(command -v sccache)" in
  /usr/local/cargo/bin/sccache) : ;;
  *) echo "FATAL: sccache resolves outside /usr/local/cargo/bin — possible fake shim" >&2; exit 1 ;;
esac
```

- [ ] **Step 3: Add an AI-greppable `vox doctor` diagnosis** (so the invariant is checkable on any host, not just at container start). In `build_health.rs`: register `sccache.shadowed_shim` in `KNOWN_DIAGNOSIS_IDS` (L17-30); add a pure classifier beside `is_real_rustc` (L44):

```rust
/// A genuine sccache resolves under the cargo bin; a `.cmd` forwarder or other
/// path is the fake-shim tell (caches nothing, slows every compile).
pub fn sccache_path_is_canonical(resolved: &str) -> bool {
    resolved.ends_with("/usr/local/cargo/bin/sccache")
        || resolved.ends_with("\\.cargo\\bin\\sccache.exe")
        && !resolved.ends_with(".cmd")
}
```

Call `which::which("sccache")` in `sccache_guard` (~L246) and emit `sccache.shadowed_shim` when not canonical. Add a unit test feeding a `.cmd` path (expect not-canonical) and the cargo-bin path (expect canonical). Run: `cargo nextest run -p vox-cli sccache_path_is_canonical`.

- [ ] **Step 4: Rebuild image; confirm build + smoke pass. Commit.**

```bash
cargo fmt -p vox-cli
git add infra/ci-runner/Dockerfile infra/ci-runner/entrypoint.sh crates/vox-cli/src/commands/ci/build_health.rs
git commit -m "ci: guard fake/shadowed sccache shim (entrypoint hard-fail + vox doctor diag)"
```

### Task 15: merge_group fan-out guard (the biggest passive-YAML gap)

**Files:** new pure fn + a vox-cli `guards-fast` entry; shared ceiling const (today only `runner_scale.rs:60 DEFAULT_MAX_RUNNERS`). (Lane G — parallel to Lane R/S.)

> Without this, the next self-hosted job whose `if:` is truthy on merge_group silently re-breaches the ceiling — the exact regression this effort prevents.

- [ ] **Step 1: Check for prior art.** Read `crates/vox-cli/src/commands/ci/fan_in_budget.rs` — if it already models job-count-vs-ceiling, extend it; else add a new pure fn.
- [ ] **Step 2: Failing test** for a pure `merge_group_self_hosted_fanout(workflow_yaml: &str) -> BTreeMap<String, usize>` (count self-hosted jobs whose `if:` is truthy under `event_name=merge_group`, keyed by required label set):

```rust
#[test]
fn fanout_counts_required_lane_within_ceiling() {
    let yaml = include_str!("../../../../../.github/workflows/ci.yml");
    let buckets = merge_group_self_hosted_fanout(yaml);
    // the general linux,x64 required lane must fit the ceiling in one wave
    let general = buckets.get("self-hosted,linux,x64").copied().unwrap_or(0);
    assert!(general <= DEFAULT_MAX_RUNNERS as usize,
        "merge_group general-label self-hosted fan-out {general} exceeds ceiling {}", DEFAULT_MAX_RUNNERS);
}
```

- [ ] **Step 3:** Implement the parser (reuse a YAML crate already in the workspace; evaluate each job's `runs-on` for self-hosted labels and its `if:` for the merge_group-truthy condition). Surface `DEFAULT_MAX_RUNNERS` as a shared const the guard imports (do not re-hardcode 6). Run after Task 10 lands so the test passes; before Task 10 it documents the breach.
- [ ] **Step 4:** Wire the guard into the `guards-fast` `vox ci` entry so it gates pre-merge. Commit.

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/<module>.rs
git commit -m "feat(ci): guard merge_group self-hosted fan-out against the runner ceiling"
```

---

## Task 8: Re-enable the autoscaler (ops verification) — END of Phase B

> **Moved from Phase A.** Re-enabling against the pre-reduction demand profile would yield a stale PASS/FAIL. Run this LAST, after T10/T11 reduce merge-time fan-out.

- [ ] **Step 1:** With T6/T7 (code) merged and T10/T11 (fan-out reduction) landed, re-register/enable the Task Scheduler job on the runner host.
- [ ] **Step 2:** Watch `ci-runner-history.jsonl` for 3-5 ticks; confirm ticks complete within PT2M (no hard-kill) and runners spawn/reap cleanly against the **reduced** workload. If a tick still exceeds PT2M, reduce fan-out further (do NOT raise the limit — PT10M needs a lock heartbeat first).
- [ ] **Step 3:** Re-run the Task-4 measurement (post-reduction) and record vs baseline. Commit any `.task.xml` change.

---

## Phase C — Elastic retreat (design-only, NO tasks)

Build only if the post-A+B measurement (Task 4 method, re-run) still exceeds the threshold agreed at the C gate. Items: (a) hosted backlog-overflow path; (b) wire `CostCircuitBreaker` (`cost_defense.rs`) into demand-based scaling with a failing-integration-test seam in `vox-integration-tests`; (c) second self-hosted host. See spec §"Phase C".

---

## Self-Review

**Spec coverage:** Phase 0 (queue-wait T1, exclude-cancelled+column T2, cache obs T3, baseline T4); Phase A (W1+guard T5, B4 T6, B2 hardening T7, safe bloat T9); Phase B (tier smokes T10, cross-platform leg T11, W6+guard T12, zombie cancel T13, sccache shim+diag T14, fan-out guard T15); autoscaler re-enable T8 at the end. Machine-enforcement section → T5/T12/T14/T15. Phase C design-only. ✅

**Placeholder scan:** No TBD; every Rust step shows code matching real signatures (chrono, 12+1 args, per-blob `count_matching_queued_jobs`, repo-var state); YAML steps show exact text + actionlint/grep verification. ✅

**Type consistency:** `queue_wait_seconds(Option<&str>,Option<&str>)->Option<i64>`; `accumulate_demand(impl Iterator<&str>,&str,u32)->u32` (per-blob); `scale_event_json(...,bool)->String` (13 args, test at L1253 updated); `zombies_for_force_cancel(&[u64],&[u64])->Vec<u64>`; `top_env_pins_incremental(&str)->bool`; `sccache_path_is_canonical(&str)->bool`; `merge_group_self_hosted_fanout(&str)->BTreeMap<String,usize>`. Names consistent across tasks. ✅

**Ordering:** T8 moved to end (post-fan-out-reduction); T4 baseline after R+S; T10-step4 re-measures; T15 passes after T10. ✅
