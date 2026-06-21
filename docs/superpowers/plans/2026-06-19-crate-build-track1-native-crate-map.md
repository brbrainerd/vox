# Track 1 — Native Crate-Map Capability (Sonnet 4.6 edition)

> **For agentic workers:** REQUIRED SUB-SKILLS: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `.../test-driven-development.skill.md`. Steps use `- [ ]`.

> **🤖 EXECUTION TARGET — READ FIRST.** Executed by **Claude Sonnet 4.6** (Claude Code or Antigravity). Sonnet 4.6 has strong reasoning, long-context recall, and low hallucination — so this plan does **not** spoon-feed against amnesia; you may reference earlier tasks and exercise judgment on the discovery steps. The disciplines below are kept because they are good engineering on a large, evolving codebase, not because the model is fragile. Suite: [`2026-06-19-crate-build-disentanglement-suite-index.md`](2026-06-19-crate-build-disentanglement-suite-index.md). Design: [`../specs/2026-06-19-crate-build-disentanglement-design.md`](../specs/2026-06-19-crate-build-disentanglement-design.md). Grounding: [`../../src/architecture/crate-build-dependency-model-2026-06-19.md`](../../src/architecture/crate-build-dependency-model-2026-06-19.md).
> **PREREQUISITE:** `cargo build -p vox-cli` first (the prebuilt binary predates recent graphify subcommands).

## Operating Rules (Sonnet 4.6)
1. **Atomic + green + committed.** A task is done only when its tests pass and you commit; a signature change fixes all callers in the same commit. Clean history, trivial revert.
2. **Verify-before-use.** Each task's first step confirms the real signatures/paths with `rg`/read. The repo drifts under you — confirm, don't assume. If reality differs from the plan, adapt and note it; do not invent APIs.
3. **TDD.** Red → green → refactor. Tests assert real behavior, not tautologies.
4. **Verification ritual before commit:** `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>` (never `--all`). Paste real output — Sonnet's main failure mode here is declaring "done" without running it.
5. **YAGNI / no gold-plating.** Build exactly what the task specifies. No speculative config, no extra abstractions, no stubs. If you see adjacent work, note it for a follow-up; don't expand scope.
6. **Judgment with guardrails.** Where a step says "investigate" or "choose," reason it out and record the decision in the commit message. Don't fabricate certainty; if blocked after a genuine attempt, surface the blocker with the last green commit SHA.
7. **Vox house rules.** Automation is `.vox` (no `.ps1/.sh/.py`); `docs/src/` `.md` needs YAML frontmatter; `cargo run -p vox-arch-check` passes before the final commit; no `.unwrap()` on I/O in library code.
8. **Determinism.** Graph/model output must be byte-stable across runs (this plan fixes a real non-determinism source — Task T1.0).

**Goal:** A native, re-runnable `vox graphify crate-map` that builds the crate build/dependency model — deterministic Leiden communities + blast-radius-seconds + transitive-dependent counts — and persists it as a searchable `crate-map` corpus, runnable on any checkout (with or without measured compile times).

**Architecture:** Pure model logic in a new `vox-graphify-reader::crate_model` (no I/O): `crate_metrics` (reverse-BFS transitive dependents → count + blast-seconds) and `build_crate_map` (assemble a deterministic graphify-shaped `serde_json::Value`). A thin CLI subcommand refreshes the committed dependency graph, reads the (optional) audit times, calls the pure builder, writes + ingests a `crate-map` corpus. Clustering is made deterministic first (T1.0) because it underpins everything and is currently seedless.

**Tech Stack:** Rust; `serde_json`; `vox-graphify-reader` (`cluster`, `graph_digest`); `vox-config::graphify` (`write_manifest`); `chrono`.

---

