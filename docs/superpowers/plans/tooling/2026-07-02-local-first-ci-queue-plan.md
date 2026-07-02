# Local-First CI Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the local runner fleet the default CI plane mechanically: a `vox ci queue` SSOT signal, auto-clear of superseded/stale runs on the autoscaler tick, hooks that block remote check-watching and inject queue state, and a concurrency guard that keeps flood-at-source protection unregressable.

**Architecture:** A new run-centric `queue.rs` module beside `runner_scale.rs` (reusing its `gh_json`/`REPO_SLUG` plumbing) provides snapshot + classification + clearing; the existing 2-minute autoscaler tick calls it (no new scheduler). Enforcement is a PreToolUse hook (`vox ci queue --hook-guard`, pure local match, exit 2 to block) plus a SessionStart snapshot injection, both via a new checked-in `.claude/settings.json`. A new Tier-1 guard in `vox-cli-ci` mirrors `runner_policy_check.rs`.

**Tech Stack:** Rust (vox-cli, vox-cli-ci), `gh` CLI with `--jq`, Claude Code hooks, GitHub Actions `concurrency` groups.

**Spec:** `docs/superpowers/specs/2026-07-02-local-first-ci-queue-design.md`

**House rules that apply here:** never `cargo fmt --all` (use `cargo fmt -p <crate>`); never pipe cargo output to `head`/`grep` (redirect to a file if needed); `clippy --workspace` must `--exclude vox-gui`.

---

### Task 1: `queue.rs` pure core — types, parsing, classification, advice

**Files:**
- Create: `crates/vox-cli/src/commands/ci/queue.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (add `pub mod queue;` alongside the existing module list)
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs:231` and `runner_scale.rs` `gh_json` (visibility only)

- [ ] **Step 1: Make the shared helpers reachable**

In `runner_scale.rs`, change two private functions to `pub(crate)` (no body changes):

```rust
pub(crate) fn now_secs() -> i64 {
```

```rust
pub(crate) fn gh_json(args: &[&str]) -> Result<String> {
```

- [ ] **Step 2: Write the failing tests**

