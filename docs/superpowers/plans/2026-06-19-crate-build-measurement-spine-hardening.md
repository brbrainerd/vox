# Crate-Build Measurement-Spine Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **🤖 EXECUTION TARGET — Claude Sonnet 4.6.** Strong reasoning + long-context recall, but the failure modes to guard against are: (a) declaring a task "done" without running the verification ritual, (b) editing a symbol it *assumed* exists instead of grepping first, and (c) YAGNI gold-plating. Every task therefore opens with a **Pre-flight** grep (verify-before-use), shows **complete code** (no "fill in"), and ends with a **verification ritual whose output you must paste into the task log before committing**. Do not improvise beyond the steps. If a Pre-flight result contradicts the plan, STOP and report — do not "fix it up" silently.

**Goal:** Make the crate-build measurement spine real, deterministic, and enforced — so the three gates already built (`dep-cycles --deny-new`, `crate-budget`, `fan-in-budget`) plus a new parity gate run in CI against a committed, reproducible blast-radius SSOT instead of gitignored/stale cache that can never fail.

**Architecture:** Today `vox ci crate-budget` reads `.vox/cache/graphify/crate-map/graph.json` — **gitignored** (absent in CI → gate silently skips), built from a **gitignored** `crate_audit.json` (blast_s degrades to count-only in CI), against **mis-calibrated** ceilings (committed 445/437/440 vs actual 349 → gate cannot fire). This plan introduces ONE committed SSOT — `contracts/ci/crate-build-map.v1.json` — embedding each crate's `compile_s` (the periodically-refreshed input, at the audit's native 1-decimal precision) plus the *derived* `dependents`/`blast_s`/`fan_in` recomputed from the committed `contracts/ci/crate-graph.v1.json`. A new `crate-build-map-parity` gate recomputes the derived fields from the two committed files and fails on drift (the same pattern as the existing crate-graph / config-registry parity gates). `crate-budget` repoints to this committed SSOT and **fails loud** when data is missing or count-only. The rich `.vox/cache` graph stays as the local artifact for `graphify query/ingest`.

**Tech Stack:** Rust (`vox-graphify-reader`, `vox-cli` CI commands, `clap 4.5`), `serde_json`, committed JSON SSOTs under `contracts/ci/`, GitHub Actions (`.github/workflows/ci.yml`), the Vox-language audit script `scripts/crate-build-audit.vox`.

---

## Operating Rules (read once, apply to every task)

- **Atomic + green + committed per task.** One task = one commit. Never start the next task with the tree dirty.
- **Verify-before-use.** Each task's Pre-flight greps confirm the exact symbols/paths exist *before* you edit. If reality differs from the plan, STOP and report (two-strike rule: after two failed attempts at the same step, stop and ask).
- **Paste verification output.** Before each commit, run the verification ritual and paste the real output into your task log. "Looks done" is not done.
- **Verification ritual** (per touched Rust crate `X`): `cargo test -p X --lib` (or the named test) → `cargo clippy -p X -- -D warnings` → `cargo fmt -p X`. For `vox-cli` use `cargo test -p vox-cli --lib`. **Never `cargo fmt --all`** (repo ban) — always `-p <crate>`.
- **`[SEQUENTIAL]` / `[PARALLEL-SAFE]`** tags on each task indicate whether it shares files with a sibling. Honor them — most tasks here are SEQUENTIAL because they touch shared files (`graphify/mod.rs`, the ci `cmd_enums.rs`/`run_body.rs`).
- **YAGNI.** Build exactly what each task specifies. The churn-weighting idea in Task 8 is explicitly deferred — do not implement it.
- **Binary path.** Use `./target/debug/vox` for all `vox ...` invocations (the installed `~/.cargo/bin/vox` is stale and lacks `crate-map`).
- Commit messages end with the trailer `Co-Authored-By: AI Assistant <noreply@anthropic.com>`.

---

## Background: verified ground truth (2026-06-19, confirmed against the live tree)

| Fact | Evidence |
|---|---|
| Actual blast_s: `workspace-hack`=492, `vox-db`/`vox-compiler`/`vox-populi`=349 (identical — cluster signature) | `.vox/cache/graphify/crate-map/graph.json` |
| Committed ceilings 445/437/440 are ~28% above actual → gate cannot fire | `contracts/ci/crate-budget.v1.json` |
| `crate_audit.json` exists locally (109 crates, 1-decimal `compile_s`) but is **gitignored** + Jun-15-stale; crate-graph has **113** crates → 4 lack `compile_s` | `graphify-out/crate_audit.json`, `contracts/ci/crate-graph.v1.json` |
| Only `dep-cycles` is wired in CI, **without `--deny-new`**; `crate-budget`/`fan-in-budget` never invoked | `.github/workflows/ci.yml` (`Dependency cycle gate` step) |
| `crate-map` manifest `lexical_ingest_sha256: null` → not agent-queryable | `.vox/cache/graphify/crate-map/.graphify_manifest.v1.json` |
| Installed `~/.cargo/bin/vox` lacks `crate-map` (`unrecognized subcommand`) | `vox graphify crate-map --help` |
| **Branch does NOT build**: `cargo check -p vox-cli` fails with 7 `E0425` errors in `vox-orchestrator-mcp::dispatch` — from **uncommitted** working-tree WIP (additive +171 lines across dispatch.rs/chat_tools/http_gateway). Committed HEAD builds. | `cargo check -p vox-cli` + `git diff --stat crates/vox-orchestrator-mcp/` |

**Implication:** the existing gates were only lib-unit-tested, never run through the binary. Task 0 establishes a green build in a clean worktree and smoke-tests the existing gates before we add the fourth.

---

## File Structure

| File | Responsibility | Task |
|---|---|---|
| (worktree setup) | Clean build env off committed HEAD; smoke existing gates | T0 |
| `crates/vox-graphify-reader/src/crate_model.rs` | Add `build_crate_summary(crate_graph, compile_times)` pure fn | T1 |
| `crates/vox-graphify-reader/tests/crate_summary_tests.rs` | Tests for the pure fn | T1 |
| `contracts/ci/crate-build-map.v1.json` | NEW committed SSOT: `compile_s` + derived `dependents`/`blast_s`/`fan_in` | T2 |
| `crates/vox-cli/src/commands/graphify/mod.rs` | `--write-summary` (T2) + `--ingest` (T7) on `CrateMap`; refactor Ingest into shared fn (T7) | T2, T7 |
| `crates/vox-cli/src/commands/ci/crate_budget.rs` | Repoint to committed SSOT; fail loud on missing/count-only | T3 |
| `contracts/ci/crate-budget.v1.json` | Recalibrate ceilings to real blast_s × 1.15 | T4 |
| `crates/vox-cli/src/commands/ci/crate_build_map_parity.rs` | NEW parity gate | T5 |
| `crates/vox-cli/src/commands/ci/{mod.rs,cmd_enums.rs,run_body.rs}` | Register `crate-build-map-parity` | T5 |
| `.github/workflows/ci.yml` | Wire all four gates | T6 |
| `docs/src/architecture/crate-build-dependency-model-2026-06-19.md` | Document blast_s semantics | T8 |
| `docs/superpowers/plans/2026-06-19-crate-build-disentanglement-suite-index.md` | Status row | T8 |