## Audit findings this edition fixes (vs the Gemini draft)
- **Non-deterministic Leiden** (`cluster.rs:39` `LeidenConfig::default()`, no seed) → community/`graph_json_sha256` churn; can't gate on Q. **Fixed in T1.0** (also benefits the shipped rebuild/modules lens).
- **`crate_audit.json` is gitignored** (not committed) → `compile_s` absent on fresh checkout. **Model now degrades** to transitive-dependent *count* when times are missing (T1.1/T1.2/T1.3).
- **`crate-graph.v1.json` is a committed snapshot that drifts.** T1.3 **regenerates it from `cargo metadata`** first.
- **No `graphify search` CLI** (only Status/Ingest/Rebuild/Index/Refresh/Gc). Validation uses `status` + graph inspection; **persistence/searchability uses `graphify ingest`** (T1.3).

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-graphify-reader/src/cluster.rs` | deterministic Leiden | Modify (T1.0) |
| `crates/vox-graphify-reader/tests/cluster_tests.rs` | determinism test | Modify (T1.0) |
| `crates/vox-graphify-reader/src/crate_model.rs` | pure model | Create (T1.1, T1.2) |
| `crates/vox-graphify-reader/src/lib.rs` | module registration | Modify (T1.1) |
| `crates/vox-graphify-reader/tests/crate_model_tests.rs` | model tests | Create (T1.1, T1.2) |
| `crates/vox-cli/src/commands/graphify/mod.rs` | `CrateMap` subcommand | Modify (T1.3) |
| `contracts/retrieval/graphify-corpora.v1.yaml` | `crate-map` corpus | Modify (T1.3) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "LeidenConfig|Leiden::new|\.run\(" crates/vox-graphify-reader/src/cluster.rs` then locate the leiden-rs source and inspect its config: `find ~/.cargo/registry/src -type d -name 'leiden-rs-*'` → `rg -n "struct LeidenConfig|seed|rng|with_seed|impl Default for LeidenConfig" <that dir>/src`. **Record whether `LeidenConfig` exposes a seed/rng field and its exact name** — T1.0 needs it.
- `rg -n "pub fn cluster_nodes|pub struct ClusterNode|pub struct ClusterEdge" crates/vox-graphify-reader/src/cluster.rs` — confirm `cluster_nodes(&[ClusterNode], &[ClusterEdge]) -> HashMap<String,String>`, `ClusterNode{id,label}`, `ClusterEdge{source,target}`.
- `rg -n "pub fn graph_digest|pub mod " crates/vox-graphify-reader/src/lib.rs` — confirm `graph_digest` + module list.
- `rg -n "pub fn write_manifest|pub struct GraphifyManifest" crates/vox-config/src/graphify.rs` — confirm `write_manifest` + manifest fields.
- `rg -n "enum GraphifyCmd|fn resolve_head_sha|use chrono::Utc|load_all_corpora|corpus_by_id" crates/vox-cli/src/commands/graphify/mod.rs` — the CLI structure.
- Discover the crate-graph regenerator: `./target/release/vox.exe ci --help 2>&1 | rg -i "affected|graph"` then `./target/release/vox.exe ci affected-crates --help` — **record the exact flag that regenerates `contracts/ci/crate-graph.v1.json`** (cmd_enums:771 documents it).
- `head -c 300 contracts/ci/crate-graph.v1.json` and `head -c 300 graphify-out/crate_audit.json` (if present) — confirm `{schema_version,crates:{name:[deps]}}` and the audit array (`crate`, `compile_s` string, `loc`, `layer`).
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task T1.0 `[SEQUENTIAL]`: Make clustering deterministic (cross-cutting fix)

`cluster_nodes` is seedless → non-deterministic partitions → unstable `graph.json`/digest for every clustered corpus. Fix it once, here.

**Files:**
- Modify: `crates/vox-graphify-reader/src/cluster.rs`
- Test: `crates/vox-graphify-reader/tests/cluster_tests.rs`

