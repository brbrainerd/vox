# Track 1 — Native Crate-Map Capability (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILLS: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `.../test-driven-development.skill.md`. Steps use `- [ ]`.

> **🤖 EXECUTION TARGET — READ FIRST.** Run by **Gemini 3.5 Flash inside Google Antigravity** (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context recall). Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Suite: [`2026-06-19-crate-build-disentanglement-suite-index.md`](2026-06-19-crate-build-disentanglement-suite-index.md). Design: [`../specs/2026-06-19-crate-build-disentanglement-design.md`](../specs/2026-06-19-crate-build-disentanglement-design.md).
> **PREREQUISITE:** `cargo build -p vox-cli` first (the prebuilt binary predates recent graphify subcommands).

## Operating Rules (apply to EVERY task)
1. **Atomic + green + committed.** A signature change fixes all callers in the SAME task. A crash between tasks leaves a compiling, tested tree.
2. **Verify-before-use.** Each task's first step is an `rg`/read confirming exact symbols. Reality differs → STOP, do not invent.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Fails twice → STOP + handoff note (what failed, last good SHA). No looping.
5. **Parallel dispatch.** Honor `[PARALLEL-SAFE]`/`[SEQUENTIAL]`; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all` (`-p <crate>`); automation is `.vox`; `docs/src/` `.md` needs frontmatter; no stubs.
7. **Verification ritual before commit** (skill `verification-before-completion`), paste output: `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`.
8. **Rollback on broken tree:** `git reset --hard HEAD`; re-attempt the single task.
9. **Skills:** `brainstorming` / `dispatching-parallel-agents` / `using-git-worktrees`.
10. **Determinism + no `.unwrap()` on I/O in lib code.** `cargo run -p vox-arch-check` passes before final commit.

**Goal:** A native, re-runnable `vox graphify crate-map` that builds the crate build/dependency model — Leiden communities (reusing `cluster::cluster_nodes`) + blast-radius-seconds — and persists it as a searchable `crate-map` corpus, replacing the offline computation.

**Architecture:** Pure model logic in a new `vox-graphify-reader::crate_model` (no I/O): `blast_radius_seconds` (reverse-BFS transitive-dependent compile-sum) and `build_crate_map` (assemble a graphify-shaped `serde_json::Value` from the dependency graph + audit data, attaching `compile_s`/`loc`/`layer`/`fan_in`/`blast_s`/`community`). A thin CLI subcommand reads `contracts/ci/crate-graph.v1.json` + `graphify-out/crate_audit.json`, calls the pure builder, writes `.vox/cache/graphify/crate-map/graph.json` + manifest, and a `crate-map` corpus entry makes it queryable via the existing `vox_graphify_*` tools.

**Tech Stack:** Rust; `serde_json`; `vox-graphify-reader` (`cluster::cluster_nodes`, `graph_digest`); `vox-config::graphify` (`write_manifest`); `chrono` (CLI).

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-graphify-reader/src/crate_model.rs` | pure model: `blast_radius_seconds`, `build_crate_map` | Create (T1.1, T1.2) |
| `crates/vox-graphify-reader/src/lib.rs` | module registration | Modify (T1.1) |
| `crates/vox-graphify-reader/tests/crate_model_tests.rs` | model tests | Create (T1.1, T1.2) |
| `crates/vox-cli/src/commands/graphify/mod.rs` | `CrateMap` subcommand | Modify (T1.3) |
| `contracts/retrieval/graphify-corpora.v1.yaml` | `crate-map` corpus entry | Modify (T1.3) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "pub fn cluster_nodes|pub struct ClusterNode|pub struct ClusterEdge" crates/vox-graphify-reader/src/cluster.rs` — confirm `cluster_nodes(nodes: &[ClusterNode], edges: &[ClusterEdge]) -> HashMap<String,String>`, `ClusterNode{id,label}`, `ClusterEdge{source,target}`.
- `rg -n "pub fn graph_digest|pub mod " crates/vox-graphify-reader/src/lib.rs` — confirm `graph_digest` exists + the module list to extend.
- `rg -n "pub fn write_manifest|pub struct GraphifyManifest" crates/vox-config/src/graphify.rs` — confirm `write_manifest(path, &GraphifyManifest)` + the manifest fields.
- `rg -n "enum GraphifyCmd|fn run\(|fn resolve_head_sha|use chrono::Utc|load_all_corpora" crates/vox-cli/src/commands/graphify/mod.rs` — the CLI structure to extend.
- `head -c 400 contracts/ci/crate-graph.v1.json` and `head -c 400 graphify-out/crate_audit.json` — confirm `{schema_version, crates:{name:[deps]}}` and the audit array shape (`crate`, `compile_s` (string), `loc`, `layer`).
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task T1.1 `[SEQUENTIAL]`: `blast_radius_seconds` (pure)

**Files:**
- Create: `crates/vox-graphify-reader/src/crate_model.rs`
- Modify: `crates/vox-graphify-reader/src/lib.rs`
- Test: `crates/vox-graphify-reader/tests/crate_model_tests.rs`

- [ ] **Step 1 (verify-before-use):** Run the Pre-flight `rg -n "pub mod " crates/vox-graphify-reader/src/lib.rs`. Note where to add `pub mod crate_model;`. STOP if the crate layout differs.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-graphify-reader/tests/crate_model_tests.rs`:

```rust
use std::collections::HashMap;
use vox_graphify_reader::crate_model::blast_radius_seconds;

#[test]
fn blast_radius_sums_transitive_dependents() {
    // a depends on b, b depends on c  =>  touching c rebuilds c,b,a
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    adj.insert("a".into(), vec!["b".into()]);
    adj.insert("b".into(), vec!["c".into()]);
    let mut self_s = HashMap::new();
    self_s.insert("a".into(), 1.0);
    self_s.insert("b".into(), 2.0);
    self_s.insert("c".into(), 4.0);

    let blast = blast_radius_seconds(&adj, &self_s);
    assert_eq!(blast["c"], 7.0); // 4 (self) + 2 (b) + 1 (a)
    assert_eq!(blast["b"], 3.0); // 2 + 1
    assert_eq!(blast["a"], 1.0); // leaf-down: nothing depends on a
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test crate_model_tests blast_radius_sums_transitive_dependents` → FAIL (module missing).

- [ ] **Step 4: Create `crate_model.rs`** with the pure function:

```rust
//! Crate build/dependency model: blast-radius build-cost + graphify-shaped crate map.
use std::collections::{HashMap, HashSet};

/// `blast_radius_seconds(X)` = `self_s(X)` + Σ `self_s` over all crates that **transitively
/// depend on** X — the wall-seconds of downstream rebuild forced by touching X.
///
/// `adj`: crate -> its direct dependencies. `self_s`: crate -> self-compile seconds.
pub fn blast_radius_seconds(
    adj: &HashMap<String, Vec<String>>,
    self_s: &HashMap<String, f64>,
) -> HashMap<String, f64> {
    // reverse edges: dependency -> dependents
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
        out.insert(n.clone(), base + dep_sum);
    }
    out
}
```

- [ ] **Step 5: Register the module.** In `crates/vox-graphify-reader/src/lib.rs`, add `pub mod crate_model;` to the module list.

- [ ] **Step 6: Run → PASS.** `cargo test -p vox-graphify-reader --test crate_model_tests blast_radius_sums_transitive_dependents` → PASS.

- [ ] **Step 7: Verify (Rule 7) + commit.**

```bash
git add crates/vox-graphify-reader/src/crate_model.rs crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/tests/crate_model_tests.rs
git commit -m "feat(graphify): blast_radius_seconds — transitive-dependent build-cost model"
```

---

