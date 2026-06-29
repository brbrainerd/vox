# CodeRabbit Review — Date-Scoped Importance Sweep + GUI (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a date-scoped, importance-ranked CodeRabbit repo sweep and a GUI review panel to the `coderabbit` subsystem that already lives on `main`.

**Architecture:** Purely additive on `main`. The `coderabbit` CLI module and the `vox-db external_review` schema are already merged behind cargo feature `coderabbit`. We (1) fix the `limits.rs` constants, (2) add `--since`, (3) add an importance ranker (`--top`/`--rank-weights`) using the existing `vox-graph-reader` crate, (4) re-sort planned chunks by importance, (5) add async GUI Tauri commands + a React panel, (6) ensure the GUI sidecar is built with the feature.

**Tech Stack:** Rust (vox-cli `coderabbit` feature, vox-graph-reader), git CLI, Tauri 2 (`crates/vox-gui/src/`), React 19/TS (`crates/vox-gui/ui`, pnpm).

**Spec:** `docs/superpowers/specs/2026-06-29-coderabbit-review-gui-and-sweep-design.md`

**CRITICAL — feature flag:** `coderabbit` is NOT a default feature. EVERY cargo command in this plan must include `--features coderabbit`. The `claude/skill-discovery-engine` branch is obsolete (schema v77, 669 behind) — **never copy from it**.

---

## Pre-flight

- [ ] **Create a worktree off current `main`** (vox-broker shim breaks `cargo` in the main dir):

```bash
cd /c/Users/Owner/vox
git fetch origin
git worktree add -b claude/coderabbit-review ../vox-coderabbit origin/main
cd ../vox-coderabbit
```

- [ ] **Confirm the module is present and builds with the feature:**

```bash
ls crates/vox-cli/src/commands/review/coderabbit/limits.rs   # exists
cargo build -p vox-cli --features coderabbit 2>&1 | tail -5    # PASS
```

---

## Task 1: Fix the limits.rs constants

**Files:**
- Modify: `crates/vox-cli/src/commands/review/coderabbit/limits.rs`

- [ ] **Step 1: Update the test expectations first (TDD)**

In the `tests` module, set the corrected values:

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

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p vox-cli --features coderabbit --lib coderabbit::limits 2>&1 | tail -20`
Expected: FAIL (code still returns 300/450).

- [ ] **Step 3: Fix the constants**

In `files_per_review`: `Pro | Enterprise => 150`.
In `reviews_per_hour`: `Pro => 5` (keep `Enterprise => 12` with `// unverified`).
In `recommended_max_files_per_pr`: `Pro | Enterprise => 140`.
Update the enum doc comments to match; delete "(summary only)" on `Free`. Set the
header `Last verified: 2026-06-29` and cite owner-observed Pro=150 + FAQ Pro=5/hr.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p vox-cli --features coderabbit --lib coderabbit::limits 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit/limits.rs
git commit -m "fix(coderabbit): correct Pro tier limits (150 files, 5/hr, 720s, 140 rec)"
```

---

## Task 2: `--since` date-scoped candidate collection

**Files:**
- Modify: `crates/vox-cli/src/commands/review/coderabbit/semantic_planner/collector.rs`
- Modify: `.../semantic_planner/types.rs` (`SemanticSubmitConfig`)
- Modify: `.../semantic_planner/mod.rs` (`pub use`)
- Modify: `.../semantic_planner/submit.rs` (branch at lines 29-37)
- Modify: `.../coderabbit/mod.rs` (clap flag)

- [ ] **Step 1: Write the failing test in `collector.rs`**

```rust
#[tokio::test]
async fn modified_since_lists_recent_files() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let run = |args: &[&str]| { std::process::Command::new("git").current_dir(p).args(args).output().unwrap(); };
    run(&["init", "-q"]); run(&["config","user.email","t@t"]); run(&["config","user.name","t"]);
    std::fs::write(p.join("old.rs"), "x").unwrap();
    run(&["add","-A"]); run(&["commit","-qm","old","--date=2020-01-01T00:00:00"]);
    std::fs::write(p.join("new.rs"), "y").unwrap();
    run(&["add","-A"]); run(&["commit","-qm","new"]);
    let files = collect_files_modified_since(p, "1 day ago").await.unwrap();
    assert!(files.iter().any(|f| f.ends_with("new.rs")));
    assert!(!files.iter().any(|f| f.ends_with("old.rs")));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p vox-cli --features coderabbit --lib modified_since_lists_recent_files 2>&1 | tail -20`
Expected: FAIL (not defined).

- [ ] **Step 3: Implement (async, mirrors existing collectors)**

```rust
use anyhow::{Context, Result};