Create `crates/vox-cli/src/commands/ci/queue.rs` with module doc, types, function stubs `todo!()`-free — write the tests first at the bottom and leave the functions unimplemented so the compile fails, or (house-pragmatic) write tests against the signatures below and implement in Step 4. Tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: u64, wf: &str, br: &str, ev: &str, st: &str, created: i64, now: i64) -> QueueRun {
        parse_run_line(
            &format!("{id}\t{wf}\t{br}\t{ev}\t{created}\t{st}"),
            now,
        )
        .unwrap()
    }

    #[test]
    fn parse_run_line_roundtrip() {
        let r = run(42, "CI", "feat/x", "push", "queued", 1000, 1600);
        assert_eq!(r.id, 42);
        assert_eq!(r.workflow, "CI");
        assert_eq!(r.branch, "feat/x");
        assert_eq!(r.age_secs, 600);
        assert!(!r.exempt);
        assert!(parse_run_line("garbage", 0).is_none());
        assert!(parse_run_line("1\tCI\tonly-three", 0).is_none());
    }

    #[test]
    fn exemptions() {
        assert!(is_exempt("main", "push"));
        assert!(is_exempt("feat/x", "merge_group"));
        assert!(is_exempt("feat/x", "schedule"));
        assert!(is_exempt("feat/x", "workflow_dispatch"));
        assert!(!is_exempt("feat/x", "push"));
        assert!(!is_exempt("feat/x", "pull_request"));
    }

    #[test]
    fn superseded_newest_survives() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "CI", "feat/x", "push", "queued", 1000, now),
            run(2, "CI", "feat/x", "push", "in_progress", 2000, now),
            run(3, "CI", "feat/x", "push", "queued", 3000, now),
            run(4, "CI", "feat/y", "push", "queued", 1000, now),
        ];
        classify_runs(&mut runs, 3600 * 24); // huge TTL: isolate superseded logic
        assert_eq!(runs[0].class, RunClass::Superseded);
        assert_eq!(runs[1].class, RunClass::Superseded); // in_progress can be superseded too
        assert_eq!(runs[2].class, RunClass::Active);
        assert_eq!(runs[3].class, RunClass::Active); // different branch key
    }

    #[test]
    fn superseded_ties_and_exempt() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "CI", "feat/x", "push", "queued", 2000, now),
            run(2, "CI", "feat/x", "push", "queued", 2000, now), // equal created: no supersede
            run(3, "CI", "main", "push", "queued", 1000, now),
            run(4, "CI", "main", "push", "queued", 9000, now), // exempt: never superseded
        ];
        classify_runs(&mut runs, 3600 * 24);
        assert_eq!(runs[0].class, RunClass::Active);
        assert_eq!(runs[1].class, RunClass::Active);
        assert_eq!(runs[2].class, RunClass::Active);
        assert!(runs[2].exempt && runs[3].exempt);
    }

    #[test]
    fn stale_ttl_boundary() {
        let now = 10_000;
        let ttl = 2700; // 45 min
        let mut runs = vec![
            run(1, "CI", "feat/x", "push", "queued", now - ttl, now),     // exactly TTL: not stale
            run(2, "CI", "feat/y", "push", "queued", now - ttl - 1, now), // past TTL: stale
            run(3, "CI", "feat/z", "push", "in_progress", 0, now),        // in_progress: never stale
            run(4, "CI", "main", "push", "queued", 0, now),               // exempt: never stale
        ];
        classify_runs(&mut runs, ttl);
        assert_eq!(runs[0].class, RunClass::Active);
        assert_eq!(runs[1].class, RunClass::Stale);
        assert_eq!(runs[2].class, RunClass::Active);
        assert_eq!(runs[3].class, RunClass::Active);
    }

    #[test]
    fn advice_phrasings() {
        assert!(advice_for(3, 4, 0, 0, false).contains("healthy"));
        let over = advice_for(14, 4, 9, 3, false);
        assert!(over.contains("vox ci queue --clear"));
        assert!(over.contains("9 superseded"));
        assert!(over.contains("3 stale"));
        assert!(advice_for(0, 4, 0, 0, true).contains("degraded"));
        assert!(advice_for(0, 4, 0, 0, true).contains("local gates"));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-cli commands::ci::queue 2> ..\..\target-test-queue.log` (or without redirect; just never pipe to head/grep)
Expected: compile error — types/functions not defined.

- [ ] **Step 4: Implement the core**

Full module head + implementation in `queue.rs`:

```rust
//! `vox ci queue` — run-centric CI queue snapshot, classification, and clearing.
//!
//! The SSOT queue signal for the local-first CI contract
//! (docs/superpowers/specs/2026-07-02-local-first-ci-queue-design.md):
//! agents verify with local gates, never watch remote checks; this command is
//! the only sanctioned way to read (`--json`/`--brief`) or clear (`--clear`)
//! the GitHub Actions queue the fleet consumes. `--hook-guard` is the
//! PreToolUse enforcement mode. Run-level only — the autoscaler's job-label
//! demand counting stays in `runner_scale.rs`.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use super::constants::REPO_SLUG;
use super::runner_scale::{gh_json, now_secs};

pub const DEFAULT_TTL_MINS: i64 = 45;
/// `--from-snapshot` refuses snapshots older than this (autoscaler ticks ~2 min).
const SNAPSHOT_STALE_SECS: i64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunClass {
    Active,
    Superseded,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueRun {
    pub id: u64,
    pub workflow: String,
    pub branch: String,
    pub event: String,
    /// `queued` | `in_progress`
    pub status: String,
    pub created_epoch: i64,
    pub age_secs: i64,
    pub class: RunClass,
    pub exempt: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub generated_at: i64,
    /// True when gh was unreachable / data is partial.
    pub degraded: bool,
    pub queued: u32,
    pub in_progress: u32,
    pub superseded: u32,
    pub stale: u32,
    pub fleet_alive: u32,
    pub fleet_max: u32,
    /// THE machine-readable signal: always present, tells the reader what to do.
    pub advice: String,
    pub runs: Vec<QueueRun>,
}

/// Main-branch, merge-queue, scheduled, and manually dispatched runs are never
/// classified for cancellation.
pub fn is_exempt(branch: &str, event: &str) -> bool {
    branch == "main" || matches!(event, "merge_group" | "schedule" | "workflow_dispatch")
}

/// One tab-separated line from the gh --jq template:
/// `id \t workflow \t branch \t event \t created_epoch \t status`.
pub fn parse_run_line(line: &str, now: i64) -> Option<QueueRun> {
    let mut p = line.split('\t');
    let id = p.next()?.trim().parse().ok()?;
    let workflow = p.next()?.trim().to_string();
    let branch = p.next()?.trim().to_string();
    let event = p.next()?.trim().to_string();
    let created_epoch: i64 = p.next()?.trim().parse().ok()?;
    let status = p.next()?.trim().to_string();
    let exempt = is_exempt(&branch, &event);
    Some(QueueRun {
        id,
        workflow,
        branch,
        event,
        status,
        created_epoch,
        age_secs: now.saturating_sub(created_epoch),
        class: RunClass::Active,
        exempt,
    })
}

/// Superseded: a strictly newer non-exempt run exists for the same
/// (workflow, branch) — only the newest run per key survives. Stale: still
/// `queued` past the TTL. Exempt runs are always Active.
pub fn classify_runs(runs: &mut [QueueRun], ttl_secs: i64) {
    for i in 0..runs.len() {
        if runs[i].exempt {
            runs[i].class = RunClass::Active;
            continue;
        }
        let newer_exists = runs.iter().any(|o| {
            !o.exempt
                && o.id != runs[i].id
                && o.workflow == runs[i].workflow
                && o.branch == runs[i].branch
                && o.created_epoch > runs[i].created_epoch
        });
        runs[i].class = if newer_exists {
            RunClass::Superseded
        } else if runs[i].status == "queued" && runs[i].age_secs > ttl_secs {
            RunClass::Stale
        } else {
            RunClass::Active
        };
    }
}

/// The advice string every LLM reads. Three shapes: healthy, over-capacity
/// (names the exact clear command + counts), degraded (says how to proceed).
pub fn advice_for(
    active_queued: u32,
    capacity: u32,
    superseded: u32,
    stale: u32,
    degraded: bool,
) -> String {
    if degraded {
        return "degraded: gh unreachable or partial data; do not retry-loop — \
                proceed on local gates (`vox ci pre-push`) and try `vox ci queue` later"
            .to_string();
    }
    if superseded + stale > 0 {
        return format!(
            "queued {active_queued} vs capacity {capacity}: run 'vox ci queue --clear' \
             (would cancel {superseded} superseded + {stale} stale)"
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
```

- [ ] **Step 5: Register the module**

In `crates/vox-cli/src/commands/ci/mod.rs`, add in alphabetical position among the existing `mod` lines:

```rust
pub mod queue;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p vox-cli commands::ci::queue`
Expected: 6 tests PASS.

- [ ] **Step 7: Clippy + commit**

Run: `cargo clippy -p vox-cli --all-targets -- -D warnings`
Expected: clean (module is not yet wired to a command; `#[allow(dead_code)]` is NOT needed because everything is `pub`).

```bash
git add crates/vox-cli/src/commands/ci/queue.rs crates/vox-cli/src/commands/ci/mod.rs crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): vox ci queue core — run classification + advice signal"
```

---

### Task 2: live fetch, snapshot file, rendering, CLI wiring

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/queue.rs` (append)
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (new `Queue` variant, place after `RunnerStatus` around line 838)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (dispatch arm near line 555)

- [ ] **Step 1: Write the failing snapshot round-trip test**

Append to the `tests` module in `queue.rs`:

```rust
    #[test]
    fn snapshot_roundtrip_and_render() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "CI", "feat/x", "push", "queued", 1000, now),
            run(2, "CI", "feat/x", "push", "queued", 3000, now),
        ];
        classify_runs(&mut runs, 2700);
        let snap = build_snapshot(runs, 2, 4, false, now);
        assert_eq!(snap.queued, 2);
        assert_eq!(snap.superseded, 1);
        let json = serde_json::to_string(&snap).unwrap();
        let back: QueueSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.superseded, 1);
        assert_eq!(back.advice, snap.advice);
        let brief = render_brief(&back);
        assert!(brief.contains("advice:"));
        assert!(brief.lines().count() <= 5);
    }

    #[test]
    fn snapshot_staleness() {
        assert!(!snapshot_is_stale(1000, 1000 + SNAPSHOT_STALE_SECS));
        assert!(snapshot_is_stale(1000, 1000 + SNAPSHOT_STALE_SECS + 1));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-cli commands::ci::queue`
Expected: compile error — `build_snapshot`, `render_brief`, `snapshot_is_stale` not defined.

- [ ] **Step 3: Implement fetch/snapshot/render**

Append to `queue.rs`:

```rust
/// Fetch queued + in-progress runs. Run-level fields only — no per-run jobs
/// fan-out (classification does not need job labels; the autoscaler's demand
/// counter keeps that concern). `fromdateiso8601` makes gh's jq emit epoch
/// seconds so no date parsing happens in Rust.
pub fn fetch_runs(now: i64) -> Result<Vec<QueueRun>> {
    let mut runs = Vec::new();
    for status in ["queued", "in_progress"] {
        let raw = gh_json(&[
            "api",
            &format!("repos/{REPO_SLUG}/actions/runs?status={status}&per_page=100"),
            "--jq",
            ".workflow_runs[]|\"\\(.id)\\t\\(.name)\\t\\(.head_branch)\\t\\(.event)\\t\\(.created_at|fromdateiso8601)\\t\\(.status)\"",
        ])?;
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            if let Some(r) = parse_run_line(line, now) {
                runs.push(r);
            }
        }
    }
    Ok(runs)
}

pub fn build_snapshot(
    runs: Vec<QueueRun>,
    fleet_alive: u32,
    fleet_max: u32,
    degraded: bool,
    now: i64,
) -> QueueSnapshot {
    let queued = runs.iter().filter(|r| r.status == "queued").count() as u32;
    let in_progress = runs.iter().filter(|r| r.status == "in_progress").count() as u32;
    let superseded = runs.iter().filter(|r| r.class == RunClass::Superseded).count() as u32;
    let stale = runs.iter().filter(|r| r.class == RunClass::Stale).count() as u32;
    let active_queued = runs
        .iter()
        .filter(|r| r.status == "queued" && r.class == RunClass::Active)
        .count() as u32;
    let advice = advice_for(active_queued, fleet_max, superseded, stale, degraded);
    QueueSnapshot {
        generated_at: now,
        degraded,
        queued,
        in_progress,
        superseded,
        stale,
        fleet_alive,
        fleet_max,
        advice,
        runs,
    }
}

fn snapshot_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-queue-snapshot.json")
}