## Task T1.2 `[SEQUENTIAL]` (same file): `build_crate_map` (pure, with Leiden communities)

Assembles a graphify-shaped graph (nodes with attrs + community, links) from the committed dependency graph + the audit data. Reuses `cluster::cluster_nodes` (Leiden) — the capability the offline pass lacked.

**Files:**
- Modify: `crates/vox-graphify-reader/src/crate_model.rs`
- Test: `crates/vox-graphify-reader/tests/crate_model_tests.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub fn cluster_nodes|pub struct ClusterNode|pub struct ClusterEdge" crates/vox-graphify-reader/src/cluster.rs`. Confirm `cluster_nodes(&[ClusterNode], &[ClusterEdge]) -> HashMap<String,String>`, `ClusterNode{id,label}`, `ClusterEdge{source,target}`. Differs → STOP.

- [ ] **Step 2: Write the failing test.** Append to `crates/vox-graphify-reader/tests/crate_model_tests.rs`:

```rust
use serde_json::json;
use vox_graphify_reader::crate_model::build_crate_map;

#[test]
fn build_crate_map_attaches_metrics_and_community() {
    let crate_graph = json!({ "schema_version": 1, "crates": {
        "a": ["b"], "b": ["c"], "c": []
    }});
    // audit: compile_s is a STRING in crate_audit.json
    let audit = json!([
        {"crate":"a","compile_s":"1.0","loc":10,"layer":5},
        {"crate":"b","compile_s":"2.0","loc":20,"layer":3},
        {"crate":"c","compile_s":"4.0","loc":40,"layer":0}
    ]);
    let map = build_crate_map(&crate_graph, &audit);
    let nodes = map["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3);
    let c = nodes.iter().find(|n| n["id"] == "c").unwrap();
    assert_eq!(c["blast_s"], 7.0);     // c + b + a
    assert_eq!(c["loc"], 40);
    assert_eq!(c["layer"], 0);
    assert_eq!(c["fan_in"], 1);        // b depends on c
    assert!(c.get("community").is_some());
    assert_eq!(map["links"].as_array().unwrap().len(), 2); // a->b, b->c
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test crate_model_tests build_crate_map_attaches_metrics_and_community` → FAIL (function missing).

- [ ] **Step 4: Implement `build_crate_map`** in `crate_model.rs` (add imports at top: `use serde_json::{Value, json};` and `use super::cluster::{ClusterEdge, ClusterNode, cluster_nodes};`):