/// Files added/copied/modified/renamed since `since` (any git date expr).
pub async fn collect_files_modified_since(repo: &std::path::Path, since: &str) -> Result<Vec<String>> {
    let out = tokio::process::Command::new("git")
        .current_dir(repo)
        .args(["log", &format!("--since={since}"), "--name-only",
               "--diff-filter=ACMR", "--pretty=format:"])
        .output().await.context("git log --since")?;
    anyhow::ensure!(out.status.success(), "git log --since failed");
    let mut seen = std::collections::BTreeSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let p = line.trim();
        if !p.is_empty() {
            seen.insert(super::super::path_policy::normalize_repo_rel_path(p));
        }
    }
    Ok(seen.into_iter().collect())
}
```

(Adjust the `path_policy` path to match the existing `use` style in `collector.rs`.)

- [ ] **Step 4: Export + config field + flag + branch**

In `mod.rs`: add `collect_files_modified_since` to the `pub use collector::{…}`.
In `types.rs` `SemanticSubmitConfig`: add `pub since: Option<String>,`.
In `coderabbit/mod.rs` `semantic-submit` args: add `#[arg(long)] since: Option<String>,`
and pass it into the config.
In `submit.rs:29-37`, make it the top-priority branch:

```rust
let mut all_files = if let Some(since) = cfg.since.as_deref() {
    collect_files_modified_since(repo, since).await.context("collect files since date")?
} else if cfg.full_repo {
    collect_all_files(repo).await.context("collect all tracked files")?
} else {
    collect_changed_files(repo).await.context("collect changed files")?
};
```

- [ ] **Step 5: Verify**

Run: `cargo test -p vox-cli --features coderabbit --lib modified_since_lists_recent_files 2>&1 | tail -10` → PASS.
Run: `cargo run -p vox-cli --features coderabbit -- review coderabbit semantic-submit --since "2026-04-01" 2>&1 | tail -10` → writes `.coderabbit/semantic-manifest.json`, no PRs (plan-only default).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit
git commit -m "feat(coderabbit): --since date-scoped candidate selection"
```

---

## Task 3: Importance ranker (`ranker.rs`)

**Files:**
- Create: `crates/vox-cli/src/commands/review/coderabbit/ranker.rs`
- Modify: `coderabbit/mod.rs` (`mod ranker;`)
- Modify: `crates/vox-cli/Cargo.toml` (`vox-graph-reader` dep under `coderabbit` feature)

- [ ] **Step 1: Add the dependency (feature-gated)**

In `crates/vox-cli/Cargo.toml`: add `vox-graph-reader = { workspace = true, optional = true }`
and append `"dep:vox-graph-reader"` to the `coderabbit = [ … ]` feature list.

- [ ] **Step 2: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn churn_dominates_and_degrades_without_graph() {
        let recency: HashMap<String,f64> = [("a.rs".into(),1.0),("b.rs".into(),1.0)].into();
        let churn:   HashMap<String,u64> = [("a.rs".into(),10),("b.rs".into(),100)].into();
        let files = vec!["a.rs".to_string(), "b.rs".to_string()];
        let ranked = rank_files(&files, &recency, &churn, None, RankWeights::default());
        assert_eq!(ranked[0], "b.rs");
    }

    #[test]
    fn file_part_strips_symbol_and_worktree() {
        assert_eq!(file_of_node("crates/x/a.rs::foo"), "crates/x/a.rs");
        assert_eq!(file_of_node(".claude/worktrees/w1/crates/x/a.rs::foo"), "crates/x/a.rs");
    }
}
```

- [ ] **Step 3: Run to verify fail**