**Dependency order:** T0 → T1 → T2 → (T3 → T4) ∥ T5 → T6 → T7 → T8. T2 must precede T3/T4/T5/T6 (they read the SSOT). T7 edits the same `CrateMap` variant as T2, so it runs after T2 (SEQUENTIAL on `graphify/mod.rs`). T8 is docs-only.

---

### Task 0: Establish a green build in a clean worktree `[SEQUENTIAL]`

**Why:** the working tree is dirty with someone else's uncommitted, non-compiling telemetry WIP. We must not build on top of it. A worktree off the committed branch tip gives a clean, building tree that still contains the committed crate-build work (T1–T4 gates).

**Files:** none (environment).

- [ ] **Step 1: Confirm the working tree is dirty and identify the WIP (do not touch it)**

```bash
git -C /c/Users/Owner/vox diff --stat crates/vox-orchestrator-mcp/
```
Expected: shows uncommitted changes in `dispatch.rs`, `chat_tools/mod.rs`, `http_gateway/mod.rs` (additive). This is the build-breaker. Leave it alone.

- [ ] **Step 2: Create a worktree off the current committed branch tip**

```bash
cd /c/Users/Owner/vox
git worktree add ../wt-spine HEAD
cd ../wt-spine
git switch -c claude/crate-build-spine-hardening
```
Expected: `Preparing worktree` + a new branch. All subsequent steps run from `../wt-spine` (a clean checkout of committed HEAD — no dirty WIP).

- [ ] **Step 3: Confirm the clean tree builds**

```bash
cargo check -p vox-cli 2>&1 | grep -E "Finished|^error" | head
```
Expected: `Finished` and NO `error` lines. **If it errors, STOP and report BLOCKED** — committed HEAD is broken and that must be fixed first (out of scope for this plan).

- [ ] **Step 4: Build the binary and confirm `crate-map` exists**

```bash
cargo build -p vox-cli 2>&1 | tail -3
./target/debug/vox graphify crate-map --help 2>&1 | head -8
```
Expected: `Finished`; help text showing `--no-refresh-graph` (the `--write-summary`/`--ingest` flags are added in T2/T7).

- [ ] **Step 5: Smoke-test the three EXISTING gates through the binary (baseline)**

```bash
./target/debug/vox ci dep-cycles 2>&1 | tail -3
./target/debug/vox ci fan-in-budget 2>&1 | tail -3
# crate-budget currently reads the gitignored cache; copy the local cache into the worktree first:
mkdir -p .vox/cache/graphify/crate-map
cp /c/Users/Owner/vox/.vox/cache/graphify/crate-map/graph.json .vox/cache/graphify/crate-map/graph.json
./target/debug/vox ci crate-budget 2>&1 | tail -5
```
Expected: each exits 0. Record the output — this is the "before" baseline. (After T3, crate-budget no longer needs the cache copy.)

No commit (no tracked files changed). Proceed to T1 from `../wt-spine`.

---

### Task 1: `build_crate_summary` pure function `[SEQUENTIAL]`

**Files:**
- Modify: `crates/vox-graphify-reader/src/crate_model.rs` (append after `build_crate_map`, ~line 181)
- Create: `crates/vox-graphify-reader/tests/crate_summary_tests.rs`

- [ ] **Step 0: Pre-flight — confirm `crate_metrics` signature + `CrateMetrics` fields**

```bash
grep -n "pub fn crate_metrics\|pub struct CrateMetrics\|pub dependents\|pub blast_s" crates/vox-graphify-reader/src/crate_model.rs
```
Expected: `pub fn crate_metrics(adj: &HashMap<String, Vec<String>>, self_s: &HashMap<String, f64>) -> HashMap<String, CrateMetrics>` and `CrateMetrics { dependents: usize, blast_s: f64 }`. Also confirm the file imports `HashMap`, `HashSet`, `json`, `Value` at the top (it does — they back `build_crate_map`). If signatures differ, STOP.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-graphify-reader/tests/crate_summary_tests.rs`:

```rust
use serde_json::json;
use std::collections::HashMap;
use vox_graphify_reader::crate_model::build_crate_summary;