```rust
/// Build a graphify-shaped crate map from `crate-graph.v1.json` (`{crates:{name:[deps]}}`)
/// and the `crate_audit.json` array (`crate`, `compile_s` [string], `loc`, `layer`).
/// Nodes carry `compile_s`/`loc`/`layer`/`fan_in`/`blast_s`/`community`; links are deps.
pub fn build_crate_map(crate_graph: &Value, audit: &Value) -> Value {
    // adjacency
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(m) = crate_graph.get("crates").and_then(|v| v.as_object()) {
        for (c, ds) in m {
            let deps: Vec<String> = ds
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            adj.insert(c.clone(), deps);
        }
    }
    // audit lookups
    let (mut self_s, mut loc, mut layer) = (HashMap::new(), HashMap::new(), HashMap::new());
    if let Some(arr) = audit.as_array() {
        for r in arr {
            let Some(name) = r.get("crate").and_then(|v| v.as_str()) else { continue };
            let cs = r
                .get("compile_s")
                .and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_f64()))
                .unwrap_or(0.0);
            self_s.insert(name.to_string(), cs);
            loc.insert(name.to_string(), r.get("loc").and_then(|v| v.as_i64()).unwrap_or(0));
            layer.insert(name.to_string(), r.get("layer").and_then(|v| v.as_i64()).unwrap_or(-1));
        }
    }
    let blast = blast_radius_seconds(&adj, &self_s);

    // node set + fan_in
    let mut nodes_set: HashSet<String> = HashSet::new();
    let mut fan_in: HashMap<String, usize> = HashMap::new();
    for (c, deps) in &adj {
        nodes_set.insert(c.clone());
        for d in deps {
            nodes_set.insert(d.clone());
            *fan_in.entry(d.clone()).or_insert(0) += 1;
        }
    }

    // Leiden communities
    let cnodes: Vec<ClusterNode> =
        nodes_set.iter().map(|n| ClusterNode { id: n.clone(), label: n.clone() }).collect();
    let mut cedges: Vec<ClusterEdge> = Vec::new();
    for (c, deps) in &adj {
        for d in deps {
            cedges.push(ClusterEdge { source: c.clone(), target: d.clone() });
        }
    }
    let comm = cluster_nodes(&cnodes, &cedges);

    // deterministic node list
    let mut names: Vec<String> = nodes_set.into_iter().collect();
    names.sort();
    let nodes_val: Vec<Value> = names
        .iter()
        .map(|n| {
            let cs = (self_s.get(n).copied().unwrap_or(0.0) * 10.0).round() / 10.0;
            json!({
                "id": n,
                "label": n,
                "community": comm.get(n).cloned().unwrap_or_else(|| "c_0".to_string()),
                "compile_s": cs,
                "loc": loc.get(n).copied().unwrap_or(0),
                "layer": layer.get(n).copied().unwrap_or(-1),
                "fan_in": fan_in.get(n).copied().unwrap_or(0),
                "blast_s": blast.get(n).copied().unwrap_or(0.0).round(),
            })
        })
        .collect();

    // deterministic link list
    let mut links_val: Vec<Value> = Vec::new();
    for (c, deps) in &adj {
        for d in deps {
            links_val.push(json!({"source": c, "target": d}));
        }
    }
    links_val.sort_by(|a, b| a.to_string().cmp(&b.to_string()));

    json!({ "nodes": nodes_val, "links": links_val })
}
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-graphify-reader --test crate_model_tests` → PASS (both tests).

- [ ] **Step 6: Verify (Rule 7) + commit.**

```bash
git add crates/vox-graphify-reader/src/crate_model.rs crates/vox-graphify-reader/tests/crate_model_tests.rs
git commit -m "feat(graphify): build_crate_map — graphify-shaped crate map with Leiden communities + blast_s"
```

---

## Task T1.3 `[SEQUENTIAL]`: `vox graphify crate-map` subcommand + corpus

**Files:**
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs`
- Modify: `contracts/retrieval/graphify-corpora.v1.yaml`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "enum GraphifyCmd|fn resolve_head_sha|use chrono::Utc|write_manifest|graph_digest" crates/vox-cli/src/commands/graphify/mod.rs`. Confirm `GraphifyCmd`, `resolve_head_sha() -> anyhow::Result<Option<String>>`, `chrono::Utc` import. Confirm `vox_config::graphify::write_manifest` and `vox_graphify_reader::graph_digest` are reachable (add `use` if needed). Differs → STOP.

- [ ] **Step 2: Register the `crate-map` corpus.** In `contracts/retrieval/graphify-corpora.v1.yaml`, add (after the `config-audit` corpus, before `graphify-search-log`):

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

(If Plan B added a `source_root` field to corpora, no change is needed — `#[serde(default)]` makes it optional.)

- [ ] **Step 3: Add the `CrateMap` subcommand variant.** In `GraphifyCmd`, add after the existing variants:

```rust
    /// Build the crate build-time x dependency map (Leiden communities + blast-radius)
    /// from contracts/ci/crate-graph.v1.json + graphify-out/crate_audit.json.
    CrateMap,
```

- [ ] **Step 4: Add the arm in `run()`** (after the existing arms; uses `GraphifyManifest` from vox-config). Add `use vox_config::graphify::{write_manifest, GraphifyManifest};` near the other imports if not present:

```rust
        GraphifyCmd::CrateMap => {
            let graph_path = repo_root.join("contracts/ci/crate-graph.v1.json");
            let audit_path = repo_root.join("graphify-out/crate_audit.json");
            let crate_graph: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&graph_path)
                    .with_context(|| format!("read {}", graph_path.display()))?,
            )
            .with_context(|| format!("parse {}", graph_path.display()))?;
            let audit: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&audit_path)
                    .with_context(|| format!("read {} (run scripts/crate-build-audit.vox first)", audit_path.display()))?,
            )
            .with_context(|| format!("parse {}", audit_path.display()))?;

            let map = vox_graphify_reader::crate_model::build_crate_map(&crate_graph, &audit);
            let out_dir = repo_root.join(".vox/cache/graphify/crate-map");
            std::fs::create_dir_all(&out_dir).context("create crate-map cache dir")?;
            let graph_out = out_dir.join("graph.json");
            let bytes = serde_json::to_string_pretty(&map)?;
            std::fs::write(&graph_out, &bytes).context("write crate-map graph.json")?;

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
            println!("crate-map: {node_count} crates, {edge_count} edges -> {}", graph_out.display());
        }
```

- [ ] **Step 5: Build + smoke test.** `cargo build -p vox-cli` → clean. Then:

```bash
cargo run -p vox-cli -- graphify crate-map
cargo run -p vox-cli -- graphify status --corpus crate-map
cargo run -p vox-cli -- graphify search --corpus crate-map --query "vox-db" --persist false
```

Expected: `crate-map` builds (~113 crates); `status --corpus crate-map` lists it `fresh` (manifest git_sha matches HEAD); `search` returns `vox-db` as a hit (the corpus is queryable via the existing MCP/CLI surface). If `crate_audit.json` is absent, the error tells the user to run `scripts/crate-build-audit.vox` — that is expected, not a bug.

- [ ] **Step 6: Verify (Rule 7) + arch-check + commit.**

```bash
git add crates/vox-cli/src/commands/graphify/mod.rs contracts/retrieval/graphify-corpora.v1.yaml
git commit -m "feat(graphify): vox graphify crate-map builds + persists a searchable crate-map corpus"
```

---

## Parallelization summary
- **T1.1 → T1.2 → T1.3 strict SEQUENTIAL** (T1.2 uses T1.1's function; T1.1/T1.2 share `crate_model.rs`; T1.3 uses both).

## Self-Review
- **Spec coverage (Track 1):** native re-runnable model (`build_crate_map` + `vox graphify crate-map`) ✔; Leiden communities via `cluster_nodes` ✔; blast-radius-seconds ✔; persisted to a searchable `crate-map` corpus (status/search/query/GUI for free) ✔. The deterministic `CRATE_BUILD_MAP.md` report is folded into T4 (it belongs with the gating/reporting spine) — noted, not dropped.
- **Placeholder scan:** none; full code in every code step. The `crate_audit.json`-absent path is an explicit, helpful error (not a stub).
- **Type consistency:** `blast_radius_seconds(adj, self_s)` and `build_crate_map(crate_graph, audit)` signatures identical across tasks + tests; node attrs (`compile_s`/`loc`/`layer`/`fan_in`/`blast_s`/`community`) match the persisted `crate-build-map.json` schema; `ClusterNode{id,label}`/`ClusterEdge{source,target}`/`cluster_nodes` used exactly as defined; `GraphifyManifest` fields match `vox-config`.
- **Antigravity fit:** atomic+green+commit; pure model logic is unit-tested deterministically; the CLI/Leiden/IO path is build+smoke-verified; verify-before-use gates the `cluster_nodes`/`write_manifest`/`graph_digest` signatures a fast model could hallucinate.
- **Reuse:** clusters via existing native Leiden; persists via existing corpus machinery (`write_manifest`, `graph_digest`); queryable via existing `vox_graphify_*` tools — no new retrieval surface invented.