pub fn write_snapshot(snap: &QueueSnapshot) -> Result<()> {
    let p = snapshot_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&p, serde_json::to_vec_pretty(snap)?)
        .with_context(|| format!("write {}", p.display()))
}

pub fn snapshot_is_stale(generated_at: i64, now: i64) -> bool {
    now.saturating_sub(generated_at) > SNAPSHOT_STALE_SECS
}

/// ≤5 lines; stdout of the SessionStart hook, injected into agent context.
pub fn render_brief(snap: &QueueSnapshot) -> String {
    format!(
        "CI queue (local-first contract: local gates = verdict; never watch remote checks):\n\
         queued {} / in-progress {} (superseded {}, stale {}); fleet {}/{}\n\
         advice: {}\n\
         commands: `vox ci queue --json` | `vox ci queue --clear`",
        snap.queued, snap.in_progress, snap.superseded, snap.stale,
        snap.fleet_alive, snap.fleet_max, snap.advice
    )
}

fn render_table(snap: &QueueSnapshot) -> String {
    let mut out = String::new();
    out.push_str("RUN_ID      AGE_MIN  CLASS       EVENT             BRANCH                    WORKFLOW\n");
    for r in &snap.runs {
        let class = match (r.exempt, r.class) {
            (true, _) => "exempt",
            (_, RunClass::Active) => "active",
            (_, RunClass::Superseded) => "superseded",
            (_, RunClass::Stale) => "stale",
        };
        out.push_str(&format!(
            "{:<11} {:>7} {:<11} {:<17} {:<25} {}\n",
            r.id,
            r.age_secs / 60,
            class,
            r.event,
            r.branch,
            r.workflow
        ));
    }
    out.push_str(&format!("\nadvice: {}\n", snap.advice));
    out
}