fn compile_times(entries: &[(&str, f64)]) -> HashMap<String, f64> {
    entries.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

#[test]
fn summary_is_sorted_and_carries_compile_times() {
    // c -> b -> a  (a is the leaf everything depends on)
    let graph = json!({ "crates": { "c": ["b"], "b": ["a"], "a": [] } });
    let times = compile_times(&[("a", 1.0), ("b", 2.0), ("c", 3.0)]);
    let s = build_crate_summary(&graph, &times);

    assert_eq!(s["schema_version"], 1);
    assert_eq!(s["has_compile_times"], true);
    assert_eq!(s["crates_without_compile_times"], 0);

    let crates = s["crates"].as_array().unwrap();
    assert_eq!(crates[0]["crate"], "a"); // sorted alphabetically
    assert_eq!(crates[1]["crate"], "b");
    assert_eq!(crates[2]["crate"], "c");

    // a's blast_s = compile_s(a)+dependents(b,c) = 1+2+3 = 6; dependents=2; fan_in=1 (b depends on a)
    assert_eq!(crates[0]["blast_s"], 6.0);
    assert_eq!(crates[0]["dependents"], 2);
    assert_eq!(crates[0]["fan_in"], 1);
    // c is a root: no dependents, blast_s = own compile_s
    assert_eq!(crates[2]["blast_s"], 3.0);
    assert_eq!(crates[2]["dependents"], 0);
}

#[test]
fn summary_flags_missing_compile_times() {
    let graph = json!({ "crates": { "a": [], "b": ["a"] } });
    let times = compile_times(&[("a", 1.0)]); // b missing
    let s = build_crate_summary(&graph, &times);
    assert_eq!(s["crates_without_compile_times"], 1);
    assert_eq!(s["has_compile_times"], true); // any time present → usable
}

#[test]
fn summary_empty_times_sets_has_compile_times_false() {
    let graph = json!({ "crates": { "a": [], "b": ["a"] } });
    let s = build_crate_summary(&graph, &HashMap::new());
    assert_eq!(s["has_compile_times"], false);
    assert_eq!(s["crates_without_compile_times"], 2);
}

#[test]
fn summary_round_trips_exactly() {
    // INVARIANT for the parity gate: rebuilding from the summary's own compile_s
    // must reproduce identical derived fields (compile_s stored at input precision).
    let graph = json!({ "crates": { "x": ["y", "z"], "y": ["z"], "z": [] } });
    let times = compile_times(&[("x", 1.0), ("y", 2.0), ("z", 3.0)]);
    let first = build_crate_summary(&graph, &times);

    // Extract compile_s back out (as the parity gate will) and rebuild.
    let mut reextracted = HashMap::new();
    for c in first["crates"].as_array().unwrap() {
        reextracted.insert(
            c["crate"].as_str().unwrap().to_string(),
            c["compile_s"].as_f64().unwrap(),
        );
    }
    let second = build_crate_summary(&graph, &reextracted);
    assert_eq!(first, second);
}
```

- [ ] **Step 2: Run test — verify it FAILS**

Run: `cargo test -p vox-graphify-reader --test crate_summary_tests 2>&1 | tail -10`
Expected: FAIL — `cannot find function build_crate_summary`.

- [ ] **Step 3: Implement `build_crate_summary`**

Append to `crates/vox-graphify-reader/src/crate_model.rs` (after `build_crate_map`'s closing `}`):

```rust
/// Build the small, committed crate-build SSOT (`contracts/ci/crate-build-map.v1.json`).
///
/// `crate_graph` is the `{crates:{name:[deps]}}` shape from `crate-graph.v1.json`.
/// `compile_times` maps crate name -> self compile seconds (audit native precision; may be partial/empty).
///
/// Output embeds `compile_s` (the periodically-refreshed INPUT) plus the DERIVED
/// `dependents`/`blast_s`/`fan_in`, so the parity gate can recompute the derived fields from
/// `crate_graph` + the embedded `compile_s` and detect drift. Deterministic: crates sorted
/// alphabetically; `blast_s` rounded to whole seconds; `compile_s` kept at input precision.
pub fn build_crate_summary(crate_graph: &Value, compile_times: &HashMap<String, f64>) -> Value {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut nodes: HashSet<String> = HashSet::new();
    let mut fan_in: HashMap<String, usize> = HashMap::new();
    if let Some(m) = crate_graph.get("crates").and_then(|v| v.as_object()) {
        for (c, ds) in m {
            nodes.insert(c.clone());
            let deps: Vec<String> = ds
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            for d in &deps {
                nodes.insert(d.clone());
                *fan_in.entry(d.clone()).or_insert(0) += 1;
            }
            adj.insert(c.clone(), deps);
        }
    }

    let metrics = crate_metrics(&adj, compile_times);

    let mut names: Vec<String> = nodes.into_iter().collect();
    names.sort();

    let mut without = 0usize;
    let crates_val: Vec<Value> = names
        .iter()
        .map(|n| {
            let cs = compile_times.get(n).copied();
            if cs.is_none() {
                without += 1;
            }
            let m = metrics.get(n);
            json!({
                "crate": n,
                "compile_s": cs.unwrap_or(0.0),
                "dependents": m.map(|x| x.dependents).unwrap_or(0),
                "blast_s": m.map(|x| x.blast_s).unwrap_or(0.0).round(),
                "fan_in": fan_in.get(n).copied().unwrap_or(0),
            })
        })
        .collect();

    json!({
        "schema_version": 1,
        "has_compile_times": !compile_times.is_empty(),
        "crates_without_compile_times": without,
        "crates": crates_val,
    })
}
```

- [ ] **Step 4: Run test — verify it PASSES**

Run: `cargo test -p vox-graphify-reader --test crate_summary_tests 2>&1 | tail -10`
Expected: PASS — 4 tests. Paste output into the task log.

- [ ] **Step 5: Verification ritual + commit**

```bash
cargo clippy -p vox-graphify-reader -- -D warnings 2>&1 | tail -5
cargo fmt -p vox-graphify-reader
git add crates/vox-graphify-reader/src/crate_model.rs crates/vox-graphify-reader/tests/crate_summary_tests.rs
git commit -m "feat(graphify): build_crate_summary — committed crate-build SSOT shape

Pure fn deriving dependents/blast_s/fan_in from crate-graph + compile_times,
embedding compile_s (input precision) so the parity gate round-trips exactly.
Deterministic + sorted. Backs contracts/ci/crate-build-map.v1.json.

Co-Authored-By: AI Assistant <noreply@anthropic.com>"
```

---

### Task 2: Emit the committed SSOT via `crate-map --write-summary` `[SEQUENTIAL]`

**Files:**
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (the `CrateMap` enum variant + its handler at ~lines 503-556)
- Create (generated): `contracts/ci/crate-build-map.v1.json`

- [ ] **Step 0: Pre-flight — confirm the variant + handler shape**

```bash
grep -n "CrateMap" crates/vox-cli/src/commands/graphify/mod.rs
sed -n '503,556p' crates/vox-cli/src/commands/graphify/mod.rs
```
Expected: a `CrateMap { no_refresh_graph: bool }` variant and a handler that reads `crate-graph.v1.json` + optional `graphify-out/crate_audit.json`, calls `build_crate_map`, writes graph.json + manifest. Confirm `audit` is a `serde_json::Value` in scope. If the handler differs materially, STOP.

- [ ] **Step 1: Add `write_summary` to the `CrateMap` enum variant**

Replace the `CrateMap { no_refresh_graph: bool }` variant with:

```rust
    /// Build the crate build-time × dependency map into `.vox/cache/graphify/crate-map/`.
    /// With `--write-summary`, also emit the committed gate SSOT.
    CrateMap {
        /// Skip regenerating contracts/ci/crate-graph.v1.json from cargo metadata.
        #[arg(long)]
        no_refresh_graph: bool,
        /// Also write the committed SSOT to this path
        /// (bare flag → contracts/ci/crate-build-map.v1.json).
        #[arg(long, num_args = 0..=1, default_missing_value = "contracts/ci/crate-build-map.v1.json")]
        write_summary: Option<String>,
    },
```

- [ ] **Step 2: Extend the handler to write the summary**

Change the match arm pattern from `GraphifyCmd::CrateMap { no_refresh_graph }` to `GraphifyCmd::CrateMap { no_refresh_graph, write_summary }`. Then insert this block immediately BEFORE the final `println!("persist for agent recall: ...");` line:

```rust
            // 4. Optionally emit the committed gate SSOT (small; parity-checked in CI).
            if let Some(summary_path) = write_summary {
                use std::collections::HashMap;
                let mut compile_times: HashMap<String, f64> = HashMap::new();
                if let Some(arr) = audit.as_array() {
                    for r in arr {
                        if let (Some(name), Some(cs)) = (
                            r.get("crate").and_then(|v| v.as_str()),
                            r.get("compile_s").and_then(|v| {
                                v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                            }),
                        ) {
                            compile_times.insert(name.to_string(), cs);
                        }
                    }
                }
                let summary = vox_graphify_reader::crate_model::build_crate_summary(
                    &crate_graph,
                    &compile_times,
                );
                let summary_abs = repo_root.join(&summary_path);
                std::fs::write(&summary_abs, serde_json::to_string_pretty(&summary)?)
                    .with_context(|| format!("write {}", summary_abs.display()))?;
                let has_times = summary["has_compile_times"].as_bool().unwrap_or(false);
                println!(
                    "summary -> {} (has_compile_times={has_times}, missing={})",
                    summary_path, summary["crates_without_compile_times"]
                );
                if !has_times {
                    println!(
                        "WARNING: no compile times — run scripts/crate-build-audit.vox first \
                         (needs `cargo build --timings` to populate target/cargo-timings/)."
                    );
                }
            }
