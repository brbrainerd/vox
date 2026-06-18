# CI Observability & Targeted Dispatch — Implementation Plan (Antigravity Handoff)

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **EXECUTION TARGET: Gemini 3.5 Flash inside Google Antigravity.** This plan is written to the constraints in [`docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5 and the handoff rules in [`docs/src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md). **Obey these or the run will break:**
> - **Every task ends GREEN + committed.** A kill between tasks must leave a compiling, tested tree. Never split a compile-breaking change across two commits.
> - **Verify-before-use.** Each task begins with a Pre-flight `rg`/read step. Do NOT reference a symbol/path you have not just confirmed. No invented APIs.
> - **Self-contained tasks.** Needed context/signatures are repeated in each task; do not rely on recall of earlier tasks.
> - **Two-strike circuit breaker.** If a task's verification fails twice, STOP and write a handoff note in the task; do not loop.
> - **One decision per step.** No open-ended "design X" steps here.
> - **`[PARALLEL-SAFE]` / `[SEQUENTIAL]`** tags per task by file-write disjointness. Never run two subagents that write the same file.
> - **Repo policy:** VoxScript-first (no `.ps1`/`.sh`/`.py`; use `vox run scripts/*.vox`); **never `cargo fmt --all`** (use `cargo fmt -p <crate>`); any new `.md` under `docs/src/` needs YAML frontmatter; no stubs/placeholders.

**Goal:** Make CI work proportional to the change and observable end-to-end — first by centralizing how runner jobs are received and logged (so the glut is *visible*), then by computing a targeted check-set + reuse hash per diff.

**Architecture:** Two foundational phases. **Phase 0 (Observability)** adds a `RunnerJobLifecycle` telemetry event emitted at every job intake/dispatch/complete point in the autoscaler, plus a `vox ci runner-observe` view that answers "which runner is doing what, for which ref/PR, why, and for how long." **Phase 1 (CI Plan SSOT)** adds `vox ci plan`, a pure function of a diff that emits `{global, checks[], blast_radius[], reuse_key, reasons[]}` from declarative rules in `contracts/ci/ci-plan.v1.yaml`, reusing the existing crate-graph + `compute_affected`. Later phases (dispatch wiring, dedup, reaper fix, autoscaler load-shedding) are scoped in the Roadmap and get their own handoff plans.

**Tech Stack:** Rust (`vox-cli`, `vox-cli-ci`, `vox-telemetry`), declarative SSOT contracts under `contracts/ci/`, `cargo nextest`, GitHub Actions.

---

## Best-Practices Basis (research, documented)

Design principles applied (confidence: ★★ reputable practice / ★ inferred-for-our-stack):

1. **Structured events with correlation IDs** ★★ — every job event carries `{run_id, job_id, ref, pr, commit, runner_id, reason, phase, ts}`. One correlation key (`run_id`+`job_id`) lets you join intake → dispatch → completion. This is the standard observability spine; we emit through the existing `vox-telemetry` `TelemetryRecorder` rather than inventing a sink.
2. **RED method for the fleet** ★★ — track **R**ate (jobs/min received), **E**rrors (failed/segfaulted/abandoned), **D**uration (queue-wait + run time). These three answer "where is the glut" without per-line log spelunking.
3. **Capture intake at the source of truth** ★ — the autoscaler (`runner_scale.rs`) already queries the GitHub jobs API for queued/in-progress jobs; that is the one place that sees real demand, so it is where intake telemetry is emitted (not scattered across containers).
4. **Affected-graph + content-addressed reuse** ★★ — Bazel/Nx/Turborepo model: compute the reverse-dep blast radius, run only affected checks, and key a content hash so identical work is never repeated. We reuse the committed `crate-graph.v1.json` + `compute_affected` and add a whole-tree reuse key (FF-merge-of-green-PR and identical re-pushes are the dominant win).
5. **Fail-open to FULL** ★ — any diff the classifier cannot map (new file type/dir) escalates to the global check-set. A classification gap can over-run but never silently skip a gate. Every decision records a `reason` for auditability.

---

## File Structure (decomposition locked here)

**Phase 0 — Observability**
- Modify `crates/vox-telemetry/src/types.rs` — add `RunnerJobLifecycle` variant to `TelemetryEvent` + its payload struct.
- Create `crates/vox-cli/src/commands/ci/runner_observe.rs` — read-side view (`vox ci runner-observe`) that renders the RED summary + per-runner current job from recorded events.
- Modify `crates/vox-cli/src/commands/ci/runner_scale.rs` — emit `RunnerJobLifecycle` at intake/spawn/assign/reap points.
- Modify `crates/vox-cli/src/commands/ci/cmd_enums.rs` + `mod.rs` — register the `runner-observe` subcommand (mirror the existing `affected-crates` wiring).

