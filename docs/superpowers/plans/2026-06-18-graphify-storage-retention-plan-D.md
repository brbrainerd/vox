# Plan D — Graphify Storage, Data-Size & Retention (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILLS: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `.../test-driven-development.skill.md`. Steps use `- [ ]`.

> **🤖 EXECUTION TARGET — READ FIRST.** Run by **Gemini 3.5 Flash inside Google Antigravity** (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context recall). Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md). Suite: [`2026-06-18-graphify-native-system-suite-index.md`](2026-06-18-graphify-native-system-suite-index.md).
> **DEPENDS ON Plan A** (rebuild writes `graph.json` + manifest). **Plan C recommended** (`refresh`). Land A (and ideally C) first.

## Operating Rules (apply to EVERY task)
1. **Atomic + green + committed.** A change that breaks compile is fixed within the same task.
2. **Verify-before-use.** First step is an `rg`/read confirming exact symbols. Differs → STOP.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Fails twice → STOP + handoff note. No looping.
5. **Parallel dispatch.** Honor tags; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all`; automation is `.vox`; `docs/src/` `.md` needs frontmatter; no stubs.
7. **Verification ritual** (skill `verification-before-completion`), paste output: `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`.
8. **Rollback on broken tree:** `git reset --hard HEAD`; re-attempt the single task.
9. **Skills:** `brainstorming` / `dispatching-parallel-agents` / `using-git-worktrees`.
10. **Determinism + no `.unwrap()` on I/O in lib code.** `cargo run -p vox-arch-check` passes before final commit.

**Goal:** Give corpora bounded, navigable history and a deterministic keep-vs-discard policy: snapshot graphs on rebuild, prune to the last N, expose a value-score + retention decision, and pick a coarse lens for oversized graphs.

**Architecture:** Two pure library modules in `vox-graphify-reader` (no time/DB deps — timestamps and signals are caller-supplied for deterministic tests): `snapshot.rs` (copy `graph.json`+manifest into `snapshots/<stamp>/`, list, prune-to-N) and `gc.rs` (`value_score`, `retention_decision`, `pick_lens`). The CLI snapshots+prunes around a rebuild and exposes `vox graphify gc --keep N`.

**Tech Stack:** Rust; `std::fs`; `chrono` (CLI only, for the snapshot stamp).

> **Scope note (deferred, not placeholder):** wiring `value_score` to *live* usage (search-log) needs a metadata-filtered DB query that does not exist in `vox-db` — that learning loop is a follow-on. Plan D ships the policy as pure, tested functions whose inputs the future loop will feed; the snapshot/prune lifecycle is fully wired and end-to-end now.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-graphify-reader/src/snapshot.rs` | snapshot / list / prune | Create (D1) |
| `crates/vox-graphify-reader/src/gc.rs` | value score / retention / lens pick | Create (D2) |
| `crates/vox-graphify-reader/src/lib.rs` | module registration | Modify (D1, D2) |
| `crates/vox-graphify-reader/tests/snapshot_tests.rs` | snapshot lifecycle | Create (D1) |
| `crates/vox-graphify-reader/tests/gc_tests.rs` | policy | Create (D2) |
| `crates/vox-cli/src/commands/graphify/mod.rs` | snapshot-on-rebuild + `gc` cmd | Modify (D3) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "pub mod " crates/vox-graphify-reader/src/lib.rs` — note where to add `pub mod snapshot;` / `pub mod gc;`.
- `rg -n "GraphifyCmd::Rebuild|fn resolve_source_dir|use chrono::Utc" crates/vox-cli/src/commands/graphify/mod.rs` — confirm the Rebuild arm + chrono import.
- `rg -n "repo_graphify_cache_dir" crates/vox-config/src/graphify.rs` — confirm `pub fn repo_graphify_cache_dir(repo_root, corpus_id)` exists (it does; in graphify.rs, NOT paths.rs).
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task D1 `[SEQUENTIAL]`: Snapshot module (copy / list / prune-to-N)

**Files:**
- Create: `crates/vox-graphify-reader/src/snapshot.rs`
- Modify: `crates/vox-graphify-reader/src/lib.rs`
- Test: `crates/vox-graphify-reader/tests/snapshot_tests.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub mod " crates/vox-graphify-reader/src/lib.rs`. Confirm the module list. STOP if the crate layout differs.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-graphify-reader/tests/snapshot_tests.rs`:

```rust
use std::fs;
use vox_graphify_reader::snapshot::{list_snapshots, prune_snapshots, snapshot_corpus};

fn seed(dir: &std::path::Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("graph.json"), "{\"nodes\":[]}").unwrap();
    fs::write(dir.join(".graphify_manifest.v1.json"), "{}").unwrap();
}

#[test]
fn snapshot_list_and_prune_keep_newest() {
    let tmp = tempfile::tempdir().unwrap();
    let corpus = tmp.path().join("corpus");
    seed(&corpus);

    // Stamps sort lexically; oldest first.
    for stamp in ["2026-06-01T00-00-00", "2026-06-02T00-00-00", "2026-06-03T00-00-00"] {
        let dst = snapshot_corpus(&corpus, stamp).unwrap();
        assert!(dst.join("graph.json").is_file(), "graph copied");
        assert!(dst.join(".graphify_manifest.v1.json").is_file(), "manifest copied");
    }
    assert_eq!(list_snapshots(&corpus).len(), 3);

    let removed = prune_snapshots(&corpus, 2).unwrap();
    assert_eq!(removed, 1);
    let kept = list_snapshots(&corpus);
    assert_eq!(kept, vec!["2026-06-02T00-00-00", "2026-06-03T00-00-00"]); // newest kept
}

#[test]
fn snapshot_of_missing_corpus_is_empty_but_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let corpus = tmp.path().join("none");
    fs::create_dir_all(&corpus).unwrap();
    let dst = snapshot_corpus(&corpus, "s1").unwrap(); // no graph.json present
    assert!(dst.is_dir());
    assert!(!dst.join("graph.json").exists());
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test snapshot_tests` → FAIL (no `snapshot` module).

- [ ] **Step 4: Create `snapshot.rs`.**

```rust
//! Bounded graph history: copy `graph.json` + manifest into `snapshots/<stamp>/`, list, prune-to-N.
//! Timestamps are caller-supplied (filesystem-safe, lexically sortable) so this stays pure + testable.
use std::fs;
use std::path::{Path, PathBuf};

const SNAPSHOT_FILES: [&str; 2] = ["graph.json", ".graphify_manifest.v1.json"];

/// Copy the corpus's current graph + manifest into `<corpus_dir>/snapshots/<stamp>/`.
/// Missing source files are skipped (a first-ever snapshot may be empty).
pub fn snapshot_corpus(corpus_dir: &Path, stamp: &str) -> std::io::Result<PathBuf> {
    let dst = corpus_dir.join("snapshots").join(stamp);
    fs::create_dir_all(&dst)?;
    for name in SNAPSHOT_FILES {
        let src = corpus_dir.join(name);
        if src.is_file() {
            fs::copy(&src, dst.join(name))?;
        }
    }
    Ok(dst)
}

/// Snapshot stamps, lexically sorted (oldest first). Empty if none.
pub fn list_snapshots(corpus_dir: &Path) -> Vec<String> {
    let base = corpus_dir.join("snapshots");
    let Ok(rd) = fs::read_dir(&base) else {
        return Vec::new();
    };
    let mut v: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    v.sort();
    v
}

/// Remove the oldest snapshots, keeping the newest `keep`. Returns how many were removed.
pub fn prune_snapshots(corpus_dir: &Path, keep: usize) -> std::io::Result<usize> {
    let snaps = list_snapshots(corpus_dir);
    if snaps.len() <= keep {
        return Ok(0);
    }
    let base = corpus_dir.join("snapshots");
    let mut removed = 0usize;
    for s in &snaps[..snaps.len() - keep] {
        fs::remove_dir_all(base.join(s))?;
        removed += 1;
    }
    Ok(removed)
}
```

- [ ] **Step 5: Register the module.** In `lib.rs`, add `pub mod snapshot;` to the module list.

- [ ] **Step 6: Run → PASS.** `cargo test -p vox-graphify-reader --test snapshot_tests` → PASS.