- [ ] **Step 1 (verify-before-use):** Run the Pre-flight leiden-rs inspection. Determine which case applies:
  - **(A) `LeidenConfig` has a seed/rng field** (e.g. `seed`, `random_seed`, `with_seed`) → use it (preferred).
  - **(B) No seed control** → the determinism guarantee moves to canonicalization + digest-exclusion (documented in T1.2 Step 4: `community` is excluded from `graph_json_sha256` and relabeled canonically). In case (B), this task instead adds a *canonicalization* helper and a test that **labels** are stable given a fixed partition; note in the commit that leiden-rs 0.8.1 has no seed.

- [ ] **Step 2: Write the failing determinism test.** Append to `crates/vox-graphify-reader/tests/cluster_tests.rs`:

```rust
#[test]
fn cluster_nodes_is_deterministic() {
    use vox_graphify_reader::cluster::{cluster_nodes, ClusterEdge, ClusterNode};
    let nodes: Vec<ClusterNode> = ["a","b","c","d","e","f"]
        .iter().map(|s| ClusterNode { id: s.to_string(), label: s.to_string() }).collect();
    let edges = vec![
        ("a","b"),("b","c"),("a","c"), // triangle
        ("d","e"),("e","f"),("d","f"), // triangle
        ("c","d"),                      // bridge
    ].into_iter().map(|(s,t)| ClusterEdge { source: s.into(), target: t.into() }).collect::<Vec<_>>();
    let r1 = cluster_nodes(&nodes, &edges);
    let r2 = cluster_nodes(&nodes, &edges);
    assert_eq!(r1, r2, "cluster_nodes must be deterministic across runs");
}
```

- [ ] **Step 3: Run → observe.** `cargo test -p vox-graphify-reader --test cluster_tests cluster_nodes_is_deterministic`. If it already passes, leiden-rs is deterministic by default — record that and skip to Step 5. If it FAILS, proceed to Step 4.

- [ ] **Step 4: Implement determinism in `cluster.rs`.**
  - **Case (A):** construct the config with a fixed seed, e.g. replace `let leiden = Leiden::new(LeidenConfig::default());` with the seed-bearing form confirmed in Step 1 (e.g. `let leiden = Leiden::new(LeidenConfig { seed: 42, ..Default::default() });` — use the real field name/builder).
  - **Case (B):** keep Leiden, but make the *returned labels* canonical so equal partitions yield equal maps. After building the `communities` map, relabel deterministically:

```rust
    // Canonicalize community labels: group → sort by (size desc, min member) → c0,c1,...
    let mut groups: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for (node, comm) in &communities {
        groups.entry(comm.clone()).or_default().push(node.clone());
    }
    let mut ordered: Vec<Vec<String>> = groups.into_values().collect();
    for g in &mut ordered { g.sort(); }
    ordered.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a[0].cmp(&b[0])));
    let mut canon = std::collections::HashMap::new();
    for (i, g) in ordered.iter().enumerate() {
        for n in g { canon.insert(n.clone(), format!("c_{i}")); }
    }
    return canon;
```

  > Canonicalization alone does NOT fix partition non-determinism (case B residual). If Step 3 still fails after canonicalization, the digest must exclude `community` — implemented in T1.2 Step 4 (the persisted bytes stay stable). Record which case you hit.

- [ ] **Step 5: Run → PASS** (case A or A-equivalent) **or** record the case-B residual. `cargo test -p vox-graphify-reader --test cluster_tests`.

- [ ] **Step 6: Verify (Rule 4) + commit.**

```bash
git add crates/vox-graphify-reader/src/cluster.rs crates/vox-graphify-reader/tests/cluster_tests.rs
git commit -m "fix(graphify): deterministic clustering (seed/canonical labels) so corpora stop churning"
```

---

## Task T1.1 `[SEQUENTIAL]`: `crate_metrics` — blast-seconds + dependent count (pure)

Both metrics so the model is useful even when measured times are absent (count) and richer when present (seconds).

**Files:**
- Create: `crates/vox-graphify-reader/src/crate_model.rs`
- Modify: `crates/vox-graphify-reader/src/lib.rs`
- Test: `crates/vox-graphify-reader/tests/crate_model_tests.rs`

