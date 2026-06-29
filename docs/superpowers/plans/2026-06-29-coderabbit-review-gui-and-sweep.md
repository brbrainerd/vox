# CodeRabbit Review (Bring-Forward + Date-Scoped Importance Sweep + GUI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the stranded CodeRabbit subsystem onto `main`, then add a date-scoped, importance-ranked repo-sweep and a GUI review panel.

**Architecture:** Copy the self-contained `coderabbit` module + `vox-db` `external_review` schema + `vox-code-audit/review` from branch `claude/skill-discovery-engine` onto a fresh branch off `main`, fixing the `limits.rs` constants in the same pass. Then extend `semantic-submit` with `--since` (git-log date scope), `--top N` + an importance ranker (recency + churn + graphify centrality, graceful-degrade), adaptive backoff on oversized-PR rejection, and a vox-gui panel backed by three shell-out Tauri commands.

**Tech Stack:** Rust (vox-cli, vox-db, vox-code-audit), git CLI, SQLite (VoxDB), Tauri 2 + React 19/TS (vox-gui), CodeRabbit (GitHub App).

**Spec:** `docs/superpowers/specs/2026-06-29-coderabbit-review-gui-and-sweep-design.md`

**Source of truth for existing code:** worktree `wt-skill-discovery-engine` (branch `claude/skill-discovery-engine`). Paths below without a worktree prefix are on the new branch off `main`.

---

## Pre-flight (do once, before Task 1)

- [ ] **Create the worktree off current `main`** (vox-broker shim breaks `cargo` in the main dir):

```bash
cd /c/Users/Owner/vox
git fetch origin
git worktree add -b claude/coderabbit-review ../vox-coderabbit origin/main
cd ../vox-coderabbit
```

All subsequent work happens in `../vox-coderabbit`. `$SRC` below = `/c/Users/Owner/vox/wt-skill-discovery-engine`.

---

## Phase A — Bring-forward (foundation)

### Task A1: Copy the coderabbit CLI module

**Files:**
- Create: `crates/vox-cli/src/commands/review/coderabbit/**` (entire tree)
- Modify: `crates/vox-cli/src/commands/review/mod.rs` (register the `coderabbit` subcommand)

- [ ] **Step 1: Copy the module tree**

```bash
mkdir -p crates/vox-cli/src/commands/review
cp -r "$SRC/crates/vox-cli/src/commands/review/coderabbit" crates/vox-cli/src/commands/review/
```

- [ ] **Step 2: Wire the subcommand into `review/mod.rs`**

Open `$SRC/crates/vox-cli/src/commands/review/mod.rs`, copy the `pub mod coderabbit;` declaration and the `Coderabbit(...)` clap variant + its dispatch arm into the new branch's `review/mod.rs`. Keep the existing `dei` module intact.

- [ ] **Step 3: Build (expect failures — deps not yet copied)**