- [ ] **Step 7: Verify (Rule 7) + commit.**

```bash
git add crates/vox-graphify-reader/src/snapshot.rs crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/tests/snapshot_tests.rs
git commit -m "feat(graphify): snapshot history (copy/list/prune-to-N) for corpora"
```

---

## Task D2 `[SEQUENTIAL]` (shares lib.rs with D1): value-score + retention + lens pick

**Files:**
- Create: `crates/vox-graphify-reader/src/gc.rs`
- Modify: `crates/vox-graphify-reader/src/lib.rs`
- Test: `crates/vox-graphify-reader/tests/gc_tests.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub mod snapshot" crates/vox-graphify-reader/src/lib.rs`. Confirm D1 landed (the module list now has `snapshot`). STOP if not.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-graphify-reader/tests/gc_tests.rs`:

```rust
use vox_graphify_reader::gc::{pick_lens, retention_decision, value_score, Retention};

#[test]
fn value_score_rewards_usage_and_recency() {
    // more usage, same everything else → higher score
    assert!(value_score(100, 1.0, 0, 10.0) > value_score(1, 1.0, 0, 10.0));
    // more recent (fewer days since use) → higher score
    assert!(value_score(10, 0.0, 0, 10.0) > value_score(10, 30.0, 0, 10.0));
}

#[test]
fn retention_decision_boundaries() {
    assert_eq!(retention_decision(5.0, 2.0, 0.5), Retention::Maintain);
    assert_eq!(retention_decision(1.0, 2.0, 0.5), Retention::Expire);
    assert_eq!(retention_decision(0.2, 2.0, 0.5), Retention::Discard);
}

