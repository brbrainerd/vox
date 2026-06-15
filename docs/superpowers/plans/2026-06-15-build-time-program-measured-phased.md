# Build-Time Reduction Program (Measured, Phased) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut Vox workspace build times — and *prove it* — by standing up a reproducible build-time measurement harness first, then landing cycle/coupling guards and inter-crate blast-radius reductions, each phase reporting its own measured before/after delta against a committed baseline, ending with affected-crate selective CI for the biggest PR-time win.

**Architecture:** A new `vox ci build-bench` subcommand runs a pinned set of wall-clock build scenarios, writes a JSON snapshot, and `--compare`s against a committed baseline to emit a phase-delta report. A new `vox ci dep-cycles` subcommand (Tarjan SCC over `cargo metadata`) inventories cyclical and back-edge coupling that `vox-arch-check` cannot see today. With those two instruments in place, the optimization phases (re-landing four un-merged feature-gating/leaf-move wins, then a slowest-unit split, then selective CI) each end by running the harness and recording the measured delta. The merge-queue full build remains the soundness backstop throughout.

**Tech Stack:** Rust (`vox-cli` `ci` subcommand surface, `serde_json`, `std::process::Command` shelling `cargo`), `cargo metadata`, `cargo check`/`cargo build --timings`, VoxScript (`.vox`) for the dependency-map refresh, GitHub Actions (`.github/workflows/ci.yml`).

---

## Scope note (read first)

This plan deliberately unifies three areas the requester named together — **selective CI**, **cyclical dependencies**, and **inter-crate dependencies** — under one measured spine. It is large (six phases). Each phase is independently shippable and individually revertable, so it can be executed and landed incrementally. **Phase 5 (selective CI) is a thin wrapper that executes the already-written [`2026-06-15-affected-crate-selective-ci.md`](2026-06-15-affected-crate-selective-ci.md) plan and measures its effect — it is not re-specified here.** If you would rather run these as separate efforts, the natural split is: (A) Phases 0–1 (instruments), (B) Phases 2–4 (local build-time wins), (C) Phase 5 (the selective-CI sub-plan). They share only the harness from Phase 0.

---

## Verified findings this plan is built on (do not re-derive; confirm if surprised)

Two research passes established the following against the working tree. The executor should confirm any that a step depends on, but these are the load-bearing facts:

| Finding | Evidence | Consequence |
|---|---|---|
| **`vox-arch-check` has NO cycle detection.** Rule 1 is a single pairwise `to_layer > from_layer` comparison on *normal* deps only (`crates/vox-arch-check/src/main.rs:1008-1021`); no SCC/DFS/topo anywhere; dev-deps skipped (`main.rs:1001`). | grep for `scc\|tarjan\|topolog\|cycle\|dfs` in `main.rs` → zero hits. | Cycle detection must be **newly built** (Phase 1), not extended. Same-layer A→B→A cycles and dev-dep back-edges are invisible today. |
| **No un-tolerated layer inversions exist.** The only genuine lower→higher edge is `vox-ml-cli`(L3)→`vox-cli`(L5), whitelisted in `[[known_inversions]]` (`docs/src/architecture/layers.toml:235-238`). | layers.toml read. | Phase 1's value is an **inventory + guard against regressions**, plus surfacing dev-dep back-edges — not removing an existing runtime cycle. |
| **The "vox-db→compiler keystone" is already resolved.** `crates/vox-db/Cargo.toml` depends on vox-compiler/vox-codegen **only under `[dev-dependencies]`** (`Cargo.toml:41-49`); both are L3 (same layer as vox-db). | vox-db Cargo.toml read. | Do not "break" this edge — it is dev-only and same-layer. Phase 1 reports it as a back-edge in the inventory; it is not a blocker. |
| **Four of five prior build-time optimizations are NOT in the tree.** Only `vox-orchestrator-mcp/src/llm_bridge/` (extraction) landed. vox-audit has no `[features]`; mcp default still includes `news-publish` (`Cargo.toml:9`); vox-sql hard-codes `postgres`+`mysql` (`Cargo.toml:17`); `retrieval.rs` is still in `crates/vox-db/src/` (`vox-db/src/lib.rs:132`), not moved to vox-db-types. | All four Cargo.tomls/files read. | Phase 2 **re-lands** these four as real work, each measured. The session task list marking them "completed" reflects a different, unmerged branch — ignore it. |
| **The measurement tool `crate-build-audit.vox` is not on main.** It lives only on unmerged `claude/crate-build-audit-tool` @ `d2759f3c58`. Its output artifacts (`graphify-out/crate_audit.json`, `CRATE_BUILD_AUDIT.md`) are present, dated. | `git show d2759f3c58:scripts/crate-build-audit.vox`. | Phase 0 **re-lands** that `.vox` map tool (cherry-pick), as the blast-radius/fan-in substrate. |
| **`vox ci build-timings` (default path) already measures wall-clock `cargo check` lanes.** `crates/vox-cli/src/commands/ci/run_body_helpers/timings.rs:112` (`run_build_timings`); `TimingRecord { lane, ok, duration_ms, error }` (`timings.rs:10-17`); NDJSON on `--json`; reads soft budgets from `docs/ci/build-timings/budgets.json`; **no file output, no compare/delta mode**; one test module `build_timing_budget_tests` (`timings.rs:238`). | timings.rs read. | The new `build-bench` borrows its lane-runner idea but is a **separate** subcommand so its snapshot/compare semantics don't perturb the budgeted CI lanes. |
| **A reusable cargo-metadata graph parser exists.** `compute_dependency_shape_summary` (`crates/vox-cli/src/commands/ci/build_timings.rs:99-186`) shells `cargo metadata --format-version 1`, builds `id_to_name`, and restricts to internal workspace edges. `crates/vox-cli-ci/src/dep_sprawl.rs:21-90` has a pure `violations_from_packages` + its own `cargo_bin()`. | both read. | Phase 1's adjacency builder mirrors this parsing; do not invent a new metadata format. |
| **Subcommand registration pattern.** No-arg: `CiCmd::CudaFeatures` (`cmd_enums.rs:431-433`) → `CiCmd::CudaFeatures => run_cuda_features()` (`run_body.rs:299`). One-flag: `CiCmd::RunnerScale { apply }` (`cmd_enums.rs:694-700`) → `=> super::runner_scale::run_scale(apply)` (`run_body.rs:513`). Dispatch `match` is `async`; sync handlers return `Result` directly. Helpers in `run_body_helpers/` are declared in `run_body_helpers/mod.rs`. `cargo_bin()` is `pub(super)` in `ci/mod.rs:88`. | all read. | Mirror exactly. |

---

## Handoff guardrails for the executor (Claude Sonnet 4.6) — non-negotiable

This plan is handed to Sonnet 4.6, which (relative to the author) is more prone to editing from memory, fabricating API signatures, and declaring success without running the verification. These are correctness guardrails, not polish:

- **ALWAYS `Read` the exact file before editing.** Line numbers in this plan are approximate and the tree drifts. Re-locate every edit by searching for the quoted **anchor string**, never by jumping to a line number.
- **Do NOT invent API signatures.** Before calling any `vox_*`/`cargo`/stdlib function, `Grep` its definition and `Read` it. If this plan references something that does not exist as written, **STOP and report** — do not fabricate a plausible substitute.
- **Run EVERY verification command and paste the actual output.** Never claim a test or measurement passed without running it. If a build/test fails, fix the **root cause** — do not `#[ignore]`, `#[allow]`, comment out, or weaken an assertion.
- **A measurement is data, not a wish.** When a task says "record the delta," you must actually run `vox ci build-bench` and paste the real numbers. **If a build-time number gets *worse* or is flat, report it honestly** — do not invent an improvement. A regression is a finding, not a failure to hide.
- **One task = one commit** with the exact message given. Do not batch tasks.
- **Never `cargo fmt --all`** (overflows the Windows arg limit → `os error 206`; use `cargo fmt -p <crate>` or `vox run scripts/fmt.vox`). **Never `--no-verify`. Never push to `main` directly.** All CI/workflow changes land via PR — the merge-queue full build is the soundness backstop.
- **Format/lint/test commands for this plan:**
  - Single crate test: `cargo test -p vox-cli build_bench:: -- --nocapture` (builds a test harness, not `vox.exe`; avoids the Windows binary-lock).
  - Format: `cargo fmt -p vox-cli`. Clippy: `cargo clippy -p vox-cli -- -D warnings`.
  - Commit trailer:
    ```
    Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
    ```

### A note on running measurements as a cloud agent

The bench scenarios are **real `cargo check`/`cargo build` invocations** and take minutes when the `target/` cache is cold. If your environment cannot complete a multi-minute build, you have two sanctioned options, in order of preference:
1. **Run the bench in CI.** Phase 0 Task 0.5 wires a non-blocking `build-bench` CI job that uploads the snapshot + report as an artifact. Trigger it on your PR and read the artifact for the numbers.
2. **Run it locally with a warmed cache.** Do one `cargo check --workspace` first (warm the cache), then run the bench — `cargo check` lanes complete in seconds-to-low-minutes warm.

Do NOT skip the measurement and write a plausible number. If you genuinely cannot measure, say so in the task report and leave the delta cell as `PENDING-CI` for the CI run to fill.

---

## Execution methodology (Workflows + subagent-driven + TDD)

This plan is designed to be executed three ways that compose. Use them as follows.

### Test-Driven Development (the default for every code task)

Every Rust task in Phases 0–1 is already structured as **write the failing test → run it to confirm it fails → implement the minimum → run it to confirm it passes → format/lint → commit**. Hold that discipline; do not write implementation before its test. The pure functions (`compute_deltas`, `format_delta_markdown`, `cycles`, `adjacency_from_metadata`) are unit-tested with zero I/O — they are the correctness core and must stay green. Side-effecting code (lane runner, cargo-metadata shell) is verified by the task's smoke step, not by a unit test; do not fake a unit test around a `cargo` shell-out.

### Subagent-driven development (the recommended execution mode)