```

- [ ] **Step 3: Build + generate the real committed file**

```bash
cargo build -p vox-cli 2>&1 | tail -3
# Bring the local audit into the worktree so compile_s is populated (gitignored, not copied by worktree):
mkdir -p graphify-out
cp /c/Users/Owner/vox/graphify-out/crate_audit.json graphify-out/crate_audit.json
./target/debug/vox graphify crate-map --no-refresh-graph --write-summary 2>&1 | tail -5
```
Expected final line: `summary -> contracts/ci/crate-build-map.v1.json (has_compile_times=true, missing=4)`.

- [ ] **Step 4: Verify the SSOT has real numbers**

```bash
python -c "import json;d=json.load(open('contracts/ci/crate-build-map.v1.json'));b={c['crate']:c['blast_s'] for c in d['crates']};print({k:b.get(k) for k in ['workspace-hack','vox-db','vox-compiler','vox-populi']})"
```
Expected: `{'workspace-hack': 492.0, 'vox-db': 349.0, 'vox-compiler': 349.0, 'vox-populi': 349.0}` (record exact values — Task 4 calibrates to them).

- [ ] **Step 5: Verification ritual + commit**

```bash
cargo test -p vox-cli --lib 2>&1 | tail -5      # compiles + existing tests pass
cargo clippy -p vox-cli -- -D warnings 2>&1 | tail -5
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/graphify/mod.rs contracts/ci/crate-build-map.v1.json
git commit -m "feat(graphify): crate-map --write-summary emits committed gate SSOT

Generates contracts/ci/crate-build-map.v1.json (compile_s input + derived
blast_s/dependents/fan_in). Replaces the gitignored .vox/cache graph as the
gate input so CI is deterministic. Warns loudly when compile times are absent.

Co-Authored-By: AI Assistant <noreply@anthropic.com>"
```

---

### Task 3: Repoint `crate-budget` to the committed SSOT, fail loud `[SEQUENTIAL]`

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/crate_budget.rs`

- [ ] **Step 0: Pre-flight — confirm current structure**

```bash
grep -n "fn run_crate_budget\|fn check_keystones\|struct BudgetFile\|struct KeystoneBudget\|fn blast_s_from_nodes\|zero_blast_s_never_violates" crates/vox-cli/src/commands/ci/crate_budget.rs
```
Expected: `run_crate_budget(root, exit_zero)`, `check_keystones(budget, blast_map)`, `BudgetFile`/`KeystoneBudget`, plus the to-be-removed `blast_s_from_nodes` + `zero_blast_s_never_violates` test. Keep `check_keystones`, `BudgetFile`, `KeystoneBudget`, and their `make_budget`/`make_map` tests unchanged.

- [ ] **Step 1: Add the failing test**

In the existing `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn blast_map_from_summary_parses_committed_shape() {
        let summary = serde_json::json!({
            "has_compile_times": true,
            "crates": [
                { "crate": "vox-db",  "compile_s": 34.0, "dependents": 25, "blast_s": 349.0, "fan_in": 5 },
                { "crate": "vox-cli", "compile_s": 10.0, "dependents": 0,  "blast_s": 10.0,  "fan_in": 0 }
            ]
        });
        let m = blast_map_from_summary(&summary);
        assert_eq!(m.get("vox-db").copied(), Some(349.0));
        assert_eq!(m.get("vox-cli").copied(), Some(10.0));
    }
```

- [ ] **Step 2: Run test — verify it FAILS**

Run: `cargo test -p vox-cli --lib -- crate_budget::tests::blast_map_from_summary 2>&1 | tail -8`
Expected: FAIL — `cannot find function blast_map_from_summary`.

- [ ] **Step 3: Replace the reader + `run_crate_budget`**

Replace the module doc comment (top of file) with:

```rust
//! `vox ci crate-budget` — gate on keystone blast-radius-seconds.
//!
//! Reads the committed SSOT `contracts/ci/crate-build-map.v1.json` (produced by
//! `vox graphify crate-map --write-summary`) and fails when any keystone in
//! `contracts/ci/crate-budget.v1.json` exceeds its `blast_s_ceiling`. Fails loud when the
//! SSOT is missing or count-only (`has_compile_times=false`). `--exit-zero` → advisory.
```

Delete the `CrateMapNode` struct, the `CrateMap` struct, and `blast_s_from_nodes`. Replace `run_crate_budget` (and add the new reader) with:

```rust
/// Extract `crate -> blast_s` from a parsed crate-build-map summary value.
pub fn blast_map_from_summary(summary: &serde_json::Value) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    if let Some(arr) = summary.get("crates").and_then(|v| v.as_array()) {
        for c in arr {
            if let (Some(name), Some(b)) = (
                c.get("crate").and_then(|v| v.as_str()),
                c.get("blast_s").and_then(|v| v.as_f64()),
            ) {
                out.insert(name.to_string(), b);
            }
        }
    }
    out
}

pub fn run_crate_budget(root: &Path, exit_zero: bool) -> Result<()> {
    let budget_path = root.join("contracts/ci/crate-budget.v1.json");
    let budget: BudgetFile = serde_json::from_str(
        &std::fs::read_to_string(&budget_path)
            .with_context(|| format!("read {}", budget_path.display()))?,
    )
    .with_context(|| format!("parse {}", budget_path.display()))?;

    // FAIL LOUD when the SSOT is missing — never silently skip the gate.
    let summary_path = root.join("contracts/ci/crate-build-map.v1.json");
    let summary: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&summary_path).with_context(|| {
            format!(
                "read {} — regenerate with `vox graphify crate-map --write-summary`",
                summary_path.display()
            )
        })?,
    )
    .with_context(|| format!("parse {}", summary_path.display()))?;

    // FAIL LOUD when blast_s is count-only — a green gate that can't fail is worse than none.
    let has_times = summary.get("has_compile_times").and_then(|v| v.as_bool()).unwrap_or(false);
    if !has_times && !exit_zero {
        anyhow::bail!(
            "{} has has_compile_times=false (count-only blast_s) — gate would be toothless. \
             Run scripts/crate-build-audit.vox then `vox graphify crate-map --write-summary`.",
            summary_path.display()
        );
    }

    let blast_map = blast_map_from_summary(&summary);

    for k in &budget.keystones {
        match blast_map.get(&k.crate_name) {
            Some(&actual) => println!(
                "{}  {} blast_s={:.0}s (ceiling {:.0}s)",
                if actual > k.blast_s_ceiling { "OVER" } else { "OK  " },
                k.crate_name, actual, k.blast_s_ceiling
            ),
            None => eprintln!("WARN: keystone '{}' not in crate-build-map (renamed?)", k.crate_name),
        }
    }

    let violations = check_keystones(&budget, &blast_map);
    if violations.is_empty() {
        println!("crate-budget: all keystones within budget.");
        return Ok(());
    }
    eprintln!("crate-budget VIOLATIONS ({}):", violations.len());
    for v in &violations {
        eprintln!("{v}");
    }
    if exit_zero {
        eprintln!("(advisory — exiting 0 due to --exit-zero)");
        return Ok(());
    }
    anyhow::bail!("{} keystone crate(s) exceed blast-radius budget", violations.len())
}
```