- [ ] **Step 1 (verify-before-use):** `rg -n "pub mod " crates/vox-graphify-reader/src/lib.rs` — note where to add `pub mod crate_model;`.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-graphify-reader/tests/crate_model_tests.rs`:

```rust
use std::collections::HashMap;
use vox_graphify_reader::crate_model::crate_metrics;

#[test]
fn crate_metrics_count_and_seconds() {
    // a -> b -> c  (a depends on b, b depends on c)
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    adj.insert("a".into(), vec!["b".into()]);
    adj.insert("b".into(), vec!["c".into()]);
    let mut self_s = HashMap::new();
    self_s.insert("a".into(), 1.0); self_s.insert("b".into(), 2.0); self_s.insert("c".into(), 4.0);

    let m = crate_metrics(&adj, &self_s);
    // transitive dependents: c<-{b,a}=2, b<-{a}=1, a<-{}=0
    assert_eq!(m["c"].dependents, 2);
    assert_eq!(m["b"].dependents, 1);
    assert_eq!(m["a"].dependents, 0);
    // blast seconds: self + dependents' self
    assert_eq!(m["c"].blast_s, 7.0);
    assert_eq!(m["a"].blast_s, 1.0);
    // cycle-safe: should not infinite-loop on a back-edge
}

#[test]
fn crate_metrics_handles_cycles() {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    adj.insert("x".into(), vec!["y".into()]);
    adj.insert("y".into(), vec!["x".into()]); // 2-cycle
    let self_s = HashMap::new(); // no times → blast_s 0, counts still computed
    let m = crate_metrics(&adj, &self_s);
    assert_eq!(m["x"].dependents, 1); // y depends on x
    assert_eq!(m["y"].dependents, 1);
    assert_eq!(m["x"].blast_s, 0.0);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test crate_model_tests` → FAIL (module/type missing).

- [ ] **Step 4: Create `crate_model.rs`.**

```rust
//! Crate build/dependency model: blast-radius cost + dependent counts (pure, cycle-safe).
use std::collections::{HashMap, HashSet};

/// Per-crate model metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct CrateMetrics {
    /// Count of crates that transitively depend on this one.
    pub dependents: usize,
    /// self_s + Σ self_s over transitive dependents (0.0 when no times are available).
    pub blast_s: f64,
}

/// Compute per-crate metrics from `adj` (crate -> its deps) and `self_s` (crate -> compile seconds).
/// `self_s` may be empty/partial — `blast_s` degrades to 0 while `dependents` stays meaningful.
pub fn crate_metrics(
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
) -> HashMap<String, CrateMetrics> {
    let mut rev: HashMap<String, Vec<String>> = HashMap::new();
    let mut nodes: HashSet<String> = HashSet::new();
    for (c, deps) in adj {
        nodes.insert(c.clone());
        for d in deps {
            nodes.insert(d.clone());
            rev.entry(d.clone()).or_default().push(c.clone());
        }
    }
    let mut out = HashMap::new();
    for n in &nodes {
        let mut seen: HashSet<String> = HashSet::new();
        let mut stack = vec![n.clone()];
        while let Some(x) = stack.pop() {
            if let Some(parents) = rev.get(&x) {
                for p in parents {
                    if seen.insert(p.clone()) {
                        stack.push(p.clone());
                    }
                }
            }
        }
        let base = self_s.get(n).copied().unwrap_or(0.0);
        let dep_sum: f64 = seen.iter().map(|x| self_s.get(x).copied().unwrap_or(0.0)).sum();
        out.insert(n.clone(), CrateMetrics { dependents: seen.len(), blast_s: base + dep_sum });
    }
    out
}
```

- [ ] **Step 5: Register the module.** In `lib.rs`, add `pub mod crate_model;`.

- [ ] **Step 6: Run → PASS** (`cargo test -p vox-graphify-reader --test crate_model_tests`), then Rule 4 + commit.

```bash
git add crates/vox-graphify-reader/src/crate_model.rs crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/tests/crate_model_tests.rs
git commit -m "feat(graphify): crate_metrics — cycle-safe blast-radius + dependent counts"
```

---

## Task T1.2 `[SEQUENTIAL]` (same file): `build_crate_map` (deterministic, audit-optional)

**Files:** Modify `crates/vox-graphify-reader/src/crate_model.rs`; Test `crates/vox-graphify-reader/tests/crate_model_tests.rs`.

- [ ] **Step 1 (verify-before-use):** Confirm the cluster API again (`rg -n "pub fn cluster_nodes|ClusterNode|ClusterEdge" crates/vox-graphify-reader/src/cluster.rs`).

- [ ] **Step 2: Write the failing test.** Append:

```rust
use serde_json::json;
use vox_graphify_reader::crate_model::build_crate_map;