Run this plan under **`superpowers:subagent-driven-development`**: dispatch a **fresh subagent per task**, then a **two-stage review** (the implementing subagent reports; a second reviewer subagent — or you — checks the diff against the task's acceptance before the commit is accepted). This matters here because the executor (Sonnet 4.6) is prone to editing from memory; a per-task fresh context plus an independent review catches fabricated signatures and skipped verifications. Keep each subagent's scope to exactly one task and one commit.

### Workflows (parallelize the independent work; serialize the measurements)

The **Workflow tool** is the right instrument for the phases that contain genuinely independent units, but build-time measurement has a hard constraint that shapes how you use it:

> **Measurements MUST be serial on a quiet machine.** Two `cargo` builds running at once contend for CPU, disk, and the shared `target/` directory — their wall-clock numbers are mutually corrupted and the `target/` state races. **Never** run two bench/build scenarios concurrently. Parallelize *code edits* (in isolated worktrees), then run *all measurements serially* afterward.

Concrete patterns:

- **Phase 2 (four independent re-lands) — parallel edit, serial measure.** The four optimizations touch disjoint crates (`vox-db`/`vox-db-types`, `vox-sql`, `vox-orchestrator-mcp`, `vox-audit`) with no ordering dependency. Fan out the *implementation* with `agent(..., { isolation: 'worktree' })` so each lands its code+unit-build in its own worktree, then **merge them, and run `vox ci build-bench` once per change serially** on the integration branch to get clean deltas. Sketch:
  ```js
  // Phase 2: parallel implement in worktrees, then SERIAL measurement.
  const recipes = [
    { id: '2.1', label: 'retrieval->leaf',         crate: 'vox-db' },
    { id: '2.2', label: 'vox-sql backend gating',  crate: 'vox-sql' },
    { id: '2.3', label: 'mcp drop news-publish',   crate: 'vox-orchestrator-mcp' },
    { id: '2.4', label: 'vox-audit ci-gates',      crate: 'vox-audit' },
  ]
  // Stage 1: implement + crate-local cargo check + unit tests, in parallel isolated worktrees.
  const built = await parallel(recipes.map(r => () =>
    agent(`Implement Task ${r.id} from the build-time plan against crate ${r.crate}. ` +
          `Do NOT run build-bench (measurement is serialized by the orchestrator). ` +
          `Verify with: cargo check -p ${r.crate} && cargo check --workspace. Commit with the task's message.`,
      { label: `impl-${r.id}`, isolation: 'worktree', schema: IMPL_RESULT })))
  // Stage 2 (back in the main session, NOT in the workflow): merge each worktree branch,
  // then run `vox ci build-bench --compare <baseline> --repeat 3 --label "Phase <id>: <label>"`
  // ONCE PER CHANGE, SERIALLY, and record each delta.
  ```
  Do the merges + the serial `build-bench` runs in the main session (or a single non-parallel workflow stage) — the measurement is the integration step, not a per-worktree step.
- **Phase 0–1 (the instruments) — do NOT parallelize across tasks.** They edit the same files (`build_bench.rs`, `dep_cycles.rs`, `cmd_enums.rs`, `run_body.rs`) in sequence; parallel edits would collide. Run them as a normal subagent-driven sequence.
- **Audit/verification fan-out is always safe to parallelize.** Read-only verification (grep/read of signatures, "does crate X still compile") has no shared mutable state — fan it out freely with `parallel(...)` or read-only `agent`s.

**Rule of thumb:** parallelize *reads* and *disjoint-file writes-in-worktrees*; serialize *measurements* and *same-file writes*.

---

## Audit pass (claims proven against the tree, 2026-06-15)

Before finalizing, every load-bearing claim was verified against the working tree. Proven true positives (with evidence) and the false positives caught + corrected:

**Proven true positives (file:line evidence):**
- Toolchain is **1.96.0** (`rust-toolchain.toml`) → `File::set_modified` (stable 1.75) **is available** — the Task 0.3 mtime touch is sound with no fallback needed.
- `cargo_bin()` is `pub(super)` at `crates/vox-cli/src/commands/ci/mod.rs:88` (returns a `PathBuf`, `.exe` on Windows).
- Registration anchors exist: `CudaFeatures` (`cmd_enums.rs:432-433`), `build-timings` (`:442`), `RunnerScale` (`:695-696`); dispatch arms `CudaFeatures` (`run_body.rs:299`), `BuildTimings` (`:300`), `RunnerScale` (`:513`).
- `compute_dependency_shape_summary` exists at `build_timings.rs:99`; `TimingRecord` at `timings.rs:11`; `run_build_timings` (`pub(crate)`) at `timings.rs:112`.
- **All four Phase-2 targets are genuinely un-landed** (re-confirmed): `crates/vox-db/src/lib.rs:132` still has `pub mod retrieval;`; `vox-audit/Cargo.toml` has **no `[features]`** and 8 `cr-*` bins (`cr-e1/a1/d3/a4/a2/e2/p1/p2`); `vox-orchestrator-mcp/Cargo.toml:9` `default = ["news-publish", "toestub-gate", "json-schema"]`; `vox-sql/Cargo.toml:17` `sqlx = { version = "0.9.0", default-features = false, features = ["runtime-tokio", "postgres", "mysql"] }`.
- `vox-db/Cargo.toml:12` has `vox-db-types = { workspace = true }` (the Task 2.1 re-export will resolve); vox-compiler/vox-codegen are **dev-deps** (`:41-49`) — the "keystone" is dev-only and same-layer, confirmed not a runtime blocker.
- `crate-build-audit.vox` is recoverable from `d2759f3c58` (file begins `// vox:caps fs process env`); arch-check runs in CI via `./target/debug/vox --quiet run scripts/arch-check.vox` (`ci.yml:715-718`) — the anchor for Task 0.5/1.4 CI wiring.

**False positives caught & corrected in this revision:**
- *FP1 — `File::set_modified` "might be unavailable" hedge (Task 0.3).* Toolchain is 1.96; the hedge + the confusing `append(true).open()` "touch" (a near-no-op that also smelled like it could append bytes to a `.rs` file) are **removed**; Task 0.3 now uses `set_modified` unconditionally.
- *FP2 — vox-sql example didn't match the real line.* The crate already sets `default-features = false`; Task 2.2's snippet is corrected to edit the **existing** features list (drop `postgres`/`mysql`, keep `runtime-tokio`), not introduce `default-features = false` afresh.
- *FP3 — Phase 1 over-claimed "catches normal cycles arch-check misses."* **Cargo forbids non-dev dependency cycles outright** (a true normal cycle won't build, so CI is already red and arch-check's blind spot is moot). The honest, *detectable* value is the **dev/build back-edge inventory** plus a cheap regression guard. Phase 1 framing is corrected below; the HARD gate on normal cycles is kept as defensive insurance, explicitly labeled near-dead-code-by-design.
- *FP4 — single-run wall-clock is noise-prone.* A one-shot `cargo check` delta can swing ±10–20% from machine load alone, which would manufacture false speedups (or hide real ones). **`build-bench` now takes `--repeat N` and reports the min of N runs** (min is the standard estimator for build benchmarks — it is the run least contaminated by background noise). All measurement commands use `--repeat 3`.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `crates/vox-cli/src/commands/ci/build_bench.rs` | **Create** | The `build-bench` subcommand: pure scenario model, snapshot read/write, delta computation, markdown report formatting, and the lane runner. Unit-tested. |
| `contracts/ci/build-bench-scenarios.v1.json` | **Create** | Pinned, committed list of build scenarios (the SSOT of *what* gets measured), so before/after runs are comparable. |
| `contracts/ci/build-bench-baseline.v1.json` | **Create** | The committed "before" snapshot every phase measures against. |
| `crates/vox-cli/src/commands/ci/dep_cycles.rs` | **Create** | The `dep-cycles` subcommand: pure cargo-metadata→adjacency parser, pure Tarjan SCC, dev-dep back-edge inventory, report formatting. Unit-tested. |
| `crates/vox-cli/src/commands/ci/cmd_enums.rs` | **Modify** | Register `BuildBench` + `DepCycles` enum variants (mirror `RunnerScale`/`CudaFeatures`). |
| `crates/vox-cli/src/commands/ci/run_body.rs` | **Modify** | Add the two dispatch arms. |
| `crates/vox-cli/src/commands/ci/mod.rs` | **Modify** | `mod build_bench;` + `mod dep_cycles;` declarations. |
| `scripts/crate-build-audit.vox` | **Re-land** | Cherry-picked dependency/blast-radius map tool (fan-in, LoC, self-time) → `graphify-out/crate_audit.json` + `CRATE_BUILD_AUDIT.md`. |
| `.github/workflows/ci.yml` | **Modify** | Non-blocking `build-bench` artifact step + re-land the `crate-build-audit` step. |
| `crates/vox-db-types/src/retrieval.rs` | **Create (move)** | Phase 2: the pure `retrieval.rs` relocated out of vox-db into the L0 leaf. |
| `crates/{vox-sql,vox-orchestrator-mcp,vox-audit}/Cargo.toml` | **Modify** | Phase 2: feature-gating. |
| `docs/src/architecture/build-time-log.md` | **Modify** | Phase 6: transcribe the headline measured deltas (the curated SSOT). |
| `docs/src/ci/build-time-program.md` | **Create** | Phase 6: SSOT doc describing the harness, the scenarios, and how to refresh the baseline. |