- [ ] **Step 4: Delete the obsolete test**

Remove the `zero_blast_s_never_violates` test (it encoded the removed silent-skip semantics). Leave `no_violations_when_all_within_budget`, `violation_when_over_ceiling`, `missing_crate_does_not_fail`, `exactly_at_ceiling_is_ok`, and the new `blast_map_from_summary` test.

- [ ] **Step 5: Run tests + smoke-test**

```bash
cargo test -p vox-cli --lib -- crate_budget 2>&1 | tail -12
./target/debug/vox ci crate-budget 2>&1 | tail -8
```
Expected: tests PASS; smoke prints per-keystone lines. (May show `OVER` until Task 4 recalibrates — that is acceptable here; Task 4 fixes it. The point is it no longer silently skips.)

- [ ] **Step 6: Verification ritual + commit**

```bash
cargo clippy -p vox-cli -- -D warnings 2>&1 | tail -5
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/crate_budget.rs
git commit -m "fix(ci): crate-budget reads committed SSOT + fails loud

Repoints from gitignored .vox/cache graph to contracts/ci/crate-build-map.v1.json
so the gate runs in CI; errors (not silent skip) when the SSOT is missing or
count-only. Closes the toothless-gate hole.

Co-Authored-By: AI Assistant <noreply@anthropic.com>"
```

---

### Task 4: Recalibrate keystone ceilings to real blast_s × 1.15 `[SEQUENTIAL]`

**Files:**
- Modify: `contracts/ci/crate-budget.v1.json`

- [ ] **Step 1: Read actual blast_s from the committed SSOT**

```bash
python -c "import json;d=json.load(open('contracts/ci/crate-build-map.v1.json'));b={c['crate']:c['blast_s'] for c in d['crates']};[print(k,b.get(k),'-> ceil',round(b.get(k,0)*1.15)) for k in ['workspace-hack','vox-db','vox-compiler','vox-populi']]"
```
Record the `-> ceil` values (with verified data: 492→566, 349→401, 349→401, 349→401).

- [ ] **Step 2: Rewrite the budget file**

Replace `contracts/ci/crate-budget.v1.json` (substitute the Step-1 ceilings if they differ):

```json
{
  "schema_version": 1,
  "_comment": "blast_s ceilings = actual (contracts/ci/crate-build-map.v1.json, 2026-06-19) x 1.15. Keystones are the heavy, frequently-changed L3 crates — NOT high-fan-in leaf type crates (vox-mesh-types/vox-crypto rank high on blast_s but rarely change; see crate-build-dependency-model doc, blast_s churn-blindness). Lower ceilings as T3 type extractions shrink these.",
  "keystones": [
    { "crate": "workspace-hack", "blast_s_ceiling": 566 },
    { "crate": "vox-db",         "blast_s_ceiling": 401 },
    { "crate": "vox-compiler",   "blast_s_ceiling": 401 },
    { "crate": "vox-populi",     "blast_s_ceiling": 401 }
  ]
}
```

- [ ] **Step 3: Verify the gate is green AND can fail**

```bash
./target/debug/vox ci crate-budget 2>&1 | tail -6   # expect: all OK + "within budget"
# Prove it has teeth: temporarily set vox-db ceiling to 300, re-run, expect OVER + nonzero exit, then REVERT:
python -c "import json;p='contracts/ci/crate-budget.v1.json';d=json.load(open(p));d['keystones'][1]['blast_s_ceiling']=300;json.dump(d,open(p,'w'),indent=2)"
./target/debug/vox ci crate-budget; echo "exit=$?"   # expect: OVER vox-db, exit=1
git checkout contracts/ci/crate-budget.v1.json       # discard the probe
```
Then re-apply Step 2 (the probe checkout reverted it). Expected: probe shows `exit=1`; final file has the 566/401 ceilings.

- [ ] **Step 4: Commit**

```bash
git add contracts/ci/crate-budget.v1.json
git commit -m "fix(ci): recalibrate crate-budget ceilings to actual blast_s x 1.15

Old ceilings (445/437/440) were ~28% above actual (349) — gate could never
fire. Reset to measured-2026-06-19 x 1.15. Documents leaf-crate exclusion.

Co-Authored-By: AI Assistant <noreply@anthropic.com>"
```

---

### Task 5: `crate-build-map-parity` drift gate `[SEQUENTIAL]`

**Files:**
- Create: `crates/vox-cli/src/commands/ci/crate_build_map_parity.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs`, `cmd_enums.rs`, `run_body.rs`

- [ ] **Step 0: Pre-flight — confirm registration templates**

```bash
grep -n "pub mod crate_budget" crates/vox-cli/src/commands/ci/mod.rs
grep -n "CrateBudget {" crates/vox-cli/src/commands/ci/cmd_enums.rs
grep -n "CiCmd::CrateBudget" crates/vox-cli/src/commands/ci/run_body.rs
```
Expected: `pub mod crate_budget;` in mod.rs; `CrateBudget { exit_zero: bool }` variant; `CiCmd::CrateBudget { exit_zero } => super::crate_budget::run_crate_budget(&root, exit_zero),` dispatch. Use these as the exact style template.

- [ ] **Step 1: Write the parity module + failing tests**

Create `crates/vox-cli/src/commands/ci/crate_build_map_parity.rs`:

```rust
//! `vox ci crate-build-map-parity` — drift gate for the committed crate-build SSOT.
//!
//! Recomputes derived fields (dependents/blast_s/fan_in) from committed
//! `crate-graph.v1.json` + the `compile_s` embedded in `crate-build-map.v1.json`, then
//! compares to the committed derived values. Fails on drift (e.g. a Cargo.toml dep changed
//! but the summary wasn't regenerated). Mirrors crate-graph / config-registry parity gates.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Compare committed vs recomputed summary. Returns drift messages (empty = in sync).
/// Only DERIVED fields are compared; `compile_s` is the input, taken from committed as-is.
pub fn diff_summaries(committed: &serde_json::Value, recomputed: &serde_json::Value) -> Vec<String> {
    let idx = |v: &serde_json::Value| -> HashMap<String, (i64, i64, i64)> {
        let mut m = HashMap::new();
        if let Some(arr) = v.get("crates").and_then(|x| x.as_array()) {
            for c in arr {
                let name = c.get("crate").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let dep = c.get("dependents").and_then(|x| x.as_i64()).unwrap_or(-1);
                let blast = c.get("blast_s").and_then(|x| x.as_f64()).unwrap_or(-1.0).round() as i64;
                let fan = c.get("fan_in").and_then(|x| x.as_i64()).unwrap_or(-1);
                m.insert(name, (dep, blast, fan));
            }
        }
        m
    };
    let a = idx(committed);
    let b = idx(recomputed);
    let mut drift = Vec::new();
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    for k in keys {
        match (a.get(k), b.get(k)) {
            (Some(x), Some(y)) if x != y => drift.push(format!(
                "  {k}: committed (dep={},blast={},fan={}) != recomputed (dep={},blast={},fan={})",
                x.0, x.1, x.2, y.0, y.1, y.2
            )),
            (Some(_), None) => drift.push(format!("  {k}: in committed summary but not recomputed")),
            (None, Some(_)) => drift.push(format!("  {k}: recomputed but missing from committed (regen needed)")),
            _ => {}
        }
    }
    drift
}

pub fn run_crate_build_map_parity(root: &Path) -> Result<()> {
    let graph_path = root.join("contracts/ci/crate-graph.v1.json");
    let summary_path = root.join("contracts/ci/crate-build-map.v1.json");

    let crate_graph: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&graph_path)
            .with_context(|| format!("read {}", graph_path.display()))?,
    )?;
    let committed: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&summary_path).with_context(|| {
            format!("read {} — regenerate with `vox graphify crate-map --write-summary`", summary_path.display())
        })?,
    )?;

    // Pull compile_s (the input) back out so recomputation is deterministic.
    let mut compile_times: HashMap<String, f64> = HashMap::new();
    if let Some(arr) = committed.get("crates").and_then(|v| v.as_array()) {
        for c in arr {
            if let (Some(name), Some(cs)) = (
                c.get("crate").and_then(|v| v.as_str()),
                c.get("compile_s").and_then(|v| v.as_f64()),
            ) {
                compile_times.insert(name.to_string(), cs);
            }
        }
    }

    let recomputed =
        vox_graphify_reader::crate_model::build_crate_summary(&crate_graph, &compile_times);
    let drift = diff_summaries(&committed, &recomputed);

    if drift.is_empty() {
        println!("crate-build-map-parity: committed summary matches crate-graph.v1.json.");
        return Ok(());
    }
    eprintln!("crate-build-map-parity DRIFT ({}):", drift.len());
    for d in &drift {
        eprintln!("{d}");
    }
    anyhow::bail!(
        "crate-build-map.v1.json is stale vs crate-graph.v1.json — \
         run `vox graphify crate-map --write-summary` and commit the result"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn identical_summaries_have_no_drift() {
        let s = json!({ "crates": [
            { "crate": "a", "compile_s": 1.0, "dependents": 0, "blast_s": 1.0, "fan_in": 0 }
        ]});
        assert!(diff_summaries(&s, &s).is_empty());
    }

    #[test]
    fn dependent_count_drift_detected() {
        let committed = json!({ "crates": [
            { "crate": "a", "compile_s": 1.0, "dependents": 2, "blast_s": 6.0, "fan_in": 1 }
        ]});
        let recomputed = json!({ "crates": [
            { "crate": "a", "compile_s": 1.0, "dependents": 3, "blast_s": 9.0, "fan_in": 1 }
        ]});
        let d = diff_summaries(&committed, &recomputed);
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("a"));
    }

    #[test]
    fn new_crate_in_recompute_flagged() {
        let committed = json!({ "crates": [] });
        let recomputed = json!({ "crates": [
            { "crate": "newbie", "compile_s": 1.0, "dependents": 0, "blast_s": 1.0, "fan_in": 0 }
        ]});
        let d = diff_summaries(&committed, &recomputed);
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("newbie"));
    }
}
```

- [ ] **Step 2: Run tests — verify they FAIL (module not registered)**

Run: `cargo test -p vox-cli --lib -- crate_build_map_parity 2>&1 | tail -8`
Expected: FAIL — module not found.

- [ ] **Step 3: Register the module (mod.rs)**

In `crates/vox-cli/src/commands/ci/mod.rs`, add after `pub mod crate_budget;`:

```rust
pub mod crate_build_map_parity;
```

- [ ] **Step 4: Add the enum variant (cmd_enums.rs)**

After the `CrateBudget { .. }` variant block, add:

```rust
    /// Verify contracts/ci/crate-build-map.v1.json is in sync with crate-graph.v1.json
    /// (recomputes derived blast_s/dependents and fails on drift).
    #[command(name = "crate-build-map-parity")]
    CrateBuildMapParity,
```

- [ ] **Step 5: Add the dispatch arm (run_body.rs)**

After the `CiCmd::CrateBudget { exit_zero } => ...` arm, add:

```rust
        CiCmd::CrateBuildMapParity => {
            super::crate_build_map_parity::run_crate_build_map_parity(&root)
        }
```

- [ ] **Step 6: Run tests + smoke-test**

```bash
cargo test -p vox-cli --lib -- crate_build_map_parity 2>&1 | tail -10
./target/debug/vox ci crate-build-map-parity 2>&1 | tail -5
```
Expected: 3 tests pass; smoke prints `committed summary matches crate-graph.v1.json.` (it MUST — T2 generated both from the same committed inputs). If it reports drift, STOP — it means the round-trip invariant (T1 Step-1 test) is violated; investigate before proceeding.

- [ ] **Step 7: Verification ritual + commit**

```bash
cargo clippy -p vox-cli -- -D warnings 2>&1 | tail -5
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/crate_build_map_parity.rs crates/vox-cli/src/commands/ci/mod.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs
git commit -m "feat(ci): crate-build-map-parity drift gate

Recomputes derived blast_s/dependents/fan_in from committed crate-graph +
embedded compile_s; fails when crate-build-map.v1.json is stale vs the dep
graph. Self-policing SSOT like crate-graph/config-registry parity.

Co-Authored-By: AI Assistant <noreply@anthropic.com>"
```

---

### Task 6: Wire all four gates into CI `[SEQUENTIAL]`

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 0: Pre-flight — read the exact block**

```bash
grep -n "Dependency cycle gate\|ci dep-cycles\|Crate build/dependency audit\|Plugin-candidacy" .github/workflows/ci.yml
sed -n '912,925p' .github/workflows/ci.yml
```
Expected: a `Dependency cycle gate` step running `./target/debug/vox --quiet ci dep-cycles` (no flags), then a `Crate build/dependency audit` step with `continue-on-error: true`. Note the exact `if:` condition used (`needs.setup.outputs.full == 'true'`).

- [ ] **Step 1: Upgrade dep-cycles to `--deny-new`**

Change the `Dependency cycle gate` step's run line from:
```yaml
        run: ./target/debug/vox --quiet ci dep-cycles
```
to:
```yaml
        run: ./target/debug/vox --quiet ci dep-cycles --deny-new
```

- [ ] **Step 2: Add three gate steps AFTER the `Crate build/dependency audit` step**

Insert immediately after that step (do NOT add a step that regenerates `contracts/ci/crate-build-map.v1.json` — parity must read the committed file as-is, so regenerating it in CI would defeat the gate):