#[test]
fn build_crate_map_is_complete_and_deterministic() {
    let crate_graph = json!({ "schema_version": 1, "crates": { "a": ["b"], "b": ["c"], "c": [] }});
    let audit = json!([
        {"crate":"a","compile_s":"1.0","loc":10,"layer":5},
        {"crate":"b","compile_s":"2.0","loc":20,"layer":3},
        {"crate":"c","compile_s":"4.0","loc":40,"layer":0}
    ]);
    let m1 = build_crate_map(&crate_graph, &audit);
    let m2 = build_crate_map(&crate_graph, &audit);
    assert_eq!(m1, m2, "crate map must be byte-identical across runs");
    let nodes = m1["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3);
    let c = nodes.iter().find(|n| n["id"] == "c").unwrap();
    assert_eq!(c["blast_s"], 7.0);
    assert_eq!(c["dependents"], 2);
    assert_eq!(c["fan_in"], 1);
    assert_eq!(c["loc"], 40);
    assert!(c.get("community").is_some());
    assert_eq!(m1["links"].as_array().unwrap().len(), 2);
}

#[test]
fn build_crate_map_works_without_audit_times() {
    let crate_graph = json!({ "schema_version": 1, "crates": { "a": ["b"], "b": [] }});
    let audit = json!([]); // no compile times available (fresh checkout)
    let m = build_crate_map(&crate_graph, &audit);
    let b = m["nodes"].as_array().unwrap().iter().find(|n| n["id"]=="b").unwrap();
    assert_eq!(b["dependents"], 1);  // a depends on b
    assert_eq!(b["blast_s"], 0.0);   // unknown times → 0, but dependents still ranks
}
```

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement `build_crate_map`** (add `use serde_json::{json, Value};` and `use super::cluster::{cluster_nodes, ClusterEdge, ClusterNode};` at the top):

```rust
/// Build a deterministic graphify-shaped crate map from `crate-graph.v1.json`
/// (`{crates:{name:[deps]}}`) and the `crate_audit.json` array (`crate`,`compile_s`[string],`loc`,`layer`).
/// Node attrs: `compile_s`,`loc`,`layer`,`fan_in`,`dependents`,`blast_s`,`community`.
/// `audit` may be empty (`[]`) — times degrade to 0, `dependents` stays meaningful.
pub fn build_crate_map(crate_graph: &Value, audit: &Value) -> Value {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(m) = crate_graph.get("crates").and_then(|v| v.as_object()) {
        for (c, ds) in m {
            let deps = ds.as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            adj.insert(c.clone(), deps);
        }
    }
    let (mut self_s, mut loc, mut layer) = (HashMap::new(), HashMap::new(), HashMap::new());
    if let Some(arr) = audit.as_array() {
        for r in arr {
            let Some(name) = r.get("crate").and_then(|v| v.as_str()) else { continue };
            let cs = r.get("compile_s")
                .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_f64()))
                .unwrap_or(0.0);
            self_s.insert(name.to_string(), cs);
            loc.insert(name.to_string(), r.get("loc").and_then(|v| v.as_i64()).unwrap_or(0));
            layer.insert(name.to_string(), r.get("layer").and_then(|v| v.as_i64()).unwrap_or(-1));
        }
    }
    let metrics = crate_metrics(&adj, &self_s);

    let mut nodes_set: HashSet<String> = HashSet::new();
    let mut fan_in: HashMap<String, usize> = HashMap::new();
    for (c, deps) in &adj {
        nodes_set.insert(c.clone());
        for d in deps { nodes_set.insert(d.clone()); *fan_in.entry(d.clone()).or_insert(0) += 1; }
    }

    let cnodes: Vec<ClusterNode> =
        nodes_set.iter().map(|n| ClusterNode { id: n.clone(), label: n.clone() }).collect();
    let mut cedges: Vec<ClusterEdge> = Vec::new();
    for (c, deps) in &adj {
        for d in deps { cedges.push(ClusterEdge { source: c.clone(), target: d.clone() }); }
    }
    let comm = cluster_nodes(&cnodes, &cedges); // deterministic after T1.0

    let mut names: Vec<String> = nodes_set.into_iter().collect();
    names.sort();
    let nodes_val: Vec<Value> = names.iter().map(|n| {
        let cs = (self_s.get(n).copied().unwrap_or(0.0) * 10.0).round() / 10.0;
        let mm = metrics.get(n);
        json!({
            "id": n, "label": n,
            "community": comm.get(n).cloned().unwrap_or_else(|| "c_0".to_string()),
            "compile_s": cs,
            "loc": loc.get(n).copied().unwrap_or(0),
            "layer": layer.get(n).copied().unwrap_or(-1),
            "fan_in": fan_in.get(n).copied().unwrap_or(0),
            "dependents": mm.map(|m| m.dependents).unwrap_or(0),
            "blast_s": mm.map(|m| m.blast_s).unwrap_or(0.0).round(),
        })
    }).collect();

    let mut links_val: Vec<Value> = Vec::new();
    for (c, deps) in &adj {
        for d in deps { links_val.push(json!({"source": c, "target": d})); }
    }
    links_val.sort_by(|a, b| a.to_string().cmp(&b.to_string()));

    json!({ "nodes": nodes_val, "links": links_val })
}
```

> **Determinism (case-B residual from T1.0):** if the cluster determinism test could not be made to pass (leiden-rs has no seed and partition order varies), set `"community": "c_0"` for all nodes here OR drop the field, so the persisted bytes are stable; surface a follow-up to add a deterministic community pass. The `build_crate_map_is_complete_and_deterministic` test is the gate — it must pass.

- [ ] **Step 5: Run → PASS** (`cargo test -p vox-graphify-reader --test crate_model_tests`), Rule 4 + commit.

```bash
git add crates/vox-graphify-reader/src/crate_model.rs crates/vox-graphify-reader/tests/crate_model_tests.rs
git commit -m "feat(graphify): build_crate_map — deterministic, audit-optional crate map with communities"
```

---

## Task T1.3 `[SEQUENTIAL]`: `vox graphify crate-map` subcommand + corpus + persistence

**Files:** Modify `crates/vox-cli/src/commands/graphify/mod.rs`; Modify `contracts/retrieval/graphify-corpora.v1.yaml`.

- [ ] **Step 1 (verify-before-use):** Run the Pre-flight CLI lines + the crate-graph regenerator discovery. Confirm `resolve_head_sha`, `chrono::Utc`, and that `vox_config::graphify::write_manifest` + `vox_graphify_reader::graph_digest` are reachable (add `use` if needed). Record the exact `vox ci` flag that regenerates `crate-graph.v1.json`.

- [ ] **Step 2: Register the `crate-map` corpus** in `contracts/retrieval/graphify-corpora.v1.yaml` (after `config-audit`, before `graphify-search-log`):

```yaml
  - id: crate-map
    title: Crate build-time x dependency map
    scope_path: "."
    graph_path: ".vox/cache/graphify/crate-map/graph.json"
    manifest_path: ".vox/cache/graphify/crate-map/.graphify_manifest.v1.json"
    extraction_mode: crate-map
    default_for_intents:
      - build_time
      - crate_arrangement