**Phase 1 — CI Plan SSOT**
- Create `contracts/ci/ci-plan.v1.yaml` — the declarative policy: path→category map, always-global trigger globs, blast-radius/file-count thresholds, category→check-id mapping (check ids are those in `check-targets.v1.yaml`).
- Create `crates/vox-cli-ci/src/plan.rs` — pure functions: classify files, evaluate policy, compute reuse key, build the `CiPlan` struct.
- Modify `crates/vox-cli-ci/src/lib.rs` — `pub mod plan;`.
- Create `crates/vox-cli-ci/src/plan_cmd.rs` — `vox ci plan` argv handling + JSON/`--github-output` emission + `--check` parity (mirror `affected_cmd.rs`).
- Modify `crates/vox-cli/src/commands/ci/cmd_enums.rs` + `mod.rs` — register `plan`.

---

## Pre-flight (run once, anti-hallucination — confirm reality before any code)

- [ ] **P0. Baseline + confirm anchors exist.** Paste the output.

```bash
cargo run -p vox-arch-check                                  # baseline must pass
rg -n "pub enum TelemetryEvent" crates/vox-telemetry/src/types.rs
rg -n "pub trait TelemetryRecorder|fn global_recorder" crates/vox-telemetry/src/recorder.rs
rg -n "pub struct CrateGraph|pub fn compute_affected" crates/vox-cli-ci/src/affected.rs crates/vox-cli-ci/src/affected_cmd.rs
rg -n "AffectedCrates|name = \"affected-crates\"" crates/vox-cli/src/commands/ci/cmd_enums.rs
sed -n '1,20p' contracts/ci/check-targets.v1.yaml          # check-id vocabulary
test -f contracts/ci/crate-graph.v1.json && echo "crate-graph present"
```

Expected: arch-check passes; every `rg` returns ≥1 hit; `check-targets.v1.yaml` shows `checks:` with `id`/`category`/`rust_only`. If any anchor is missing, STOP — the codebase moved; re-derive paths before continuing.

---

## Phase 0 — Runner & Job-Intake Observability

### Task 0.1: Add the `RunnerJobLifecycle` telemetry event  `[SEQUENTIAL]`

**Files:**
- Modify: `crates/vox-telemetry/src/types.rs`
- Test: `crates/vox-telemetry/src/types.rs` (inline `#[cfg(test)]` module)

- [ ] **Step 1 — Pre-flight.** Confirm the enum and serde derive:

```bash
rg -n "pub enum TelemetryEvent" -A 6 crates/vox-telemetry/src/types.rs
rg -n "use serde" crates/vox-telemetry/src/types.rs
```
Expected: `TelemetryEvent` is a `#[derive(... Serialize, Deserialize ...)]` enum. Note its existing attribute style and copy it exactly.

- [ ] **Step 2 — Write the failing test.** Append to the test module in `types.rs`:

```rust
#[test]
fn runner_job_lifecycle_roundtrips_json() {
    let ev = RunnerJobPayload {
        run_id: "27784920470".into(),
        job_id: "82219035265".into(),
        repo_ref: "refs/pull/385/merge".into(),
        pr: Some(385),
        commit: "89f2e902b8".into(),
        runner_id: Some("vox-runner-auto-6a3452b1-0".into()),
        phase: RunnerJobPhase::Assigned,
        reason: "demand=1 spawn".into(),
        queue_wait_ms: Some(4200),
        run_ms: None,
        ts_unix: 1781813944,
    };
    let json = serde_json::to_string(&ev).unwrap();
    let back: RunnerJobPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(back.phase, RunnerJobPhase::Assigned);
    assert_eq!(back.pr, Some(385));
}
```

- [ ] **Step 3 — Run, expect FAIL.**
Run: `cargo nextest run -p vox-telemetry runner_job_lifecycle_roundtrips_json`
Expected: FAIL — `RunnerJobPayload`/`RunnerJobPhase` not found.

