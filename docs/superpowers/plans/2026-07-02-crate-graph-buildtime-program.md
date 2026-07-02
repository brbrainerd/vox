# Crate-Graph Build-Time Program Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add what-if simulation, symbol-weighted dependency edges, and a rebuild-cause diagnostic to Graphify's crate map, then run one measured map pass and write a ranked crate-restructuring proposal.

**Architecture:** Three pure-analysis modules land in `vox-graph-reader` (the low-dep graph engine that already hosts `crate_model`), surfaced through the existing `vox graphify crate-map` subcommand plus one new `vox graphify why-rebuilt` subcommand in vox-cli. Phase 2 is a run-book session producing an evidence pack in `graphify-out/`; Phase 3 is a proposal doc in `docs/src/architecture/`. Spec: `docs/superpowers/specs/2026-07-02-crate-graph-buildtime-program-design.md`.

**Tech Stack:** Rust (serde_json only — vox-graph-reader's existing deps), clap subcommands in vox-cli, `cargo check` + `CARGO_LOG` fingerprint tracing for the diagnostic.

---

## Context an engineer needs (read once)

- **Repo root:** `C:\Users\Owner\vox`. Never run `cargo fmt --all` (Windows arg-limit); use `cargo fmt -p <crate>`. Clippy gate: `cargo clippy -p <crate> --all-targets -- -D warnings`; the workspace-wide form must `--exclude vox-gui`.
- **Crate dependency graph SSOT:** `contracts/ci/crate-graph.v1.json`, shape `{"schema_version":1,"crates":{"<name>":["<dep>",...]}}` — 121 workspace crates, workspace-internal deps only.
- **Compile times:** `graphify-out/crate_audit.json` (gitignored, may be absent/stale), array of rows `{"crate":"vox-db","compile_s":"12.3"|12.3,"loc":...,"layer":...}`. Produced by `scripts/crate-build-audit.vox` from `cargo build --timings` HTML.
- **Symbol graph:** the `repo-code-graph` corpus `graph.json` (NetworkX export). Nodes: `{"id","label","source_file":"crates/<crate>/src/...","source_location":"L55",...}`. Edges under `"links"`: `{"source","target","relation":"contains|imports_from|references|calls|method|implements",...}`. ~5.5k nodes / ~11.7k links today — **partial coverage**, which is why zero-weight edges are only ever "candidates".
- **Existing engine:** `crates/vox-graph-reader/src/crate_model.rs` has `crate_metrics(adj, self_s) -> HashMap<String, CrateMetrics>` (fields `dependents: usize`, `blast_s: f64`; cycle-safe reverse-BFS). Reuse it — do not reimplement BFS.
- **CLI surface:** `crates/vox-cli/src/commands/graphify/mod.rs` — `GraphifyCmd` clap enum + one big `run(cmd, repo_root)` match. The `CrateMap` arm already loads `crate-graph.v1.json` and `crate_audit.json`.
- **vox-graph-reader tests** live inline in `#[cfg(test)] mod tests` blocks in each module file. Follow that pattern.

---

### Task 1: What-if simulation (`vox-graph-reader::what_if`)

**Files:**
- Create: `crates/vox-graph-reader/src/what_if.rs`
- Modify: `crates/vox-graph-reader/src/lib.rs` (add `pub mod what_if;` after `pub mod snapshot;`... keep the mod list alphabetical: insert after `pub mod registry;` → actually after `snapshot` alphabetically `what_if` sorts last; append `pub mod what_if;` at the end of the mod list)

- [ ] **Step 1: Write the failing tests**

Create `crates/vox-graph-reader/src/what_if.rs` containing ONLY the test module for now:

```rust
//! What-if simulation over the crate dependency graph: cut one edge or split a
//! crate, recompute blast_s/dependents via `crate_model::crate_metrics`, and
//! report deltas. Pure functions; no I/O.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// b and c each take 10s; a takes 1s. a -> b -> c.
    /// blast(c) = 10 + 10 + 1 = 21 (self + b + a); blast(b) = 10 + 1 = 11; blast(a) = 1.
    fn toy() -> (HashMap<String, Vec<String>>, HashMap<String, f64>) {
        let adj = HashMap::from([
            ("a".to_string(), vec!["b".to_string()]),
            ("b".to_string(), vec!["c".to_string()]),
        ]);
        let self_s = HashMap::from([
            ("a".to_string(), 1.0),
            ("b".to_string(), 10.0),
            ("c".to_string(), 10.0),
        ]);
        (adj, self_s)
    }

    #[test]
    fn cut_edge_recomputes_blast() {
        let (adj, self_s) = toy();
        let d = what_if_cut(&adj, &self_s, "a", "b").unwrap();
        // Cutting a->b: blast(b) = 10 (loses a), blast(c) = 20 (loses a).
        assert_eq!(d.total_blast_s_before, 21.0 + 11.0 + 1.0);
        assert_eq!(d.total_blast_s_after, 20.0 + 10.0 + 1.0);
        let b = d.changed.iter().find(|c| c.krate == "b").unwrap();
        assert_eq!(b.blast_s_before, 11.0);
        assert_eq!(b.blast_s_after, 10.0);
        assert_eq!(b.dependents_before, 1);
        assert_eq!(b.dependents_after, 0);
        // 'a' unchanged -> not listed.
        assert!(d.changed.iter().all(|c| c.krate != "a"));
    }

    #[test]
    fn cut_missing_edge_errors() {
        let (adj, self_s) = toy();
        assert!(what_if_cut(&adj, &self_s, "a", "c").is_err());
        assert!(what_if_cut(&adj, &self_s, "nope", "b").is_err());
    }

    #[test]
    fn split_moves_dep_edges_to_new_leaf_node() {
        let (adj, self_s) = toy();
        // Split b: the part of b that uses c moves out. b no longer depends on c;
        // b__split (nobody depends on it) takes the c edge.
        let d = what_if_split(&adj, &self_s, "b", &["c".to_string()]).unwrap();
        // After: blast(c) = 10 (only itself; b__split has 0 self time).
        let c = d.changed.iter().find(|x| x.krate == "c").unwrap();
        assert_eq!(c.blast_s_before, 21.0);
        assert_eq!(c.blast_s_after, 10.0);
        assert_eq!(c.dependents_after, 1); // b__split
    }

    #[test]
    fn split_validates_moved_deps() {
        let (adj, self_s) = toy();
        assert!(what_if_split(&adj, &self_s, "b", &["zzz".to_string()]).is_err());
        assert!(what_if_split(&adj, &self_s, "zzz", &["c".to_string()]).is_err());
    }

    #[test]
    fn top_cuts_ranks_by_total_saving() {
        let (adj, self_s) = toy();
        let cuts = top_cuts(&adj, &self_s, 10);
        assert_eq!(cuts.len(), 2); // two edges exist
        // Cutting b->c saves blast(c): 21 -> 10 = 11s. Cutting a->b saves 1s off b and c = 2s.
        assert_eq!(cuts[0].description, "cut b -> c");
        let s0 = cuts[0].total_blast_s_before - cuts[0].total_blast_s_after;
        let s1 = cuts[1].total_blast_s_before - cuts[1].total_blast_s_after;
        assert!(s0 >= s1);
    }

    #[test]
    fn cycle_safe() {
        // a <-> b cycle plus times; must terminate and produce numbers.
        let adj = HashMap::from([
            ("a".to_string(), vec!["b".to_string()]),
            ("b".to_string(), vec!["a".to_string()]),
        ]);
        let self_s = HashMap::from([("a".to_string(), 1.0), ("b".to_string(), 2.0)]);
        let d = what_if_cut(&adj, &self_s, "a", "b").unwrap();
        assert!(d.total_blast_s_after <= d.total_blast_s_before);
    }
}
```

- [ ] **Step 2: Add the mod line and run tests to verify they fail to compile**

In `crates/vox-graph-reader/src/lib.rs`, append after the last `pub mod` line:

```rust
pub mod what_if;
```

Run: `cargo test -p vox-graph-reader what_if 2>&1 | tail -20`
Expected: compile FAIL — `what_if_cut` etc. not found.

- [ ] **Step 3: Implement**

Prepend to `crates/vox-graph-reader/src/what_if.rs` (above the test mod, below the `//!` docs):

```rust
use std::collections::HashMap;

use crate::crate_model::{CrateMetrics, crate_metrics};
use serde::Serialize;

/// One crate whose metrics change under a hypothetical edit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrateDelta {
    pub krate: String,
    pub blast_s_before: f64,
    pub blast_s_after: f64,
    pub dependents_before: usize,
    pub dependents_after: usize,
}

/// Result of one hypothetical edit. `total_blast_s_*` sums blast_s over every
/// crate — a comparative index, not wall-clock. Self-time attribution for
/// splits stays with the original crate (the synthetic `__split` node has 0
/// self time), so split savings are an UPPER BOUND on the dependency-shape win.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WhatIfDelta {
    pub description: String,
    pub total_blast_s_before: f64,
    pub total_blast_s_after: f64,
    /// Crates whose blast_s or dependents changed, sorted by blast saving desc.
    pub changed: Vec<CrateDelta>,
}

fn total(m: &HashMap<String, CrateMetrics>) -> f64 {
    m.values().map(|x| x.blast_s).sum()
}

fn diff(
    description: String,
    before: &HashMap<String, CrateMetrics>,
    after: &HashMap<String, CrateMetrics>,
) -> WhatIfDelta {
    let zero = CrateMetrics {
        dependents: 0,
        blast_s: 0.0,
    };
    let mut changed: Vec<CrateDelta> = Vec::new();
    let mut names: Vec<&String> = before.keys().chain(after.keys()).collect();
    names.sort();
    names.dedup();
    for n in names {
        let b = before.get(n).unwrap_or(&zero);
        let a = after.get(n).unwrap_or(&zero);
        if b.blast_s != a.blast_s || b.dependents != a.dependents {
            changed.push(CrateDelta {
                krate: n.clone(),
                blast_s_before: b.blast_s,
                blast_s_after: a.blast_s,
                dependents_before: b.dependents,
                dependents_after: a.dependents,
            });
        }
    }
    changed.sort_by(|x, y| {
        let sx = x.blast_s_before - x.blast_s_after;
        let sy = y.blast_s_before - y.blast_s_after;
        sy.partial_cmp(&sx)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(x.krate.cmp(&y.krate))
    });
    WhatIfDelta {
        description,
        total_blast_s_before: total(before),
        total_blast_s_after: total(after),
        changed,
    }
}

/// Remove the dependency edge `from -> to` and report metric deltas.
pub fn what_if_cut(
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
    from: &str,
    to: &str,
) -> Result<WhatIfDelta, String> {
    let deps = adj.get(from).ok_or_else(|| format!("unknown crate '{from}'"))?;
    if !deps.iter().any(|d| d == to) {
        return Err(format!("no dependency edge {from} -> {to}"));
    }
    let before = crate_metrics(adj, self_s);
    let mut cut = adj.clone();
    if let Some(v) = cut.get_mut(from) {
        v.retain(|d| d != to);
    }
    let after = crate_metrics(&cut, self_s);
    Ok(diff(format!("cut {from} -> {to}"), &before, &after))
}

/// Model extracting the part of `krate` that uses `moved` deps into a new leaf
/// crate `<krate>__split` (which nobody depends on). `krate` keeps its
/// dependents and self time; it just stops depending on `moved`.
pub fn what_if_split(
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
    krate: &str,
    moved: &[String],
) -> Result<WhatIfDelta, String> {
    let deps = adj
        .get(krate)
        .ok_or_else(|| format!("unknown crate '{krate}'"))?;
    for m in moved {
        if !deps.iter().any(|d| d == m) {
            return Err(format!("'{krate}' does not depend on '{m}'"));
        }
    }
    let before = crate_metrics(adj, self_s);
    let mut split = adj.clone();
    if let Some(v) = split.get_mut(krate) {
        v.retain(|d| !moved.iter().any(|m| m == d));
    }
    split.insert(format!("{krate}__split"), moved.to_vec());
    let after = crate_metrics(&split, self_s);
    Ok(diff(
        format!("split {krate}: move deps [{}] to {krate}__split", moved.join(", ")),
        &before,
        &after,
    ))
}

/// Evaluate cutting every existing edge, return the `n` best by total blast saved.
// ponytail: brute force — one crate_metrics per edge. 121 crates / ~1.4k edges is
// instant; revisit with incremental recompute only if the workspace 10×es.
pub fn top_cuts(
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
    n: usize,
) -> Vec<WhatIfDelta> {
    let mut edges: Vec<(String, String)> = Vec::new();
    for (c, deps) in adj {
        for d in deps {
            edges.push((c.clone(), d.clone()));
        }
    }
    edges.sort();
    let mut out: Vec<WhatIfDelta> = edges
        .iter()
        .filter_map(|(a, b)| what_if_cut(adj, self_s, a, b).ok())
        .collect();
    out.sort_by(|x, y| {
        let sx = x.total_blast_s_before - x.total_blast_s_after;
        let sy = y.total_blast_s_before - y.total_blast_s_after;
        sy.partial_cmp(&sx)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(x.description.cmp(&y.description))
    });
    out.truncate(n);
    out
}
```

Note: `CrateMetrics` currently derives `Debug, Clone, PartialEq` — no change needed. `serde` is already a dependency of vox-graph-reader.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-graph-reader what_if 2>&1 | tail -10`
Expected: `test result: ok. 6 passed`

- [ ] **Step 5: Lint + commit**

```bash
cargo clippy -p vox-graph-reader --all-targets -- -D warnings
cargo fmt -p vox-graph-reader
cd C:/Users/Owner/vox && git add crates/vox-graph-reader/src/what_if.rs crates/vox-graph-reader/src/lib.rs
git commit -m "feat(graph-reader): what-if cut/split/top-cuts simulation over crate map"
```

---

### Task 2: Symbol-weighted dependency edges (`vox-graph-reader::edge_weights`)

**Files:**
- Create: `crates/vox-graph-reader/src/edge_weights.rs`
- Modify: `crates/vox-graph-reader/src/lib.rs` (add `pub mod edge_weights;` keeping the list alphabetical — after `pub mod crate_model;`)

- [ ] **Step 1: Write the failing tests**

Create `crates/vox-graph-reader/src/edge_weights.rs` with only docs + tests:

```rust
//! Symbol-weighted crate dependency edges: join the repo code graph
//! (`graph.json` symbol nodes/links) against the crate dependency adjacency to
//! count how many distinct target-crate symbols each dep edge actually uses.
//!
//! Honesty contract: the symbol graph is PARTIAL (macros, derives, trait
//! impls, re-exports are invisible), so `symbols_used == 0` is only ever a
//! "candidate — verify by removal", never a claim of unusedness.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn sym_graph() -> serde_json::Value {
        json!({
            "nodes": [
                {"id": "n1", "label": "caller()",  "source_file": "crates/aaa/src/lib.rs"},
                {"id": "n2", "label": "callee()",  "source_file": "crates/bbb/src/lib.rs"},
                {"id": "n3", "label": "Other",     "source_file": "crates/bbb/src/other.rs"},
                {"id": "n4", "label": "stray()",   "source_file": "crates/ccc/src/lib.rs"},
                {"id": "n5", "label": "mod.rs",    "source_file": "docs/whatever.md"}
            ],
            "links": [
                {"source": "n1", "target": "n2", "relation": "calls"},
                {"source": "n1", "target": "n3", "relation": "references"},
                {"source": "n1", "target": "n2", "relation": "calls"},
                {"source": "n1", "target": "n1", "relation": "contains"},
                {"source": "n4", "target": "n2", "relation": "calls"}
            ]
        })
    }

    fn adj() -> HashMap<String, Vec<String>> {
        HashMap::from([
            ("aaa".to_string(), vec!["bbb".to_string(), "ddd".to_string()]),
            ("bbb".to_string(), vec![]),
            ("ddd".to_string(), vec![]),
        ])
    }

    #[test]
    fn counts_distinct_cross_crate_symbols_per_dep_edge() {
        let out = weigh_edges(&sym_graph(), &adj(), &HashMap::new());
        let rows = out["edges"].as_array().unwrap();
        // One row per adjacency edge: aaa->bbb and aaa->ddd.
        assert_eq!(rows.len(), 2);
        let ab = rows.iter().find(|r| r["from"] == "aaa" && r["to"] == "bbb").unwrap();
        // n2 referenced twice but distinct symbols = {callee(), Other} = 2.
        assert_eq!(ab["symbols_used"], 2);
        let sample: Vec<&str> = ab["symbols_sample"].as_array().unwrap()
            .iter().map(|v| v.as_str().unwrap()).collect();
        assert!(sample.contains(&"callee()"));
    }

    #[test]
    fn zero_weight_edge_is_candidate_not_unused() {
        let out = weigh_edges(&sym_graph(), &adj(), &HashMap::new());
        let rows = out["edges"].as_array().unwrap();
        let ad = rows.iter().find(|r| r["from"] == "aaa" && r["to"] == "ddd").unwrap();
        assert_eq!(ad["symbols_used"], 0);
        assert_eq!(ad["status"], "candidate-unused — verify by removal");
    }

    #[test]
    fn refs_outside_dep_graph_counted_in_meta() {
        // ccc -> bbb symbol ref exists but ccc->bbb is not a declared dep edge.
        let out = weigh_edges(&sym_graph(), &adj(), &HashMap::new());
        assert_eq!(out["meta"]["refs_not_in_dep_graph"], 1);
    }

    #[test]
    fn contains_and_same_crate_edges_ignored() {
        let g = json!({
            "nodes": [
                {"id": "x", "label": "a()", "source_file": "crates/aaa/src/a.rs"},
                {"id": "y", "label": "b()", "source_file": "crates/aaa/src/b.rs"}
            ],
            "links": [{"source": "x", "target": "y", "relation": "calls"}]
        });
        let out = weigh_edges(&g, &adj(), &HashMap::new());
        let ab = out["edges"].as_array().unwrap().iter()
            .find(|r| r["from"] == "aaa" && r["to"] == "bbb").unwrap().clone();
        assert_eq!(ab["symbols_used"], 0);
    }

    #[test]
    fn target_blast_included_when_times_present() {
        let self_s = HashMap::from([("bbb".to_string(), 30.0), ("aaa".to_string(), 5.0)]);
        let out = weigh_edges(&sym_graph(), &adj(), &self_s);
        let ab = out["edges"].as_array().unwrap().iter()
            .find(|r| r["from"] == "aaa" && r["to"] == "bbb").unwrap().clone();
        // blast(bbb) = 30 + 5 (aaa depends on it) = 35.
        assert_eq!(ab["target_blast_s"], 35.0);
    }
}
```

- [ ] **Step 2: Add mod line, verify compile failure**

Add `pub mod edge_weights;` to `lib.rs` after `pub mod crate_model;`.
Run: `cargo test -p vox-graph-reader edge_weights 2>&1 | tail -5`
Expected: FAIL — `weigh_edges` not found.

- [ ] **Step 3: Implement**

Insert above the test mod:

```rust
use std::collections::{BTreeSet, HashMap};