```

- [ ] **Step 3: Add the `CrateMap` subcommand variant** to `GraphifyCmd`:

```rust
    /// Build the crate build-time x dependency map (deterministic Leiden communities +
    /// blast-radius) from contracts/ci/crate-graph.v1.json + graphify-out/crate_audit.json.
    CrateMap {
        /// Skip regenerating crate-graph.v1.json from cargo metadata (use the committed snapshot).
        #[arg(long)]
        no_refresh_graph: bool,
    },
```

- [ ] **Step 4: Add the `run()` arm.** Add `use vox_config::graphify::{write_manifest, GraphifyManifest};` near the other imports if absent.

```rust
        GraphifyCmd::CrateMap { no_refresh_graph } => {
            // 1. Freshen the committed dependency graph from cargo metadata unless suppressed.
            if !no_refresh_graph {
                // Use the regenerator confirmed in Step 1, e.g.:
                //   vox ci affected-crates --regen   (replace with the real flag from cmd_enums:771)
                // Spawn it as a child process; non-fatal if it fails (fall back to the snapshot).
                if let Err(e) = regenerate_crate_graph(repo_root) {
                    tracing::warn!("crate-graph regen failed, using committed snapshot: {e}");
                }
            }
            let graph_path = repo_root.join("contracts/ci/crate-graph.v1.json");
            let crate_graph: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&graph_path)
                    .with_context(|| format!("read {}", graph_path.display()))?,
            ).with_context(|| format!("parse {}", graph_path.display()))?;

            // 2. Audit times are OPTIONAL (graphify-out/ is gitignored; absent on fresh checkout).
            let audit_path = repo_root.join("graphify-out/crate_audit.json");
            let audit: serde_json::Value = match std::fs::read_to_string(&audit_path) {
                Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!([])),
                Err(_) => {
                    println!("note: {} absent — building count-only map (run scripts/crate-build-audit.vox for compile times)", audit_path.display());
                    serde_json::json!([])
                }
            };

            // 3. Build + persist.
            let map = vox_graphify_reader::crate_model::build_crate_map(&crate_graph, &audit);
            let out_dir = repo_root.join(".vox/cache/graphify/crate-map");
            std::fs::create_dir_all(&out_dir).context("create crate-map cache dir")?;
            let bytes = serde_json::to_string_pretty(&map)?;
            std::fs::write(out_dir.join("graph.json"), &bytes).context("write crate-map graph.json")?;
            let node_count = map["nodes"].as_array().map(|a| a.len() as u64).unwrap_or(0);
            let edge_count = map["links"].as_array().map(|a| a.len() as u64).unwrap_or(0);
            let manifest = GraphifyManifest {
                corpus_id: Some("crate-map".to_string()),
                built_at: Some(Utc::now().to_rfc3339()),
                git_sha: resolve_head_sha()?,
                scope_path: Some(".".to_string()),
                node_count: Some(node_count),
                edge_count: Some(edge_count),
                graph_json_sha256: Some(vox_graphify_reader::graph_digest(bytes.as_bytes())),
                extraction_mode: Some("crate-map".to_string()),
                lexical_ingest_sha256: None,
            };
            write_manifest(&out_dir.join(".graphify_manifest.v1.json"), &manifest)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            println!("crate-map: {node_count} crates, {edge_count} edges -> .vox/cache/graphify/crate-map/graph.json");
            println!("persist for agent recall: vox graphify ingest --corpus crate-map");
        }