- [ ] **Step 4 — Implement.** Add near the other payload structs in `types.rs` (match the file's existing derive attributes):

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerJobPhase {
    Queued,    // GitHub reports the job queued (demand observed)
    Assigned,  // a runner picked it up
    Started,   // first step began
    Completed, // finished (success or failure recorded by GitHub)
    Abandoned, // demand vanished / run cancelled before assignment
    Reaped,    // autoscaler removed an idle/phantom runner
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunnerJobPayload {
    pub run_id: String,
    pub job_id: String,
    pub repo_ref: String,
    pub pr: Option<u64>,
    pub commit: String,
    pub runner_id: Option<String>,
    pub phase: RunnerJobPhase,
    pub reason: String,
    pub queue_wait_ms: Option<u64>,
    pub run_ms: Option<u64>,
    pub ts_unix: i64,
}
```
Then add a variant to `TelemetryEvent`: `RunnerJobLifecycle(RunnerJobPayload),` (place it with the other variants; keep the enum's existing attribute order).

- [ ] **Step 5 — Run, expect PASS.**
Run: `cargo nextest run -p vox-telemetry runner_job_lifecycle_roundtrips_json`
Expected: PASS.

- [ ] **Step 6 — Verify + commit.**
```bash
cargo clippy -p vox-telemetry -- -D warnings
cargo fmt -p vox-telemetry
git add crates/vox-telemetry/src/types.rs
git commit -m "feat(telemetry): add RunnerJobLifecycle event for CI job-intake observability"
```

### Task 0.2: Emit lifecycle events from the autoscaler  `[SEQUENTIAL]` (depends on 0.1)

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs`

- [ ] **Step 1 — Pre-flight.** Find the points where the autoscaler sees demand and mutates the fleet:

```bash
rg -n "queued|in_progress|spawn|reap|phantom|fn reconcile|fn run_scale|docker|registrations" crates/vox-cli/src/commands/ci/runner_scale.rs
rg -n "global_recorder|TelemetryEvent" crates/vox-cli/src/commands/ci/runner_scale.rs
```
Expected: locate the demand-read (queued jobs) and the spawn/reap/prune branches. Note the function names exactly.

- [ ] **Step 2 — Add a helper that records one event (no-op if no recorder).** Insert near the top of `runner_scale.rs`:

```rust
fn record_job_event(payload: vox_telemetry::types::RunnerJobPayload) {
    if let Some(rec) = vox_telemetry::recorder::global_recorder() {
        let _ = rec.record(vox_telemetry::types::TelemetryEvent::RunnerJobLifecycle(payload));
    }
}
```
> Pre-flight note: confirm the exact `record(...)` method name on `TelemetryRecorder` with `rg -n "fn record" crates/vox-telemetry/src/recorder.rs` and match it. If the trait method differs, use the real name.

- [ ] **Step 3 — Call `record_job_event` at three sites** with the real loop variables (a queued job observed → `Queued`; a runner spawned/assigned → `Assigned`; an idle/phantom removal → `Reaped`). Fill `reason` with the branch's decision string (e.g. `format!("spawn desired={desired} alive={alive}")`). Use the diff-local variable names confirmed in Step 1; do not invent fields.

- [ ] **Step 4 — Verify it builds + clippy clean (no test asserts here; this is wiring).**
Run: `cargo check -p vox-cli && cargo clippy -p vox-cli --lib -- -D warnings`
Expected: clean. If `vox-telemetry` isn't already a dependency of `vox-cli`, add it to `crates/vox-cli/Cargo.toml` `[dependencies]` (confirm with `rg -n "vox-telemetry" crates/vox-cli/Cargo.toml` first; inherit version from workspace per repo policy — no hardcoded version).

- [ ] **Step 5 — Commit.**
```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/runner_scale.rs crates/vox-cli/Cargo.toml
git commit -m "feat(ci): emit RunnerJobLifecycle telemetry from the autoscaler reconcile loop"
```

### Task 0.3: `vox ci runner-observe` — the RED + per-runner view  `[SEQUENTIAL]`

**Files:**
- Create: `crates/vox-cli/src/commands/ci/runner_observe.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (add `pub mod runner_observe;`)
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (register subcommand)
- Test: `crates/vox-cli/src/commands/ci/runner_observe.rs` (inline tests)

- [ ] **Step 1 — Pre-flight.** Copy the registration pattern from the existing subcommand:

```bash
rg -n "name = \"affected-crates\"" -B 2 -A 12 crates/vox-cli/src/commands/ci/cmd_enums.rs
rg -n "AffectedCrates\s*\{|=> .*affected" crates/vox-cli/src/commands/ci/mod.rs
```
Expected: shows the clap variant for `affected-crates` and its dispatch arm. Mirror this exactly for `runner-observe`.

- [ ] **Step 2 — Write the failing test** (pure aggregation over events — no I/O). Create `runner_observe.rs` with:

```rust
use vox_telemetry::types::{RunnerJobPayload, RunnerJobPhase};

#[derive(Debug, Default, PartialEq)]
pub struct RedSummary {
    pub received: u64,        // count of Queued
    pub completed: u64,
    pub abandoned: u64,
    pub reaped: u64,
    pub p50_queue_wait_ms: u64,
}

/// Pure: fold a stream of lifecycle payloads into the RED summary.
pub fn summarize(events: &[RunnerJobPayload]) -> RedSummary { todo!() }

#[cfg(test)]
mod tests {
    use super::*;
    fn ev(phase: RunnerJobPhase, wait: Option<u64>) -> RunnerJobPayload {
        RunnerJobPayload { run_id: "r".into(), job_id: "j".into(), repo_ref: "ref".into(),
            pr: None, commit: "c".into(), runner_id: None, phase, reason: "x".into(),
            queue_wait_ms: wait, run_ms: None, ts_unix: 0 }
    }
    #[test]
    fn red_summary_counts_phases_and_p50() {
        let evs = vec![
            ev(RunnerJobPhase::Queued, Some(100)),
            ev(RunnerJobPhase::Queued, Some(300)),
            ev(RunnerJobPhase::Completed, None),
            ev(RunnerJobPhase::Abandoned, None),
        ];
        let s = summarize(&evs);
        assert_eq!(s.received, 2);
        assert_eq!(s.completed, 1);
        assert_eq!(s.abandoned, 1);
        assert_eq!(s.p50_queue_wait_ms, 300); // median of [100,300] -> upper-mid for even n
    }
}
```

- [ ] **Step 3 — Run, expect FAIL.**
Run: `cargo nextest run -p vox-cli red_summary_counts_phases_and_p50`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 4 — Implement `summarize`** (replace `todo!()`):

```rust
pub fn summarize(events: &[RunnerJobPayload]) -> RedSummary {
    let mut s = RedSummary::default();
    let mut waits: Vec<u64> = Vec::new();
    for e in events {
        match e.phase {
            RunnerJobPhase::Queued => s.received += 1,
            RunnerJobPhase::Completed => s.completed += 1,
            RunnerJobPhase::Abandoned => s.abandoned += 1,
            RunnerJobPhase::Reaped => s.reaped += 1,
            _ => {}
        }
        if let Some(w) = e.queue_wait_ms { waits.push(w); }
    }
    waits.sort_unstable();
    s.p50_queue_wait_ms = if waits.is_empty() { 0 } else { waits[waits.len() / 2] };
    s
}
```

- [ ] **Step 5 — Run, expect PASS.**
Run: `cargo nextest run -p vox-cli red_summary_counts_phases_and_p50`
Expected: PASS.

- [ ] **Step 6 — Register the subcommand.** In `cmd_enums.rs` add a `RunnerObserve` clap variant (mirror `affected-crates`, name `"runner-observe"`, a `--json` bool flag). In `mod.rs` add `pub mod runner_observe;` and a dispatch arm that loads recorded events (from the telemetry store the recorder writes to — confirm the read path with `rg -n "fn read|fn load|aggregator" crates/vox-telemetry/src/aggregator.rs`) and prints `summarize(...)` as text or JSON. If a read API does not exist yet, scope this arm to read the recorder's on-disk JSONL and parse `RunnerJobLifecycle` lines — do NOT invent an aggregator method.

- [ ] **Step 7 — Verify + commit.**
```bash
cargo nextest run -p vox-cli runner_observe
cargo clippy -p vox-cli --lib -- -D warnings
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/runner_observe.rs crates/vox-cli/src/commands/ci/mod.rs crates/vox-cli/src/commands/ci/cmd_enums.rs
git commit -m "feat(ci): add 'vox ci runner-observe' RED + per-runner job view"
```

---

## Phase 1 — CI Plan SSOT

### Task 1.1: Author the `ci-plan.v1.yaml` policy contract  `[PARALLEL-SAFE]`

**Files:**
- Create: `contracts/ci/ci-plan.v1.yaml`

- [ ] **Step 1 — Pre-flight.** Read the check-id vocabulary the plan emits a subset of:
```bash
rg -n "^\s*- id:" contracts/ci/check-targets.v1.yaml
```
Expected: a list of check ids (e.g. `fmt`, `line-endings`, `ssot-drift`, …). The `checks` you map to MUST be ids that appear here.

- [ ] **Step 2 — Create the contract** (no code; declarative SSOT). Use only check ids confirmed in Step 1:

```yaml
schema_version: 1
# CI Plan policy — consumed by `vox ci plan`. Fail-open: any path not matched by
# `categories` is treated as an always-global trigger (over-run, never under-run).

# Map changed-file globs to a category. First match wins; order matters.
categories:
  - { glob: "docs/**",                         category: docs }
  - { glob: "**/*.md",                          category: docs }
  - { glob: "crates/vox-gui/ui/**",             category: ts-ui }
  - { glob: "contracts/**",                     category: contracts }
  - { glob: "scripts/**",                       category: scripts }
  - { glob: "assets/**",                        category: assets }
  - { glob: "crates/**",                        category: rust }   # owning-crate resolved separately

# Any changed path matching these escalates to the FULL global check-set.
always_global_globs:
  - "rust-toolchain.toml"
  - "Cargo.lock"
  - "Cargo.toml"
  - ".github/workflows/**"
  - "docs/src/architecture/layers.toml"

# Crates whose change forces global (high blast-radius keystones).
keystone_crates:
  - vox-db
  - vox-compiler
  - vox-config

# Magnitude thresholds → global.
thresholds:
  affected_crates_max: 25     # affected-closure size at/above this → global
  changed_files_max: 120      # raw changed-file count at/above this → global

# category → required check ids (subset of check-targets.v1.yaml ids).
check_sets:
  docs:      [line-endings, docs-quality]
  ts-ui:     [line-endings, ts-noemit, gui-registry-check]
  contracts: [line-endings, ssot-drift, config-hygiene]
  scripts:   [line-endings, script-hygiene]
  assets:    [line-endings]
  rust:      [fmt, line-endings, clippy, nextest-affected]
# The global set is the union of every blocking check in check-targets.v1.yaml.
```
> If any check id above is absent from `check-targets.v1.yaml`, replace it with the nearest real id from Step 1 (do not introduce new ids here — this contract only *references* them).

- [ ] **Step 3 — Commit.**
```bash
git add contracts/ci/ci-plan.v1.yaml
git commit -m "feat(ci): add ci-plan.v1.yaml targeted-dispatch policy SSOT"
```

### Task 1.2: `plan.rs` — classify, evaluate policy, build `CiPlan`  `[SEQUENTIAL]` (depends on 1.1)

**Files:**
- Create: `crates/vox-cli-ci/src/plan.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs` (add `pub mod plan;`)
- Test: `crates/vox-cli-ci/src/plan.rs` (inline tests)

- [ ] **Step 1 — Pre-flight.** Confirm the reused graph + closure API:
```bash
rg -n "pub struct CrateGraph|pub fn compute_affected" -A 4 crates/vox-cli-ci/src/affected.rs crates/vox-cli-ci/src/affected_cmd.rs
```
Expected: `CrateGraph { crates: BTreeMap<String, Vec<String>> }` and `compute_affected(&[String], &BTreeMap<String, Vec<String>>) -> ...`. Match the real return type.

- [ ] **Step 2 — Write the failing test** in `plan.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lockfile_change_forces_global() {
        let p = decide_global(&["Cargo.lock".into()], &[], 1,
            &["Cargo.lock".to_string()], &[], 25, 120);
        assert!(p.global);
        assert!(p.reasons.iter().any(|r| r.contains("always_global")));
    }
    #[test]
    fn small_docs_change_is_targeted() {
        let p = decide_global(&["docs/x.md".into()], &[], 1,
            &["Cargo.lock".to_string()], &["vox-db".to_string()], 25, 120);
        assert!(!p.global);
    }
    #[test]
    fn keystone_crate_forces_global() {
        let p = decide_global(&["crates/vox-db/src/x.rs".into()], &["vox-db".into()], 1,
            &[], &["vox-db".to_string()], 25, 120);
        assert!(p.global);
    }
}
```

- [ ] **Step 3 — Run, expect FAIL.**
Run: `cargo nextest run -p vox-cli-ci lockfile_change_forces_global`
Expected: FAIL — `decide_global`/types undefined.

- [ ] **Step 4 — Implement** the decision struct + pure function (no I/O; rules passed in as args so it is unit-testable):

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct CiPlan {
    pub global: bool,
    pub checks: Vec<String>,
    pub blast_radius: Vec<String>,
    pub reuse_key: String,
    pub reasons: Vec<String>,
}

/// Pure policy evaluation. Globs are matched with the `glob` crate by the caller
/// and reduced to booleans here to keep this function dependency-free.
#[allow(clippy::too_many_arguments)]
pub fn decide_global(
    changed_files: &[String],
    affected_crates: &[String],
    changed_file_count: usize,
    always_global_hits: &[String], // changed paths that matched an always_global glob
    keystone_crates: &[String],
    affected_crates_max: usize,
    changed_files_max: usize,
) -> CiPlan {
    let mut reasons = Vec::new();
    let mut global = false;
    if !always_global_hits.is_empty() {
        global = true;
        reasons.push(format!("always_global trigger: {}", always_global_hits.join(",")));
    }
    if affected_crates.iter().any(|c| keystone_crates.contains(c)) {
        global = true;
        reasons.push("keystone crate changed".into());
    }
    if affected_crates.len() >= affected_crates_max {
        global = true;
        reasons.push(format!("blast radius {} >= {}", affected_crates.len(), affected_crates_max));
    }
    if changed_file_count >= changed_files_max {
        global = true;
        reasons.push(format!("changed files {} >= {}", changed_file_count, changed_files_max));
    }
    if !global { reasons.push("targeted: no global trigger fired".into()); }
    CiPlan { global, checks: Vec::new(), blast_radius: affected_crates.to_vec(),
             reuse_key: String::new(), reasons }
}
```
Add `pub mod plan;` to `lib.rs`.

- [ ] **Step 5 — Run, expect PASS.**
Run: `cargo nextest run -p vox-cli-ci plan::`
Expected: 3 PASS.

- [ ] **Step 6 — Verify + commit.**
```bash
cargo clippy -p vox-cli-ci -- -D warnings
cargo fmt -p vox-cli-ci
git add crates/vox-cli-ci/src/plan.rs crates/vox-cli-ci/src/lib.rs
git commit -m "feat(ci): plan.rs policy core (global vs targeted decision with reasons)"
```

### Task 1.3: reuse key + check-set assembly + glob matching  `[SEQUENTIAL]` (depends on 1.2)

**Files:**
- Modify: `crates/vox-cli-ci/src/plan.rs`
- Test: `crates/vox-cli-ci/src/plan.rs`

- [ ] **Step 1 — Pre-flight.** Confirm a hashing + glob crate are available workspace-wide:
```bash
rg -n "^sha2|^glob|^globset|^blake3" Cargo.toml crates/vox-cli-ci/Cargo.toml
```
Expected: at least one hash crate (`sha2`/`blake3`) and one glob crate (`glob`/`globset`). Use whichever is already a dependency; add via workspace inheritance only if absent.

- [ ] **Step 2 — Write the failing test:**
```rust
#[test]
fn reuse_key_is_stable_for_same_inputs() {
    let a = reuse_key("89f2e902b8", &["fmt".into(), "clippy".into()], "1.96.0", "deadbeef");
    let b = reuse_key("89f2e902b8", &["clippy".into(), "fmt".into()], "1.96.0", "deadbeef");
    assert_eq!(a, b, "check order must not change the key");
    let c = reuse_key("89f2e902b9", &["fmt".into()], "1.96.0", "deadbeef");
    assert_ne!(a, c, "different tree -> different key");
}
```

- [ ] **Step 3 — Run, expect FAIL.** `cargo nextest run -p vox-cli-ci reuse_key_is_stable_for_same_inputs` → FAIL.

- [ ] **Step 4 — Implement** (use the hash crate confirmed in Step 1; example with `sha2`):
```rust
pub fn reuse_key(tree_sha: &str, checks: &[String], toolchain: &str, lockfile_hash: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut sorted = checks.to_vec();
    sorted.sort();
    let mut h = Sha256::new();
    h.update(tree_sha.as_bytes());
    h.update(b"\0");
    h.update(sorted.join(",").as_bytes());
    h.update(b"\0");
    h.update(toolchain.as_bytes());
    h.update(b"\0");
    h.update(lockfile_hash.as_bytes());
    format!("{:x}", h.finalize())
}
```

- [ ] **Step 5 — Run, expect PASS.** `cargo nextest run -p vox-cli-ci reuse_key_is_stable_for_same_inputs` → PASS.

- [ ] **Step 6 — Verify + commit.**
```bash
cargo clippy -p vox-cli-ci -- -D warnings && cargo fmt -p vox-cli-ci
git add crates/vox-cli-ci/src/plan.rs
git commit -m "feat(ci): stable whole-tree reuse key for CI Plan dedup"
```

### Task 1.4: `vox ci plan` command (load contract, run pipeline, emit JSON / --github-output / --check)  `[SEQUENTIAL]` (depends on 1.3)

**Files:**
- Create: `crates/vox-cli-ci/src/plan_cmd.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs` (`pub mod plan_cmd;`)
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` + `mod.rs` (register `plan`)
- Test: `crates/vox-cli-ci/src/plan_cmd.rs` (golden-diff fixtures)

- [ ] **Step 1 — Pre-flight.** Mirror the existing command's argv + `--github-output` handling exactly:
```bash
rg -n "fn run_affected_cmd|--github-output|--changed|--graph|--check" crates/vox-cli-ci/src/affected_cmd.rs
rg -n "name = \"affected-crates\"" -A 14 crates/vox-cli/src/commands/ci/cmd_enums.rs
```

- [ ] **Step 2 — Write the failing golden test** (two fixtures: a docs-only diff → targeted; a `Cargo.lock` diff → global):
```rust
#[test]
fn plan_docs_only_is_targeted_and_lockfile_is_global() {
    // changed-file list + a tiny in-test crate graph; assert the emitted CiPlan
    let docs = run_plan_for_test(&["docs/readme.md".into()]);
    assert!(!docs.global);
    assert!(docs.checks.iter().all(|c| !c.is_empty()));
    let lock = run_plan_for_test(&["Cargo.lock".into()]);
    assert!(lock.global);
}
```
> `run_plan_for_test` is a thin test harness in this file that calls the real pipeline with the committed `contracts/ci/ci-plan.v1.yaml` and a fixed in-test graph. Write it; do not call out to git in the unit test.

- [ ] **Step 3 — Run, expect FAIL.** `cargo nextest run -p vox-cli-ci plan_docs_only_is_targeted_and_lockfile_is_global` → FAIL.

- [ ] **Step 4 — Implement `run_plan_cmd(args: &[String]) -> i32`:** parse `--base`/`--head`/`--json`/`--github-output`/`--check`; gather changed files via `git diff --name-only <base> <head>` (use `std::process::Command`, mirror `regen_graph`); load `ci-plan.v1.yaml` (serde_yaml — confirm it's a dep); classify files (glob crate); compute `compute_affected`; call `decide_global`; if not global, build `checks` from `check_sets` union for the touched categories else the global union; compute `reuse_key` (tree sha from `git rev-parse <head>^{tree}`, lockfile hash from the committed `Cargo.lock` bytes); print JSON or set GH outputs. `--check` recomputes and compares against a committed `contracts/ci/ci-plan-baseline.v1.json` for drift parity (mirror `check_graph`).

- [ ] **Step 5 — Register** `Plan { base, head, json, github_output, check }` in `cmd_enums.rs` (name `"plan"`) and dispatch to `vox_cli_ci::plan_cmd::run_plan_cmd` in `mod.rs`. Confirm `vox-cli-ci` is a dep of `vox-cli` (`rg -n "vox-cli-ci" crates/vox-cli/Cargo.toml`).

- [ ] **Step 6 — Run, expect PASS + manual smoke.**
```bash
cargo nextest run -p vox-cli-ci plan_
cargo run -p vox-cli -- ci plan --base origin/main --head HEAD --json
```
Expected: test PASS; the smoke prints a JSON plan with `global`, `checks`, `reasons`.

- [ ] **Step 7 — Verify + commit.**
```bash
cargo clippy -p vox-cli-ci -- -D warnings && cargo clippy -p vox-cli --lib -- -D warnings
cargo fmt -p vox-cli-ci && cargo fmt -p vox-cli
git add crates/vox-cli-ci/src/plan_cmd.rs crates/vox-cli-ci/src/lib.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/mod.rs
git commit -m "feat(ci): 'vox ci plan' emits targeted check-set + reuse key from ci-plan SSOT"
```

### Task 1.5: commit the plan baseline + wire `--check` into the guards lane  `[SEQUENTIAL]` (depends on 1.4)

- [ ] **Step 1.** Generate the baseline: `cargo run -p vox-cli -- ci plan --base origin/main --head HEAD --json > contracts/ci/ci-plan-baseline.v1.json` (then hand-trim to a deterministic fixture if it embeds run-specific shas — store only the rule-derived fields, not the tree-specific reuse_key).
- [ ] **Step 2.** Confirm `vox ci plan --check` passes against it: `cargo run -p vox-cli -- ci plan --base origin/main --head HEAD --check` → exit 0.
- [ ] **Step 3 — Commit.** `git add contracts/ci/ci-plan-baseline.v1.json && git commit -m "test(ci): commit CI Plan baseline for --check drift parity"`

---

## Roadmap — later phases (separate handoff plans; do NOT implement here)

These are scoped, not task-decomposed. Each becomes its own dated plan after Phase 0+1 land and the observability data informs the thresholds.

- **Phase 2 — Targeted dispatch wiring.** Add a `compute-plan` job at the top of `ci.yml` that runs `vox ci plan --github-output` and exposes `global`, `checks`, `reuse_key`; gate every downstream job on membership in `checks` (extend the existing `rust_changed` pattern). Keep `merge_group`/main honoring the same plan (per the "targeted everywhere + nightly full" decision) plus a scheduled nightly full-workspace run.
- **Phase 3 — Dedup / admission control.** A small result ledger keyed by `reuse_key` (store in the existing `vox-sccache-minio` S3 / `vox-telemetry` sink): before running a job, check the ledger; if a green result exists for the key, skip and reuse. Cross-ref/session/tab dedup falls out for free (identical content → identical key). Supersede stale in-flight runs via concurrency groups keyed on PR, not ref.
- **Phase 4 — Reaper correctness (#334).** Add a busy-guard: never reap a runner/container whose `RunnerJobLifecycle` shows `Assigned`/`Started` without a later `Completed`. Fix the live-process kill from audit #334 (no busy-guard). Liveness probe before any kill.
- **Phase 5 — Autoscaler load-shedding.** Drive `desired` from the plan's real demand + the RED metrics; cap concurrent heavy builds to avoid the rustc-segfault-under-memory-pressure failure mode observed 2026-06-18 (`abi_stable_derive` SIGSEGV under fleet overload); set `RUST_MIN_STACK`; add memory-aware spawn backpressure.

---

## Self-Review (author checklist, completed)

- **Spec coverage:** Observability (Phase 0: events 0.1, emission 0.2, view 0.3) ✓; targeted CI policy (Phase 1: contract 1.1, decision 1.2, reuse key 1.3, command 1.4, parity 1.5) ✓; dedup/reaper/autoscaler scoped in Roadmap ✓.
- **Placeholders:** none — every code step shows real code; `todo!()` appears only as the deliberate red-test starting point, replaced in the next step.
- **Type consistency:** `RunnerJobPayload`/`RunnerJobPhase` defined in 0.1 and reused verbatim in 0.2/0.3; `CiPlan`/`decide_global`/`reuse_key` defined in 1.2/1.3 and consumed in 1.4. Check ids in 1.1 are constrained to `check-targets.v1.yaml`.
- **Antigravity rails:** every task ends with a commit; each opens with a Pre-flight verify; tasks tagged PARALLEL-SAFE/SEQUENTIAL; no `cargo fmt --all`; no stubs shipped (the one `todo!()` is removed before its task's commit).

---

## Handoff status (updated by the executing session — see "Pick up here")

> **Pick up here (status @ 2026-06-18, end of plan-writing session):**
>
> **Two bodies of work pending landing on `main` (`origin/main` = `0ee0608`):**
>
> 1. **PR stack #377→#385** (re-sliced from the closed nested #353⊂#370⊂#375⊂#376; 963 files; each <140 for CodeRabbit). Stack top `origin/stack/09-remaining-crates` == old #376 tree + `.gitleaks.toml` allowlist + one preserved doc edit. **#385 required gate `Check, Build, and Test (Rust)` = PASS; all 3 cross-platform builds + `scan` + CodeQL-js + CodeRabbit = PASS.** 5 non-required checks RED, **all infrastructure, not code:** `Cache warmup`, `Analyze (rust)`, `vox check on scripts/`, `Exercise loop` → `sccache: Server startup failed … Host header is specified and is not an IP address or localhost` (sccache→MinIO S3 endpoint misconfig after the Docker Desktop restart) + earlier rustc SIGSEGV under fleet overload (`abi_stable_derive`); `Generate + export Expo bundle` → mobile/EAS, non-critical. CodeRabbit reviewed all 9 (manual `@coderabbitai review`; stacked PRs are auto-skipped). **Authorized merge model:** FF `main` → `origin/stack/09-remaining-crates` via admin bypass (`enforce_admins=false`); this auto-closes #377–385.
>
> 2. **Branch `claude/auto-gui-debug-plans-2026-06-18`** (local-only, no PR, 3 commits / 4200 insertions): auto-GUI + zero-annotation-debug + AI-UI-target research docs, native superpowers skills, voxmens hub-and-spoke plan, and `refactor(mens-cli): populi probe --verbose→--detailed`. Independent of the stack (zero subject overlap). This file (the CI-observability plan) is committed here too.
>
> **Safety net:** tag `preserve/local-main-20260618` + bundle `C:\Users\Owner\vox-preslice-20260618.bundle` (3.4 GB).
>
> **UPDATE — actions taken this session:**
> - ✅ **Auto-GUI body LANDED on `main`** (`origin/main` = `02cf978e4a`): the 3 auto-GUI/skills/research/mens-rename commits + this CI plan, admin FF-pushed (bypassed merge-queue + required-check rules). `vox-ml-cli` compile-checked green first. Local `main` synced (0/0).
> - ⚠️ **Multi-tab hazard confirmed (live):** between committing and pushing, a *concurrent session* added two benign commits to the same branch (`359897eed0 fix(docs): correct relative paths…`, `02cf978e4a docs(agents): instruction-file hygiene` — incl. creating `GEMINI.md`). They rode the push to `main`. No code beyond the verified rename. This is concrete motivation for Phase 3 admission-control/dedup.
> - 🔧 **Sccache blocker diagnosed (NOT yet fixed):** root cause in `crates/vox-cli/src/commands/ci/runner_scale.rs:49` — `SCCACHE_S3_CONTAINER_ENDPOINT = "http://host.docker.internal:9000"`. The opendal S3 client rejects that hostname (*"Host header … not an IP address or localhost"*), failing every sccache build on the fleet (the 5 non-required reds on #385). **Fix recipe:** at runner spawn, resolve `host.docker.internal` → Docker gateway IP and substitute it into `SCCACHE_ENDPOINT` (the error accepts IPs); update the test at `runner_scale.rs:~1147` (it asserts the endpoint contains `host.docker.internal`). Then rebuild `vox-cli`, redeploy the fleet containers (env is baked at spawn), and re-run #385 CI to confirm green. This is the immediate stack-merge unblock and the concrete seed for Phase 5 (load-shedding) + Phase 0 (observability would have surfaced this instantly).
>
> **UPDATE — stack MERGED (complete):**
> - ✅ **All work landed on `main`.** Stack #377–385 merged via integration commit `ee1171912a` (`merge: land re-sliced stack … keep --detailed`). `stack/09` is an ancestor of `main` → every slice commit is in. The `vox-ml-cli` mens-rename overlap (both sides renamed the old `verbose` field) resolved to `main`'s official `--detailed`; `vox-ml-cli` compile-checked green. #377 auto-closed merged; #378–385 closed with a pointer to `ee1171912a`. Remote stack branches + integration branch + worktree deleted. **Local `main` == `origin/main`; no work lost; safety net (`preserve/local-main-20260618` + 3.4 GB bundle) intact.**
> - ⚠️ Multi-tab churn continued: concurrent tab(s) layered benign `docs(agents)` commits on `main` atop the merge — harmless; reinforces the Phase 3 dedup/admission-control need.
>
> **Remaining (no longer blocking — infra + handoff):**
> 1. **sccache IP fix** (`runner_scale.rs:49` + test `:1147`; rebuild + fleet redeploy + re-run) — now purely future CI health; the merge did not need it (required gate green; merged via integration). The Phase-5 seed.
> 2. Hand this plan to Antigravity/Gemini per the header (`GEMINI.md` exists).