/// Managed fleet counts for the summary block: alive containers + configured
/// max, via the runner-scale helpers. Best-effort — (0, 0) when docker is down.
fn fleet_counts() -> (u32, u32) {
    let alive = super::runner_scale::managed_running_count().unwrap_or(0);
    (alive, super::runner_scale::max_runners())
}
```

**Note on `fleet_counts`:** `runner_scale.rs` has `managed_containers(state) -> Vec<String>` and `max_runners()` as private fns. Add to `runner_scale.rs`:

```rust
/// Count of managed runner containers currently running (queue snapshot summary).
pub(crate) fn managed_running_count() -> Result<u32> {
    Ok(managed_containers("running").len() as u32)
}
```

and make `max_runners` `pub(crate) fn max_runners()` (visibility-only change).

- [ ] **Step 4: Implement `run` + live/snapshot entry points**

Append to `queue.rs`:

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

/// Build a live snapshot (fetch + classify + fleet counts) and persist it.
pub fn live_snapshot(ttl_mins: i64, now: i64) -> Result<QueueSnapshot> {
    let mut runs = fetch_runs(now)?;
    classify_runs(&mut runs, ttl_mins * 60);
    let (alive, max) = fleet_counts();
    let snap = build_snapshot(runs, alive, max, false, now);
    write_snapshot(&snap)?;
    Ok(snap)
}

pub fn run(args: QueueArgs) -> Result<()> {
    if args.hook_guard {
        return hook_guard_main(); // Task 4
    }
    let now = now_secs();
    let ttl = args.ttl_mins.unwrap_or(DEFAULT_TTL_MINS);

    let snap = if args.from_snapshot {
        match std::fs::read_to_string(snapshot_path())
            .ok()
            .and_then(|s| serde_json::from_str::<QueueSnapshot>(&s).ok())
        {
            Some(s) if !snapshot_is_stale(s.generated_at, now) => s,
            _ => {
                println!(
                    "queue snapshot unavailable/stale — run `vox ci queue` for live state"
                );
                return Ok(());
            }
        }
    } else {
        match live_snapshot(ttl, now) {
            Ok(s) => s,
            Err(e) if args.clear => return Err(e).context("--clear needs live gh data"),
            Err(e) => {
                eprintln!("queue: gh query failed: {e:#}");
                build_snapshot(Vec::new(), 0, 0, true, now)
            }
        }
    };

    if args.clear {
        return clear_runs(&snap, args.dry_run); // Task 3
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&snap)?);
    } else if args.brief {
        println!("{}", render_brief(&snap));
    } else {
        println!("{}", render_table(&snap));
    }
    Ok(())
}
```

Until Tasks 3 and 4 land, add temporary stubs so this compiles — **implemented in the very next tasks, not shipped**:

```rust
fn clear_runs(_snap: &QueueSnapshot, _dry_run: bool) -> Result<()> {
    Err(anyhow!("--clear lands in Task 3"))
}
fn hook_guard_main() -> Result<()> {
    Err(anyhow!("--hook-guard lands in Task 4"))
}
```

- [ ] **Step 5: Wire the clap variant**

In `cmd_enums.rs`, after the `RunnerStatus` variant (~line 838):

```rust
    /// Run-centric CI queue snapshot: classifies runs active/superseded/stale, emits a
    /// machine-readable `advice` signal, and clears cancellable backlog. The SSOT queue
    /// interaction for agents under the local-first CI contract.
    #[command(name = "queue")]
    Queue {
        /// Emit the full QueueSnapshot as JSON.
        #[arg(long)]
        json: bool,
        /// One-paragraph summary (SessionStart hook uses this).
        #[arg(long)]
        brief: bool,
        /// Read ~/.vox/ci-queue-snapshot.json (no network; ≤10 min staleness).
        #[arg(long)]
        from_snapshot: bool,
        /// Cancel superseded + stale runs (main/merge-group/schedule/dispatch exempt).
        #[arg(long)]
        clear: bool,
        /// With --clear: print the cancellation plan without cancelling.
        #[arg(long)]
        dry_run: bool,
        /// Stale TTL in minutes for queued runs (default 45).
        #[arg(long)]
        ttl_mins: Option<i64>,
        /// PreToolUse hook mode: read hook JSON on stdin; exit 2 on banned remote-watch commands.
        #[arg(long)]
        hook_guard: bool,
    },
```

In `run_body.rs`, next to the `RunnerScale` arm (~line 555):

```rust
        CiCmd::Queue {
            json,
            brief,
            from_snapshot,
            clear,
            dry_run,
            ttl_mins,
            hook_guard,
        } => super::queue::run(super::queue::QueueArgs {
            json,
            brief,
            from_snapshot,
            clear,
            dry_run,
            ttl_mins,
            hook_guard,
        }),
```

- [ ] **Step 6: Tests + clippy**

Run: `cargo test -p vox-cli commands::ci::queue` — Expected: 8 tests PASS.
Run: `cargo clippy -p vox-cli --all-targets -- -D warnings` — Expected: clean.

- [ ] **Step 7: Smoke test live (requires gh auth; fine to run)**