Run: `cargo build -p vox-cli 2>&1 | tail -30`
Expected: errors referencing `vox_db` external_review items and/or `vox_code_audit::review`. That is fine — fixed in A2/A3.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli/src/commands/review
git commit -m "feat(coderabbit): copy CLI module from skill-discovery-engine"
```

### Task A2: Copy the vox-db external_review schema + store

**Files:**
- Create: `crates/vox-db/src/schema/domains/external_review.rs`
- Create: `crates/vox-db/src/store/ops_external_review.rs`
- Create: `crates/vox-db/tests/ops_external_review_tests.rs`
- Modify: `crates/vox-db/src/schema/domains/mod.rs`, `crates/vox-db/src/store/mod.rs` (register modules)

- [ ] **Step 1: Copy files**

```bash
cp "$SRC/crates/vox-db/src/schema/domains/external_review.rs" crates/vox-db/src/schema/domains/
cp "$SRC/crates/vox-db/src/store/ops_external_review.rs" crates/vox-db/src/store/
cp "$SRC/crates/vox-db/tests/ops_external_review_tests.rs" crates/vox-db/tests/
```

- [ ] **Step 2: Register modules**

Add `pub mod external_review;` to `crates/vox-db/src/schema/domains/mod.rs` and `pub mod ops_external_review;` to `crates/vox-db/src/store/mod.rs` (match the exact `pub mod`/`mod` style already used in each file). If the schema is registered in a migration/registry list (search `domains::` references in `schema/mod.rs`), add `external_review` there too — copy how a sibling domain is registered.

- [ ] **Step 3: Build vox-db**

Run: `cargo build -p vox-db 2>&1 | tail -30`
Expected: PASS (or only `vox-cli`-side errors remain).

- [ ] **Step 4: Run the DB tests**

Run: `cargo test -p vox-db ops_external_review 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-db
git commit -m "feat(coderabbit): copy external_review schema + store into vox-db"
```

### Task A3: Copy vox-code-audit review types + contracts

**Files:**
- Create: `crates/vox-code-audit/src/review/**`
- Modify: `crates/vox-code-audit/src/lib.rs` (register `review` module)
- Create: `contracts/review/coderabbit-semantic-groups.v1.yaml`

- [ ] **Step 1: Copy files**

```bash
cp -r "$SRC/crates/vox-code-audit/src/review" crates/vox-code-audit/src/
mkdir -p contracts/review
cp "$SRC/contracts/review/coderabbit-semantic-groups.v1.yaml" contracts/review/ 2>/dev/null || \
  find "$SRC" -name 'coderabbit-semantic-groups*.yaml' -exec cp {} contracts/review/ \;
```

- [ ] **Step 2: Register module**

Add `pub mod review;` to `crates/vox-code-audit/src/lib.rs` if not already present (check `$SRC` version for the exact line).

- [ ] **Step 3: Build the workspace touched crates**

Run: `cargo build -p vox-code-audit -p vox-cli 2>&1 | tail -40`
Expected: PASS. Resolve any remaining import-path drift by matching `$SRC`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-code-audit contracts/review
git commit -m "feat(coderabbit): copy review types + semantic-groups contract"
```

### Task A4: Fix the limits.rs constants (the verified bugs)

**Files:**
- Modify: `crates/vox-cli/src/commands/review/coderabbit/limits.rs`

- [ ] **Step 1: Update the failing-value tests first (TDD)**

In the `tests` module of `limits.rs`, change the expectations to the corrected values:

```rust
#[test]
fn tier_files_per_review() {
    assert_eq!(CodeRabbitTier::Free.files_per_review(), 150);
    assert_eq!(CodeRabbitTier::Pro.files_per_review(), 150);
}

#[test]
fn tier_min_delay_secs() {
    assert_eq!(CodeRabbitTier::Pro.min_delay_between_prs_secs(), 720);
}

#[test]
fn clamp_max_respects_tier_cap() {
    assert_eq!(clamp_max_files_per_pr(CodeRabbitTier::Pro, 500), 150);
    assert_eq!(clamp_max_files_per_pr(CodeRabbitTier::Oss, 500), 150);
    assert_eq!(clamp_max_files_per_pr(CodeRabbitTier::Pro, 0), 1);
}

#[test]
fn clamp_batch_caps_both_bounded() {
    let (max, hard) = clamp_batch_caps(CodeRabbitTier::Pro, 400, 500);
    assert_eq!(hard, 150);
    assert_eq!(max, 150);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-cli --lib coderabbit::limits 2>&1 | tail -20`
Expected: FAIL (current code returns 300 / 450).

- [ ] **Step 3: Fix the constants**

In `files_per_review`: `Pro | Enterprise => 150`.
In `reviews_per_hour`: `Pro => 5` (leave `Enterprise => 12` with a `// unverified` comment).
In `recommended_max_files_per_pr`: `Pro | Enterprise => 140`.
Update the doc comments on the `Pro`/`Enterprise`/`Free` enum variants to match; delete the "(summary only)" note on `Free` (reviews are full on all tiers). Update the `Last verified` header to `2026-06-29` and cite owner-observed Pro cap = 150 + FAQ Pro = 5/hr.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-cli --lib coderabbit::limits 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit/limits.rs
git commit -m "fix(coderabbit): correct Pro tier limits (150 files, 5/hr, 720s delay)"
```

### Task A5: Green the e2e + clippy, then PR the foundation

**Files:**
- Create: `crates/vox-cli/tests/coderabbit_e2e.rs` (copy)

- [ ] **Step 1: Copy the e2e test**

```bash
cp "$SRC/crates/vox-cli/tests/coderabbit_e2e.rs" crates/vox-cli/tests/
```

- [ ] **Step 2: Run e2e + clippy on touched crates**

Run: `cargo test -p vox-cli --test coderabbit_e2e 2>&1 | tail -20`
Expected: PASS.
Run: `cargo clippy -p vox-cli -p vox-db -p vox-code-audit -- -D warnings 2>&1 | tail -30`
Expected: no warnings. (Per house rule, do NOT `cargo fmt --all`; use `cargo fmt -p <crate>` on each touched crate.)

- [ ] **Step 3: Commit + push + open foundation PR**

```bash
git add -A && git commit -m "test(coderabbit): bring forward e2e test"
git push -u origin claude/coderabbit-review
gh pr create --fill --title "feat(coderabbit): bring review subsystem to main + fix Pro limits"
```

---

## Phase B — Date scope (`--since`)

### Task B1: `collect_files_modified_since`

**Files:**
- Modify: `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/collector.rs`
- Test: same file (`#[cfg(test)]` module) or `semantic_planner/mod.rs` tests

- [ ] **Step 1: Write the failing test**

Add to `collector.rs` tests (use the existing test-repo helper in this module if present; otherwise init a temp git repo):

```rust
#[test]
fn modified_since_lists_recent_files() {
    let repo = init_temp_repo_with_two_commits(); // helper: commit "old.rs" then "new.rs"
    let files = collect_files_modified_since(repo.path(), "1 hour ago").unwrap();
    assert!(files.iter().any(|f| f.ends_with("new.rs")));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cli --lib modified_since_lists_recent_files 2>&1 | tail -20`
Expected: FAIL (function not defined).

- [ ] **Step 3: Implement**

```rust
/// Files added/copied/modified/renamed since `since` (any git date expr, e.g.
/// "2026-04-01" or "2 weeks ago"). Deletions excluded.
pub fn collect_files_modified_since(repo: &std::path::Path, since: &str) -> anyhow::Result<Vec<String>> {
    let out = std::process::Command::new("git")
        .current_dir(repo)
        .args(["log", &format!("--since={since}"), "--name-only",
               "--diff-filter=ACMR", "--pretty=format:"])
        .output()?;
    anyhow::ensure!(out.status.success(), "git log --since failed");
    let mut seen = std::collections::BTreeSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let p = line.trim();
        if !p.is_empty() { seen.insert(crate::commands::review::coderabbit::path_policy::normalize_repo_rel_path(p)); }
    }
    Ok(seen.into_iter().collect())
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-cli --lib modified_since_lists_recent_files 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit/semantic_planner/collector.rs
git commit -m "feat(coderabbit): collect_files_modified_since for date-scoped sweeps"
```

### Task B2: Wire `--since` into semantic-submit

**Files:**
- Modify: `crates/vox-cli/src/commands/review/coderabbit/mod.rs` (clap flag)
- Modify: `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/types.rs` (`SemanticSubmitConfig.since`)
- Modify: `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/submit.rs` (use it)

- [ ] **Step 1: Add the config field + flag**

In `SemanticSubmitConfig` add `pub since: Option<String>`. In `mod.rs` `semantic-submit` args add `#[arg(long)] since: Option<String>,` and pass it into the config.

- [ ] **Step 2: Use it in the collector branch of `run_semantic_submit`**

Where `submit.rs` currently chooses `collect_all_files` (full-repo) vs `collect_changed_files`, add a higher-priority branch:

```rust
let candidates = if let Some(since) = cfg.since.as_deref() {
    collect_files_modified_since(repo, since)?
} else if cfg.full_repo {
    collect_all_files(repo)?
} else {
    collect_changed_files(repo)?
};
```

- [ ] **Step 3: Build + manual smoke (plan-only)**

Run: `cargo run -p vox-cli -- review coderabbit semantic-submit --since "2026-04-01" --plan 2>&1 | tail -20`
Expected: writes `.coderabbit/semantic-manifest.json` scoped to recent files; no PRs opened.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit
git commit -m "feat(coderabbit): --since date-scoped candidate selection"
```

---

## Phase C — Importance ranker (`--top`)

### Task C1: Centrality loader (graceful-degrade)

**Files:**
- Create: `crates/vox-cli/src/commands/review/coderabbit/ranker.rs`
- Modify: `coderabbit/mod.rs` (`mod ranker;`)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn centrality_absent_returns_none() {
    let repo = tempfile::tempdir().unwrap();
    assert!(load_centrality(repo.path()).is_none());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cli --lib coderabbit::ranker::tests::centrality_absent 2>&1 | tail -20`
Expected: FAIL (not defined).

- [ ] **Step 3: Implement loader**

```rust
use std::collections::HashMap;
use std::path::Path;

pub type CentralityMap = HashMap<String, f64>;

/// Best-effort: read node degrees from an existing graphify graph under
/// `graphify-out/`. Any missing-file / parse error -> None (ranker drops the term).
pub fn load_centrality(repo: &Path) -> Option<CentralityMap> {
    let root = repo.join("graphify-out");
    let mut graph_json = None;
    for entry in walkdir_shallow(&root)? {
        if entry.file_name().to_string_lossy() == "graph.json" { graph_json = Some(entry.path()); break; }
    }
    let text = std::fs::read_to_string(graph_json?).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let nodes = v.get("nodes")?.as_array()?;
    let mut m = CentralityMap::new();
    for n in nodes {
        if let (Some(id), Some(deg)) = (n.get("id").and_then(|x| x.as_str()),
                                        n.get("degree").and_then(|x| x.as_f64())) {
            m.insert(crate::commands::review::coderabbit::path_policy::normalize_repo_rel_path(id), deg);
        }
    }
    if m.is_empty() { None } else { Some(m) }
}

fn walkdir_shallow(dir: &Path) -> Option<Vec<std::fs::DirEntry>> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).ok()? { let e = e.ok()?;
        if e.path().is_dir() { for inner in std::fs::read_dir(e.path()).ok()? { out.push(inner.ok()?); } }
        else { out.push(e); } }
    Some(out)
}
```

> Note: the graphify graph node schema (`id`/`degree`) is assumed. If the real schema differs, adjust the two `.get(...)` keys only; the graceful-degrade contract (None on any mismatch) keeps the rest safe.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-cli --lib coderabbit::ranker::tests::centrality_absent 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit/ranker.rs crates/vox-cli/src/commands/review/coderabbit/mod.rs
git commit -m "feat(coderabbit): centrality loader with graceful-degrade"
```

### Task C2: Rank + select top-N

**Files:**
- Modify: `crates/vox-cli/src/commands/review/coderabbit/ranker.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn rank_orders_by_weighted_signal_and_degrades_without_graph() {
    // higher churn => earlier; centrality term dropped when graph is None.
    let churn: HashMap<String,u64> = [("a.rs".into(),10u64),("b.rs".into(),100u64)].into();
    let recency: HashMap<String,f64> = [("a.rs".into(),1.0),("b.rs".into(),1.0)].into();
    let files = vec!["a.rs".to_string(), "b.rs".to_string()];
    let ranked = rank_files(&files, &recency, &churn, None, RankWeights::default());
    assert_eq!(ranked[0], "b.rs");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cli --lib rank_orders_by_weighted 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
#[derive(Clone, Copy)]
pub struct RankWeights { pub recency: f64, pub churn: f64, pub centrality: f64 }
impl Default for RankWeights { fn default() -> Self { Self { recency: 1.0, churn: 1.0, centrality: 1.0 } } }

fn norm(map: &HashMap<String, f64>, key: &str, max: f64) -> f64 {
    if max <= 0.0 { 0.0 } else { map.get(key).copied().unwrap_or(0.0) / max }
}

/// Returns `files` sorted by descending importance. `centrality = None` drops that
/// term and renormalizes implicitly (its weight contributes 0).
pub fn rank_files(
    files: &[String],
    recency: &HashMap<String, f64>,
    churn: &HashMap<String, u64>,
    centrality: Option<&CentralityMap>,
    w: RankWeights,
) -> Vec<String> {
    let churn_f: HashMap<String, f64> = churn.iter().map(|(k,v)| (k.clone(), *v as f64)).collect();
    let rmax = recency.values().cloned().fold(0.0, f64::max);
    let cmax = churn_f.values().cloned().fold(0.0, f64::max);
    let gmax = centrality.map(|g| g.values().cloned().fold(0.0, f64::max)).unwrap_or(0.0);
    let mut scored: Vec<(f64, String)> = files.iter().map(|f| {
        let mut s = w.recency * norm(recency, f, rmax) + w.churn * norm(&churn_f, f, cmax);
        if let Some(g) = centrality { s += w.centrality * norm(g, f, gmax); }
        (s, f.clone())
    }).collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, f)| f).collect()
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-cli --lib rank_orders_by_weighted 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit/ranker.rs
git commit -m "feat(coderabbit): weighted importance ranker"
```

### Task C3: Wire `--top` + `--rank-weights` into semantic-submit

**Files:**
- Modify: `coderabbit/mod.rs`, `semantic_planner/types.rs`, `semantic_planner/submit.rs`

- [ ] **Step 1: Add config + flags**

`SemanticSubmitConfig`: add `pub top: Option<usize>` and `pub rank_weights: ranker::RankWeights`. In `mod.rs`: `#[arg(long)] top: Option<usize>,` and `#[arg(long, value_parser = parse_weights)] rank_weights: Option<RankWeights>,` (write `parse_weights` to split `"r,c,g"` into three f64; default to `RankWeights::default()`).

- [ ] **Step 2: Apply ranking after candidate collection in `run_semantic_submit`**

```rust
if cfg.top.is_some() || cfg.since.is_some() {
    let churn = collect_churn(repo, cfg.since.as_deref())?; // numstat sums; reuse git::collect_git_diffs weights
    let recency = collect_recency(repo, cfg.since.as_deref())?; // commits-per-file count
    let central = ranker::load_centrality(repo);
    let mut ranked = ranker::rank_files(&candidates, &recency, &churn, central.as_ref(), cfg.rank_weights);
    if let Some(n) = cfg.top { ranked.truncate(n); }
    candidates = ranked;
}
```

Implement `collect_churn` and `collect_recency` as small helpers in `collector.rs` (parse `git log --numstat --since` and `git log --since --name-only` respectively; both keyed by normalized path). Ranking determines slice order (highest-importance PRs first) because the chunker preserves input order within a group.

- [ ] **Step 3: Smoke test**

Run: `cargo run -p vox-cli -- review coderabbit semantic-submit --since "2026-04-01" --top 300 --plan 2>&1 | tail -20`
Expected: manifest with ≤300 files, highest-importance chunks ordered first.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit
git commit -m "feat(coderabbit): --top N importance selection + --rank-weights"
```

---

## Phase D — Adaptive backoff

### Task D1: Split oversized chunk on rejection

**Files:**
- Modify: `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/submit.rs`
- Modify/Create test: `coderabbit/semantic_planner/mod.rs` tests

- [ ] **Step 1: Write the failing test for the split helper**

```rust
#[test]
fn split_chunk_halves_and_bounds() {
    let files: Vec<String> = (0..10).map(|i| format!("f{i}.rs")).collect();
    let (a, b) = split_chunk_files(&files);
    assert_eq!(a.len(), 5);
    assert_eq!(b.len(), 5);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cli --lib split_chunk_halves 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 3: Implement helper + wire into the per-chunk loop**

```rust
pub(crate) fn split_chunk_files(files: &[String]) -> (Vec<String>, Vec<String>) {
    let mid = files.len().div_ceil(2);
    (files[..mid].to_vec(), files[mid..].to_vec())
}
```

In the per-chunk submit loop: after `create_chunk_pr_via_worktree` + `wait`, if the review outcome reports oversized/cancelled (detect via the ingest/`wait` signal — a CodeRabbit comment containing the too-many-files marker), and the chunk has not already been split twice, split it and resubmit the two halves as new chunks; record `split_depth` in run-state. Cap at depth 2, then log a warning and continue.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-cli --lib split_chunk_halves 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit
git commit -m "feat(coderabbit): adaptive backoff splits oversized PRs (max depth 2)"
```

---

## Phase E — GUI panel + Tauri commands

### Task E1: Explore the existing vox-gui command + route patterns

- [ ] **Step 1: Find the registration pattern (no code yet)**

Run: `ls crates/vox-gui/src-tauri/src` and grep for an existing `#[tauri::command]` plus its `.invoke_handler(tauri::generate_handler![...])` registration, and find how a React route/panel is added under `crates/vox-gui/ui/src`. Record the exact files to mirror. (vox-gui frontend is pnpm-managed — never `npm`.)

### Task E2: Three shell-out Tauri commands

**Files:**
- Create: `crates/vox-gui/src-tauri/src/commands/coderabbit.rs`
- Modify: the Tauri command module index + `generate_handler!` list (paths from E1)

- [ ] **Step 1: Write the command module**

```rust
use std::process::Command;
use serde_json::Value;

fn vox_review(args: &[&str]) -> Result<String, String> {
    let out = Command::new("vox").args(["review", "coderabbit"]).args(args)
        .output().map_err(|e| e.to_string())?;
    if !out.status.success() { return Err(String::from_utf8_lossy(&out.stderr).into()); }
    Ok(String::from_utf8_lossy(&out.stdout).into())
}

#[tauri::command]
pub fn coderabbit_plan(since: String, cap: u32, rank_weights: String) -> Result<Value, String> {
    vox_review(&["semantic-submit", "--since", &since, "--max-files-per-pr",
                 &cap.to_string(), "--rank-weights", &rank_weights, "--plan"])?;
    let m = std::fs::read_to_string(".coderabbit/semantic-manifest.json").map_err(|e| e.to_string())?;
    serde_json::from_str(&m).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn coderabbit_run(since: String, cap: u32, rank_weights: String, top: u32) -> Result<Value, String> {
    let s = vox_review(&["semantic-submit", "--since", &since, "--top", &top.to_string(),
                 "--max-files-per-pr", &cap.to_string(), "--rank-weights", &rank_weights, "--execute"])?;
    Ok(Value::String(s))
}

#[tauri::command]
pub fn coderabbit_report() -> Result<Value, String> {
    let s = vox_review(&["db-report", "--json"])?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn coderabbit_token_present() -> bool {
    std::env::var("FORGE_TOKEN").or_else(|_| std::env::var("GITHUB_TOKEN")).is_ok()
}
```

- [ ] **Step 2: Register the four commands** in the `generate_handler!` list (pattern from E1).

- [ ] **Step 3: Build the Tauri side**

Run: `cargo build -p vox-gui 2>&1 | tail -20` (note: clippy on vox-gui needs `--exclude vox-gui` workspace-wide; build is fine).
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/src-tauri
git commit -m "feat(gui): coderabbit_plan/run/report/token Tauri commands"
```

### Task E3: React panel

**Files:**
- Create: `crates/vox-gui/ui/src/routes/CodeRabbitReview.tsx` (path style from E1)
- Modify: the route registry + nav entry (paths from E1)

- [ ] **Step 1: Build the panel** from the approved mockup
  (`coderabbit_review_panel_vox_gui` in the design session): a `<input type="date">` for "modified since", numeric cap (default 140), ranking chips (recency+churn, centrality), Plan/Run buttons, dry-run checkbox, the slice list with importance bars + status pills, the 5/hr budget line, and 3 findings metric cards. Use existing vox-gui ("Limes") design tokens; map status pills to added/modified/merged/changes colors. Wire buttons to `invoke('coderabbit_plan' | 'coderabbit_run' | 'coderabbit_report')`. Show a read-only "token: present/absent" indicator from `coderabbit_token_present`.

- [ ] **Step 2: Add the nav/route entry** mirroring an existing route.

- [ ] **Step 3: Typecheck + unit test the data mapping**

Run: `cd crates/vox-gui/ui && pnpm test 2>&1 | tail -20`
Add one vitest that maps a sample manifest JSON to slice rows (counts + status). Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui
git commit -m "feat(gui): CodeRabbit review panel"
```

### Task E4: Docs + final verification

**Files:**
- Modify: `docs/src/reference/cli.md` (document `--since`, `--top`, `--rank-weights`) — regenerate the generated CLI surface, do not hand-edit `*.generated.md`.

- [ ] **Step 1: Document the new flags** in the hand-authored CLI doc; rerun the CLI-surface generator if one exists (search `cli-command-surface.generated`).

- [ ] **Step 2: Full verification on touched crates**

Run: `cargo test -p vox-cli -p vox-db -p vox-code-audit 2>&1 | tail -20` → PASS.
Run: `cargo clippy -p vox-cli -p vox-db -p vox-code-audit -- -D warnings 2>&1 | tail -20` → clean.
Run: `cd crates/vox-gui/ui && pnpm test 2>&1 | tail -10` → PASS.

- [ ] **Step 3: Commit + push**

```bash
git add -A && git commit -m "docs(coderabbit): document --since/--top/--rank-weights"
git push
```

---

## Self-Review notes (author)

- **Spec coverage:** bring-forward (A1–A5), `--since` (B), ranker+`--top`+weights (C), backoff (D), GUI+Tauri+read-only token (E2/E3), limits bugs (A4), worktree-PR test gap (covered by manifest test in C3/D1 smoke — add a dedicated manifest integration test if the executing agent finds the existing e2e insufficient), cadence = manual (documented in E4). All spec sections map to a task.
- **Assumptions flagged inline:** graphify node schema (`id`/`degree`) in C1 and the oversized-rejection signal in D1 — both guarded by graceful-degrade / depth cap, so a wrong guess fails safe.
- **Type consistency:** `RankWeights`, `CentralityMap`, `rank_files`, `load_centrality`, `collect_files_modified_since`, `split_chunk_files`, `SemanticSubmitConfig.{since,top,rank_weights}` used consistently across B/C/D/E.