```yaml
      - name: Crate-build-map parity gate
        if: needs.setup.outputs.full == 'true'
        run: ./target/debug/vox --quiet ci crate-build-map-parity

      - name: Crate blast-radius budget gate
        if: needs.setup.outputs.full == 'true'
        run: ./target/debug/vox --quiet ci crate-budget

      - name: Fan-in budget gate
        if: needs.setup.outputs.full == 'true'
        run: ./target/debug/vox --quiet ci fan-in-budget
```

> The committed `crate-build-map.v1.json` is the SSOT. `crate-build-map-parity` recomputes derived fields from the committed `crate-graph.v1.json` + embedded `compile_s`, so it is deterministic and catches dep-graph drift even without a compile-time refresh. `compile_s` is refreshed manually (see Runbook), not per-PR.

- [ ] **Step 3: Validate YAML**

```bash
python -c "import yaml;yaml.safe_load(open('.github/workflows/ci.yml'));print('YAML OK')"
```
Expected: `YAML OK`.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: wire crate-budget, fan-in-budget, crate-build-map-parity + dep-cycles --deny-new

Three of four crate-build gates were never invoked in CI; dep-cycles ran
without --deny-new. Wires all four after the existing audit step. Parity is
deterministic (recomputes from committed inputs; no in-CI regeneration).