Run: `cargo test -p vox-cli --features coderabbit --lib coderabbit::ranker 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 4: Implement**

```rust
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Copy)]
pub struct RankWeights { pub recency: f64, pub churn: f64, pub centrality: f64 }
impl Default for RankWeights { fn default() -> Self { Self { recency: 1.0, churn: 1.0, centrality: 1.0 } } }

/// "<file>::<symbol>" -> "<file>", stripping a leading ".claude/worktrees/<seg>/".
pub(crate) fn file_of_node(id: &str) -> String {
    let file = id.split("::").next().unwrap_or(id);
    if let Some(rest) = file.strip_prefix(".claude/worktrees/") {
        if let Some((_, tail)) = rest.split_once('/') { return tail.to_string(); }
    }
    file.to_string()
}

fn norm(map: &HashMap<String, f64>, key: &str, max: f64) -> f64 {
    if max <= 0.0 { 0.0 } else { map.get(key).copied().unwrap_or(0.0) / max }
}

pub fn rank_files(
    files: &[String],
    recency: &HashMap<String, f64>,
    churn: &HashMap<String, u64>,
    centrality: Option<&HashMap<String, f64>>,
    w: RankWeights,
) -> Vec<String> {
    let churn_f: HashMap<String, f64> = churn.iter().map(|(k, v)| (k.clone(), *v as f64)).collect();
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

/// File-aggregated node degree from the graphify cache. None on any failure or zero matches.
pub fn load_file_centrality(repo: &Path) -> Option<HashMap<String, f64>> {
    let path = repo.join(".vox/cache/graphify/repo-code-graph/graph.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let reader = vox_graph_reader::GraphifyReader::from_value(value).ok()?;
    let n = reader.node_count();
    let mut by_file: HashMap<String, f64> = HashMap::new();
    for (id, deg) in reader.god_nodes(n) {
        *by_file.entry(file_of_node(&id)).or_insert(0.0) += deg as f64;
    }
    if by_file.is_empty() { None } else { Some(by_file) }
}
```

> Verified: `vox-graph-reader` exposes `from_value`, `node_count`, `god_nodes(top_n) -> Vec<(String, usize)>`. Graph nodes are `file::symbol` with no stored degree (degree is computed by `god_nodes`).

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p vox-cli --features coderabbit --lib coderabbit::ranker 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit/ranker.rs crates/vox-cli/src/commands/review/coderabbit/mod.rs crates/vox-cli/Cargo.toml
git commit -m "feat(coderabbit): importance ranker (recency+churn+graph centrality)"
```

---

## Task 4: git churn/recency helpers + wire `--top`/`--rank-weights`/`--rank-order`

**Files:**
- Modify: `collector.rs` (helpers), `types.rs` (config), `mod.rs` (flags), `submit.rs` (apply)

- [ ] **Step 1: Add the two async helpers in `collector.rs` (with tests)**

```rust
/// Sum of (insertions+deletions) per file since `since`.
pub async fn churn_since(repo: &std::path::Path, since: &str) -> Result<std::collections::HashMap<String,u64>> {
    let out = tokio::process::Command::new("git").current_dir(repo)
        .args(["log", &format!("--since={since}"), "--numstat", "--pretty=format:"])
        .output().await.context("git log --numstat")?;
    let mut m = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.splitn(3, '\t');
        let (a, b, p) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        if p.is_empty() { continue; }
        let w = a.parse::<u64>().unwrap_or(0) + b.parse::<u64>().unwrap_or(0);
        *m.entry(super::super::path_policy::normalize_repo_rel_path(p)).or_insert(0) += w;
    }
    Ok(m)
}

/// Count of commits touching each file since `since` (recency proxy).
pub async fn recency_since(repo: &std::path::Path, since: &str) -> Result<std::collections::HashMap<String,f64>> {
    let out = tokio::process::Command::new("git").current_dir(repo)
        .args(["log", &format!("--since={since}"), "--name-only", "--pretty=format:"])
        .output().await.context("git log --name-only")?;
    let mut m = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let p = line.trim();
        if !p.is_empty() { *m.entry(super::super::path_policy::normalize_repo_rel_path(p)).or_insert(0.0) += 1.0; }
    }
    Ok(m)
}
```

Add a `#[tokio::test]` that commits a file twice and asserts `churn_since` and
`recency_since` both report it. Run with `--features coderabbit`; expect PASS after impl.

- [ ] **Step 2: Add config fields + flags**

`types.rs` `SemanticSubmitConfig`: add `pub top: Option<usize>`,
`pub rank_weights: ranker::RankWeights`, `pub rank_order: bool`.
`coderabbit/mod.rs` `semantic-submit` args: `#[arg(long)] top: Option<usize>`,
`#[arg(long)] rank_weights: Option<String>`, `#[arg(long)] rank_order: Option<bool>`.
Write a small `parse_rank_weights(&str) -> RankWeights` (split `"r,c,g"` on commas,
parse 3 f64, default missing to 1.0). Default `rank_order` to
`top.is_some() || rank_weights.is_some() || since.is_some()`.

- [ ] **Step 3: Apply ranking after collection in `run_semantic_submit`** (after the Task 2 branch, before `planner.plan(...)`)

```rust
if cfg.top.is_some() || cfg.rank_weights.recency != 1.0 || cfg.rank_weights.churn != 1.0
   || cfg.rank_weights.centrality != 1.0 || cfg.since.is_some() {
    let win = cfg.since.as_deref().unwrap_or("3 months ago");
    let churn = collector::churn_since(repo, win).await?;
    let recency = collector::recency_since(repo, win).await?;
    let central = if cfg.rank_weights.centrality > 0.0 { ranker::load_file_centrality(repo) } else { None };
    let mut ranked = ranker::rank_files(&all_files, &recency, &churn, central.as_ref(), cfg.rank_weights);
    if let Some(n) = cfg.top { ranked.truncate(n); }
    all_files = ranked;
}
```

- [ ] **Step 4: Verify**

Run: `cargo run -p vox-cli --features coderabbit -- review coderabbit semantic-submit --since "2026-04-01" --top 300 --rank-weights 1,2,1 2>&1 | tail -10`
Expected: manifest with ≤300 files.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit
git commit -m "feat(coderabbit): --top/--rank-weights importance selection"
```

---

## Task 5: Importance-first chunk order (post-plan re-sort)

**Files:**
- Modify: `semantic_planner/submit.rs` (after `planner.plan(...)`, gated by `cfg.rank_order`)
- Test: `semantic_planner/mod.rs` tests

- [ ] **Step 1: Write the failing test** (use the public `SemanticChunk` + a helper)

```rust
#[test]
fn reorder_chunks_by_aggregate_score_desc() {
    let score: std::collections::HashMap<String,f64> =
        [("a".into(),10.0),("b".into(),1.0),("c".into(),5.0)].into();
    let mut chunks = vec![
        SemanticChunk { order: 1, name: "low".into(), files: vec!["b".into()] },
        SemanticChunk { order: 2, name: "high".into(), files: vec!["a".into()] },
        SemanticChunk { order: 3, name: "mid".into(), files: vec!["c".into()] },
    ];
    reorder_chunks_by_score(&mut chunks, &score);
    assert_eq!(chunks[0].name, "high");
    assert_eq!(chunks[2].name, "low");
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p vox-cli --features coderabbit --lib reorder_chunks_by_aggregate 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 3: Implement helper + call it**

```rust
pub(crate) fn reorder_chunks_by_score(
    chunks: &mut [SemanticChunk],
    score: &std::collections::HashMap<String, f64>,
) {
    let agg = |c: &SemanticChunk| -> f64 {
        if c.files.is_empty() { return 0.0; }
        c.files.iter().map(|f| score.get(f).copied().unwrap_or(0.0)).sum::<f64>() / c.files.len() as f64
    };
    chunks.sort_by(|a, b| agg(b).partial_cmp(&agg(a)).unwrap_or(std::cmp::Ordering::Equal)
        .then(a.order.cmp(&b.order)));
}
```

In `run_semantic_submit`, when `cfg.rank_order` and ranking ran, build a
`score: HashMap<String,f64>` from the same recency/churn/central inputs (reuse a
`ranker::score_map(...)` extracted from `rank_files`), then call
`reorder_chunks_by_score(&mut manifest.chunks, &score)` before the submit loop. (Add
`ranker::score_map` returning the per-file score map; have `rank_files` call it.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p vox-cli --features coderabbit --lib reorder_chunks_by_aggregate 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/review/coderabbit
git commit -m "feat(coderabbit): --rank-order puts highest-importance PRs first"
```

---

## Task 6: GUI sidecar feature build + async Tauri commands

**Files:**
- Create: `crates/vox-gui/src/commands/coderabbit.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs` (`pub mod coderabbit;`), `crates/vox-gui/src/main.rs` (`generate_handler!`)

- [ ] **Step 1: Confirm the registration + sidecar patterns**

Read `crates/vox-gui/src/commands/execute.rs` (sidecar shell-out pattern) and
`crates/vox-gui/src/commands/research.rs::start_research_async` (return-immediately +
background). Note the exact `app.shell().sidecar("vox")` call and the
`app_handle.emit(...)` usage.

- [ ] **Step 2: Write the command module** (async; mirror `execute.rs`)

```rust
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;
use serde_json::Value;

async fn vox(app: &AppHandle, args: Vec<String>) -> Result<String, String> {
    let out = app.shell().sidecar("vox").map_err(|e| e.to_string())?
        .args(args).output().await.map_err(|e| e.to_string())?;
    if !out.status.success() { return Err(String::from_utf8_lossy(&out.stderr).into()); }
    Ok(String::from_utf8_lossy(&out.stdout).into())
}

#[tauri::command]
pub async fn coderabbit_plan(app: AppHandle, since: String, cap: u32, rank_weights: String) -> Result<Value, String> {
    vox(&app, vec!["review".into(),"coderabbit".into(),"semantic-submit".into(),
        "--since".into(),since,"--max-files-per-pr".into(),cap.to_string(),
        "--rank-weights".into(),rank_weights]).await?;
    let m = std::fs::read_to_string(".coderabbit/semantic-manifest.json").map_err(|e| e.to_string())?;
    serde_json::from_str(&m).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coderabbit_run_async(app: AppHandle, since: String, cap: u32, rank_weights: String, top: u32) -> Result<Value, String> {
    tauri::async_runtime::spawn(async move {
        let res = vox(&app, vec!["review".into(),"coderabbit".into(),"semantic-submit".into(),
            "--since".into(),since,"--top".into(),top.to_string(),
            "--max-files-per-pr".into(),cap.to_string(),"--rank-weights".into(),rank_weights,
            "--execute".into()]).await;
        let payload = match &res { Ok(_) => serde_json::json!({"status":"done"}),
                                   Err(e) => serde_json::json!({"status":"error","error":e}) };
        let _ = tauri::Emitter::emit(&app, "coderabbit://progress", payload);
    });
    Ok(serde_json::json!({"status":"running"}))
}

#[tauri::command]
pub async fn coderabbit_report(app: AppHandle) -> Result<Value, String> {
    let run_state = std::fs::read_to_string(".coderabbit/run-state.json").ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null);
    let db = vox(&app, vec!["review".into(),"coderabbit".into(),"db-status".into(),"--json".into()]).await
        .ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null);
    Ok(serde_json::json!({"run_state": run_state, "db_status": db}))
}

#[tauri::command]
pub fn coderabbit_token_present() -> bool {
    std::env::var("FORGE_TOKEN").or_else(|_| std::env::var("GITHUB_TOKEN")).is_ok()
}
```

> Adjust `ShellExt`/`Emitter` imports to match what `execute.rs`/`research.rs` already
> import (the repo's Tauri version may re-export these differently — copy their `use`).

- [ ] **Step 3: Register the four commands** in `main.rs` `generate_handler![…]` and `pub mod coderabbit;` in `commands/mod.rs`.

- [ ] **Step 4: Build the GUI Rust side with the sidecar built for the feature**

```bash
cargo build -p vox-cli --features coderabbit --release   # produces target/release/vox sidecar
cargo build -p vox-gui 2>&1 | tail -10
```
Expected: PASS. (Document that release packaging must build the sidecar with
`--features coderabbit`; if a packaging script pins sidecar features, add the flag.)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/src
git commit -m "feat(gui): async CodeRabbit Tauri commands (sidecar shell-out)"
```

---

## Task 7: React panel + nav

**Files:**
- Create: `crates/vox-gui/ui/src/views/CodeRabbitReview.tsx`
- Modify: `crates/vox-gui/ui/src/lib/navigation.ts`, `crates/vox-gui/ui/src/App.tsx`
- Test: `crates/vox-gui/ui/src/views/CodeRabbitReview.test.ts` (data mapping)

- [ ] **Step 1: Add the route** — entry in `PARENT_CHILD_MAP` + `NAV_LABELS` in `navigation.ts`; add `'coderabbit'` to the `View` union in `App.tsx`; render `<CodeRabbitReview/>` for that view (mirror an existing view's wiring).

- [ ] **Step 2: Build the panel** from the approved mockup (`coderabbit_review_panel_vox_gui`):
`<input type="date">` (modified since), numeric cap (default 140), ranking weight
chips, Plan/Run buttons, dry-run note, slice list with importance bars + status pills,
5/hr budget line, findings metric cards. Wire:
`invoke('coderabbit_plan', { since, cap, rankWeights })`,
`invoke('coderabbit_run_async', {…})`, `invoke('coderabbit_report')`,
`invoke('coderabbit_token_present')`, and
`listen('coderabbit://progress', …)` for run updates. Use existing vox-gui ("Limes")
tokens; map run-state chunk `status` to pill colors.

- [ ] **Step 3: Add the mapping unit test**

Write a vitest that maps a sample `{run_state:{chunks:[…]}, db_status:{…}}` to slice
rows (name, file count, status). Run: `cd crates/vox-gui/ui && pnpm test 2>&1 | tail -15`
Expected: PASS. (pnpm only — never npm.)

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui
git commit -m "feat(gui): CodeRabbit review panel"
```

---

## Task 8: Docs + full verification

**Files:**
- Modify: `docs/src/reference/cli.md` (document `--since`, `--top`, `--rank-weights`, `--rank-order`) — rerun the CLI-surface generator; never hand-edit `*.generated.md`.

- [ ] **Step 1: Document the new flags** in the hand-authored CLI doc; rerun the generator if one exists (search `cli-command-surface.generated`).

- [ ] **Step 2: Full verification**

```bash
cargo test -p vox-cli --features coderabbit 2>&1 | tail -15          # PASS
cargo clippy -p vox-cli --features coderabbit -- -D warnings 2>&1 | tail -20   # clean
cargo build -p vox-gui 2>&1 | tail -5                                 # PASS (clippy on vox-gui needs --exclude vox-gui workspace-wide)
cd crates/vox-gui/ui && pnpm test 2>&1 | tail -10                     # PASS
```

- [ ] **Step 3: Commit + push + PR**

```bash
git add -A && git commit -m "docs(coderabbit): document --since/--top/--rank-weights/--rank-order"
git push -u origin claude/coderabbit-review
gh pr create --fill --title "feat(coderabbit): date-scoped importance sweep + GUI review panel"
```

---

## Self-Review notes (author)

- **Spec coverage:** limits fix (T1), `--since` (T2), ranker+`--top`/weights (T3/T4),
  importance-first order (T5), sidecar feature + async Tauri commands incl.
  non-blocking `--execute` (T6), GUI panel + nav + read-only token (T7), docs/cadence
  (T8). Bring-forward/schema/backoff intentionally absent (audit: already-on-main /
  impossible-by-construction).
- **Placeholder scan:** real code in every code step; the only "match existing `use`"
  notes are in T6 (Tauri import paths) and T2/T4 (`path_policy` use style) — genuine
  per-repo style alignment, not deferred logic.
- **Type consistency:** `RankWeights`, `rank_files`, `score_map`, `load_file_centrality`,
  `file_of_node`, `collect_files_modified_since`, `churn_since`, `recency_since`,
  `reorder_chunks_by_score`, `SemanticSubmitConfig.{since,top,rank_weights,rank_order}`,
  `SemanticChunk{order,name,files}` used consistently across T2–T7.
- **Feature flag:** every cargo invocation carries `--features coderabbit`; the sidecar
  build (T6) is the single point that makes the GUI path functional.
- **Residual assumption:** graph node ids are `file::symbol` and worktree paths start
  `.claude/worktrees/<seg>/` — guarded by graceful-degrade (None on zero matches) and a
  unit test (`file_part_strips_symbol_and_worktree`).