#[test]
fn pick_lens_switches_above_threshold() {
    assert_eq!(pick_lens(100, 50_000), "structural");
    assert_eq!(pick_lens(60_000, 50_000), "modules");
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test gc_tests` → FAIL (no `gc` module).

- [ ] **Step 4: Create `gc.rs`.**

```rust
//! Deterministic keep-vs-discard policy for corpora. Pure functions; the future learning loop
//! feeds real signals (usage from the search-log, churn from manifest diffs, cost from builds).

/// Higher = more worth maintaining. `usage` = recent query/search hits; `recency_days` = days
/// since last use; `churn` = node/community delta magnitude since last rebuild; `cost_secs` =
/// last build wall-time. Bounded, monotone in usage and recency.
pub fn value_score(usage: u64, recency_days: f64, churn: u64, cost_secs: f64) -> f64 {
    let usage_term = (usage as f64 + 1.0).ln();
    let recency_term = 1.0 / (1.0 + recency_days.max(0.0));
    let churn_term = (churn as f64 + 1.0).ln();
    let cost_penalty = 1.0 / (1.0 + cost_secs.max(0.0) / 60.0);
    usage_term * recency_term + 0.5 * churn_term * cost_penalty
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Retention {
    Maintain,
    Expire,
    Discard,
}

/// `score >= maintain_above` → keep fresh; `score < discard_below` → GC; else let TTL expire.
pub fn retention_decision(score: f64, maintain_above: f64, discard_below: f64) -> Retention {
    if score >= maintain_above {
        Retention::Maintain
    } else if score < discard_below {
        Retention::Discard
    } else {
        Retention::Expire
    }
}

/// Data-size escape hatch: above `threshold` nodes, prefer the coarse `modules` lens (Plan B).
pub fn pick_lens(node_count: usize, threshold: usize) -> &'static str {
    if node_count > threshold {
        "modules"
    } else {
        "structural"
    }
}
```

- [ ] **Step 5: Register the module.** In `lib.rs`, add `pub mod gc;`.

- [ ] **Step 6: Run → PASS.** `cargo test -p vox-graphify-reader --test gc_tests` → PASS.

- [ ] **Step 7: Verify (Rule 7) + commit.**

```bash
git add crates/vox-graphify-reader/src/gc.rs crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/tests/gc_tests.rs
git commit -m "feat(graphify): value-score + retention policy + data-size lens pick"
```

---

## Task D3 `[SEQUENTIAL]` (shares mod.rs with A/B/C): snapshot-on-rebuild + `gc` command

**Files:**
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "GraphifyCmd::Rebuild|let output_file = repo_root.join|enum GraphifyCmd|use chrono::Utc" crates/vox-cli/src/commands/graphify/mod.rs`. Confirm the Rebuild arm builds `output_file` and that `chrono::Utc` is imported.

- [ ] **Step 2: Snapshot the old graph before overwriting it.** In the `GraphifyCmd::Rebuild` arm, immediately BEFORE the `rebuild_graph(...)` call, insert:

```rust
            // Preserve the previous graph as a bounded history before overwriting.
            if output_file.is_file() {
                if let Some(corpus_dir) = output_file.parent() {
                    let stamp = Utc::now().to_rfc3339().replace(':', "-");
                    let _ = vox_graphify_reader::snapshot::snapshot_corpus(corpus_dir, &stamp);
                    let _ = vox_graphify_reader::snapshot::prune_snapshots(corpus_dir, 5);
                }
            }
```

(Snapshot failures are non-fatal — `let _ =` — a rebuild must not fail because history couldn't be copied.)

- [ ] **Step 3: Add the `Gc` subcommand.** Add to `GraphifyCmd` after the other variants:

```rust
    /// Prune corpus graph snapshots, keeping the newest N per corpus.
    Gc {
        /// Corpus id (default: all corpora).
        #[arg(long)]
        corpus: Option<String>,
        /// How many snapshots to keep per corpus.
        #[arg(long, default_value_t = 5)]
        keep: usize,
    },
```

Add the arm in `run()`:

```rust
        GraphifyCmd::Gc { corpus, keep } => {
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            for c in selected_corpora(&reg, &corpus).map_err(|e| anyhow::anyhow!(e.to_string()))? {
                let output_file = repo_root.join(&c.graph_path);
                if let Some(corpus_dir) = output_file.parent() {
                    let removed = vox_graphify_reader::snapshot::prune_snapshots(corpus_dir, keep)
                        .map_err(|e| anyhow::anyhow!("prune {}: {e}", c.id))?;
                    println!("gc {} kept<= {keep} removed={removed}", c.id);
                }
            }
        }
```

> If Plan B did NOT land, replace `load_all_corpora` with `load_graphify_corpora`. `selected_corpora` already exists in this file.

- [ ] **Step 4: Build + smoke.** `cargo build -p vox-cli` → clean. `cargo run -p vox-cli -- graphify rebuild --corpus repo-code-graph` twice → second run creates a `snapshots/<stamp>/` under the corpus dir. `cargo run -p vox-cli -- graphify gc --corpus repo-code-graph --keep 1` → prunes to 1.

- [ ] **Step 5: Verify (Rule 7) + arch-check + commit.**

```bash
git add crates/vox-cli/src/commands/graphify/mod.rs
git commit -m "feat(graphify): snapshot-on-rebuild + gc command (bounded history)"
```

---

## Parallelization summary
- **D1 → D2 SEQUENTIAL** (both edit `lib.rs`). **D3 SEQUENTIAL** (shares `mod.rs` with A/B/C and uses D1's snapshot fns).

## Self-Review
- **Spec coverage:** "store and cache as necessary" + "dealing with data sizes" (D2 `pick_lens` + Plan B `modules` lens), "which graphs to maintain vs discard with history we can learn from" (D1 snapshots = history; D2 value-score/retention = the learnable policy).
- **Placeholder scan:** none. The live-usage feed is DEFERRED (needs a non-existent DB query) and stated as such — the policy ships as tested pure functions, the snapshot lifecycle ships fully wired.
- **Type consistency:** `snapshot_corpus`/`list_snapshots`/`prune_snapshots` identical across D1 + D3; `Retention`/`value_score`/`pick_lens` identical across gc.rs + tests; `pick_lens` returns the same `"modules"`/`"structural"` strings Plan B's lens dispatch keys on.
- **Antigravity fit:** atomic+green+commit; pure fs/math functions are unit-tested deterministically (no time/DB nondeterminism); snapshot stamp is filesystem-safe (colons replaced) — avoids a Windows path trap a fast model would miss.