Co-Authored-By: AI Assistant <noreply@anthropic.com>"
```

---

### Task 7: Ingest the crate-map so it is agent-queryable `[SEQUENTIAL]`

**Files:**
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (refactor `Ingest` arm into a shared fn; add `--ingest` to `CrateMap`)

- [ ] **Step 0: Pre-flight — read the Ingest arm + its helpers**

```bash
sed -n '306,332p' crates/vox-cli/src/commands/graphify/mod.rs
grep -n "fn resolve_ingest_corpus_id\|fn load_projected_nodes\|fn upsert_projected_nodes\|fn corpus_by_id\|set_lexical_ingest_sha256\|fn graph_digest\|load_all_corpora" crates/vox-cli/src/commands/graphify/mod.rs
```
Expected: the `Ingest { corpus, dry_run }` arm body using `load_all_corpora`, `resolve_ingest_corpus_id`, `load_projected_nodes`, `upsert_projected_nodes`, `corpus_by_id`, `vox_config::graphify::set_lexical_ingest_sha256`, `vox_graphify_reader::graph_digest`. Confirm these exact names before refactoring.

- [ ] **Step 1: Extract the Ingest body into a shared async fn**

Add this free function near the other helpers in `graphify/mod.rs` (e.g. after `regenerate_crate_graph`), copying the EXACT logic from the `Ingest` arm read in Step 0 (adjust names if Step 0 showed different ones):

```rust
/// Project a corpus's graph nodes into Turso and stamp lexical_ingest_sha256.
/// Shared by `graphify ingest` and `graphify crate-map --ingest`.
async fn run_graphify_ingest(
    repo_root: &std::path::Path,
    corpus: Option<String>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let corpus_id =
        resolve_ingest_corpus_id(&reg, corpus).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let nodes = load_projected_nodes(repo_root, &reg, &corpus_id)?;
    if dry_run {
        println!("dry-run: corpus={corpus_id} nodes={}", nodes.len());
        return Ok(());
    }
    let upserted = upsert_projected_nodes(&nodes).await?;
    println!("graphify ingest: corpus={corpus_id} upserted={upserted}");
    let corpus = corpus_by_id(&reg, &corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let graph_bytes = std::fs::read(repo_root.join(&corpus.graph_path))
        .with_context(|| format!("read graph for digest: {}", corpus.graph_path))?;
    let digest = vox_graphify_reader::graph_digest(&graph_bytes);
    vox_config::graphify::set_lexical_ingest_sha256(
        &repo_root.join(&corpus.manifest_path),
        &digest,
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(())
}
```

Then replace the `GraphifyCmd::Ingest { corpus, dry_run } => { ... }` arm body with:

```rust
        GraphifyCmd::Ingest { corpus, dry_run } => {
            run_graphify_ingest(repo_root, corpus, dry_run).await?;
        }
```

- [ ] **Step 2: Add `--ingest` to the `CrateMap` variant**

Add a third field to the `CrateMap` variant (which after T2 has `no_refresh_graph` + `write_summary`):

```rust
        /// After building, project the crate-map into Turso for agent recall
        /// (stamps lexical_ingest_sha256).
        #[arg(long)]
        ingest: bool,
```

- [ ] **Step 3: Call ingest at the end of the CrateMap handler**

Update the match pattern to `GraphifyCmd::CrateMap { no_refresh_graph, write_summary, ingest }` and, after the `println!("persist for agent recall: ...");` line, add:

```rust
            if ingest {
                run_graphify_ingest(repo_root, Some("crate-map".to_string()), false).await?;
                println!("ingested crate-map (lexical_ingest_sha256 stamped)");
            }
```

- [ ] **Step 4: Build, run full priming, verify stamp**

```bash
cargo build -p vox-cli 2>&1 | tail -3
./target/debug/vox graphify crate-map --no-refresh-graph --write-summary --ingest 2>&1 | tail -6
python -c "import json;m=json.load(open('.vox/cache/graphify/crate-map/.graphify_manifest.v1.json'));print('lexical_ingest_sha256:',m['lexical_ingest_sha256'])"
```
Expected: a non-null sha (was `null`). **If ingest requires a running DB that is unavailable in the worktree**, capture the error and mark this task DONE_WITH_CONCERNS — the `--ingest` flag + shared fn are correct and will run wherever the DB is reachable (e.g. CI/dev with Turso configured). Do NOT fake the stamp.

- [ ] **Step 5: Verification ritual + commit**

```bash
cargo test -p vox-cli --lib 2>&1 | tail -5
cargo clippy -p vox-cli -- -D warnings 2>&1 | tail -5
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/graphify/mod.rs
git commit -m "feat(graphify): crate-map --ingest + shared ingest fn

Refactors the Ingest arm into run_graphify_ingest, reused by --ingest for
one-shot priming (build + write-summary + ingest). Stamps lexical_ingest_sha256
so vox_graphify_search/query recall crate-map nodes (build_time intent).

Co-Authored-By: AI Assistant <noreply@anthropic.com>"
```

---

### Task 8: Document blast_s semantics + update suite index `[PARALLEL-SAFE]`

**Files:**
- Modify: `docs/src/architecture/crate-build-dependency-model-2026-06-19.md`
- Modify: `docs/superpowers/plans/2026-06-19-crate-build-disentanglement-suite-index.md`

- [ ] **Step 0: Pre-flight — confirm both docs exist**

```bash
ls docs/src/architecture/crate-build-dependency-model-2026-06-19.md docs/superpowers/plans/2026-06-19-crate-build-disentanglement-suite-index.md
```
Expected: both paths exist. If the model doc is missing, STOP (it is the grounding SSOT referenced throughout).

- [ ] **Step 1: Append the semantics section to the model doc**

Append to the END of `docs/src/architecture/crate-build-dependency-model-2026-06-19.md`:

```markdown
## blast_s semantics & keystone selection (added 2026-06-19)

`blast_s(c) = compile_s(c) + Σ compile_s(d)` over all transitive dependents `d` of `c`
(reverse-BFS over the dep graph). It answers: "if `c` changes, how many compile-seconds of
downstream rebuild does that trigger?"

**Known limitation — churn-blindness.** `blast_s` weights by *fan-out × compile time*, not by
*how often a crate actually changes*. Stable pure-type leaf crates therefore rank high:
`vox-mesh-types` (419s) and `vox-crypto` (410s) outrank `vox-db`/`vox-compiler`/`vox-populi`
(349s each) despite changing far less. The three heavyweights are identical (349s) because they
share the same transitive-dependent closure — a cluster signature of the dependency tangle.

**Consequence for gating.** `contracts/ci/crate-budget.v1.json` gates only the heavy,
frequently-changed L3 crates (`vox-db`, `vox-compiler`, `vox-populi`, `workspace-hack`) — NOT
high-blast_s leaf type crates, which would produce false pressure to split stable code.

**SSOT + parity.** `contracts/ci/crate-build-map.v1.json` is the committed gate input
(`compile_s` from the audit + derived `dependents`/`blast_s`/`fan_in`). `vox ci crate-build-map-parity`
recomputes the derived fields from `crate-graph.v1.json` + embedded `compile_s` and fails on drift.
Refresh `compile_s` periodically via the runbook in the measurement-spine plan.

**Follow-on (out of scope):** a churn-weighted `blast_s_weighted = blast_s × commits_90d` mined
from `git log` would rank by real rebuild cost. Tracked, not yet built.
```

- [ ] **Step 2: Add a status row to the suite index**

In `docs/superpowers/plans/2026-06-19-crate-build-disentanglement-suite-index.md`, add to the Tracks table:

```markdown
| **T5** | [Measurement-spine hardening](2026-06-19-crate-build-measurement-spine-hardening.md) | **WRITTEN (9 tasks)** | Committed `crate-build-map.v1.json` SSOT (replaces gitignored cache); `crate-budget` fails loud + recalibrated ceilings (566/401); `crate-build-map-parity` drift gate; all four gates wired in CI (`--deny-new`, crate-budget, fan-in-budget, parity); crate-map ingested; blast_s churn-blindness documented. Runs in a clean worktree (committed HEAD builds; working tree has unrelated WIP). |
```

- [ ] **Step 3: Commit**

```bash
git add docs/src/architecture/crate-build-dependency-model-2026-06-19.md docs/superpowers/plans/2026-06-19-crate-build-disentanglement-suite-index.md
git commit -m "docs(crate-build): blast_s semantics + measurement-spine plan (T5) in suite index

Co-Authored-By: AI Assistant <noreply@anthropic.com>"
```

---

## Final integration check (after all tasks)

Run the whole gate set end-to-end through the binary, exactly as CI will:

```bash
cargo build -p vox-cli 2>&1 | tail -2
./target/debug/vox ci dep-cycles --deny-new   ; echo "dep-cycles=$?"
./target/debug/vox ci crate-build-map-parity  ; echo "parity=$?"
./target/debug/vox ci crate-budget            ; echo "budget=$?"
./target/debug/vox ci fan-in-budget           ; echo "fan-in=$?"
```
Expected: all four exit 0. Then use **superpowers:finishing-a-development-branch** to land the worktree branch (rebase/merge into `claude/auto-gui-debug-plans-2026-06-18` once its uncommitted WIP is resolved, or open a PR). Remove the worktree with `git worktree remove ../wt-spine` after merge.

---

## Runbook: refreshing compile times (periodic, manual — NOT per-PR)

`compile_s` is the only non-deterministic input; it needs a timed full build:

```bash
cargo build --workspace --timings 2>&1 | tail -3          # populates target/cargo-timings/cargo-timing.html
./target/debug/vox run --mode interp scripts/crate-build-audit.vox   # -> graphify-out/crate_audit.json
./target/debug/vox graphify crate-map --write-summary     # rebuild committed SSOT from fresh audit
./target/debug/vox ci crate-budget                        # sanity-check numbers
git add contracts/ci/crate-build-map.v1.json contracts/ci/crate-graph.v1.json
git commit -m "chore(ci): refresh crate-build compile-time snapshot"
```
Dep-topology drift (deps added/removed) is caught per-PR by `crate-build-map-parity` even without a compile-time refresh, because parity recomputes derived fields from the embedded `compile_s`.

---

## Self-Review

**1. Spec coverage (defects D1–D7 → tasks):**
- D1 stale/missing binary → T0 (worktree + build + crate-map check). ✓
- D2 gitignored cache, no regen → T2 (committed SSOT) + T3 (repoint). ✓
- D3 mis-calibrated ceilings → T4 (recalibrate + prove-it-can-fail probe). ✓
- D4 gates not in CI → T6. ✓
- D5 not ingested → T7. ✓
- D6 audit fragility → T2 embeds `compile_s`; T3 fails loud on count-only; Runbook covers refresh. ✓
- D7 churn-blind metric → T4 keystone selection + T8 docs. ✓
- **Build blocker (new, from audit)** → T0 worktree off committed HEAD. ✓

**2. Placeholder scan:** No "TBD"/"handle errors"/"similar to Task N". Code is complete in every code step. T7's extracted fn is copied verbatim from the Ingest arm read in its Pre-flight (names verified before edit).

**3. Type consistency:**
- `build_crate_summary(&Value, &HashMap<String,f64>) -> Value` — defined T1; called T2, T5, T7-runbook. ✓
- SSOT fields (`crate`, `compile_s`, `dependents`, `blast_s`, `fan_in`, `has_compile_times`, `crates_without_compile_times`) — consistent across T1 (emit), T3 (`blast_map_from_summary` + `has_compile_times` read), T5 (`diff_summaries` reads `dependents`/`blast_s`/`fan_in`; recompute pulls `compile_s`). ✓
- Command names `crate-build-map-parity` / `crate-budget` / `fan-in-budget` / `dep-cycles --deny-new` match registered clap names (verified in T5/T6 Pre-flights). ✓
- T3 deletes `CrateMapNode`/`CrateMap`/`blast_s_from_nodes` and the `zero_blast_s_never_violates` test that encoded removed semantics. ✓

**4. Code-review fixes applied vs first draft:**
- Removed the CI "Refresh SSOT before parity" step that would have made parity pass trivially. ✓
- Corrected clippy/test target to `vox-cli` (CI modules live there, not `vox-cli-ci`). ✓
- Added round-trip determinism test (T1) guaranteeing parity won't false-positive on fresh generation. ✓
- Made T7 concrete by refactoring the real Ingest arm into a shared fn. ✓
- Added T0 worktree to dodge the uncommitted build-breaking WIP. ✓
- `crate-budget` parses the summary once (not twice). ✓

No gaps found.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-19-crate-build-measurement-spine-hardening.md`. Two execution options:

**1. Subagent-Driven (recommended)** — fresh subagent per task, two-stage review (spec then quality) between tasks.

**2. Inline Execution** — execute in this session via executing-plans, batch with checkpoints.

Which approach?