Run: `cargo run -p vox-cli -- ci queue --brief`
Expected: 4-line brief with real counts, and `~/.vox/ci-queue-snapshot.json` created.
Run: `cargo run -p vox-cli -- ci queue --from-snapshot --brief`
Expected: same output, instant, no network.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli/src/commands/ci/queue.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): vox ci queue — live snapshot, snapshot file, brief/json/table render"
```

---

### Task 3: `--clear` — cancel superseded + stale

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/queue.rs` (replace Task 2's `clear_runs` stub)

- [ ] **Step 1: Write the failing test for the cancellation plan**

The plan (which runs get cancelled) is pure; the `gh` call is thin. Append:

```rust
    #[test]
    fn clear_plan_selects_only_cancellable() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "CI", "feat/x", "push", "queued", 1000, now), // superseded by 2
            run(2, "CI", "feat/x", "push", "queued", 3000, now), // active
            run(3, "CI", "feat/y", "push", "queued", 1, now),    // stale
            run(4, "CI", "main", "push", "queued", 1, now),      // exempt
        ];
        classify_runs(&mut runs, 2700);
        let snap = build_snapshot(runs, 2, 4, false, now);
        let ids: Vec<u64> = clear_plan(&snap).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 3]);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli commands::ci::queue::tests::clear_plan_selects_only_cancellable`
Expected: compile error — `clear_plan` not defined.

- [ ] **Step 3: Implement**

Replace the Task 2 stub:

```rust
/// Runs `--clear` will cancel: non-exempt, classified Superseded or Stale.
pub fn clear_plan(snap: &QueueSnapshot) -> Vec<&QueueRun> {
    snap.runs
        .iter()
        .filter(|r| !r.exempt && r.class != RunClass::Active)
        .collect()
}

/// Cancel each run in the plan, best-effort: one failed cancel (e.g. the run
/// completed in the snapshot→cancel race) logs and continues, never aborts.
fn clear_runs(snap: &QueueSnapshot, dry_run: bool) -> Result<()> {
    let plan = clear_plan(snap);
    if plan.is_empty() {
        println!("queue clear: nothing cancellable ({})", snap.advice);
        return Ok(());
    }
    let mut cancelled = 0u32;
    let mut failed = 0u32;
    for r in &plan {
        let tag = format!("{} ({} / {} / {:?})", r.id, r.workflow, r.branch, r.class);
        if dry_run {
            println!("would cancel {tag}");
            continue;
        }
        match gh_json(&[
            "api",
            "-X",
            "POST",
            &format!("repos/{REPO_SLUG}/actions/runs/{}/cancel", r.id),
        ]) {
            Ok(_) => {
                println!("cancelled {tag}");
                cancelled += 1;
            }
            Err(e) => {
                // 409/422 = already finished — the race resolved itself.
                eprintln!("cancel {tag} failed (continuing): {e:#}");
                failed += 1;
            }
        }
    }
    if !dry_run {
        println!("queue clear: cancelled {cancelled}, failed {failed}, of {}", plan.len());
        // Refresh the snapshot so the next --from-snapshot reader sees the cleared state.
        let now = now_secs();
        if let Ok(snap) = live_snapshot(DEFAULT_TTL_MINS, now) {
            println!("post-clear: {}", snap.advice);
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Tests + clippy pass**

Run: `cargo test -p vox-cli commands::ci::queue` — Expected: 9 tests PASS.
Run: `cargo clippy -p vox-cli --all-targets -- -D warnings` — Expected: clean.

- [ ] **Step 5: Smoke test dry-run against the real queue**

Run: `cargo run -p vox-cli -- ci queue --clear --dry-run`
Expected: either "nothing cancellable" or `would cancel …` lines; NO actual cancellations.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/queue.rs
git commit -m "feat(ci): vox ci queue --clear — cancel superseded + stale runs (exempt-aware)"
```

---

### Task 4: `--hook-guard` — PreToolUse enforcement

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/queue.rs` (replace Task 2's `hook_guard_main` stub)

- [ ] **Step 1: Write the failing pattern tests**

```rust
    #[test]
    fn hook_guard_patterns() {
        // Banned: remote check-watching in any form.
        assert!(hook_guard_matches("gh pr checks 431 --watch"));
        assert!(hook_guard_matches("gh pr checks"));
        assert!(hook_guard_matches("gh run watch 12345"));
        assert!(hook_guard_matches("gh run view 12345 --log --watch"));
        assert!(hook_guard_matches("gh api repos/o/r/commits/abc/check-runs"));
        assert!(hook_guard_matches("gh api repos/o/r/check_runs --paginate"));
        assert!(hook_guard_matches("vox ci watch-run --sha abc"));
        assert!(hook_guard_matches("cargo run -p vox-cli -- ci watch-run"));
        // Allowed near-misses.
        assert!(!hook_guard_matches("gh run view 12345 --log"));
        assert!(!hook_guard_matches("gh run list --status queued"));
        assert!(!hook_guard_matches("gh pr view 431"));
        assert!(!hook_guard_matches("gh api repos/o/r/actions/runs"));
        assert!(!hook_guard_matches("vox ci queue --json"));
        assert!(!hook_guard_matches("git push"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli commands::ci::queue::tests::hook_guard_patterns`
Expected: compile error — `hook_guard_matches` not defined.

- [ ] **Step 3: Implement**

Replace the Task 2 stub. Purely local matching — no file IO beyond stdin, no network (this runs on **every** Bash tool call):

```rust
/// Coarse substring match on the Bash command an agent is about to run.
/// Known collateral: a banned phrase inside a quoted string still blocks —
/// acceptable; the deny message names the sanctioned alternative.
pub fn hook_guard_matches(cmd: &str) -> bool {
    let c = cmd.to_lowercase();
    c.contains("gh pr checks")
        || c.contains("gh run watch")
        || (c.contains("gh run view") && c.contains("--watch"))
        || (c.contains("gh api") && (c.contains("check-runs") || c.contains("check_runs")))
        || c.contains("ci watch-run")
}

const HOOK_GUARD_DENY: &str = "Local-first CI: remote check-watching is disabled.\n\
- Verdict: run local gates (`vox ci pre-push`); green = done, push and move on.\n\
- Queue state: `vox ci queue --json` (the `advice` field tells you what to do).\n\
- Clear backlog: `vox ci queue --clear`.";

/// PreToolUse mode: read the Claude Code hook JSON from stdin, extract
/// `tool_input.command`, exit 2 (block, stderr fed to the model) on a banned
/// pattern. Everything else — including unparseable input — exits 0 (fail-open
/// on infrastructure, fail-closed only on the banned patterns).
fn hook_guard_main() -> Result<()> {
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

- [ ] **Step 4: Tests + clippy pass**

Run: `cargo test -p vox-cli commands::ci::queue` — Expected: 10 tests PASS.
Run: `cargo clippy -p vox-cli --all-targets -- -D warnings` — Expected: clean.

- [ ] **Step 5: End-to-end guard check (PowerShell-safe)**

```bash
echo '{"tool_input":{"command":"gh pr checks 431 --watch"}}' | cargo run -p vox-cli -- ci queue --hook-guard; echo "exit=$?"
```

Expected: deny message on stderr, `exit=2`.

```bash
echo '{"tool_input":{"command":"git push"}}' | cargo run -p vox-cli -- ci queue --hook-guard; echo "exit=$?"
```

Expected: silent, `exit=0`.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/queue.rs
git commit -m "feat(ci): vox ci queue --hook-guard — block remote check-watching (exit 2)"
```

---

### Task 5: auto-clear + snapshot on the autoscaler tick

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/queue.rs` (one new function)
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs` (`run_scale` ~line 758, `scale_event_json` ~line 699 and its call site + any test using it)

- [ ] **Step 1: Add the tick entry point in `queue.rs`**

```rust
/// Autoscaler-tick entry: clear cancellable runs (apply mode only), then write
/// the snapshot. Returns (cleared_superseded, cleared_stale) for the scale
/// ledger. gh failure returns Err — caller logs degraded and continues.
pub fn auto_clear_and_snapshot(dry_run: bool, now: i64) -> Result<(u32, u32)> {
    let mut runs = fetch_runs(now)?;
    classify_runs(&mut runs, DEFAULT_TTL_MINS * 60);
    let (alive, max) = fleet_counts();
    let snap = build_snapshot(runs, alive, max, false, now);
    let mut sup = 0u32;
    let mut stale = 0u32;
    for r in clear_plan(&snap) {
        if !dry_run
            && gh_json(&[
                "api",
                "-X",
                "POST",
                &format!("repos/{REPO_SLUG}/actions/runs/{}/cancel", r.id),
            ])
            .is_err()
        {
            continue; // race: run finished; next tick's snapshot self-corrects
        }
        match r.class {
            RunClass::Superseded => sup += 1,
            RunClass::Stale => stale += 1,
            RunClass::Active => {}
        }
    }
    write_snapshot(&snap)?;
    Ok((sup, stale))
}
```

- [ ] **Step 2: Call it at the top of `run_scale`**

In `runner_scale.rs::run_scale`, immediately after the lock block (after `let _lock = …;`):

```rust
    // 0. Local-first CI: auto-clear superseded/stale runs and refresh the
    //    queue snapshot every tick (dry-run counts but never cancels).
    let (cleared_superseded, cleared_stale) =
        super::queue::auto_clear_and_snapshot(dry_run, now).unwrap_or_else(|e| {
            eprintln!("runner-scale: queue auto-clear skipped (degraded): {e:#}");
            (0, 0)
        });
```

- [ ] **Step 3: Extend the scale-event ledger**

Add two params to `scale_event_json` (keeping the `#[allow(clippy::too_many_arguments)]`):

```rust
    cleared_superseded: u32,
    cleared_stale: u32,
```

and extend the format string with `"cleared_superseded":{cleared_superseded},"cleared_stale":{cleared_stale}` before the closing brace. Update the single call site in `run_scale` to pass the two new values, and update any existing unit test asserting the JSON shape (search: `scale_event_json` in the `#[cfg(test)]` module of `runner_scale.rs`) to include the new fields.

- [ ] **Step 4: Tests + clippy pass**

Run: `cargo test -p vox-cli commands::ci::runner_scale` and `cargo test -p vox-cli commands::ci::queue`
Expected: all PASS.
Run: `cargo clippy -p vox-cli --all-targets -- -D warnings` — Expected: clean.

- [ ] **Step 5: Smoke test the tick (dry-run, safe)**

Run: `cargo run -p vox-cli -- ci runner-scale`
Expected: dry-run output includes no cancellations; `~/.vox/ci-queue-snapshot.json` mtime refreshed.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/queue.rs crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): autoscaler tick auto-clears superseded/stale runs + writes queue snapshot"
```

---

### Task 6: checked-in Claude Code hooks

**Files:**
- Create: `.claude/settings.json` (project scope — this file does not exist yet; only `settings.local.json` does)

- [ ] **Step 1: Create the settings file**

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
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

- [ ] **Step 2: Verify hook behavior manually**

The PreToolUse guard: from a fresh Claude Code session in the repo, attempt `gh pr checks` via Bash — expect the deny message. The SessionStart injection: start a session and confirm the queue brief appears in context. If a stale `vox.exe` is a concern (Windows lock gotcha), note that hooks resolve `vox` from PATH — confirm `vox ci queue --brief --from-snapshot` works from a plain terminal first.

- [ ] **Step 3: Commit**

```bash
git add .claude/settings.json
git commit -m "feat(hooks): block remote check-watching + inject CI queue state at session start"
```

---

### Task 7: concurrency sweep + `workflow-concurrency-guard`

**Files:**
- Modify: `.github/workflows/ci-health-watchdog-test.yml` (the only push/PR-triggered workflow missing `concurrency`; release-binaries/gui/installers are tag-push-only and scorecard is main-push-only → exceptions)
- Create: `docs/src/ci/concurrency-exceptions.md`
- Create: `crates/vox-cli-ci/src/workflow_concurrency_guard.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs` (add module)
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (new variant, after `RunnerPolicyCheck` ~line 345)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (dispatch, near line 299)
- Modify: `crates/vox-cli/src/commands/ci/pre_push.rs` (new step after `runner-policy-check` ~line 520, step fn near line 1081)

- [ ] **Step 1: Sweep — add the block to ci-health-watchdog-test.yml**

Insert after its `on:` block, before `permissions:`:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
```

- [ ] **Step 2: Create the exceptions doc**

`docs/src/ci/concurrency-exceptions.md` (frontmatter mirrors `github-hosted-exceptions.md`):

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
flooding the fleet. A workflow may be listed here (backticked filename + reason)
when cancel-in-progress would be incorrect.

- `release-binaries.yml` — tag-push only; a release build must never be cancelled by a later tag.
- `release-gui.yml` — tag-push only; same as above.
- `release-installers.yml` — tag-push only; same as above.
- `scorecard.yml` — pushes to `main` only; supply-chain scorecard runs should complete, and main runs are exempt from queue clearing anyway.
```

- [ ] **Step 3: Write the failing guard tests**

Create `crates/vox-cli-ci/src/workflow_concurrency_guard.rs` starting with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_detection_handles_yaml_11_on_key() {
        // serde_yaml parses a bare `on:` key as Bool(true) (YAML 1.1).
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

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p vox-cli-ci workflow_concurrency`
Expected: compile error — module/functions not defined (add `pub mod workflow_concurrency_guard;` to `crates/vox-cli-ci/src/lib.rs` alphabetically first, then the error becomes missing functions).

- [ ] **Step 5: Implement the guard**

Above the tests in `workflow_concurrency_guard.rs`:

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
        serde_yaml::Value::Sequence(seq) => {
            seq.iter().any(|v| v.as_str().map(hit).unwrap_or(false))
        }
        serde_yaml::Value::Mapping(m) => m
            .keys()
            .any(|k| k.as_str().map(hit).unwrap_or(false)),
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
    let mut violations = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&wf_dir)
        .with_context(|| format!("read {}", wf_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|x| x == "yml" || x == "yaml")
                .unwrap_or(false)
        })
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&text)
            .with_context(|| format!("parse {}", path.display()))?;
        if needs_concurrency(&doc) && !has_concurrency(&doc) && !is_excepted(&exceptions_text, &name)
        {
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
    if strict {
        Err(anyhow!(msg))
    } else {
        eprintln!("WARN {msg}");
        Ok(())
    }
}
```

- [ ] **Step 6: Run guard tests to verify they pass**

Run: `cargo test -p vox-cli-ci workflow_concurrency`
Expected: 3 tests PASS.

- [ ] **Step 7: Wire CLI + pre-push**

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

`run_body.rs` next to the `RunnerPolicyCheck` arm (line 299):

```rust
        CiCmd::WorkflowConcurrencyGuard { strict } => {
            vox_cli_ci::workflow_concurrency_guard::run(&root, strict)
        }
```

`pre_push.rs`: add after the `runner-policy-check` step entry (~line 520):

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

- [ ] **Step 8: Run the guard against the real tree**

Run: `cargo run -p vox-cli -- ci workflow-concurrency-guard --strict`
Expected: `workflow-concurrency-guard OK` (Step 1's sweep + Step 2's exceptions make the tree clean; if it flags anything else, the trigger audit missed a workflow — add the block or an exception row, whichever the trigger shape warrants).

- [ ] **Step 9: Full checks + commit**

Run: `cargo clippy -p vox-cli -p vox-cli-ci --all-targets -- -D warnings` — Expected: clean.
Run: `cargo test -p vox-cli-ci` — Expected: PASS.

```bash
git add .github/workflows/ci-health-watchdog-test.yml docs/src/ci/concurrency-exceptions.md crates/vox-cli-ci/src/workflow_concurrency_guard.rs crates/vox-cli-ci/src/lib.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs crates/vox-cli/src/commands/ci/pre_push.rs
git commit -m "feat(ci): workflow-concurrency-guard + sweep — flood prevention unregressable"
```

---

### Task 8: the written contract — AGENTS.md + docs page

**Files:**
- Modify: `AGENTS.md` (new section immediately after "## Local CI Gate Tiers (SSOT)", line 302 block)
- Create: `docs/src/ci/local-first-ci.md`
- Modify: `docs/src/ci/runner-autoscaling.md` (one cross-link line)

- [ ] **Step 1: Add the AGENTS.md section**

Insert after the "Local CI Gate Tiers (SSOT)" section body:

```markdown
## Local-First CI Verification Contract (Required, SSOT)

The local runner fleet is the CI plane. For agents:

- **Local gates green = the verdict.** Run the tiered local gates (`vox ci pre-push`);
  green means done — push and move on.
- **Fleet CI is an async safety net.** Never sit in a watch loop on remote checks.
  `gh pr checks`, `gh run watch`, `gh run view --watch`, check-runs polling, and
  `vox ci watch-run` are blocked for agent sessions by a PreToolUse hook
  (`.claude/settings.json` → `vox ci queue --hook-guard`).
- **Queue interactions:** `vox ci queue --json` to read (the `advice` field says what
  to do); `vox ci queue --clear` to cancel superseded + stale runs. Main-branch,
  merge-group, scheduled, and manually dispatched runs are always exempt from clearing.
- **Auto-heal:** the `runner-scale` tick (every ~2 min) auto-clears superseded/stale
  runs and refreshes `~/.vox/ci-queue-snapshot.json`.
- **No new hosted jobs / unguarded workflows:** GitHub-hosted `runs-on` needs a row in
  `docs/src/ci/github-hosted-exceptions.md` (`vox ci runner-policy-check`); push/PR
  workflows need a `concurrency:` block or a row in
  `docs/src/ci/concurrency-exceptions.md` (`vox ci workflow-concurrency-guard`).

Details: `docs/src/ci/local-first-ci.md`.
```

- [ ] **Step 2: Create the docs page**

`docs/src/ci/local-first-ci.md`:

```markdown
---
title: "Local-first CI: queue signal and agent contract"
description: "The vox ci queue SSOT signal, superseded/stale auto-clearing, and the hooks that keep agents on the local runner fleet."
category: "CI & Quality"
last_updated: "2026-07-02"
training_eligible: true

schema_type: "TechArticle"
---

# Local-first CI: queue signal and agent contract

The local runner fleet is the CI plane; GitHub Actions remains the job queue it
consumes. This page documents the machinery that keeps every harness call and
every agent on that plane mechanically.

## The contract

Local gates green (`vox ci pre-push` tier) is the verdict: push and move on.
Fleet CI is an async safety net — failures come back as a signal, never by an
agent sitting in a watch loop. Remote check-watching (`gh pr checks`,
`gh run watch`, `gh run view --watch`, check-runs polling, `vox ci watch-run`)
is blocked for agent sessions by the PreToolUse hook in `.claude/settings.json`.

## `vox ci queue`

Run-centric queue snapshot with classification:

- **exempt** — `main` branch, `merge_group`, `schedule`, `workflow_dispatch`
  events. Never cancelled.
- **superseded** — a strictly newer run exists for the same (workflow, branch);
  only the newest survives.
- **stale** — still `queued` past the TTL (default 45 min, `--ttl-mins`).

Flags: `--json` (full snapshot), `--brief` (SessionStart injection),
`--from-snapshot` (no network; reads `~/.vox/ci-queue-snapshot.json`, refuses
snapshots older than 10 min), `--clear [--dry-run]` (cancel superseded + stale),
`--hook-guard` (PreToolUse mode).

The snapshot's `advice` field is the machine-readable signal: it always states
queue health and, when clearing would help, the exact command and counts.

## Auto-heal

Every `vox ci runner-scale` tick (~2 min via Task Scheduler) first auto-clears
superseded/stale runs (apply mode) and rewrites the snapshot file, then
reconciles the fleet. Clear counts land in the scale-event ledger
(`cleared_superseded`, `cleared_stale`).

## Flood prevention at the source

Push/PR-triggered workflows must declare
`concurrency: { group: workflow-ref, cancel-in-progress: true }` —
enforced by `vox ci workflow-concurrency-guard` (strict in pre-push), with
exceptions registered in [concurrency-exceptions](concurrency-exceptions.md).

## Deferred roadmap

Local verdict ledger (`vox ci verdict <sha>`), a local orchestration plane that
bypasses the Actions queue entirely, and migrating the remaining GitHub-hosted
jobs (`vox ci runner-policy-check --strict` flip) are deliberate follow-ons;
see the design spec `docs/superpowers/specs/2026-07-02-local-first-ci-queue-design.md`.
```

- [ ] **Step 3: Cross-link from runner-autoscaling.md**

Add one line in the intro of `docs/src/ci/runner-autoscaling.md`:

```markdown
Queue clearing and the agent-facing queue signal are documented in
[local-first-ci](local-first-ci.md).
```

- [ ] **Step 4: Verify docs gates**

Run: `cargo run -p vox-cli -- ci check-links`
Expected: OK (both new/modified docs resolve). Do NOT touch `SUMMARY.md` — it is
gitignored and generated from frontmatter at Astro build time.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md docs/src/ci/local-first-ci.md docs/src/ci/runner-autoscaling.md
git commit -m "docs(ci): local-first CI contract — AGENTS.md section + ci docs page"
```

---

### Task 9: full-gate verification

- [ ] **Step 1: Full local gates**

Run: `cargo run -p vox-cli -- ci pre-push`
Expected: all steps PASS, including the two new guards. If the locked-`vox.exe`
pre-push gotcha fires, stop the main-dir `vox.exe` process first.

- [ ] **Step 2: Workspace clippy (house invocation)**

Run: `cargo clippy --workspace --all-targets --exclude vox-gui -- -D warnings`
Expected: clean.

- [ ] **Step 3: Push and confirm**

```bash
git push -u origin claude/interesting-leavitt-246363
```

Then verify with `gh pr view` per house rule (push "error" output can be spurious).
Note: after this branch lands, per the new contract — do NOT watch the remote
checks; local gates already passed.