```

Add the helper near the other free functions in this file (implement using the regenerator flag from Step 1; if the regenerator is itself a `GraphifyCmd`/`CiCmd` you can call its function directly instead of spawning):

```rust
fn regenerate_crate_graph(repo_root: &std::path::Path) -> anyhow::Result<()> {
    // Replace args with the real regenerator from Step 1 (cmd_enums:771).
    let status = std::process::Command::new(std::env::current_exe()?)
        .current_dir(repo_root)
        .args(["ci", "affected-crates", "--regen"]) // <-- confirm exact flag in Step 1
        .status()
        .context("spawn crate-graph regenerator")?;
    if !status.success() {
        anyhow::bail!("crate-graph regenerator exited non-zero");
    }
    Ok(())
}
```

> If Step 1 shows the regenerator does not exist as a flag, drop the `--no_refresh_graph` default-regen and just read the committed snapshot (it is only 1 day stale); note the follow-up. Do not invent a flag.

- [ ] **Step 5: Build + validate (correct surface — NO `graphify search` CLI).** `cargo build -p vox-cli` → clean. Then:

```bash
cargo run -p vox-cli -- graphify crate-map
cargo run -p vox-cli -- graphify status --corpus crate-map
cargo run -p vox-cli -- graphify ingest --corpus crate-map
```

Expected: crate-map builds (~113 crates) and reports node/edge counts; running it twice produces a **byte-identical** `graph.json` (determinism — diff the two outputs); `status --corpus crate-map` lists it `fresh` (manifest git_sha == HEAD); `ingest` projects nodes into Turso so agents can recall via the MCP `vox_graphify_search`/`vox_knowledge_query` tools (the agent-facing search surface — there is no `graphify search` CLI). Inspect top keystones: `python -c "import json;d=json.load(open('.vox/cache/graphify/crate-map/graph.json'));print(sorted(d['nodes'],key=lambda n:-n['blast_s'])[:5])"`.

- [ ] **Step 6: Verify (Rule 4) + `cargo run -p vox-arch-check` + commit.**

```bash
git add crates/vox-cli/src/commands/graphify/mod.rs contracts/retrieval/graphify-corpora.v1.yaml
git commit -m "feat(graphify): vox graphify crate-map (regen graph, audit-optional, persisted + ingestable corpus)"
```

---

## Parallelization summary
**T1.0 → T1.1 → T1.2 → T1.3 strict SEQUENTIAL.** T1.0 (cluster.rs) is independent of T1.1/T1.2 (crate_model.rs) at the file level, but T1.2's determinism test depends on T1.0 landing, so keep the order. Sonnet should run them in one focused session.

## Self-Review
- **Spec coverage (Track 1):** native re-runnable model ✔ (audit-optional, count+seconds); deterministic Leiden communities ✔ (T1.0 + canonicalization); blast-radius ✔; persisted + **ingestable** searchable corpus ✔; crate-graph freshness ✔ (regen).
- **Scenarios added vs prior draft:** non-deterministic Leiden (T1.0); missing `crate_audit.json` on fresh checkout (degrade to counts); stale committed crate-graph (regen); cycles in `crate_metrics` (cycle-safe + tested); no `graphify search` CLI (validate via status/ingest + MCP). 
- **Placeholder scan:** none. The two "confirm the exact flag/field in Step 1" points are verify-before-use anchors (leiden seed field; crate-graph regen flag) with explicit fallbacks, not TBDs.
- **Type consistency:** `crate_metrics(adj, self_s) -> HashMap<String,CrateMetrics{dependents,blast_s}>` and `build_crate_map(crate_graph, audit)` identical across tasks/tests; node attrs match the persisted `crate-build-map.json` schema (plus the new `dependents`); `cluster_nodes`/`ClusterNode`/`ClusterEdge`/`write_manifest`/`GraphifyManifest`/`graph_digest` used exactly as defined.
- **Sonnet 4.6 fit:** operating rules reframed (no amnesia/two-strike scaffolding); judgment latitude on the two discovery anchors with explicit fallbacks; verification ritual emphasized (Sonnet's real failure mode is unverified "done"); YAGNI guard against gold-plating.