use serde_json::{Value, json};

/// Symbol-graph relations that represent one crate *using* another. `contains`
/// is structural nesting, not usage.
const CROSS_RELATIONS: &[&str] = &["calls", "references", "imports_from", "method", "implements"];
const SAMPLE_CAP: usize = 20;

fn crate_of(source_file: &str) -> Option<&str> {
    source_file.strip_prefix("crates/")?.split('/').next()
}

/// For every declared dep edge in `adj`, count distinct target-crate symbols
/// referenced from the source crate in `symbol_graph`. Rows are emitted for
/// ALL adjacency edges (including zero-weight), sorted by
/// (`target_blast_s` desc, `symbols_used` asc) — i.e. best cut candidates first.
pub fn weigh_edges(
    symbol_graph: &Value,
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
) -> Value {
    // node id -> (crate, label)
    let mut node_crate: HashMap<&str, (&str, &str)> = HashMap::new();
    if let Some(nodes) = symbol_graph.get("nodes").and_then(|v| v.as_array()) {
        for n in nodes {
            let (Some(id), Some(sf)) = (
                n.get("id").and_then(|v| v.as_str()),
                n.get("source_file").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            if let Some(c) = crate_of(sf) {
                let label = n.get("label").and_then(|v| v.as_str()).unwrap_or(id);
                node_crate.insert(id, (c, label));
            }
        }
    }

    // Distinct target symbols per (from_crate, to_crate).
    let mut used: HashMap<(String, String), BTreeSet<String>> = HashMap::new();
    let mut refs_not_in_dep_graph = 0u64;
    let links = symbol_graph
        .get("links")
        .or_else(|| symbol_graph.get("edges"))
        .and_then(|v| v.as_array());
    if let Some(links) = links {
        for e in links {
            let rel = e.get("relation").and_then(|v| v.as_str()).unwrap_or("");
            if !CROSS_RELATIONS.contains(&rel) {
                continue;
            }
            let (Some(s), Some(t)) = (
                e.get("source").and_then(|v| v.as_str()),
                e.get("target").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            let (Some((cs, _)), Some((ct, tl))) = (node_crate.get(s), node_crate.get(t)) else {
                continue;
            };
            if cs == ct {
                continue;
            }
            let declared = adj
                .get(*cs)
                .map(|deps| deps.iter().any(|d| d == ct))
                .unwrap_or(false);
            if declared {
                used.entry(((*cs).to_string(), (*ct).to_string()))
                    .or_default()
                    .insert((*tl).to_string());
            } else {
                refs_not_in_dep_graph += 1;
            }
        }
    }

    let metrics = crate::crate_model::crate_metrics(adj, self_s);
    let mut rows: Vec<Value> = Vec::new();
    let mut sorted_edges: Vec<(&String, &String)> = adj
        .iter()
        .flat_map(|(c, deps)| deps.iter().map(move |d| (c, d)))
        .collect();
    sorted_edges.sort();
    for (from, to) in sorted_edges {
        let syms = used.get(&(from.clone(), to.clone()));
        let count = syms.map(|s| s.len()).unwrap_or(0);
        let sample: Vec<&String> = syms
            .map(|s| s.iter().take(SAMPLE_CAP).collect())
            .unwrap_or_default();
        let blast = metrics.get(to).map(|m| m.blast_s).unwrap_or(0.0);
        let mut row = json!({
            "from": from,
            "to": to,
            "symbols_used": count,
            "symbols_sample": sample,
            "target_blast_s": blast,
        });
        if count == 0 {
            row["status"] = json!("candidate-unused — verify by removal");
        }
        rows.push(row);
    }
    rows.sort_by(|a, b| {
        let ba = a["target_blast_s"].as_f64().unwrap_or(0.0);
        let bb = b["target_blast_s"].as_f64().unwrap_or(0.0);
        bb.partial_cmp(&ba)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let ua = a["symbols_used"].as_u64().unwrap_or(0);
                let ub = b["symbols_used"].as_u64().unwrap_or(0);
                ua.cmp(&ub)
            })
            .then_with(|| a["from"].as_str().cmp(&b["from"].as_str()))
            .then_with(|| a["to"].as_str().cmp(&b["to"].as_str()))
    });

    json!({
        "schema_version": 1,
        "meta": {
            "symbol_nodes_mapped": node_crate.len(),
            "refs_not_in_dep_graph": refs_not_in_dep_graph,
            "note": "symbol graph is partial; zero-weight edges are candidates only",
        },
        "edges": rows,
    })
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-graph-reader edge_weights 2>&1 | tail -5`
Expected: `5 passed`

- [ ] **Step 5: Lint + commit**

```bash
cargo clippy -p vox-graph-reader --all-targets -- -D warnings
cargo fmt -p vox-graph-reader
git add crates/vox-graph-reader/src/edge_weights.rs crates/vox-graph-reader/src/lib.rs
git commit -m "feat(graph-reader): symbol-weighted crate dependency edges"
```

---

### Task 3: Rebuild-cause parser (`vox-graph-reader::rebuild_causes`)

**Files:**
- Create: `crates/vox-graph-reader/src/rebuild_causes.rs`
- Create: `crates/vox-graph-reader/tests/fixtures/fingerprint_mixed.log`
- Modify: `crates/vox-graph-reader/src/lib.rs` (add `pub mod rebuild_causes;` after `pub mod reachability;`)

- [ ] **Step 1: Write the fixture**

Create `crates/vox-graph-reader/tests/fixtures/fingerprint_mixed.log` (hand-authored approximations of cargo's tracing output; Phase 2 validates against a real capture and any real line that lands in `unknown` gets added here):

```text
   0.040196800s  INFO prepare_target{force=false package_id=vox-db v0.1.0 (path+file:///C:/Users/Owner/vox/crates/vox-db) target="vox-db"}: cargo::core::compiler::fingerprint: fingerprint dirty for vox-db/Check { test: false }/TargetInner { ..: lib_target("vox-db", ["lib"], ...) }
   0.040199900s  INFO prepare_target{force=false package_id=vox-db v0.1.0 (path+file:///C:/Users/Owner/vox/crates/vox-db) target="vox-db"}: cargo::core::compiler::fingerprint:     dirty: the list of features changed
   0.050000000s  INFO prepare_target{force=false package_id=vox-cli v0.1.0 (path+file:///C:/Users/Owner/vox/crates/vox-cli) target="vox-cli"}: cargo::core::compiler::fingerprint: fingerprint dirty for vox-cli/Check { test: false }/TargetInner { .. }
   0.050000100s  INFO prepare_target{force=false package_id=vox-cli v0.1.0 (path+file:///C:/Users/Owner/vox/crates/vox-cli) target="vox-cli"}: cargo::core::compiler::fingerprint:     dirty: the dependency vox_db was rebuilt
   0.060000000s  INFO prepare_target{force=false package_id=vox-secrets v0.1.0 (path+file:///C:/Users/Owner/vox/crates/vox-secrets) target="vox-secrets"}: cargo::core::compiler::fingerprint:     dirty: the env variable VOX_SECRETS_PROFILE changed
   0.070000000s  INFO prepare_target{force=false package_id=vox-gui v0.1.0 (path+file:///C:/Users/Owner/vox/crates/vox-gui) target="build-script-build"}: cargo::core::compiler::fingerprint:     dirty: the file `crates/vox-gui/build.rs` has changed (rerun-if-changed)
   0.080000000s  INFO prepare_target{force=false package_id=vox-term v0.1.0 (path+file:///C:/Users/Owner/vox/crates/vox-term) target="vox-term"}: cargo::core::compiler::fingerprint:     dirty: the rustflags changed
   0.090000000s  INFO prepare_target{force=false package_id=vox-ast v0.1.0 (path+file:///C:/Users/Owner/vox/crates/vox-ast) target="vox-ast"}: cargo::core::compiler::fingerprint:     dirty: the file `crates/vox-ast/src/lib.rs` has changed (1970-01-01 vs fingerprint)
   0.100000000s  INFO prepare_target{force=false package_id=vox-weird v0.1.0 (path+file:///C:/Users/Owner/vox/crates/vox-weird) target="vox-weird"}: cargo::core::compiler::fingerprint:     dirty: some future cargo reason we have never seen
totally unrelated stderr line that must be ignored
```

- [ ] **Step 2: Write the failing tests**

Create `crates/vox-graph-reader/src/rebuild_causes.rs` with docs + tests only:

```rust
//! Classify cargo fingerprint-log lines (from
//! `CARGO_LOG=cargo::core::compiler::fingerprint=info`) into rebuild causes.
//! Pure text -> classification; capture/reporting live in the vox-cli
//! `graphify why-rebuilt` command. The parser NEVER guesses: unmatched dirty
//! reasons classify as `Unknown` with the raw line preserved.

#[cfg(test)]
mod tests {
    use super::*;

    const MIXED: &str = include_str!("../tests/fixtures/fingerprint_mixed.log");

    #[test]
    fn classifies_each_known_cause() {
        let causes = parse_fingerprint_log(MIXED);
        let class_of = |k: &str| {
            causes
                .iter()
                .find(|c| c.krate == k && c.class != CauseClass::Unknown)
                .map(|c| c.class)
                .or_else(|| causes.iter().find(|c| c.krate == k).map(|c| c.class))
                .unwrap()
        };
        assert_eq!(class_of("vox-db"), CauseClass::FeatureDrift);
        assert_eq!(class_of("vox-cli"), CauseClass::DepRebuilt);
        assert_eq!(class_of("vox-secrets"), CauseClass::EnvChange);
        assert_eq!(class_of("vox-gui"), CauseClass::BuildScriptRerun);
        assert_eq!(class_of("vox-term"), CauseClass::ConfigChange);
        assert_eq!(class_of("vox-ast"), CauseClass::FileDirty);
    }

    #[test]
    fn unknown_preserves_raw_line() {
        let causes = parse_fingerprint_log(MIXED);
        let weird = causes
            .iter()
            .find(|c| c.krate == "vox-weird")
            .unwrap();
        assert_eq!(weird.class, CauseClass::Unknown);
        assert!(weird.raw.contains("some future cargo reason"));
    }

    #[test]
    fn garbage_input_yields_nothing() {
        assert!(parse_fingerprint_log("hello\nworld\n").is_empty());
    }

    #[test]
    fn summary_counts_and_unknown_rate() {
        let causes = parse_fingerprint_log(MIXED);
        let s = summarize(&causes);
        assert_eq!(s.total, causes.len());
        assert_eq!(*s.counts.get("unknown").unwrap(), 1);
        assert!(s.unknown_rate > 0.0 && s.unknown_rate < 0.5);
    }

    #[test]
    fn per_crate_dedup_prefers_specific_over_unknown() {
        // vox-db emits a bare "fingerprint dirty for" line (unknown) AND a
        // "features changed" line; per_crate must keep FeatureDrift.
        let causes = parse_fingerprint_log(MIXED);
        let per = per_crate(&causes);
        assert_eq!(*per.get("vox-db").unwrap(), CauseClass::FeatureDrift);
    }
}
```

- [ ] **Step 3: Add mod line, verify compile failure**

Add `pub mod rebuild_causes;` to `lib.rs` (after `pub mod reachability;`).
Run: `cargo test -p vox-graph-reader rebuild_causes 2>&1 | tail -5`
Expected: FAIL — items not found.

- [ ] **Step 4: Implement**

Insert above the test mod:

```rust
use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CauseClass {
    FeatureDrift,
    EnvChange,
    BuildScriptRerun,
    DepRebuilt,
    ConfigChange,
    FileDirty,
    Unknown,
}

impl CauseClass {
    pub fn as_str(self) -> &'static str {
        match self {
            CauseClass::FeatureDrift => "feature_drift",
            CauseClass::EnvChange => "env_change",
            CauseClass::BuildScriptRerun => "build_script_rerun",
            CauseClass::DepRebuilt => "dep_rebuilt",
            CauseClass::ConfigChange => "config_change",
            CauseClass::FileDirty => "file_dirty",
            CauseClass::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RebuildCause {
    pub krate: String,
    pub class: CauseClass,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    pub total: usize,
    /// cause class name -> line count, deterministic order.
    pub counts: BTreeMap<String, usize>,
    pub unknown_rate: f64,
}

/// Extract the package name: prefer the tracing span field `package_id=<name> <ver>`,
/// fall back to the token after "fingerprint dirty|error for ".
fn extract_package(line: &str) -> String {
    if let Some(i) = line.find("package_id=") {
        let rest = &line[i + "package_id=".len()..];
        if let Some(tok) = rest.split_whitespace().next() {
            return tok.to_string();
        }
    }
    for marker in ["fingerprint dirty for ", "fingerprint error for "] {
        if let Some(i) = line.find(marker) {
            let rest = &line[i + marker.len()..];
            return rest
                .split(['/', ' '])
                .next()
                .unwrap_or("?")
                .to_string();
        }
    }
    "?".to_string()
}

/// Substring classification of a fingerprint log line. Ordering matters:
/// specific causes (env/features/build-script) are checked before the broad
/// `file ... changed` pattern. Anything unmatched is Unknown, never guessed.
fn classify(line: &str) -> CauseClass {
    let l = line.to_lowercase();
    if l.contains("features changed") || l.contains("declared features") {
        CauseClass::FeatureDrift
    } else if l.contains("env variable") || l.contains("environment variable") {
        CauseClass::EnvChange
    } else if l.contains("rerun-if") || l.contains("build-script") || l.contains("build script") {
        CauseClass::BuildScriptRerun
    } else if l.contains("was rebuilt")
        || l.contains("dependency info changed")
        || l.contains("unit dependency")
        || l.contains("number of dependencies")
    {
        CauseClass::DepRebuilt
    } else if l.contains("rustflags")
        || l.contains("profile configuration")
        || l.contains("config settings")
        || l.contains("compile kind")
        || l.contains("metadata changed")
        || l.contains("target configuration")
    {
        CauseClass::ConfigChange
    } else if (l.contains("file") && (l.contains("changed") || l.contains("stale")))
        || l.contains("fsstatusoutdated")
    {
        CauseClass::FileDirty
    } else {
        CauseClass::Unknown
    }
}

/// One entry per fingerprint log line that reports dirtiness. A crate usually
/// emits a header line ("fingerprint dirty for X", Unknown) plus a
/// "    dirty: <reason>" detail line; both are kept — `per_crate` collapses.
pub fn parse_fingerprint_log(log: &str) -> Vec<RebuildCause> {
    let mut out = Vec::new();
    for line in log.lines() {
        let is_fp = line.contains("cargo::core::compiler::fingerprint");
        let relevant = is_fp
            && (line.contains("fingerprint dirty for")
                || line.contains("fingerprint error for")
                || line.contains("dirty:")
                || line.contains("err:"));
        if !relevant {
            continue;
        }
        out.push(RebuildCause {
            krate: extract_package(line),
            class: classify(line),
            raw: line.to_string(),
        });
    }
    out
}

/// Collapse to one class per crate, preferring any specific class over Unknown.
pub fn per_crate(causes: &[RebuildCause]) -> BTreeMap<String, CauseClass> {
    let mut out: BTreeMap<String, CauseClass> = BTreeMap::new();
    for c in causes {
        match out.get(&c.krate) {
            Some(CauseClass::Unknown) | None => {
                out.insert(c.krate.clone(), c.class);
            }
            Some(_) => {}
        }
    }
    out
}

pub fn summarize(causes: &[RebuildCause]) -> Summary {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for c in causes {
        *counts.entry(c.class.as_str().to_string()).or_insert(0) += 1;
    }
    let unknown = counts.get("unknown").copied().unwrap_or(0);
    let total = causes.len();
    Summary {
        total,
        counts,
        unknown_rate: if total == 0 {
            0.0
        } else {
            unknown as f64 / total as f64
        },
    }
}
```

Note on the `per_crate` test expectation: vox-db's header line classifies... check the fixture — the vox-db header line contains "fingerprint dirty for vox-db/Check" and no reason keywords → `Unknown`; its detail line has "the list of features changed" → `FeatureDrift`. `per_crate` upgrades Unknown → FeatureDrift regardless of order. That is exactly what the test pins.

- [ ] **Step 5: Run tests**

Run: `cargo test -p vox-graph-reader rebuild_causes 2>&1 | tail -5`
Expected: `5 passed`

- [ ] **Step 6: Lint + commit**

```bash
cargo clippy -p vox-graph-reader --all-targets -- -D warnings
cargo fmt -p vox-graph-reader
git add crates/vox-graph-reader/src/rebuild_causes.rs crates/vox-graph-reader/tests/fixtures/fingerprint_mixed.log crates/vox-graph-reader/src/lib.rs
git commit -m "feat(graph-reader): cargo fingerprint-log rebuild-cause parser"
```

---

### Task 4: CLI — analysis flags on `vox graphify crate-map`

**Files:**
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (CrateMap variant ~line 96-108; CrateMap arm ~line 723; add helpers + tests)

- [ ] **Step 1: Extend the CrateMap variant**

In the `GraphifyCmd` enum, add fields to `CrateMap` (after `ingest`):

```rust
        /// Simulate cutting one dependency edge and print blast_s deltas (JSON).
        #[arg(long, value_name = "FROM:TO")]
        what_if_cut: Option<String>,
        /// Simulate splitting CRATE by moving DEPS to a new leaf crate (JSON).
        #[arg(long, value_name = "CRATE=DEP1,DEP2")]
        what_if_split: Option<String>,
        /// Rank the N best single-edge cuts by total blast_s saved (JSON).
        #[arg(long, value_name = "N")]
        top_cuts: Option<usize>,
        /// Emit symbol-weighted dependency edges to graphify-out/edge_weights.json.
        #[arg(long)]
        edges: bool,
```

- [ ] **Step 2: Add pure helpers + unit tests (bottom of mod.rs, in the existing `mod tests`)**

Add above `run()`:

```rust
/// `{crates:{name:[deps]}}` -> adjacency map (shared by the analysis flags).
fn adj_from_crate_graph(
    crate_graph: &serde_json::Value,
) -> std::collections::HashMap<String, Vec<String>> {
    let mut adj = std::collections::HashMap::new();
    if let Some(m) = crate_graph.get("crates").and_then(|v| v.as_object()) {
        for (c, ds) in m {
            let deps = ds
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            adj.insert(c.clone(), deps);
        }
    }
    adj
}

/// crate_audit.json rows -> crate name -> compile seconds (string or number).
fn times_from_audit(audit: &serde_json::Value) -> std::collections::HashMap<String, f64> {
    let mut out = std::collections::HashMap::new();
    if let Some(arr) = audit.as_array() {
        for r in arr {
            if let (Some(name), Some(cs)) = (
                r.get("crate").and_then(|v| v.as_str()),
                r.get("compile_s").and_then(|v| {
                    v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                }),
            ) {
                out.insert(name.to_string(), cs);
            }
        }
    }
    out
}

/// Parse `--what-if-split` spec "crate=d1,d2" -> (crate, deps).
fn parse_split_spec(spec: &str) -> anyhow::Result<(String, Vec<String>)> {
    let (krate, deps) = spec
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("expected CRATE=DEP1,DEP2, got '{spec}'"))?;
    let deps: Vec<String> = deps
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if krate.trim().is_empty() || deps.is_empty() {
        anyhow::bail!("expected CRATE=DEP1,DEP2, got '{spec}'");
    }
    Ok((krate.trim().to_string(), deps))
}

/// Atomic write: temp file in the same dir, then rename over the target.
fn write_atomic(path: &std::path::Path, contents: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("rename to {}", path.display()))?;
    Ok(())
}
```

Unit tests to add in the existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn parse_split_spec_shapes() {
        assert_eq!(
            parse_split_spec("vox-cli=vox-db,vox-forge").unwrap(),
            (
                "vox-cli".to_string(),
                vec!["vox-db".to_string(), "vox-forge".to_string()]
            )
        );
        assert!(parse_split_spec("vox-cli").is_err());
        assert!(parse_split_spec("=a").is_err());
        assert!(parse_split_spec("a=").is_err());
    }

    #[test]
    fn adj_and_times_extractors() {
        let g = serde_json::json!({"crates": {"a": ["b"], "b": []}});
        let adj = adj_from_crate_graph(&g);
        assert_eq!(adj.get("a").unwrap(), &vec!["b".to_string()]);
        let audit = serde_json::json!([
            {"crate": "a", "compile_s": "1.5"},
            {"crate": "b", "compile_s": 2.5}
        ]);
        let t = times_from_audit(&audit);
        assert_eq!(t.get("a"), Some(&1.5));
        assert_eq!(t.get("b"), Some(&2.5));
    }
```

- [ ] **Step 3: Verify tests fail, then wire the CrateMap arm**

Run: `cargo test -p vox-cli parse_split_spec 2>&1 | tail -5` — expected FAIL (helpers exist but arm doesn't use them → dead_code warning under `-D warnings`; wire first, then test).

In the `GraphifyCmd::CrateMap { .. }` match arm, destructure the new fields, and insert **after** the crate_graph/audit loading (step "2. Audit times are OPTIONAL") and **before** "3. Build + persist":

```rust
            // Analysis flags: run the requested analysis and return early —
            // they read the same inputs as the map build but skip persisting
            // the map/manifest (read-only questions, not corpus refreshes).
            let analysis_requested =
                what_if_cut.is_some() || what_if_split.is_some() || top_cuts.is_some() || edges;
            if analysis_requested {
                let adj = adj_from_crate_graph(&crate_graph);
                let times = times_from_audit(&audit);
                if times.is_empty() {
                    println!(
                        "WARNING: no compile times — deltas are dependents-only (blast_s=0). \
                         Run scripts/crate-build-audit.vox first."
                    );
                }
                if let Some(spec) = &what_if_cut {
                    let (from, to) = spec.split_once(':').ok_or_else(|| {
                        anyhow::anyhow!("expected FROM:TO, got '{spec}'")
                    })?;
                    let d = vox_graph_reader::what_if::what_if_cut(&adj, &times, from, to)
                        .map_err(|e| anyhow::anyhow!(e))?;
                    println!("{}", serde_json::to_string_pretty(&d)?);
                }
                if let Some(spec) = &what_if_split {
                    let (krate, moved) = parse_split_spec(spec)?;
                    let d =
                        vox_graph_reader::what_if::what_if_split(&adj, &times, &krate, &moved)
                            .map_err(|e| anyhow::anyhow!(e))?;
                    println!("{}", serde_json::to_string_pretty(&d)?);
                }
                if let Some(n) = top_cuts {
                    let cuts = vox_graph_reader::what_if::top_cuts(&adj, &times, n);
                    println!("{}", serde_json::to_string_pretty(&cuts)?);
                }
                if edges {
                    // Symbol graph: the repo-code-graph corpus (registry-resolved).
                    let reg = load_all_corpora(repo_root)
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    let corpus = corpus_by_id(&reg, "repo-code-graph")
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    let sg_path = repo_root.join(&corpus.graph_path);
                    let sg: serde_json::Value = serde_json::from_str(
                        &std::fs::read_to_string(&sg_path)
                            .with_context(|| format!("read symbol graph {}", sg_path.display()))?,
                    )?;
                    let out = vox_graph_reader::edge_weights::weigh_edges(&sg, &adj, &times);
                    let out_path = repo_root.join("graphify-out/edge_weights.json");
                    write_atomic(&out_path, &serde_json::to_string_pretty(&out)?)?;
                    let n_zero = out["edges"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter(|r| r["symbols_used"].as_u64() == Some(0))
                                .count()
                        })
                        .unwrap_or(0);
                    println!(
                        "edge weights -> {} ({} edges, {} zero-weight candidates; symbol graph is partial — candidates only)",
                        out_path.display(),
                        out["edges"].as_array().map(|a| a.len()).unwrap_or(0),
                        n_zero
                    );
                }
                return Ok(());
            }
```

- [ ] **Step 4: Build + test**

Run: `cargo test -p vox-cli parse_split_spec adj_and_times 2>&1 | tail -5`
Expected: PASS.
Run: `cargo clippy -p vox-cli -- -D warnings 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 5: Smoke it for real**

```bash
cd C:/Users/Owner/vox
cargo run -p vox-cli -- graphify crate-map --no-refresh-graph --what-if-cut vox-cli:vox-db 2>&1 | head -30
cargo run -p vox-cli -- graphify crate-map --no-refresh-graph --top-cuts 5 2>&1 | head -40
```

Expected: pretty JSON deltas (dependents-only warning if `crate_audit.json` is stale/absent — acceptable for the smoke). If `vox-cli` does not directly depend on `vox-db`, the first command errors with `no dependency edge` — pick any edge from `contracts/ci/crate-graph.v1.json` instead; the point is exercising the path.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/graphify/mod.rs
git commit -m "feat(graphify): what-if cut/split/top-cuts + --edges analysis flags on crate-map"
```

---

### Task 5: CLI — `vox graphify why-rebuilt`

**Files:**
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (new enum variant + arm + helper)

- [ ] **Step 1: Add the variant**

After `CrateMap { .. }` in `GraphifyCmd`:

```rust
    /// Classify why crates recompiled (cargo fingerprint-log analysis).
    /// Either analyze a captured log (--log) or capture one now (--capture).
    WhyRebuilt {
        /// Parse this previously captured fingerprint log file.
        #[arg(long, conflicts_with = "capture")]
        log: Option<String>,
        /// Run `cargo check --workspace --exclude vox-gui` twice (second run
        /// instrumented) and analyze the second run. Uses check, not build, so
        /// it never relinks a running vox.exe.
        #[arg(long)]
        capture: bool,
        /// Write classification JSON here.
        #[arg(long, default_value = "graphify-out/rebuild_causes.json")]
        out: String,
    },
```

- [ ] **Step 2: Add the capture helper**

Above `run()`:

```rust
/// Run `cargo check` twice; the second run has fingerprint tracing enabled and
/// its stderr is returned (and saved to graphify-out/rebuild_fingerprint.log).
/// An idle second run SHOULD be a no-op: every dirty line it emits is a
/// rebuild-hygiene finding.
fn capture_fingerprint_log(repo_root: &std::path::Path) -> anyhow::Result<String> {
    let check_args = ["check", "--workspace", "--exclude", "vox-gui"];
    println!("why-rebuilt: warm-up cargo check (this may take a while)...");
    let warm = std::process::Command::new("cargo")
        .current_dir(repo_root)
        .args(check_args)
        .status()
        .context("spawn warm-up cargo check")?;
    if !warm.success() {
        anyhow::bail!("warm-up cargo check failed — fix the build first");
    }
    println!("why-rebuilt: instrumented cargo check...");
    let out = std::process::Command::new("cargo")
        .current_dir(repo_root)
        .args(check_args)
        .env("CARGO_LOG", "cargo::core::compiler::fingerprint=info")
        .output()
        .context("spawn instrumented cargo check")?;
    if !out.status.success() {
        anyhow::bail!("instrumented cargo check failed — fix the build first");
    }
    let log_text = String::from_utf8_lossy(&out.stderr).to_string();
    let log_path = repo_root.join("graphify-out/rebuild_fingerprint.log");
    write_atomic(&log_path, &log_text)?;
    println!("why-rebuilt: raw log -> {}", log_path.display());
    Ok(log_text)
}
```

- [ ] **Step 3: Add the arm**

```rust
        GraphifyCmd::WhyRebuilt { log, capture, out } => {
            let log_text = if capture {
                capture_fingerprint_log(repo_root)?
            } else {
                let path = log.ok_or_else(|| {
                    anyhow::anyhow!("pass --log <file> or --capture")
                })?;
                std::fs::read_to_string(repo_root.join(&path))
                    .with_context(|| format!("read log {path}"))?
            };
            let causes = vox_graph_reader::rebuild_causes::parse_fingerprint_log(&log_text);
            let summary = vox_graph_reader::rebuild_causes::summarize(&causes);
            let per = vox_graph_reader::rebuild_causes::per_crate(&causes);

            if causes.is_empty() {
                println!(
                    "why-rebuilt: no fingerprint-dirty lines found — nothing recompiled \
                     (clean) or the log wasn't captured with CARGO_LOG fingerprint tracing."
                );
            }
            // Never guess: a high unknown rate means cargo's log shape moved.
            if summary.total > 0 && summary.unknown_rate > 0.2 {
                let payload = serde_json::json!({
                    "summary": summary, "per_crate": per, "causes": causes,
                });
                write_atomic(
                    &repo_root.join(&out),
                    &serde_json::to_string_pretty(&payload)?,
                )?;
                anyhow::bail!(
                    "unknown-cause rate {:.0}% exceeds 20% — cargo's fingerprint log \
                     format likely changed; update vox_graph_reader::rebuild_causes::classify \
                     (raw lines preserved in {})",
                    summary.unknown_rate * 100.0,
                    out
                );
            }
            let payload = serde_json::json!({
                "summary": summary, "per_crate": per, "causes": causes,
            });
            write_atomic(&repo_root.join(&out), &serde_json::to_string_pretty(&payload)?)?;
            println!("why-rebuilt: {} dirty lines across {} crates -> {}", summary.total, per.len(), out);
            for (class, count) in &summary.counts {
                println!("  {class:<20} {count}");
            }
            let hygiene: Vec<&String> = per
                .iter()
                .filter(|(_, c)| {
                    !matches!(
                        c,
                        vox_graph_reader::rebuild_causes::CauseClass::FileDirty
                            | vox_graph_reader::rebuild_causes::CauseClass::DepRebuilt
                    )
                })
                .map(|(k, _)| k)
                .collect();
            if !hygiene.is_empty() {
                println!(
                    "HYGIENE FINDINGS (recompiled without source changes): {}",
                    hygiene
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
```

- [ ] **Step 4: Test with the fixture as a fake log**

```bash
cargo build -p vox-cli
cd C:/Users/Owner/vox
./target/debug/vox graphify why-rebuilt --log crates/vox-graph-reader/tests/fixtures/fingerprint_mixed.log --out graphify-out/rebuild_causes_smoke.json
```

Expected: summary table with all six specific classes + 1-2 unknown, HYGIENE FINDINGS listing vox-db, vox-secrets, vox-gui, vox-term; JSON written. (The fixture's unknown rate is under 20% — if the bail triggers, count the fixture lines and adjust the fixture, not the threshold.)

- [ ] **Step 5: Lint + commit**

```bash
cargo clippy -p vox-cli -- -D warnings
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/graphify/mod.rs
git commit -m "feat(graphify): why-rebuilt rebuild-cause diagnostic (capture + classify)"
```

---

### Task 6: Full verification pass

- [ ] **Step 1: Workspace gates**

```bash
cd C:/Users/Owner/vox
cargo test -p vox-graph-reader 2>&1 | tail -3
cargo test -p vox-cli --lib 2>&1 | tail -3
cargo clippy -p vox-graph-reader -p vox-cli --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: all pass, clippy clean. Do NOT run workspace-wide clippy with vox-gui included.

- [ ] **Step 2: Commit anything outstanding; push per repo flow**

Follow the repo's normal PR flow (branch → push → PR). Pre-push may fail on a locked `target/debug/vox.exe` — stop the running vox process first, and verify push success with `git log origin/<branch>` rather than trusting the error text.

---

### Task 7: Phase 2 — run the map (evidence pack)

This is a run-book, not code. Budget: the cold timings build is hours-scale wall clock; everything else is minutes.

- [ ] **Step 1: Fresh compile times**

```bash
cd C:/Users/Owner/vox
VOX_AUDIT_BUILD=1 vox run --mode interp scripts/crate-build-audit.vox
```

Expected: regenerated `graphify-out/crate_audit.json` + `CRATE_BUILD_AUDIT.md`. Gate: count rows with `compile_s > 0` ≥ 90% of the 121 crates in `contracts/ci/crate-graph.v1.json`; if lower, the timings HTML didn't cover the workspace — rerun the cold build (see `reference_cargo_timings_html_format`: `const UNIT_DATA`, durations are float seconds).

- [ ] **Step 2: Fresh symbol graph**

```bash
vox graphify refresh --auto
vox graphify status
```

Gate: `repo-code-graph` fresh. Pollution check — the graph must contain no nodes whose `source_file` starts with `.claude/worktrees/` or `dist/` (known past ~20% inflation): inspect a sample of `source_file` values in the corpus `graph.json` before trusting edge weights.

- [ ] **Step 3: Rebuild-cause capture**

```bash
vox graphify why-rebuilt --capture
```

Interpretation: an idle repo should produce ZERO dirty lines. Anything in `feature_drift` / `env_change` / `config_change` / `build_script_rerun` is a hygiene bug — record each offender in the evidence pack; these become Phase 3 recommendations ranked ABOVE structural changes (hygiene fixes are cheaper and benefit every build). If `unknown` rate trips the 20% bail: pull real raw lines from `graphify-out/rebuild_fingerprint.log`, extend `classify` + the fixture with them, re-run.

- [ ] **Step 4: Structural analyses**

```bash
vox graphify crate-map --no-refresh-graph --top-cuts 20 > graphify-out/top_cuts.json
vox graphify crate-map --no-refresh-graph --edges
```

- [ ] **Step 5: Targeted what-ifs**

For each of the top-5 blast crates in `CRATE_BUILD_AUDIT.md` (expect vox-db near the top), run `--what-if-cut` for its thinnest incoming edges (lowest `symbols_used` in `edge_weights.json`) and `--what-if-split` for plausible types-crate extractions (e.g. `vox-db=<heavy deps>` — the existing `vox-db-types` precedent is the pattern). Save outputs to `graphify-out/whatif_<name>.json`.

- [ ] **Step 6: Zero-weight verification (checks only, reverted)**

For up to 5 zero-weight candidate edges: remove the dep from the consumer's `Cargo.toml`, run `cargo check -p <consumer>`, record PASS (truly unused) or FAIL (symbol graph blind spot — note why: macro/derive/re-export), then `git checkout -- <Cargo.toml> Cargo.lock`. Record results in the evidence pack; do not land removals in this program.

---

### Task 8: Phase 3 — restructuring proposal doc

**Files:**
- Create: `docs/src/architecture/crate-restructuring-proposal-2026-07.md`

- [ ] **Step 1: Write the proposal from the evidence pack**

Required frontmatter (repo policy for `docs/src/`):

```markdown
---
title: "Crate Restructuring Proposal (2026-07)"
description: "Ranked, evidence-backed dependency cuts and crate splits from the crate-graph build-time program; hygiene findings first."
category: "Architecture SSOTs"
---
```

Required structure:

1. **Rebuild hygiene findings** (from `rebuild_causes.json`) — each offender crate, its cause class, the fix (pin the env var / fix the build script / align features), and "benefits every build" framing. If hygiene dominates, say so up front.
2. **Ranked structural recommendations** — one row per item: edge/split | blast_s saved (from what-if) | symbols_used evidence (from edge_weights) | risk class (`dep-removal` < `feature-gate` < `crate-split`) | verification status (for zero-weight candidates checked in Task 7 Step 6).
3. **Reconciliation with in-flight vox-cli extraction** — read `docs/superpowers/specs/2026-06-30-vox-cli-split-design.md` and the vox-cli-contracts extraction spec; every recommendation touching vox-cli must state whether it's already covered by the remaining planned work (Tier-2 guards, dispatcher move, HeavyGuardHost, model/runtime) or genuinely new.
4. **Stated limitations** — split savings are upper bounds (self-time attribution unmodeled); symbol graph partial; timings from one machine.

- [ ] **Step 2: Regenerate doc indexes if required, commit**

Never hand-edit `SUMMARY.md` / `architecture-index.md` — they are tool-regenerated; run the repo's doc pipeline if the new file must be indexed.

```bash
git add docs/src/architecture/crate-restructuring-proposal-2026-07.md
git commit -m "docs(architecture): ranked crate restructuring proposal from build-time program"
```

- [ ] **Step 3: Done-check against the spec**

Every spec deliverable exists: `rebuild_causes.json`, `edge_weights.json`, `top_cuts.json`, what-if outputs, proposal doc with all four required sections. Program ends here — executing any recommendation is a new spec→plan cycle.

---

## Self-review notes (already applied)

- Spec coverage: component 1 → Tasks 3+5, component 2 → Tasks 1+4, component 3 → Tasks 2+4, Phase 2 → Task 7 (incl. sanity gates + zero-weight verification), Phase 3 → Task 8 (incl. hygiene-first ranking + reconciliation). Error-handling requirements: dependents-only warning (Task 4 Step 3), unknown-never-guess + >20% bail (Tasks 3/5), atomic writes (`write_atomic`), candidate-only labeling (Task 2).
- Type consistency: `what_if_cut/what_if_split/top_cuts` signatures match between Task 1 (definitions) and Task 4 (call sites); `CauseClass`/`parse_fingerprint_log`/`summarize`/`per_crate` match between Tasks 3 and 5; `write_atomic` defined in Task 4, used in Task 5.
- Known risk, accepted: the fingerprint fixture lines are approximations of cargo's real tracing output; the parser is substring-based specifically so approximation drift lands in `unknown` (loud), and Task 7 Step 3 feeds real lines back into the fixture.