**Phasing (each phase = shippable, and ends with a measured report except where noted):**
- **Phase 0 (Tasks 0.1–0.5):** the measurement spine. `build-bench` + scenarios + baseline + map re-land + CI wiring. *Deliverable: a committed baseline snapshot.*
- **Phase 1 (Tasks 1.1–1.4):** `dep-cycles` SCC + back-edge inventory. *Deliverable: the coupling inventory (Phase 1's "measurement" is the inventory + blast-radius table, NOT a build-time delta — stated honestly).*
- **Phase 2 (Tasks 2.1–2.4):** re-land the four blast-radius/feature-gating wins, each measured.
- **Phase 3 (Task 3.1):** one slowest-unit split, measured.
- **Phase 4 — reserved/optional:** further splits only if Phase 2–3 deltas indicate a remaining hot crate (data-driven; no speculative work).
- **Phase 5 (Task 5.1):** execute + measure the affected-crate selective-CI sub-plan.
- **Phase 6 (Task 6.1):** aggregate all deltas into the curated log + SSOT doc.

---

## Phase 0 — The measurement spine

### Task 0.1: Re-land the crate dependency/blast-radius map (`crate-build-audit.vox`)

This `.vox` tool is the source of fan-in / blast-radius numbers the later phases target. It is not on main; cherry-pick it.

**Files:**
- Re-land: `scripts/crate-build-audit.vox` (from commit `d2759f3c58`)

- [ ] **Step 1: Recover the script from the unmerged branch**

Run:
```bash
git show d2759f3c58:scripts/crate-build-audit.vox > scripts/crate-build-audit.vox
```
Expected: the file is written (~406 lines). If `d2759f3c58` is not reachable, run `git fetch origin 'refs/heads/claude/crate-build-audit-tool:refs/remotes/origin/claude/crate-build-audit-tool'` first, then retry the `git show`. If still unreachable, **STOP and report** — do not hand-write the tool from scratch.

- [ ] **Step 2: Verify it parses and runs (interp lane)**

Run:
```bash
cargo run -q -p vox-cli -- check scripts/crate-build-audit.vox
cargo run -q -p vox-cli -- run --mode interp scripts/crate-build-audit.vox
```
Expected: `vox check` reports no errors (this is the `scripts/**/*.vox` CI gate); the run regenerates `graphify-out/crate_audit.json` + `graphify-out/CRATE_BUILD_AUDIT.md`. Spot-check: `crate_audit.json` is a JSON array of ~109 objects, each with fields `crate, layer, loc, fan_in, fan_out, compile_s, max_loc, max_dependents, deps`. If the file has far fewer rows or missing fields, **STOP and report** (cargo metadata likely ran in the wrong cwd).

- [ ] **Step 3: Record the blast-radius baseline (informational, for Phase 1–2 targeting)**

Run (reads the JSON the previous step wrote):
```bash
cargo run -q -p vox-cli -- run --mode interp scripts/crate-build-audit.vox
```
From `graphify-out/CRATE_BUILD_AUDIT.md`, note the top blast-radius crates (compile_s × fan_in). Expected order (from the committed artifact): `vox-db` (~21.2s × 26 ≈ 551) is #1, `vox-compiler` (~14.5 × 23) #2. These are the Phase 2 targets. Paste the top-5 into your task report.

- [ ] **Step 4: Commit**

```bash
git add scripts/crate-build-audit.vox
git commit -m "chore(ci): re-land crate-build-audit dependency/blast-radius map tool

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 0.2: `build-bench` subcommand — scenario model + snapshot write (TDD)

**Files:**
- Create: `crates/vox-cli/src/commands/ci/build_bench.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (add `mod build_bench;`)

> *Sonnet note: this whole task is pure Rust with no CI risk. The lane runner shells `cargo` but the scenario model, snapshot (de)serialization, and delta math are pure and fully unit-tested. Do NOT add the enum variant yet — that is Task 0.4. Build the module and its tests first.*

- [ ] **Step 1: Write the failing tests for the pure model**

Create `crates/vox-cli/src/commands/ci/build_bench.rs`:

```rust
//! `vox ci build-bench` — reproducible wall-clock build scenarios with a
//! committed baseline and phase-delta reporting. Separate from `build-timings`
//! (which carries soft-budget + telemetry semantics): this command's only job is
//! to measure a pinned scenario set, snapshot it, and diff snapshots so each
//! optimization phase can report a real before/after delta.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result};

/// One thing to measure: "bump `touch`'s mtime, then `cargo check <args>`, time
/// it." Touching before EVERY run makes the measurement a reproducible
/// *incremental rebuild* — without it, a repeated `cargo check` is a cache no-op
/// on runs 2..N and `--repeat`'s min would collapse to ~0. Touch a base crate +
/// build a top dependent to measure blast radius; touch a crate + build itself
/// to measure its own incremental compile cost.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scenario {
    pub id: String,
    /// File whose mtime is bumped before each timed build, e.g.
    /// "crates/vox-db/src/lib.rs". Required.
    pub touch: String,
    /// cargo args after `check`, e.g. ["-p","vox-db"].
    pub args: Vec<String>,
}

/// The committed list of scenarios.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScenarioFile {
    pub schema_version: u32,
    pub scenarios: Vec<Scenario>,
}

/// One measured result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchRecord {
    pub id: String,
    pub ok: bool,
    pub wall_ms: u128,
}

/// A full run: an ordered set of records, keyed by id for compare.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub schema_version: u32,
    pub label: String,
    pub records: Vec<BenchRecord>,
}

impl Snapshot {
    pub fn by_id(&self) -> BTreeMap<&str, &BenchRecord> {
        self.records.iter().map(|r| (r.id.as_str(), r)).collect()
    }
}

/// One row of a delta report.
#[derive(Debug, Clone, PartialEq)]
pub struct DeltaRow {
    pub id: String,
    pub base_ms: u128,
    pub new_ms: u128,
    pub delta_ms: i128,
    pub pct: f64,
}

/// Compute per-scenario deltas of `new` vs `base`. Only ids present in BOTH and
/// `ok` in both are compared; others are skipped (reported separately by caller).
pub fn compute_deltas(base: &Snapshot, new: &Snapshot) -> Vec<DeltaRow> {
    let b = base.by_id();
    let mut rows = Vec::new();
    for nr in &new.records {
        if !nr.ok {
            continue;
        }
        if let Some(br) = b.get(nr.id.as_str()) {
            if !br.ok {
                continue;
            }
            let delta = nr.wall_ms as i128 - br.wall_ms as i128;
            let pct = if br.wall_ms > 0 {
                (delta as f64) / (br.wall_ms as f64) * 100.0
            } else {
                0.0
            };
            rows.push(DeltaRow {
                id: nr.id.clone(),
                base_ms: br.wall_ms,
                new_ms: nr.wall_ms,
                delta_ms: delta,
                pct,
            });
        }
    }
    rows
}

/// Render a delta report as Markdown (the per-phase artifact body).
pub fn format_delta_markdown(label: &str, rows: &[DeltaRow]) -> String {
    let mut out = format!("### {label}\n\n| Scenario | Base | New | Δ | Δ% |\n|---|--:|--:|--:|--:|\n");
    for r in rows {
        let sign = if r.delta_ms <= 0 { "" } else { "+" };
        out.push_str(&format!(
            "| {} | {} ms | {} ms | {sign}{} ms | {sign}{:.1}% |\n",
            r.id, r.base_ms, r.new_ms, r.delta_ms, r.pct
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(label: &str, recs: &[(&str, bool, u128)]) -> Snapshot {
        Snapshot {
            schema_version: 1,
            label: label.into(),
            records: recs
                .iter()
                .map(|(id, ok, ms)| BenchRecord {
                    id: (*id).into(),
                    ok: *ok,
                    wall_ms: *ms,
                })
                .collect(),
        }
    }

    #[test]
    fn delta_is_new_minus_base_with_pct() {
        let base = snap("baseline", &[("check_vox_db", true, 1000)]);
        let new = snap("phase2", &[("check_vox_db", true, 600)]);
        let d = compute_deltas(&base, &new);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].delta_ms, -400);
        assert!((d[0].pct - (-40.0)).abs() < 0.001);
    }

    #[test]
    fn failed_or_missing_scenarios_are_skipped() {
        let base = snap("b", &[("a", true, 100), ("b", true, 100)]);
        let new = snap("n", &[("a", false, 50), ("c", true, 50)]); // a failed, b missing, c new
        let d = compute_deltas(&base, &new);
        assert!(d.is_empty(), "no comparable ok-in-both pairs");
    }

    #[test]
    fn markdown_marks_improvement_without_plus_sign() {
        let rows = vec![DeltaRow {
            id: "check_vox_db".into(),
            base_ms: 1000,
            new_ms: 600,
            delta_ms: -400,
            pct: -40.0,
        }];
        let md = format_delta_markdown("Phase 2", &rows);
        assert!(md.contains("-400 ms"));
        assert!(md.contains("-40.0%"));
        assert!(!md.contains("+-400"), "improvement must not get a + prefix");
    }
}
```

- [ ] **Step 2: Declare the module**

In `crates/vox-cli/src/commands/ci/mod.rs`, add (next to the other `mod …;` lines — search for the anchor `mod runner_scale;` and add beside it):
```rust
mod build_bench;
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test -p vox-cli build_bench::tests -- --nocapture`
Expected: 3 passed (`delta_is_new_minus_base_with_pct`, `failed_or_missing_scenarios_are_skipped`, `markdown_marks_improvement_without_plus_sign`).

- [ ] **Step 4: Format, lint, commit**

```bash
cargo fmt -p vox-cli
cargo clippy -p vox-cli -- -D warnings
git add crates/vox-cli/src/commands/ci/build_bench.rs crates/vox-cli/src/commands/ci/mod.rs
git commit -m "feat(ci): build-bench scenario model + snapshot/delta logic (TDD)

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 0.3: `build-bench` lane runner + CLI entry point

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/build_bench.rs` (add the runner + `run_build_bench`)

> *Sonnet note: `cargo_bin()` is `pub(super)` in `ci/mod.rs:88` — call it as `super::cargo_bin()`. Do NOT shell the literal string "cargo"; the repo resolves the toolchain cargo via that helper (confirm by reading `ci/mod.rs:88` first). The runner is side-effecting (it builds), so it is NOT unit-tested — only the pure functions from Task 0.2 are. The smoke test in Step 3 is the verification. `File::set_modified` is stable since Rust 1.75 and the pinned toolchain is 1.96 (`rust-toolchain.toml`) — no fallback needed.*

- [ ] **Step 1: Add the runner + entry point**

Append to `build_bench.rs`:

```rust
/// Run one scenario `repeat` times and return the record with the MIN wall time
/// (the run least contaminated by background noise — the standard estimator for
/// build benchmarks). Each run first bumps the mtime of `touch` so the
/// `cargo check` is a real incremental rebuild, not a cache no-op. `ok` is true
/// only if EVERY run succeeded.
fn run_scenario(root: &Path, s: &Scenario, repeat: u32) -> BenchRecord {
    let runs = repeat.max(1);
    let mut best: Option<u128> = None;
    let mut all_ok = true;
    for _ in 0..runs {
        // Invalidate the input so cargo recompiles (mtime bump; File::set_modified
        // is stable since Rust 1.75 — toolchain here is 1.96).
        let p = root.join(&s.touch);
        if let Ok(f) = std::fs::File::open(&p) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
        let start = Instant::now();
        let status = Command::new(super::cargo_bin())
            .current_dir(root)
            .arg("check")
            .args(&s.args)
            .status();
        let wall_ms = start.elapsed().as_millis();
        let ok = matches!(status, Ok(st) if st.success());
        all_ok &= ok;
        best = Some(best.map_or(wall_ms, |b| b.min(wall_ms)));
    }
    BenchRecord {
        id: s.id.clone(),
        ok: all_ok,
        wall_ms: best.unwrap_or(0),
    }
}

/// Load the committed scenario file.
fn load_scenarios(root: &Path) -> Result<ScenarioFile> {
    let p = root.join("contracts/ci/build-bench-scenarios.v1.json");
    let s = std::fs::read_to_string(&p)
        .with_context(|| format!("read scenarios {}", p.display()))?;
    serde_json::from_str(&s).with_context(|| "parse build-bench-scenarios.v1.json")
}

/// `vox ci build-bench [--label L] [--write OUT] [--compare BASELINE] [--repeat N]`
pub fn run_build_bench(
    root: &Path,
    label: Option<String>,
    write: Option<String>,
    compare: Option<String>,
    repeat: u32,
) -> Result<()> {
    let sf = load_scenarios(root)?;
    let label = label.unwrap_or_else(|| "adhoc".to_string());
    eprintln!(
        "build-bench: running {} scenario(s) [{}] × {} (min) …",
        sf.scenarios.len(),
        label,
        repeat.max(1)
    );
    let mut records = Vec::new();
    for s in &sf.scenarios {
        let r = run_scenario(root, s, repeat);
        eprintln!(
            "  {:<36} {}  {} ms",
            r.id,
            if r.ok { "ok" } else { "FAIL" },
            r.wall_ms
        );
        records.push(r);
    }
    let snap = Snapshot {
        schema_version: 1,
        label: label.clone(),
        records,
    };

    if let Some(out) = &write {
        let json = serde_json::to_string_pretty(&snap)? + "\n";
        if let Some(parent) = Path::new(out).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(out, json).with_context(|| format!("write snapshot {out}"))?;
        eprintln!("build-bench: wrote snapshot {out}");
    }

    if let Some(base_path) = &compare {
        let base_str = std::fs::read_to_string(base_path)
            .with_context(|| format!("read baseline {base_path}"))?;
        let base: Snapshot = serde_json::from_str(&base_str)
            .with_context(|| format!("parse baseline {base_path}"))?;
        let rows = compute_deltas(&base, &snap);
        let md = format_delta_markdown(&label, &rows);
        // Print to stdout AND append to the cumulative report under graphify-out/.
        print!("{md}");
        let report_dir = root.join("graphify-out/build-bench");
        std::fs::create_dir_all(&report_dir).ok();
        let report = report_dir.join("REPORT.md");
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&report)
        {
            let _ = write!(f, "\n{md}");
        }
        // Per-label snapshot for provenance.
        let snap_json = serde_json::to_string_pretty(&snap)? + "\n";
        let _ = std::fs::write(report_dir.join(format!("{label}.json")), snap_json);
    }

    Ok(())
}
```

- [ ] **Step 2: Verify it still builds + tests pass**

Run: `cargo test -p vox-cli build_bench:: -- --nocapture` then `cargo clippy -p vox-cli -- -D warnings`
Expected: tests pass, clippy clean. (No new tests — the runner is side-effecting; pure logic is already covered.)

> *Sonnet note: if `File::set_modified` is not available on the pinned Rust, it was stabilized in Rust 1.75. Confirm the toolchain (`cat rust-toolchain.toml`); if older, replace the mtime bump with the `filetime` crate ONLY if it's already a dependency (`Grep "filetime" Cargo.lock`) — otherwise keep the append-open fallback and note in the task report that incremental scenarios depend on append-open touching mtime. Do NOT add a new dependency for this.*

- [ ] **Step 3: Commit (entry point wired in Task 0.4)**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/build_bench.rs
git commit -m "feat(ci): build-bench lane runner + write/compare entry point

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 0.4: Register `build-bench` + pin the scenario set

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (add `BuildBench` variant)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (add dispatch arm)
- Create: `contracts/ci/build-bench-scenarios.v1.json`

> *Sonnet note: mirror `RunnerScale` exactly (it has flags). The dispatch `match` is async; `run_build_bench` is sync, so the arm returns its `Result` directly (no `.await`). Read `cmd_enums.rs` around the `RunnerScale` variant and `run_body.rs` around its arm BEFORE editing.*

- [ ] **Step 1: Add the enum variant**

In `cmd_enums.rs`, after the `RunnerScale { … }` variant (anchor: `#[command(name = "runner-scale")]`), add:

```rust
    /// Reproducible wall-clock build scenarios with a committed baseline; --compare emits a phase delta.
    #[command(name = "build-bench")]
    BuildBench {
        /// Label for this run (used in the delta report heading + snapshot filename).
        #[arg(long)]
        label: Option<String>,
        /// Write the full snapshot JSON to this path.
        #[arg(long)]
        write: Option<String>,
        /// Compare against a baseline snapshot JSON and emit a delta report.
        #[arg(long)]
        compare: Option<String>,
        /// Run each scenario N times and keep the min wall time (default 3).
        #[arg(long, default_value_t = 3)]
        repeat: u32,
    },
```

- [ ] **Step 2: Add the dispatch arm**

In `run_body.rs`, after the `CiCmd::RunnerScale { apply } => …` arm (anchor: `super::runner_scale::run_scale`), add:

```rust
        CiCmd::BuildBench {
            label,
            write,
            compare,
            repeat,
        } => super::build_bench::run_build_bench(&root, label, write, compare, repeat),
```

- [ ] **Step 3: Pin the scenario set**

Create `contracts/ci/build-bench-scenarios.v1.json`. Each scenario bumps the mtime of `touch` then `cargo check`s `args`, so it measures an *incremental rebuild*. Five lanes touch a crate and rebuild itself (the feature-gating wins shrink these); one lane touches the #1 blast-radius crate `vox-db` and rebuilds a top dependent (`vox-cli`) to measure churn surface. **All six `touch` paths were verified to exist (2026-06-15): `crates/{vox-db,vox-sql,vox-orchestrator-mcp,vox-audit,vox-cli}/src/lib.rs` all present.** Re-confirm before committing.

```json
{
  "schema_version": 1,
  "scenarios": [
    { "id": "check_vox_db",    "touch": "crates/vox-db/src/lib.rs",               "args": ["-p", "vox-db"] },
    { "id": "check_vox_sql",   "touch": "crates/vox-sql/src/lib.rs",              "args": ["-p", "vox-sql"] },
    { "id": "check_vox_mcp",   "touch": "crates/vox-orchestrator-mcp/src/lib.rs", "args": ["-p", "vox-orchestrator-mcp"] },
    { "id": "check_vox_audit", "touch": "crates/vox-audit/src/lib.rs",            "args": ["-p", "vox-audit"] },
    { "id": "check_vox_cli",   "touch": "crates/vox-cli/src/lib.rs",              "args": ["-p", "vox-cli"] },
    { "id": "blastradius_vox_db_to_cli", "touch": "crates/vox-db/src/lib.rs",     "args": ["-p", "vox-cli"] }
  ]
}
```

> *Note on scenario order: lanes run in file order and earlier `cargo check`s warm later ones. That is fine — deltas stay valid as long as the order is identical between baseline and comparison runs (it is, same file). **Do NOT reorder, add, or remove scenarios without regenerating the baseline (Task 0.5)** — a changed scenario set invalidates every prior delta.*

- [ ] **Step 4: Build + smoke (warm the cache first)**

```bash
cargo check -p vox-cli            # build the CLI
cargo check --workspace           # warm the cache so lanes are fast (optional but recommended)
cargo run -q -p vox-cli -- ci build-bench --label smoke --repeat 1 --write /tmp/smoke.json
```
Expected: prints one `ok  <N> ms` line per scenario; `/tmp/smoke.json` is a `Snapshot` with 6 `records`. (`--repeat 1` for a fast smoke; the recorded baseline + phase deltas use the default `--repeat 3`.) If any scenario is `FAIL`, read its `args` — a `FAIL` here means the crate doesn't compile on the current tree (a real problem to fix, not a bench bug).

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt -p vox-cli
cargo clippy -p vox-cli -- -D warnings
git add crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs contracts/ci/build-bench-scenarios.v1.json
git commit -m "feat(ci): register vox ci build-bench + pin scenario SSOT

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 0.5: Capture the committed baseline + wire CI

**Files:**
- Create: `contracts/ci/build-bench-baseline.v1.json`
- Modify: `.github/workflows/ci.yml` (non-blocking build-bench + crate-build-audit steps)

- [ ] **Step 1: Capture the baseline snapshot**

With a warm cache (run `cargo check --workspace` first), on a **quiet machine** (no other heavy processes — measurement noise corrupts the baseline every later phase compares against):
```bash
cargo check --workspace
cargo run -q -p vox-cli -- ci build-bench --label baseline --repeat 3 --write contracts/ci/build-bench-baseline.v1.json
```
Expected: all 6 scenarios `ok`; the file is written (min-of-3 per scenario). **Paste the six `wall_ms` values into your task report** — this is the "before" the entire program is measured against. (If you cannot run a warm build in your environment, generate the baseline via the CI job from Step 2 instead, download the artifact, and commit that. **The baseline and all phase comparisons should run on the same machine/runner class** — wall-clock is not portable across hardware.)

- [ ] **Step 2: Add the non-blocking CI steps**

In `.github/workflows/ci.yml`, find the arch-check step (anchor: search for `vox-arch-check` or `arch-check`). After it, add two non-blocking steps. (Re-Read the surrounding job to match its `shell`/`run` style; this is illustrative.)

```yaml
      - name: Crate build/dependency audit (refresh leaf-design map)
        run: ./target/debug/vox --quiet run --mode interp scripts/crate-build-audit.vox
        continue-on-error: true

      - name: Build-bench (wall-clock scenarios vs committed baseline — ROUGH signal)
        # --repeat 1 here: CI is a noisy shared runner, so this is a rough trend
        # signal only, NOT the recorded phase delta. The recorded deltas come from
        # `--repeat 3` on a quiet machine of the same class as the baseline.
        run: |
          ./target/debug/vox --quiet ci build-bench \
            --label "ci-${{ github.run_number }}" \
            --repeat 1 \
            --compare contracts/ci/build-bench-baseline.v1.json \
            --write graphify-out/build-bench/ci-snapshot.json
        continue-on-error: true

      - name: Upload build-time maps
        uses: actions/upload-artifact@v7
        with:
          name: build-time-maps
          path: |
            graphify-out/crate_audit.json
            graphify-out/CRATE_BUILD_AUDIT.md
            graphify-out/build-bench/
          if-no-files-found: ignore
```

- [ ] **Step 3: Validate YAML + commit**

```bash
cargo run -q -p vox-cli -- ci ssot-drift   # expect OK (or run the workflow linter the repo uses)
git add contracts/ci/build-bench-baseline.v1.json .github/workflows/ci.yml
git commit -m "feat(ci): commit build-bench baseline + non-blocking build-time CI artifact

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

**Phase 0 is complete: the instrument exists and the baseline is committed. Every later phase ends by running `vox ci build-bench --repeat 3 --compare contracts/ci/build-bench-baseline.v1.json --label "<phase>"` and pasting the delta.**

---

## Phase 1 — Cyclical & inter-crate coupling detection

> **Honest framing (read carefully — this corrects an overclaim):** Phase 1 does NOT reduce build time by itself, and its cycle gate is mostly defensive. **Cargo already forbids normal (non-dev) dependency cycles** — a real one won't compile, so it can never land, and arch-check's lack of cycle detection is therefore moot for *normal* edges. The genuinely detectable, useful output is: (a) the **dev/build-dependency back-edge inventory** (legal in cargo, invisible to arch-check — e.g. vox-db's test-only edge to vox-compiler), and (b) the **blast-radius map** from Task 0.1, which is the actionable intercrate data the later phases target. The HARD gate on normal cycles is kept as cheap regression insurance (near-dead-code by design — it can only ever fire on the rare build-dependency cycle cargo permits). Phase 1's "measurement" is this inventory + map, NOT a build-time delta — **do not fabricate one.**

### Task 1.1: Pure Tarjan SCC (TDD)

**Files:**
- Create: `crates/vox-cli/src/commands/ci/dep_cycles.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (add `mod dep_cycles;`)

- [ ] **Step 1: Write the failing tests + the pure SCC**

Create `crates/vox-cli/src/commands/ci/dep_cycles.rs`:

```rust
//! `vox ci dep-cycles` — detect dependency cycles and inventory back-edges that
//! `vox-arch-check` cannot see. arch-check does pairwise layer-ordering on normal
//! deps only; it has no SCC pass, so same-layer cycles and dev-dependency
//! back-edges are invisible to it. This command builds the workspace adjacency
//! from `cargo metadata` and runs Tarjan's SCC to find any cycle, plus reports
//! dev-dep back-edges (e.g. vox-db's test-only edge to vox-compiler).

use std::collections::BTreeMap;

/// Strongly-connected components of a directed graph (adjacency: node -> out-neighbours).
/// Returns only components of size > 1 (i.e. actual cycles), each as a sorted Vec.
pub fn cycles(adj: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    // Iterative Tarjan to avoid stack overflow on a 110-node graph chain.
    let mut index_counter = 0usize;
    let mut indices: BTreeMap<String, usize> = BTreeMap::new();
    let mut lowlink: BTreeMap<String, usize> = BTreeMap::new();
    let mut on_stack: BTreeMap<String, bool> = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();
    let mut out: Vec<Vec<String>> = Vec::new();

    // Explicit DFS frames: (node, next-neighbour-index).
    for start in adj.keys() {
        if indices.contains_key(start) {
            continue;
        }
        let mut work: Vec<(String, usize)> = vec![(start.clone(), 0)];
        while let Some((v, ni)) = work.last().cloned() {
            if ni == 0 {
                indices.insert(v.clone(), index_counter);
                lowlink.insert(v.clone(), index_counter);
                index_counter += 1;
                stack.push(v.clone());
                on_stack.insert(v.clone(), true);
            }
            let neighbours = adj.get(&v).cloned().unwrap_or_default();
            if ni < neighbours.len() {
                // advance this frame's cursor
                let last = work.len() - 1;
                work[last].1 = ni + 1;
                let w = neighbours[ni].clone();
                if !indices.contains_key(&w) {
                    work.push((w, 0));
                } else if *on_stack.get(&w).unwrap_or(&false) {
                    let lw = *indices.get(&w).unwrap();
                    let lv = *lowlink.get(&v).unwrap();
                    lowlink.insert(v.clone(), lv.min(lw));
                }
            } else {
                // done with v: if root of an SCC, pop it
                if lowlink.get(&v) == indices.get(&v) {
                    let mut comp = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack.insert(w.clone(), false);
                        comp.push(w.clone());
                        if w == v {
                            break;
                        }
                    }
                    if comp.len() > 1 {
                        comp.sort();
                        out.push(comp);
                    }
                }
                work.pop();
                // propagate lowlink to parent
                if let Some((parent, _)) = work.last().cloned() {
                    let lp = *lowlink.get(&parent).unwrap();
                    let lv = *lowlink.get(&v).unwrap();
                    lowlink.insert(parent, lp.min(lv));
                }
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(edges: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        edges
            .iter()
            .map(|(n, ds)| (n.to_string(), ds.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    #[test]
    fn acyclic_graph_has_no_cycles() {
        let adj = g(&[("a", &["b"]), ("b", &["c"]), ("c", &[])]);
        assert!(cycles(&adj).is_empty());
    }

    #[test]
    fn two_node_cycle_detected() {
        let adj = g(&[("a", &["b"]), ("b", &["a"])]);
        assert_eq!(cycles(&adj), vec![vec!["a".to_string(), "b".to_string()]]);
    }

    #[test]
    fn three_node_cycle_detected_but_tail_excluded() {
        // a->b->c->a is a cycle; d->a is a tail (not in the SCC).
        let adj = g(&[("a", &["b"]), ("b", &["c"]), ("c", &["a"]), ("d", &["a"])]);
        let cs = cycles(&adj);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0], vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[test]
    fn self_loop_is_a_cycle() {
        // size-1 SCC is excluded, but a self-loop forms a 1-node SCC that IS a cycle;
        // Tarjan yields comp.len()==1 here, so we do NOT report it (cargo forbids
        // self-deps anyway). Document the choice via this test.
        let adj = g(&[("a", &["a"])]);
        assert!(cycles(&adj).is_empty());
    }
}
```

- [ ] **Step 2: Declare the module + run tests**

In `mod.rs`, add `mod dep_cycles;` beside `mod build_bench;`.
Run: `cargo test -p vox-cli dep_cycles::tests -- --nocapture`
Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
cargo fmt -p vox-cli
cargo clippy -p vox-cli -- -D warnings
git add crates/vox-cli/src/commands/ci/dep_cycles.rs crates/vox-cli/src/commands/ci/mod.rs
git commit -m "feat(ci): pure Tarjan SCC for dependency-cycle detection (TDD)

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 1.2: cargo-metadata adjacency builder (TDD)

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/dep_cycles.rs`

> *Sonnet note: mirror the parsing approach in `crates/vox-cli/src/commands/ci/build_timings.rs:99-186` (`compute_dependency_shape_summary`) — it builds `id_to_name` and restricts to workspace members. Read it first. The pure builder below takes already-parsed JSON so it is unit-testable WITHOUT shelling cargo; the shelling wrapper is separate and not unit-tested.*

- [ ] **Step 1: Add the pure builder + test**

Append to `dep_cycles.rs`:

```rust
use serde_json::Value;

/// Build `crate -> workspace-dep-names` from parsed `cargo metadata` JSON.
/// `include_nonlink` controls whether **non-link-time** edges (dev AND build
/// dependencies) are included. The HARD cycle gate wants them OFF (only true
/// link-time/normal edges — the ones cargo forbids cycles in); the back-edge
/// inventory wants them ON. Folding build-deps into the link-time graph would
/// risk a false-positive cycle on a build-dep back-edge cargo permits, so build
/// is excluded from the link-time graph exactly like dev.
pub fn adjacency_from_metadata(meta: &Value, include_nonlink: bool) -> BTreeMap<String, Vec<String>> {
    let empty = vec![];
    let members: std::collections::BTreeSet<String> = meta["workspace_members"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    // name set of workspace members (members are package ids; map ids->names)
    let mut id_name: BTreeMap<String, String> = BTreeMap::new();
    for p in meta["packages"].as_array().unwrap_or(&empty) {
        if let (Some(id), Some(name)) = (p["id"].as_str(), p["name"].as_str()) {
            id_name.insert(id.to_string(), name.to_string());
        }
    }
    let member_names: std::collections::BTreeSet<String> = members
        .iter()
        .filter_map(|id| id_name.get(id).cloned())
        .collect();

    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in meta["packages"].as_array().unwrap_or(&empty) {
        let name = match p["name"].as_str() {
            Some(n) if member_names.contains(n) => n.to_string(),
            _ => continue,
        };
        let mut deps: Vec<String> = Vec::new();
        for d in p["dependencies"].as_array().unwrap_or(&empty) {
            let dname = match d["name"].as_str() {
                Some(n) => n,
                None => continue,
            };
            if !member_names.contains(dname) {
                continue; // third-party
            }
            let kind = d["kind"].as_str().unwrap_or(""); // "" (normal), "dev", "build"
            let is_link_time = kind.is_empty(); // only normal deps are link-time
            if !is_link_time && !include_nonlink {
                continue; // exclude BOTH dev and build from the link-time graph
            }
            if dname != name {
                deps.push(dname.to_string());
            }
        }
        deps.sort();
        deps.dedup();
        adj.entry(name).or_default().extend(deps);
    }
    for v in adj.values_mut() {
        v.sort();
        v.dedup();
    }
    adj
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    fn meta() -> Value {
        serde_json::json!({
            "workspace_members": ["a 0.1 (path+file:///a)", "b 0.1 (path+file:///b)"],
            "packages": [
                {"id":"a 0.1 (path+file:///a)","name":"a","dependencies":[
                    {"name":"b","kind":null},
                    {"name":"serde","kind":null}
                ]},
                {"id":"b 0.1 (path+file:///b)","name":"b","dependencies":[
                    {"name":"a","kind":"dev"}
                ]}
            ]
        })
    }

    #[test]
    fn normal_edges_only_when_dev_excluded() {
        let adj = adjacency_from_metadata(&meta(), false);
        assert_eq!(adj["a"], vec!["b".to_string()]); // serde dropped (third-party)
        assert_eq!(adj["b"], Vec::<String>::new()); // dev edge b->a excluded
        // therefore NO cycle when dev excluded:
        assert!(cycles(&adj).is_empty());
    }

    #[test]
    fn dev_edges_included_reveal_back_edge_cycle() {
        let adj = adjacency_from_metadata(&meta(), true);
        assert_eq!(adj["b"], vec!["a".to_string()]); // dev edge now present
        // a->b->a is a cycle ONLY through the dev edge:
        assert_eq!(cycles(&adj), vec![vec!["a".to_string(), "b".to_string()]]);
    }
}
```

- [ ] **Step 2: Run** — `cargo test -p vox-cli dep_cycles::metadata_tests -- --nocapture` → 2 passed.

- [ ] **Step 3: Commit** — `git commit -am "feat(ci): cargo-metadata workspace adjacency builder (dev-aware, TDD)"` (with trailer).

---

### Task 1.3: `dep-cycles` subcommand + inventory report

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/dep_cycles.rs` (add `run_dep_cycles`)
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (add `DepCycles` variant)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (add dispatch arm)

> *Sonnet note: `super::cargo_bin()` (pub(super) in ci/mod.rs:88) for the cargo path. The command must (a) HARD-fail on a NORMAL-dep cycle (cargo would reject it anyway, so this is defensive + catches the build.rs/proc-macro corner), and (b) only REPORT (advisory) dev-dep back-edges and same-layer normal cycles, writing them to graphify-out/. Do not make dev-dep back-edges fail CI — vox-db's test edge is legitimate.*

- [ ] **Step 1: Add the runner**

Append to `dep_cycles.rs`:

```rust
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

fn cargo_metadata(root: &Path) -> Result<Value> {
    let out = Command::new(super::cargo_bin())
        .current_dir(root)
        .args(["metadata", "--format-version", "1"])
        .output()
        .context("run cargo metadata")?;
    if !out.status.success() {
        anyhow::bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    serde_json::from_slice(&out.stdout).context("parse cargo metadata")
}

/// `vox ci dep-cycles` — HARD-fail on normal (link-time) cycles; inventory
/// dev/build back-edge cycles (advisory).
pub fn run_dep_cycles(root: &Path) -> Result<()> {
    let meta = cargo_metadata(root)?;
    let link_time = adjacency_from_metadata(&meta, false); // normal deps only
    let with_nonlink = adjacency_from_metadata(&meta, true); // + dev + build

    let normal_cycles = cycles(&link_time);
    let nonlink_cycles = cycles(&with_nonlink);
    // back-edge cycles = present only once dev/build edges are added.
    let link_set: std::collections::BTreeSet<Vec<String>> = normal_cycles.iter().cloned().collect();
    let nonlink_only: Vec<Vec<String>> = nonlink_cycles
        .into_iter()
        .filter(|c| !link_set.contains(c))
        .collect();

    let mut report = String::from("# Dependency cycle & back-edge inventory\n\n");
    report.push_str(&format!(
        "Normal (link-time) dep cycles (HARD — cargo would reject these): {}\n",
        normal_cycles.len()
    ));
    for c in &normal_cycles {
        report.push_str(&format!("  - CYCLE: {}\n", c.join(" -> ")));
    }
    report.push_str(&format!(
        "\nDev/build back-edge cycles (advisory — legal in cargo): {}\n",
        nonlink_only.len()
    ));
    for c in &nonlink_only {
        report.push_str(&format!("  - back-edge-cycle: {}\n", c.join(" -> ")));
    }

    let dir = root.join("graphify-out");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join("DEP_CYCLES.md"), &report).ok();
    print!("{report}");

    if !normal_cycles.is_empty() {
        anyhow::bail!(
            "{} normal-dependency cycle(s) detected — see graphify-out/DEP_CYCLES.md",
            normal_cycles.len()
        );
    }
    Ok(())
}
```

- [ ] **Step 2: Register the subcommand**

In `cmd_enums.rs`, after `BuildBench`:
```rust
    /// Detect dependency cycles (HARD on normal-dep cycles) and inventory dev-dep back-edges.
    #[command(name = "dep-cycles")]
    DepCycles,
```
In `run_body.rs`, after the `BuildBench` arm:
```rust
        CiCmd::DepCycles => super::dep_cycles::run_dep_cycles(&root),
```

- [ ] **Step 3: Build + run against the real workspace**

```bash
cargo run -q -p vox-cli -- ci dep-cycles
```
Expected (per the verified findings): **0 normal (link-time) cycles** (exit 0) and a small list of dev/build back-edge cycles — at minimum the `vox-db <-> vox-compiler`/`vox-codegen` test edge if it forms a 2-cycle (it only does if vox-compiler also dev-depends on vox-db; if not, `nonlink_only` may be empty and the back-edge shows only in the adjacency, which is fine). Paste `graphify-out/DEP_CYCLES.md` into your task report. **If a NORMAL cycle is reported, STOP and report it** — that is a real, surprising finding (cargo should have rejected it) and must be understood before proceeding.

- [ ] **Step 4: Format, lint, commit**

```bash
cargo fmt -p vox-cli
cargo clippy -p vox-cli -- -D warnings
git add crates/vox-cli/src/commands/ci/dep_cycles.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs
git commit -m "feat(ci): vox ci dep-cycles — SCC cycle gate + dev back-edge inventory

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 1.4: Wire `dep-cycles` as a gate + record the inventory

**Files:**
- Modify: `.github/workflows/ci.yml` (add `dep-cycles` to an existing guard step)

- [ ] **Step 1: Add to CI**

Find the guards/arch-check job (anchor: the `vox-arch-check` step from Task 0.5). Add a step (blocking — normal cycles must never land):
```yaml
      - name: Dependency cycle gate
        run: ./target/debug/vox --quiet ci dep-cycles
```
(It exits 0 today, so it is safe to make blocking; it only fails if a normal-dep cycle is ever introduced.)

- [ ] **Step 2: Record the Phase-1 deliverable**

This phase's "measurement" is the inventory, not a build-time delta. In your task report, include: (a) `graphify-out/DEP_CYCLES.md` (cycles + back-edges), and (b) the top-5 blast-radius crates from `graphify-out/CRATE_BUILD_AUDIT.md` (Task 0.1). State plainly: "No normal-dependency cycles exist; N dev-dep back-edges inventoried; the blast-radius targets for Phase 2 are [list]."

- [ ] **Step 3: Commit**

```bash
cargo run -q -p vox-cli -- ci ssot-drift
git add .github/workflows/ci.yml
git commit -m "ci: gate normal-dependency cycles via vox ci dep-cycles

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Phase 2 — Inter-crate blast-radius & default-build reductions (measured)

Each task here re-lands one un-merged optimization, then **measures**. The measurement command is the same every time; only the `--label` changes:

```bash
cargo check --workspace   # warm the cache for comparable timing
cargo run -q -p vox-cli -- ci build-bench --repeat 3 --compare contracts/ci/build-bench-baseline.v1.json --label "Phase 2.N: <what>"
```
Paste the printed delta table into the task report. The cumulative report accrues in `graphify-out/build-bench/REPORT.md`.

### Task 2.1: Move pure `retrieval.rs` from vox-db into the vox-db-types L0 leaf

Shrinks vox-db's churn surface (the #1 blast-radius crate). `retrieval.rs` is pure data (`serde` only), so it belongs in the leaf; no current dependent can drop vox-db (all hold a live `VoxDb` handle), but moving the type lets future consumers depend only on the leaf.

**Files:**
- Create (move): `crates/vox-db-types/src/retrieval.rs`
- Modify: `crates/vox-db-types/src/lib.rs` (declare + re-export)
- Modify: `crates/vox-db/src/lib.rs` (drop `pub mod retrieval;`, re-export from the leaf for back-compat)
- Modify: `crates/vox-db/Cargo.toml` / `crates/vox-db-types/Cargo.toml` if `retrieval.rs` needs a dep the leaf lacks

> *Sonnet note: FIRST `Read crates/vox-db/src/retrieval.rs` in full and `Grep` every `use .*retrieval` / `retrieval::` reference across `crates/` to know all call sites. The type is currently reached as `vox_db::retrieval::…`. Keep that path working via a re-export (`pub use vox_db_types::retrieval;` in vox-db) so no downstream edit is needed. If retrieval.rs imports anything beyond `serde` (Grep its `use` lines), that dep must exist in vox-db-types/Cargo.toml — add it only if missing.*

- [ ] **Step 1: Read the file + all call sites**

```bash
# read it
# then enumerate references:
```
Run `Grep -rn "retrieval::" crates/ --glob '*.rs'` and `Grep -rn "mod retrieval" crates/vox-db/`. Confirm the only declaration is `vox-db/src/lib.rs:132` (`pub mod retrieval;`). List every external reference in the task report.

- [ ] **Step 2: Move the file**

```bash
git mv crates/vox-db/src/retrieval.rs crates/vox-db-types/src/retrieval.rs
```

- [ ] **Step 3: Declare it in the leaf + re-export from vox-db**

In `crates/vox-db-types/src/lib.rs`, add `pub mod retrieval;` (match the file's existing module-declaration style — Read it first).
In `crates/vox-db/src/lib.rs`, replace `pub mod retrieval;` (line ~132) with:
```rust
// retrieval types live in the vox-db-types L0 leaf; re-exported for back-compat.
pub use vox_db_types::retrieval;
```
Confirm `vox-db/Cargo.toml` already depends on `vox-db-types` (the verified findings show it does, line ~? — Grep `vox-db-types` in it).

- [ ] **Step 4: Build the affected crates**

```bash
cargo check -p vox-db-types
cargo check -p vox-db
cargo check -p vox-cli
```
Expected: all compile. If a call site breaks (`retrieval::X` not found), the re-export is wrong — fix the re-export, do NOT edit the call site. If `retrieval.rs` needed a non-serde dep, add it to `vox-db-types/Cargo.toml` and re-run.

- [ ] **Step 5: Measure + commit**

```bash
cargo check --workspace
cargo run -q -p vox-cli -- ci build-bench --repeat 3 --compare contracts/ci/build-bench-baseline.v1.json --label "Phase 2.1: retrieval->leaf"
```
Paste the delta (watch `blastradius_vox_db_to_cli` and `check_vox_db`). Then:
```bash
git add crates/vox-db-types/ crates/vox-db/
git commit -m "refactor(vox-db): move pure retrieval types into vox-db-types L0 leaf

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 2.2: Feature-gate vox-sql SQL backends (libsql default; pg/mysql opt-in)

`crates/vox-sql/Cargo.toml:17` hard-codes `sqlx` features `postgres` + `mysql`, compiling both unconditionally. Gate them.

**Files:**
- Modify: `crates/vox-sql/Cargo.toml`
- Modify: any `vox-sql` source guarded behind the backends (cfg the pg/mysql code paths)

> *Sonnet note: Read `crates/vox-sql/Cargo.toml` AND grep the crate's src for `postgres`/`mysql`/`Postgres`/`MySql` usage to know what code must go behind `#[cfg(feature = "postgres")]` / `#[cfg(feature = "mysql")]`. Removing the feature from sqlx without cfg-gating the code that uses it will FAIL to compile — gate both together. If the crate uses these backends unconditionally in a way that can't be cleanly gated, STOP and report; do not delete working functionality to win a build-time number.*

- [ ] **Step 1: Read Cargo.toml + grep backend usage**

`Read crates/vox-sql/Cargo.toml`; `Grep -rn "Postgres\|MySql\|postgres\|mysql" crates/vox-sql/src/`. List what depends on each backend.

- [ ] **Step 2: Add a `[features]` table + gate sqlx**

In `crates/vox-sql/Cargo.toml`, the verified real line (`:17`) is:
```toml
sqlx = { version = "0.9.0", default-features = false, features = ["runtime-tokio", "postgres", "mysql"] }
```
It is **not** a workspace dep and already sets `default-features = false`, so only the `features` list changes — drop `postgres`/`mysql`, keep `runtime-tokio` — plus add a `[features]` table that re-enables them on demand:
```toml
[features]
default = []
postgres = ["sqlx/postgres"]
mysql = ["sqlx/mysql"]

[dependencies]
# was: features = ["runtime-tokio", "postgres", "mysql"]
sqlx = { version = "0.9.0", default-features = false, features = ["runtime-tokio"] }
```

- [ ] **Step 3: cfg-gate the backend code**

Wrap pg/mysql-specific code (from Step 1) in `#[cfg(feature = "postgres")]` / `#[cfg(feature = "mysql")]`. Build with and without:
```bash
cargo check -p vox-sql
cargo check -p vox-sql --features postgres,mysql
```
Expected: both compile. Then confirm no other workspace crate silently relied on vox-sql always having pg/mysql: `cargo check --workspace` must pass (if a dependent needs a backend, it must now enable the feature — add it to that dependent's `vox-sql = { …, features = ["postgres"] }`).

- [ ] **Step 4: Measure + commit**

```bash
cargo check --workspace
cargo run -q -p vox-cli -- ci build-bench --repeat 3 --compare contracts/ci/build-bench-baseline.v1.json --label "Phase 2.2: vox-sql backend gating"
```
Paste the delta (watch `check_vox_sql`). Then:
```bash
git add crates/vox-sql/
git commit -m "perf(vox-sql): feature-gate postgres/mysql backends (libsql default)

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 2.3: Drop `news-publish` from vox-orchestrator-mcp default features

`crates/vox-orchestrator-mcp/Cargo.toml:9` has `default = ["news-publish", "toestub-gate", "json-schema"]`, pulling `vox-publisher` (~heavy) into every default build of the slowest single crate.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml`
- Modify: any dependent that relied on `news-publish` being on by default

> *Sonnet note: Read the `[features]` table fully. Confirm `news-publish` gates the vox-publisher dependency (Grep `news-publish` across the crate's src + Cargo.toml). Dropping it from default must not break code that is NOT itself cfg-gated. If `news-publish` code is unconditional, you must also cfg-gate it. Anything that genuinely needs news-publish (a dependent crate, a binary, CI) must now opt in — find those via `Grep -rn "vox-orchestrator-mcp" crates/**/Cargo.toml` and add `features = ["news-publish"]` where required.*

- [ ] **Step 1: Read + map the feature**

`Read crates/vox-orchestrator-mcp/Cargo.toml`; `Grep -rn "news-publish\|news_publish" crates/vox-orchestrator-mcp/`. Identify every consumer.

- [ ] **Step 2: Remove from default; keep as opt-in**

Change line 9 to `default = ["toestub-gate", "json-schema"]` (keep `news-publish` defined in `[features]`, just not in `default`).

- [ ] **Step 3: Restore opt-in where needed**

```bash
cargo check -p vox-orchestrator-mcp
cargo check -p vox-orchestrator-mcp --features news-publish
cargo check --workspace
```
Expected: all pass. If `--workspace` fails because a dependent or binary needed news-publish, add `features = ["news-publish"]` to that crate's `vox-orchestrator-mcp` dependency line (not to the default).

- [ ] **Step 4: Measure + commit**

```bash
cargo check --workspace
cargo run -q -p vox-cli -- ci build-bench --repeat 3 --compare contracts/ci/build-bench-baseline.v1.json --label "Phase 2.3: mcp drop news-publish default"
```
Paste the delta (watch `check_vox_mcp`). Then:
```bash
git add crates/vox-orchestrator-mcp/ crates/
git commit -m "perf(orchestrator-mcp): drop news-publish from default features (opt-in)

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

### Task 2.4: Gate vox-audit's eight `cr-*` bins behind a `ci-gates` feature

`crates/vox-audit/Cargo.toml` has no `[features]`; all 9 `[[bin]]` entries (vox-audit + cr-e1/a1/d3/a4/a2/e2/p1/p2) build unconditionally. Gate the eight `cr-*` criteria bins behind `ci-gates` so a normal `cargo check -p vox-audit` (and anything that builds the workspace) doesn't compile all nine binaries.

**Files:**
- Modify: `crates/vox-audit/Cargo.toml`

> *Sonnet note: Read the `[[bin]]` entries (lines ~39-92). Add `required-features = ["ci-gates"]` to the EIGHT `cr-*` bins only; leave the primary `vox-audit` bin ungated. Add a `[features]` table with `ci-gates = []` (and `default = []`). Then anything in CI that runs the cr-* gates must build with `--features ci-gates` — Grep `.github/workflows/` and `scripts/` for `cr-e1`/`cr-a1`/etc. invocations and add the feature there. If a cr-* bin shares code with the main lib that won't compile without the feature, gate that code too. Do not delete the bins.*

- [ ] **Step 1: Read the bins + find their CI/script callers**

`Read crates/vox-audit/Cargo.toml`; `Grep -rn "cr-e1\|cr-a1\|cr-d3\|cr-a4\|cr-a2\|cr-e2\|cr-p1\|cr-p2" .github/ scripts/ contracts/`. List every place the gated bins are invoked.

- [ ] **Step 2: Add the feature + gate the eight bins**

In `crates/vox-audit/Cargo.toml`, add:
```toml
[features]
default = []
ci-gates = []
```
and add `required-features = ["ci-gates"]` to each of the eight `cr-*` `[[bin]]` blocks (not the `vox-audit` bin).

- [ ] **Step 3: Restore the gated build where the bins are used**

```bash
cargo check -p vox-audit                      # builds only the primary bin now
cargo build -p vox-audit --features ci-gates  # builds all nine
```
Expected: both pass. Update every CI/script caller from Step 1 that runs a `cr-*` bin to pass `--features ci-gates` (or build with it). Confirm `cargo check --workspace` is unaffected.

- [ ] **Step 4: Measure + commit**

```bash
cargo check --workspace
cargo run -q -p vox-cli -- ci build-bench --repeat 3 --compare contracts/ci/build-bench-baseline.v1.json --label "Phase 2.4: vox-audit ci-gates"
```
Paste the delta (watch `check_vox_audit`). Then:
```bash
git add crates/vox-audit/ .github/ scripts/
git commit -m "perf(vox-audit): gate cr-* criteria bins behind ci-gates feature

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Phase 3 — Slowest-unit split (measured)

### Task 3.1: Reduce the incremental cost of the slowest crate

`vox-orchestrator-mcp` (63.6s self-time, over its 40k LoC budget) is the slowest single unit. The `llm_bridge/` extraction already landed; identify the next-largest cohesive module and extract it to a sibling crate OR behind a feature, reducing the incremental recompile when unrelated mcp code changes.

**Files:**
- Modify/Create: depends on the chosen split (a new `crates/vox-orchestrator-mcp-<area>/` or a feature gate)

> *Sonnet note: this task is data-driven — do NOT guess what to split. First run `cargo run -q -p vox-cli -- run --mode interp scripts/crate-build-audit.vox` and read the LoC/module breakdown, and `Grep` the mcp `src/` module tree (`ls crates/vox-orchestrator-mcp/src/`) to find the largest independent module after llm_bridge. A clean split is one with few inbound edges from the rest of mcp. If no clean split exists (everything is tightly coupled), the correct outcome is to gate a heavy optional area behind a feature instead — and if even that isn't clean, REPORT that the crate needs a dedicated refactor plan and do NOT force a messy split to chase a number. A measured −0 with an honest "no clean split available" note is the correct deliverable in that case.*

- [ ] **Step 1: Identify the split candidate**

```bash
ls crates/vox-orchestrator-mcp/src/
cargo run -q -p vox-cli -- run --mode interp scripts/crate-build-audit.vox
```
From the module sizes + your read of the `src/` tree, name the largest cohesive, low-inbound-edge module. Document the candidate and its inbound edge count in the task report **before** moving anything.

- [ ] **Step 2: Extract behind a feature (lower-risk than a new crate)**

Prefer gating the heavy area behind a feature in `vox-orchestrator-mcp/Cargo.toml` (`default` includes it; PR-time `cargo check` without the feature skips it). If a new crate is clearly cleaner and the inbound edges are few, create `crates/vox-orchestrator-mcp-<area>/` and move the module, updating `mod`/`use` paths. Follow the layers.toml + arch-check rules (a new crate needs a `[crates]` entry in `docs/src/architecture/layers.toml` and a `where-things-live.md` row).

- [ ] **Step 3: Build + arch-check**

```bash
cargo check -p vox-orchestrator-mcp
cargo check --workspace
cargo run -q -p vox-arch-check     # must pass (LoC budget, orphans, layers)
```
Expected: all pass; if you created a crate, arch-check's orphan rule requires it have ≥1 in-tree consumer — wire it.

- [ ] **Step 4: Measure + commit**

```bash
cargo check --workspace
cargo run -q -p vox-cli -- ci build-bench --repeat 3 --compare contracts/ci/build-bench-baseline.v1.json --label "Phase 3.1: mcp slowest-unit split"
```
Paste the delta (watch `check_vox_mcp`). Then commit with an accurate message describing the actual split performed (with trailer). **If no clean split was possible, commit nothing and record the "no clean split" finding in the task report instead.**

---

## Phase 4 — Reserved (data-driven, optional)

Do not pre-write tasks here. After Phases 2–3, re-read `graphify-out/build-bench/REPORT.md` and `CRATE_BUILD_AUDIT.md`. If a single crate still dominates the deltas, add one targeted task mirroring Phase 3's shape. Otherwise skip to Phase 5. This phase exists to prevent speculative splitting — only act on what the measurements show.

---

## Phase 5 — Affected-crate selective CI (the biggest PR-time win), measured

### Task 5.1: Execute + measure the selective-CI sub-plan

The largest pileup/build-cost driver at PR time is `ci.yml` rebuilding the full 110-crate workspace on every push. That work is fully specified in a dedicated plan — execute it, then measure the wall-clock win.

**Files:** as specified in [`docs/superpowers/plans/2026-06-15-affected-crate-selective-ci.md`](2026-06-15-affected-crate-selective-ci.md).

- [ ] **Step 1: Execute the selective-CI plan end to end**

Open and follow `docs/superpowers/plans/2026-06-15-affected-crate-selective-ci.md` (Phases 1–5 there). It is self-contained, TDD, and carries its own guardrails. **Do not duplicate or re-derive it here.** Honor its **Phase 4 shadow-mode acceptance gate** before flipping its Phase 5 — that gate is the safeguard against a false-green CI.

- [ ] **Step 2: Measure the PR-time delta**

The selective-CI plan's shadow mode already produces the data: for a representative leaf PR, the affected-only `tests`/`lints`/`audits` wall-clock vs the full `--workspace` run. Record both from the GitHub Actions job timings (the `setup`-emitted `affected_crates` and the job durations). Report: full-workspace PR minutes → affected-only PR minutes, as a percentage. This is the program's headline number.

- [ ] **Step 3: No separate commit** — the selective-CI plan commits its own tasks. Just record the measured PR-time delta in your task report and carry it into Phase 6.

---

## Phase 6 — Aggregate & record

### Task 6.1: Headline log + SSOT doc

**Files:**
- Modify: `docs/src/architecture/build-time-log.md` (curated headline table)
- Create: `docs/src/ci/build-time-program.md` (SSOT for the harness)

> *Sonnet note: both files live under `docs/src/`, so each needs YAML frontmatter (`title`/`description`/`category`) as its first lines or the pre-push doc-pipeline blocks the push. `build-time-log.md` already has frontmatter — append a new dated phase section, do NOT add a second frontmatter block. `build-time-program.md` is new — write frontmatter first.*

- [ ] **Step 1: Append the measured results to the curated log**

In `docs/src/architecture/build-time-log.md`, add a section `## 2026-06-15 — Build-Time Reduction Program` with one row per phase, pulling the real numbers from `graphify-out/build-bench/REPORT.md` and the Phase 5 CI delta:

```markdown
## 2026-06-15 — Build-Time Reduction Program

| Phase | Change | Scenario | Δ% (measured) |
|---|---|---|---|
| 2.1 | retrieval → vox-db-types leaf | blastradius_vox_db_to_cli | <fill from REPORT.md> |
| 2.2 | vox-sql backend gating | check_vox_sql | <fill> |
| 2.3 | mcp drop news-publish default | check_vox_mcp | <fill> |
| 2.4 | vox-audit ci-gates | check_vox_audit | <fill> |
| 3.1 | mcp slowest-unit split | check_vox_mcp | <fill> |
| 5.1 | affected-crate selective CI | PR-time wall-clock | <fill from CI job timings> |
```

Use the ACTUAL measured values. If any is `PENDING-CI`, leave it and note that the CI artifact fills it.

- [ ] **Step 2: Write the SSOT doc**

Create `docs/src/ci/build-time-program.md`:

```markdown
---
title: Build-Time Reduction Program
description: How Vox build times are measured (vox ci build-bench), the committed scenario set + baseline, the cycle/coupling guards (vox ci dep-cycles), and how each optimization phase reports its delta.
category: "CI & Quality"
---

# Build-Time Reduction Program

## Instruments
- **`vox ci build-bench`** — runs `contracts/ci/build-bench-scenarios.v1.json`, writes a snapshot, and `--compare`s against `contracts/ci/build-bench-baseline.v1.json` to emit a per-scenario delta. Cumulative report: `graphify-out/build-bench/REPORT.md`.
- **`vox ci dep-cycles`** — Tarjan SCC over `cargo metadata`. HARD-fails on normal-dependency cycles; inventories dev-dep back-edges to `graphify-out/DEP_CYCLES.md`. (Fills the gap that `vox-arch-check` does only pairwise layer-ordering with no cycle detection.)
- **`scripts/crate-build-audit.vox`** — the dependency/blast-radius map (fan-in, LoC, self-time) → `graphify-out/crate_audit.json` + `CRATE_BUILD_AUDIT.md`.

## Refreshing the baseline
After an intentional, accepted change to build cost, regenerate:
`vox ci build-bench --label baseline --write contracts/ci/build-bench-baseline.v1.json` (warm cache first), and commit. Adding/removing a scenario means editing `build-bench-scenarios.v1.json` and regenerating the baseline in the same PR.

## The soundness backstop
PR-time selective CI (see [affected-crate-selective-ci](affected-crate-selective-ci.md)) builds only affected crates; the merge-queue gate + nightly run the full `--workspace`. No build-time optimization here weakens that backstop.
```

- [ ] **Step 3: Validate + commit**

```bash
cargo run -q -p vox-cli -- ci ssot-drift
git add docs/src/architecture/build-time-log.md docs/src/ci/build-time-program.md
git commit -m "docs(ci): record measured build-time program deltas + harness SSOT

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec coverage** (requester's asks → tasks):

| Ask | Phase/Task |
|---|---|
| "faster build times **as measured and reported phase by phase**" | Phase 0 builds the instrument (`build-bench` + committed baseline + CI artifact); every Phase-2/3/5 task ends with a `--compare` delta; Phase 6 aggregates. ✅ |
| "**cyclical** dependency issues" | Phase 1: new Tarjan SCC `dep-cycles` (arch-check has none), HARD gate on normal cycles + dev back-edge inventory. ✅ |
| "**intercrate** dependency issues" | Phase 1 inventory + blast-radius map (Task 0.1); Phase 2 re-lands the four blast-radius/feature-gating reductions; Phase 3 slowest-unit split. ✅ |
| "Scope **that** [selective CI] as well" | Phase 5 executes + measures the existing selective-CI sub-plan (referenced, not duplicated). ✅ |
| "given [Sonnet 4.6's] limitations" | Dedicated guardrails section; every new API verified against real source with file:line; measurement honesty rules; "run in CI if you can't build locally" path. ✅ |

No ask is left without a task.

**2. Placeholder scan:** No `TBD`/`TODO`/"add error handling". The `<fill from REPORT.md>` cells in Task 6.1 are intentional — they are filled with the *measured* values produced by the plan's own earlier steps, and the step says so explicitly. Phase 4 is deliberately empty (data-driven) with an explicit rule against speculative work — that is a design choice, not a placeholder. Config/YAML tasks (0.5, 1.4) substitute a documented `ssot-drift`/CI verification for a unit test, stated at each.

**3. Type/name consistency:**
- `Scenario{id,touch,args}` (no `kind` — the touch-before-each-run model unifies the two old kinds), `ScenarioFile{schema_version,scenarios}`, `BenchRecord{id,ok,wall_ms}`, `Snapshot{schema_version,label,records}`, `DeltaRow{id,base_ms,new_ms,delta_ms,pct}` — defined in Task 0.2, consumed identically in 0.3 and the tests. `run_scenario(&Path,&Scenario,repeat:u32)->BenchRecord` and `run_build_bench(&Path,Option<String>,Option<String>,Option<String>,repeat:u32)->Result<()>` thread the `repeat` arg consistently from the `BuildBench{label,write,compare,repeat}` enum variant (0.4) through the dispatch arm. `compute_deltas(&Snapshot,&Snapshot)->Vec<DeltaRow>` and `format_delta_markdown(&str,&[DeltaRow])->String` signatures match across def, runner, and tests.
- `cycles(&BTreeMap<String,Vec<String>>)->Vec<Vec<String>>` (Task 1.1) and `adjacency_from_metadata(&Value, include_nonlink: bool)->BTreeMap<String,Vec<String>>` (Task 1.2) — consumed by `run_dep_cycles` (Task 1.3) with matching types; the link-time/non-link split uses `include_nonlink` consistently (false = normal deps only for the HARD gate; true = +dev +build for the inventory).
- New CLI variants `BuildBench{label,write,compare}` + `DepCycles` (cmd_enums.rs) ↔ dispatch arms (run_body.rs) ↔ `run_build_bench(&root,label,write,compare)` / `run_dep_cycles(&root)` — names align.
- `super::cargo_bin()` used in both new modules — verified `pub(super)` at `ci/mod.rs:88`.
- Scenario ids in `build-bench-scenarios.v1.json` (`check_vox_db`, `check_vox_sql`, `check_vox_mcp`, `check_vox_audit`, `check_vox_cli`, `blastradius_vox_db_to_cli`) ↔ the crates optimized in Phase 2 ↔ the rows in Task 6.1's table — consistent. All six `touch` paths (`crates/*/src/lib.rs`) verified present 2026-06-15.

No inconsistencies found.

**Open verification items for the executor** (flagged at point of use): (1) `File::set_modified` toolchain availability (Task 0.3 note); (2) the exact `sqlx` feature wiring — workspace-level vs crate-level (Task 2.2 note); (3) whether dropping `news-publish`/gating `cr-*` bins requires opt-in restoration in CI/scripts (Tasks 2.3/2.4 grep steps); (4) whether a clean mcp split exists at all (Task 3.1 — honest "no split" is an allowed outcome).

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-15-build-time-program-measured-phased.md`. Two execution options:

**1. Subagent-Driven (recommended)** — fresh subagent per task, two-stage review. Phase 0–1 (the instruments) are pure Rust with zero CI risk and should land first; Phases 2–4 each carry their own measured delta so review is data-driven; Phase 5 is the referenced sub-plan.

**2. Inline Execution** — execute in this session with checkpoints.

Which approach? (Recommendation: land **Phase 0 first and commit the baseline** before any optimization — without the committed `build-bench-baseline.v1.json`, no later phase can produce the measured deltas the requester asked for.)
