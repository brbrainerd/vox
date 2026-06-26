# Vox Search Fusion — `vox_discover` + Structural-Overlay Ranking — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This plan is **workflow-ready**: every task is tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`, grouped into explicit fan-out batches, and ends in a concrete `git -C /c/Users/Owner/vox-graphify-gui` add+commit. Sub-agents MUST NOT run any other git verb (no checkout/reset/clean/push/rebase).

**Goal:** Build the fused graph-RAG entry point `vox_discover` (search-seed → graph-expand → composite re-rank) as **lexical-seed-first** (embedding lane behind a flag), fix the standalone KnowledgeGraph corpus relevance bug, and wire structural signals (proximity / centrality / dead-end, community scope) into ranking as a **query-time, provenance-labeled overlay** that never mutates `graph.json`. `mode:"structure_only"` is byte-deterministic.

**Architecture:** Three landing zones, mirroring the design's §3.4 table. (1) **`vox-graphify-reader`** gains a pure, deterministic `resolve.rs` (hit→node_id) and centrality/proximity/reachability helpers on `GraphifyReader`. (2) **`vox-search`** gains a pure `graph_overlay.rs` composite ranker + weight config, and a relevance-ranked KG FTS path (the 0.0-score fix lives in `vox-db`). (3) **`vox-orchestrator-mcp`** gains the `vox_discover` handler + JSON schema + one dispatch arm + catalog entry. The honesty firewall (§5 of the fusion design, §2.5 of the umbrella spec) is enforced structurally: the ranker takes the graph by `&` (read-only), returns scores in the response only, and stamps every result `provenance: "structural" | "overlay"`.

**Tech Stack:** Rust (`serde`, `serde_json`, `std::collections`), the existing `vox-graphify-reader` `GraphifyReader`, `vox-config::graphify::{lexical_search_graph, load_graphify_corpora}`, `vox-db::query_knowledge_nodes*`, `vox-orchestrator-mcp` dispatch + `input_schemas.rs`, and `contracts/operations/catalog.v1.yaml` → `vox ci operations-sync --target mcp --write`. No new crate; no embedding stack (Vox Search owns it). Tests: `cargo test -p vox-graphify-reader`, `-p vox-search`, `-p vox-orchestrator-mcp`.

**Spec:** `docs/superpowers/specs/2026-06-26-graphify-voxsearch-fusion-design.md` (the source design — read first) under the umbrella SSOT `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md` (§2.6, §3.1 `vox_discover` row). Read both before coding.

**Resolved decisions baked in (no deferral):**
- Embeddings are **owned by Vox Search**; this plan ships **lexical-seed fusion first**; the embedding lane is a strict upgrade behind `VOX_SEARCH_DISCOVER_EMBED=1` (Task F3, off by default, no model call when off).
- The KG-score bug is fixed **independently** (relevance-ranked FTS in `vox-db`) **and** masked by the composite ranker — both, per design fork §6.3.
- `vox_discover` keeps its **working name** (a verb, not a layer-suffix), per umbrella §3.1.
- Honesty: structural core deterministic; overlay scores are query-time only, never persisted as edges/node fields; `mode:"structure_only"` skips the search lane entirely and is byte-reproducible.

**Cross-plan dependencies (must precede this plan):**
- **P0 (Absorption + structural-core enrichment)** — `docs/superpowers/plans/2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint.md`. This plan **depends on P0** for: (a) the enriched structural index + `coverage.rs`/`CoverageStatus`, (b) edge-`confidence` on the graph schema, (c) the final tool *names*. **Mitigation so this plan is executable now:** this plan adds `vox_discover` as a **NEW** tool (not a rename of `vox_graphify_*`), reuses the existing `crates/vox-graphify-reader/src/coverage.rs` `CoverageStatus` enum that already exists on this branch, and does not touch the graphify→Vox Search rename. It therefore runs against the **current** `vox_graphify_*` surface and the **current** `coverage.rs`; the rename (P0) re-keys `vox_discover` into the `vox_search_*` family later with no code change to the ranker. Where P0 has not landed, every reference below is to a verified-current symbol.
- This plan is **P2** in the umbrella plan index (§9). **Downstream:** P3 (semantic overlay) depends on this plan's `SearchCorpus::GraphifyNodes` corpus hook (Task F1); P5 (GUI) consumes `vox_discover` as a pane.

**Base-branch note:** authored/executed on `claude/graphify-general-gui-ia` at worktree `/c/Users/Owner/vox-graphify-gui`. `main` does not compile (`vox-cli` `db_cli` WIP). Prefer per-crate tests (`cargo test -p vox-graphify-reader` / `-p vox-search`) which are fast and isolated; build `vox-orchestrator-mcp` only for Phase E/F.

---

## Key internals (verified against the code — exact)

- **`crates/vox-graphify-reader/src/lib.rs`** — `pub struct GraphifyReader { nodes: HashMap<String,(String,Option<String>)>, adjacency: HashMap<String,Vec<String>> }`. `from_value(serde_json::Value) -> Result<Self, GraphifyReaderError>` (reads `nodes[]` with `id`/`label`/`name`/`community`, edges from `links` **or** `edges`, builds **undirected** adjacency, sorts+dedups). Existing methods: `node_count()`, `edge_count()`, `bfs_from_seeds(&[&str], max_depth: u8, limit) -> Vec<TraversalHit>`, `shortest_path(from,to) -> Option<Vec<String>>`, `god_nodes(top_n) -> Vec<(String,usize)>` (degree-sorted), `community_members(&str) -> Vec<String>`. `TraversalHit { node_id, label, depth: u8, path: Vec<String> }`. **No per-node degree/centrality/community accessor yet** — Task A1 adds `degree(id)`, `community_of(id)`, `label_of(id)`, `contains(id)`.
- **`crates/vox-graphify-reader/src/coverage.rs`** — `pub enum CoverageStatus { OrphanBackend, DeadEnd, Surfaced, CliOnly }` already exists on this branch (verified lines 23–27). Reused for the `dead_end_penalty` and `reachability_class` fields; do **not** redefine it.
- **`crates/vox-config/src/graphify.rs`** — `pub fn lexical_search_graph(value, _corpus_id, query, limit) -> Vec<LexicalGraphHit>` where `LexicalGraphHit { node_id: String, label: String, score: usize }` (token-overlap, `score` = overlap count, sorted desc then label asc). `load_graphify_corpora(repo_root) -> Result<GraphifyCorporaRegistry, GraphifyError>`. `GraphifyCorpus { id, graph_path, .. }`.
- **`crates/vox-search/src/execution.rs`** — KG lane (lines ~380–411): `db.query_knowledge_nodes(query, limit)` → `Vec<(id,label,snippet)>`, mapped to `UnifiedHit { score: 1.0 / (1.0 + rank as f64), provenance: vec!["knowledge:fts"] }`. The `1/(1+rank)` formula is correct **given a relevance-ordered row vector**; the bug is upstream — the rows are NOT relevance-ordered.
- **`crates/vox-db/src/store/ops_memory/knowledge.rs`** — `query_knowledge_nodes(query, limit)` tries FTS then LIKE. **BUG (the "KG score 0.0 / meaningless" defect):** `query_knowledge_nodes_fts` (line ~170) orders `ORDER BY k.created_at DESC` — **not** by `bm25()` rank — so `rank` in execution.rs reflects *insertion time*, not relevance; for Graphify-projected nodes (near-identical timestamps) the resulting `1/(1+rank)` ordering is arbitrary and the top hit is whatever was inserted last. The LIKE fallback (line ~151) likewise orders `created_at DESC`. Fix: order FTS by `bm25(knowledge_nodes_fts)` ascending (best first) so `rank` is a real relevance rank.
- **`crates/vox-orchestrator-mcp/src/graphify_tools.rs`** — handler pattern: `pub async fn graphify_query(state: &ServerState, params: GraphifyQueryParams) -> String`, loads registry via `load_graphify_corpora`, resolves corpus via `resolve_search_corpus`, loads graph via `load_graph_json(repo_root, corpus)`, builds `GraphifyReader::from_value`, returns `ToolResult::ok(json).to_json()`. `knowledge_id(corpus_id, node_id)` → `"graphify:{corpus}:node:{id}"`. `query_slug(query)` exists for hit persistence. `REM_GRAPHIFY` remediation constant. `resolve_head_sha(state)` async helper.
- **`crates/vox-orchestrator-mcp/src/dispatch.rs`** — line ~627–641: `match name` arms `"vox_graphify_status" | "vox_graphify_search" | "vox_graphify_query" | "vox_graphify_path" | "vox_graphify_compare"` each `Ok(crate::graphify_tools::<fn>(state, serde_json::from_value(args)?).await)`. The match IS the SSOT (no `tool-registry.canonical.yaml`).
- **`crates/vox-orchestrator-mcp/src/input_schemas.rs`** — line ~471–485: per-tool `"vox_graphify_*" => parse_obj(r#"{...json schema...}"#)` arms.
- **`contracts/operations/catalog.v1.yaml`** — line ~6220: `- id: graphify.search` with `mcp: { name: vox_graphify_search, http_read_role_eligible: true, tier: core }`, `product_lane: platform`, `intent_tags: [retrieval, graph]`, `side_effect_class: none`, `requires_repo: true`. Regenerated via `vox ci operations-sync --target mcp --write`.
- **`crates/vox-db-types/src/retrieval.rs`** — `pub enum SearchCorpus { Memory, KnowledgeGraph, DocumentChunks, RepoInventory, WebResearch, SymbolProximity }` (`#[serde(rename_all="snake_case")]`). Task F1 adds `GraphifyNodes`.

---

## File Structure

**Created**
- `crates/vox-graphify-reader/src/resolve.rs` — pure deterministic seed resolver (hit→node_id) + the three resolvers (direct / path-symbol / label).
- `crates/vox-search/src/graph_overlay.rs` — pure composite ranker + `GraphOverlayWeights` config (env-loaded).
- `crates/vox-orchestrator-mcp/src/discover_tools.rs` — `vox_discover` handler (composition of the above).
- `crates/vox-graphify-reader/tests/resolve.rs`, `crates/vox-search/tests/graph_overlay.rs` *(new test files)*.

**Modified**
- `crates/vox-graphify-reader/src/lib.rs` — `pub mod resolve;` + new `GraphifyReader` accessors (`degree`, `community_of`, `label_of`, `contains`, `centrality_normalized`).
- `crates/vox-db/src/store/ops_memory/knowledge.rs` — relevance-ranked FTS (the 0.0 fix).
- `crates/vox-db-types/src/retrieval.rs` — `SearchCorpus::GraphifyNodes` variant.
- `crates/vox-orchestrator-mcp/src/dispatch.rs` — one `"vox_discover"` arm.
- `crates/vox-orchestrator-mcp/src/input_schemas.rs` — one `"vox_discover"` schema arm.
- `crates/vox-orchestrator-mcp/src/lib.rs` — `mod discover_tools;` (sibling of `mod graphify_tools;`).
- `contracts/operations/catalog.v1.yaml` — `- id: discover` entry (regenerated registry).

---

## Workflow batch structure (fan-out plan)

```
BATCH 1  (parallel — independent files, no shared edits)
  ├─ A1  reader accessors + centrality            [vox-graphify-reader/src/lib.rs]
  ├─ B1  KG FTS relevance fix                      [vox-db/.../knowledge.rs]
  └─ C1  GraphOverlayWeights config (env)          [vox-search/src/graph_overlay.rs]
          (C1 creates the file with ONLY the weights struct; the ranker fn is D1)

BATCH 2  (parallel — each depends only on its BATCH-1 sibling)
  ├─ A2  resolve.rs seed resolver (depends A1)     [vox-graphify-reader/src/resolve.rs]
  └─ D1  composite ranker fn (depends A1 + C1)     [vox-search/src/graph_overlay.rs]

BATCH 3  (sequential — all touch vox-orchestrator-mcp; serialize to avoid merge churn)
  ├─ E1  discover_tools.rs handler (depends A2,D1,B1)
  ├─ E2  schema + dispatch arm + mod wire (depends E1)
  └─ E3  catalog entry + operations-sync regen (depends E2)

BATCH 4  (parallel off BATCH 3)
  ├─ F1  SearchCorpus::GraphifyNodes variant       [vox-db-types]  (P3 hook; isolated)
  ├─ F2  KG-corpus integration test (depends B1,E2)
  └─ F3  embedding-lane flag (depends D1,E1)        [graph_overlay.rs + discover_tools.rs]

BATCH 5  (sequential — final gate)
  └─ G1  full workspace test + self-review checklist
```

Tasks within a batch carry no shared-file edits and are safe to dispatch concurrently. Between batches there is a hard dependency edge. **Workflow cap: 3 concurrent sub-agents.**

---

# BATCH 1 — Foundations (parallel)

## Task A1 — `GraphifyReader` accessors + normalized centrality (TDD) — [PARALLEL-SAFE]

**Files:** `crates/vox-graphify-reader/src/lib.rs`, `crates/vox-graphify-reader/tests/accessors.rs` (new).

The composite ranker needs per-node degree, community, label, membership, and a normalized centrality in `[0,1]`. Only `god_nodes` (a sorted list) exists today; add cheap O(1)/O(n) accessors.

**Step 1 — write the failing test.** Create `crates/vox-graphify-reader/tests/accessors.rs`:

```rust
use vox_graphify_reader::GraphifyReader;

fn reader() -> GraphifyReader {
    // a (deg 2) — b (deg 1), a — c (deg 2), c — d (deg 1); communities tagged.
    let v = serde_json::json!({
        "nodes": [
            {"id":"a","label":"alpha","community":"c1"},
            {"id":"b","label":"beta","community":"c1"},
            {"id":"c","label":"gamma","community":"c2"},
            {"id":"d","label":"delta","community":"c2"}
        ],
        "links": [
            {"source":"a","target":"b"},
            {"source":"a","target":"c"},
            {"source":"c","target":"d"}
        ]
    });
    GraphifyReader::from_value(v).unwrap()
}

#[test]
fn degree_counts_undirected_neighbors() {
    let r = reader();
    assert_eq!(r.degree("a"), 2);
    assert_eq!(r.degree("b"), 1);
    assert_eq!(r.degree("c"), 2);
    assert_eq!(r.degree("missing"), 0);
}

#[test]
fn community_and_label_and_contains() {
    let r = reader();
    assert_eq!(r.community_of("a").as_deref(), Some("c1"));
    assert_eq!(r.community_of("d").as_deref(), Some("c2"));
    assert_eq!(r.community_of("missing"), None);
    assert_eq!(r.label_of("a").as_deref(), Some("alpha"));
    assert!(r.contains("a"));
    assert!(!r.contains("missing"));
}

#[test]
fn centrality_is_normalized_zero_to_one_and_max_node_is_one() {
    let r = reader();
    // max degree is 2 (a and c). Their normalized centrality must be 1.0.
    assert!((r.centrality_normalized("a") - 1.0).abs() < 1e-9);
    assert!((r.centrality_normalized("c") - 1.0).abs() < 1e-9);
    // b has degree 1 → 0.5; missing → 0.0.
    assert!((r.centrality_normalized("b") - 0.5).abs() < 1e-9);
    assert_eq!(r.centrality_normalized("missing"), 0.0);
}

#[test]
fn centrality_empty_graph_is_zero_not_nan() {
    let r = GraphifyReader::from_value(serde_json::json!({"nodes":[],"links":[]})).unwrap();
    assert_eq!(r.centrality_normalized("anything"), 0.0);
}
```

Run: `cargo test -p vox-graphify-reader --test accessors` → **expected: fails to compile (`no method named degree`)**.

**Step 2 — implement.** In `crates/vox-graphify-reader/src/lib.rs`, inside `impl GraphifyReader` (after `community_members`, before the closing `}` at line ~201), add:

```rust
    /// Undirected degree of `id` (number of distinct neighbors). 0 if absent.
    pub fn degree(&self, id: &str) -> usize {
        self.adjacency.get(id).map_or(0, std::vec::Vec::len)
    }

    /// Community id of `id`, if the node carries a `"community"` field.
    pub fn community_of(&self, id: &str) -> Option<String> {
        self.nodes.get(id).and_then(|(_, c)| c.clone())
    }

    /// Human-readable label of `id`, if present.
    pub fn label_of(&self, id: &str) -> Option<String> {
        self.nodes.get(id).map(|(label, _)| label.clone())
    }

    /// Whether `id` is a node in the graph.
    pub fn contains(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    /// Degree centrality normalized to `[0.0, 1.0]` by the graph's max degree.
    /// Returns 0.0 for an absent node or an edgeless graph (never NaN).
    pub fn centrality_normalized(&self, id: &str) -> f64 {
        let max = self
            .adjacency
            .values()
            .map(std::vec::Vec::len)
            .max()
            .unwrap_or(0);
        if max == 0 {
            return 0.0;
        }
        self.degree(id) as f64 / max as f64
    }
```

Run: `cargo test -p vox-graphify-reader --test accessors` → **expected: `test result: ok. 4 passed`**.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/tests/accessors.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify-reader): degree/community/label/centrality accessors for fusion ranking"
```

- [ ] A1 complete

---

## Task B1 — Fix KnowledgeGraph FTS relevance ordering (the 0.0/meaningless-score bug) (TDD) — [PARALLEL-SAFE]

**Files:** `crates/vox-db/src/store/ops_memory/knowledge.rs`.

`query_knowledge_nodes_fts` orders by `k.created_at DESC` so the `rank` consumed in `execution.rs` (`1/(1+rank)`) is insertion order, not relevance. Order by FTS `bm25()` so the best lexical match is rank 0.

**Step 1 — write the failing test.** Append to the `#[cfg(test)]` module at the bottom of `crates/vox-db/src/store/ops_memory/knowledge.rs` (if no test module exists there, add one):

```rust
#[cfg(test)]
mod fts_relevance_tests {
    use crate::VoxDb;

    #[tokio::test]
    async fn fts_orders_by_relevance_not_insertion_time() {
        let db = VoxDb::connect_in_memory().await.expect("in-memory db");
        // Insert a weak match FIRST (older) and a strong match SECOND (newer).
        // If ordering were created_at DESC, the strong match would only win by luck;
        // we make the OLDER row the strong match so a relevance order is required to surface it.
        db.upsert_knowledge_node(
            "n_strong",
            "authentication login flow",
            "authentication login flow credential session token guard",
            Some("module"),
            None,
            None,
        )
        .await
        .unwrap();
        db.upsert_knowledge_node(
            "n_weak",
            "misc utilities",
            "authentication mentioned once among unrelated helpers",
            Some("module"),
            None,
            None,
        )
        .await
        .unwrap();

        let rows = db
            .query_knowledge_nodes("authentication login credential", 10)
            .await
            .unwrap();
        assert!(!rows.is_empty(), "expected hits");
        // The strong match must rank first regardless of insertion order.
        assert_eq!(
            rows[0].0, "n_strong",
            "FTS must order by bm25 relevance, got: {rows:?}"
        );
    }
}
```

Run: `cargo test -p vox-db fts_orders_by_relevance` → **expected: FAILS** (strong row not first, or FTS table absent → assert on order fails). If your local in-memory DB has no FTS table, the LIKE fallback returns `created_at DESC`; the test still asserts relevance ordering — see Step 2 which also fixes the LIKE tiebreak for the FTS-absent case.

> If `VoxDb::connect_in_memory` does not exist, use the test-DB constructor already used by neighboring tests in this file (grep the module for `connect_` / `test` helpers and reuse the exact one). Do not invent a constructor.

**Step 2 — implement.** In `query_knowledge_nodes_fts` change the `ORDER BY`:

```rust
                "SELECT k.id, k.label, COALESCE(SUBSTR(k.content, 1, 200), '')
                 FROM knowledge_nodes_fts f
                 JOIN knowledge_nodes k ON k.rowid = f.rowid
                 WHERE knowledge_nodes_fts MATCH ?1
                 ORDER BY bm25(knowledge_nodes_fts) ASC, k.created_at DESC LIMIT ?2",
```

(`bm25()` returns a score where **lower = more relevant**, so `ASC` puts the best match first; `created_at DESC` is the stable tiebreak.) Leave `query_knowledge_nodes_like` ordering as-is (LIKE has no relevance signal; `created_at DESC` is the honest fallback) — the composite ranker (D1) supplies the structural signal when FTS is absent.

Run: `cargo test -p vox-db fts_orders_by_relevance` → **expected: `test result: ok. 1 passed`** (FTS path). If FTS is unavailable in the in-memory build and the test exercises LIKE, mark the test `#[ignore]` with a comment `// requires FTS5; covered by execution-layer test F2` rather than weakening the assertion.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-db/src/store/ops_memory/knowledge.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "fix(vox-db): order knowledge_nodes FTS by bm25 relevance so KG rank/score is meaningful"
```

- [ ] B1 complete

---

## Task C1 — `GraphOverlayWeights` env-loaded config (TDD) — [PARALLEL-SAFE]

**Files:** `crates/vox-search/src/graph_overlay.rs` (new — weights struct ONLY; the ranker fn is D1), `crates/vox-search/src/lib.rs` (add `pub mod graph_overlay;`), `crates/vox-search/tests/graph_overlay.rs` (new).

Per the design's composite formula `fused = w_search·search + w_prox·proximity_decay(hops) + w_cent·centrality − w_dead·dead_end_penalty`, with `VOX_SEARCH_GRAPH_*` env tuning and **structural-dominant defaults** so behavior is safe/opt-in.

**Step 1 — failing test.** Create `crates/vox-search/tests/graph_overlay.rs`:

```rust
use vox_search::graph_overlay::GraphOverlayWeights;

#[test]
fn defaults_are_structural_dominant_and_finite() {
    let w = GraphOverlayWeights::default();
    assert!((w.search - 0.40).abs() < 1e-9);
    assert!((w.proximity - 0.30).abs() < 1e-9);
    assert!((w.centrality - 0.20).abs() < 1e-9);
    assert!((w.dead_end - 0.10).abs() < 1e-9);
    // structural signals (prox+cent+dead) must dominate raw search.
    assert!(w.proximity + w.centrality + w.dead_end > w.search);
}

#[test]
fn env_overrides_are_read_and_clamped_nonnegative() {
    // Use a builder that takes an explicit getter to stay deterministic in tests.
    let w = GraphOverlayWeights::from_getter(|k| match k {
        "VOX_SEARCH_GRAPH_W_SEARCH" => Some("0.5".into()),
        "VOX_SEARCH_GRAPH_W_PROX" => Some("-1.0".into()), // clamps to 0.0
        _ => None,
    });
    assert!((w.search - 0.5).abs() < 1e-9);
    assert_eq!(w.proximity, 0.0, "negative weight clamps to 0");
    assert!((w.centrality - 0.20).abs() < 1e-9, "unset keeps default");
}
```

Run: `cargo test -p vox-search --test graph_overlay` → **expected: fails to compile**.

**Step 2 — implement.** Create `crates/vox-search/src/graph_overlay.rs`:

```rust
//! Query-time structural overlay ranking for Vox Search fusion (`vox_discover`).
//!
//! HONESTY: this module computes scores from a read-only `&` view of the structural
//! graph and returns them in the response only. It never mutates `graph.json`,
//! creates edges, or persists node fields. Every produced result is provenance-labeled
//! by the caller (`structural` for exact/seed nodes, `overlay` for re-ranked neighbors).

/// Composite-ranker weights. Defaults are structural-dominant so enabling the overlay
/// cannot let raw lexical score swamp graph evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphOverlayWeights {
    pub search: f64,
    pub proximity: f64,
    pub centrality: f64,
    pub dead_end: f64,
}

impl Default for GraphOverlayWeights {
    fn default() -> Self {
        Self { search: 0.40, proximity: 0.30, centrality: 0.20, dead_end: 0.10 }
    }
}

impl GraphOverlayWeights {
    /// Load from the process environment (`VOX_SEARCH_GRAPH_W_{SEARCH,PROX,CENT,DEAD}`).
    pub fn from_env() -> Self {
        Self::from_getter(|k| std::env::var(k).ok())
    }

    /// Load via an explicit getter (testable; no global env access).
    pub fn from_getter(get: impl Fn(&str) -> Option<String>) -> Self {
        let d = Self::default();
        let read = |key: &str, fallback: f64| -> f64 {
            get(key)
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|f| f.is_finite())
                .map(|f| f.max(0.0)) // clamp negatives to 0
                .unwrap_or(fallback)
        };
        Self {
            search: read("VOX_SEARCH_GRAPH_W_SEARCH", d.search),
            proximity: read("VOX_SEARCH_GRAPH_W_PROX", d.proximity),
            centrality: read("VOX_SEARCH_GRAPH_W_CENT", d.centrality),
            dead_end: read("VOX_SEARCH_GRAPH_W_DEAD", d.dead_end),
        }
    }
}
```

In `crates/vox-search/src/lib.rs`, add `pub mod graph_overlay;` in the module list (alphabetical neighborhood near `pub mod execution;`).

Run: `cargo test -p vox-search --test graph_overlay` → **expected: `test result: ok. 2 passed`**.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-search/src/graph_overlay.rs crates/vox-search/src/lib.rs crates/vox-search/tests/graph_overlay.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): GraphOverlayWeights env config (structural-dominant defaults) for fusion ranking"
```

- [ ] C1 complete

---

# BATCH 2 — Resolver + Ranker (parallel; each depends on its Batch-1 sibling)

## Task A2 — `resolve.rs` seed resolver (hit → node_id) (TDD) — [PARALLEL-SAFE] (depends A1)

**Files:** `crates/vox-graphify-reader/src/resolve.rs` (new), `crates/vox-graphify-reader/src/lib.rs` (`pub mod resolve;`), `crates/vox-graphify-reader/tests/resolve.rs` (new).

Pure, deterministic three-resolver per design §3.1, priority order: **direct** (the hit is already a Graphify node id, e.g. `graphify:<corpus>:node:<id>` or a bare id present in the graph) → **path/symbol** (the hit carries a file/symbol that matches a node id by suffix) → **label** (fall back to `lexical_search_graph` over labels, supplied by the caller as already-resolved label hits — resolve.rs does NOT call vox-config to avoid a dep cycle; it accepts label candidates).

**Step 1 — failing test.** Create `crates/vox-graphify-reader/tests/resolve.rs`:

```rust
use vox_graphify_reader::resolve::{resolve_seed, SeedCandidate, SeedWhy};
use vox_graphify_reader::GraphifyReader;

fn reader() -> GraphifyReader {
    let v = serde_json::json!({
        "nodes": [
            {"id":"crates/vox-search/src/execution.rs::execute_search_plan","label":"execute_search_plan"},
            {"id":"auth","label":"authentication module"}
        ],
        "links": []
    });
    GraphifyReader::from_value(v).unwrap()
}

#[test]
fn direct_resolves_knowledge_id_prefix() {
    let r = reader();
    let got = resolve_seed(&r, "repo-code-graph", &SeedCandidate::knowledge_id(
        "graphify:repo-code-graph:node:auth", 0.9));
    assert_eq!(got.unwrap(), ("auth".to_string(), SeedWhy::Direct, 0.9));
}

#[test]
fn direct_resolves_bare_node_id_present_in_graph() {
    let r = reader();
    let got = resolve_seed(&r, "repo-code-graph", &SeedCandidate::node_id("auth", 0.7));
    assert_eq!(got.unwrap().0, "auth");
    assert_eq!(got.unwrap().1, SeedWhy::Direct);
}

#[test]
fn path_symbol_resolves_by_node_id_suffix() {
    let r = reader();
    // A repo/symbol hit whose path is a suffix of a fully-qualified node id.
    let got = resolve_seed(&r, "repo-code-graph",
        &SeedCandidate::path_symbol("execution.rs::execute_search_plan", 0.5));
    assert_eq!(got.unwrap().0, "crates/vox-search/src/execution.rs::execute_search_plan");
    assert_eq!(got.unwrap().1, SeedWhy::PathSymbol);
}

#[test]
fn label_resolves_only_when_node_present() {
    let r = reader();
    // Caller already mapped a label hit to a candidate node id via lexical_search_graph.
    let got = resolve_seed(&r, "repo-code-graph", &SeedCandidate::label("auth", 0.3));
    assert_eq!(got.unwrap().1, SeedWhy::Label);
    // Unknown node id → no resolution (honesty: never fabricate a seed).
    let none = resolve_seed(&r, "repo-code-graph", &SeedCandidate::node_id("ghost", 0.9));
    assert!(none.is_none());
}
```

Run: `cargo test -p vox-graphify-reader --test resolve` → **expected: fails to compile**.

**Step 2 — implement.** Create `crates/vox-graphify-reader/src/resolve.rs`:

```rust
//! Deterministic seed resolution: map a search hit to a structural node id.
//!
//! HONESTY: a candidate that resolves to NO graph node returns `None` — a seed is
//! never fabricated. This is pure (no I/O, no embeddings) and byte-deterministic.

use crate::GraphifyReader;

/// Why a candidate resolved to a node (provenance for the seed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedWhy {
    /// The hit was already a graph node id (or a `graphify:…:node:<id>` knowledge id).
    Direct,
    /// The hit's file/symbol path matched a node id by suffix.
    PathSymbol,
    /// The hit was resolved from a label match (caller-supplied candidate node id).
    Label,
}

/// One search hit reduced to a resolvable candidate.
#[derive(Debug, Clone)]
pub struct SeedCandidate {
    /// Raw identifier from the hit (knowledge id, node id, or path/symbol string).
    pub raw: String,
    /// Resolution lane to attempt for this candidate.
    pub kind: CandidateKind,
    /// Upstream search relevance score carried through to the ranker.
    pub search_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    KnowledgeId,
    NodeId,
    PathSymbol,
    Label,
}

impl SeedCandidate {
    pub fn knowledge_id(raw: impl Into<String>, score: f64) -> Self {
        Self { raw: raw.into(), kind: CandidateKind::KnowledgeId, search_score: score }
    }
    pub fn node_id(raw: impl Into<String>, score: f64) -> Self {
        Self { raw: raw.into(), kind: CandidateKind::NodeId, search_score: score }
    }
    pub fn path_symbol(raw: impl Into<String>, score: f64) -> Self {
        Self { raw: raw.into(), kind: CandidateKind::PathSymbol, search_score: score }
    }
    pub fn label(raw: impl Into<String>, score: f64) -> Self {
        Self { raw: raw.into(), kind: CandidateKind::Label, search_score: score }
    }
}

/// Resolve a candidate to `(node_id, why, search_score)` or `None` if no node matches.
pub fn resolve_seed(
    reader: &GraphifyReader,
    corpus_id: &str,
    cand: &SeedCandidate,
) -> Option<(String, SeedWhy, f64)> {
    let score = cand.search_score;
    match cand.kind {
        CandidateKind::KnowledgeId => {
            // Strip the `graphify:<corpus>:node:` prefix if present.
            let prefix = format!("graphify:{corpus_id}:node:");
            let id = cand.raw.strip_prefix(&prefix).unwrap_or(&cand.raw);
            reader.contains(id).then(|| (id.to_string(), SeedWhy::Direct, score))
        }
        CandidateKind::NodeId | CandidateKind::Label => {
            let why = if cand.kind == CandidateKind::NodeId { SeedWhy::Direct } else { SeedWhy::Label };
            reader.contains(&cand.raw).then(|| (cand.raw.clone(), why, score))
        }
        CandidateKind::PathSymbol => resolve_by_suffix(reader, &cand.raw)
            .map(|id| (id, SeedWhy::PathSymbol, score)),
    }
}

/// Find the node id that ends with `needle` (longest, then lexicographically smallest
/// for determinism). Used for path/symbol hits whose id is a suffix of a qualified node id.
fn resolve_by_suffix(reader: &GraphifyReader, needle: &str) -> Option<String> {
    let mut best: Option<String> = None;
    for id in reader.node_ids() {
        if id == needle || id.ends_with(needle) {
            match &best {
                None => best = Some(id.clone()),
                Some(b) if id.len() > b.len() || (id.len() == b.len() && *id < *b) => {
                    best = Some(id.clone());
                }
                _ => {}
            }
        }
    }
    best
}
```

This needs `node_ids()` on the reader. Add to `impl GraphifyReader` in `lib.rs`:

```rust
    /// Iterator over all node ids (deterministic order is the caller's responsibility).
    pub fn node_ids(&self) -> impl Iterator<Item = &String> {
        self.nodes.keys()
    }
```

Add `pub mod resolve;` to `lib.rs` module list.

Run: `cargo test -p vox-graphify-reader --test resolve` → **expected: `test result: ok. 4 passed`**.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/resolve.rs crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/tests/resolve.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify-reader): deterministic seed resolver (direct/path-symbol/label) for fusion"
```

- [ ] A2 complete

---

## Task D1 — Composite ranker fn over the read-only graph (TDD) — [PARALLEL-SAFE] (depends A1 + C1)

**Files:** `crates/vox-search/src/graph_overlay.rs` (extend with the ranker), `crates/vox-search/tests/graph_overlay.rs` (extend).

The ranker takes `&GraphifyReader` (read-only), the seeds, the BFS frontier, and the weights, and emits ranked `RankedNode`s with explainable components + provenance. `vox-search` already depends on `vox-graphify-reader`? **Verify first:** `grep -n 'vox-graphify-reader' crates/vox-search/Cargo.toml`. If absent, add `vox-graphify-reader = { path = "../vox-graphify-reader" }` under `[dependencies]` (it is a leaf, pure, no cycle — `vox-graphify-reader` does not depend on `vox-search`; confirm with `grep vox-search crates/vox-graphify-reader/Cargo.toml` → must be empty).

**Step 1 — failing test.** Append to `crates/vox-search/tests/graph_overlay.rs`:

```rust
use vox_search::graph_overlay::{rank_overlay, RankInput, SeedRef};
use vox_graphify_reader::GraphifyReader;

fn reader() -> GraphifyReader {
    // hub (deg 3) — a, hub — b, hub — c ; leaf d isolated.
    let v = serde_json::json!({
        "nodes": [
            {"id":"hub","label":"hub","community":"core"},
            {"id":"a","label":"a","community":"core"},
            {"id":"b","label":"b","community":"core"},
            {"id":"c","label":"c","community":"core"},
            {"id":"d","label":"orphan","community":"misc"}
        ],
        "links": [
            {"source":"hub","target":"a"},
            {"source":"hub","target":"b"},
            {"source":"hub","target":"c"}
        ]
    });
    GraphifyReader::from_value(v).unwrap()
}

#[test]
fn central_neighbor_outranks_leaf_with_equal_search_score() {
    let r = reader();
    let out = rank_overlay(RankInput {
        reader: &r,
        seeds: &[SeedRef { node_id: "a".into(), search_score: 0.5 }],
        // candidates: hub (1 hop, high centrality) vs d (unreachable leaf), equal search score
        candidates: &[("hub".into(), 1u8, 0.5), ("d".into(), 255u8, 0.5)],
        weights: &Default::default(),
        community_scope: None,
    });
    assert_eq!(out[0].node_id, "hub", "central reachable node must rank first: {out:?}");
    // hub is overlay (re-ranked neighbor), provenance must say so.
    assert_eq!(out[0].provenance, "overlay");
    // explainable components present and finite.
    assert!(out[0].components.centrality > 0.0);
    assert!(out[0].fused_score.is_finite());
}

#[test]
fn seed_node_is_labeled_structural_not_overlay() {
    let r = reader();
    let out = rank_overlay(RankInput {
        reader: &r,
        seeds: &[SeedRef { node_id: "a".into(), search_score: 0.9 }],
        candidates: &[("a".into(), 0u8, 0.9)], // the seed itself (0 hops)
        weights: &Default::default(),
        community_scope: None,
    });
    assert_eq!(out[0].node_id, "a");
    assert_eq!(out[0].provenance, "structural", "0-hop seed is structural ground truth");
}

#[test]
fn community_scope_filters_out_of_community_candidates() {
    let r = reader();
    let out = rank_overlay(RankInput {
        reader: &r,
        seeds: &[SeedRef { node_id: "hub".into(), search_score: 0.5 }],
        candidates: &[("a".into(), 1u8, 0.5), ("d".into(), 2u8, 0.5)],
        weights: &Default::default(),
        community_scope: Some("core".into()),
    });
    assert!(out.iter().all(|n| n.node_id != "d"), "out-of-community node must be dropped");
}
```

Run: `cargo test -p vox-search --test graph_overlay` → **expected: fails to compile**.

**Step 2 — implement.** Append to `crates/vox-search/src/graph_overlay.rs`:

```rust
use vox_graphify_reader::GraphifyReader;

/// A resolved seed carried into ranking.
#[derive(Debug, Clone)]
pub struct SeedRef {
    pub node_id: String,
    pub search_score: f64,
}

/// Input to the composite ranker. `candidates` are `(node_id, hops_from_nearest_seed, search_score)`.
pub struct RankInput<'a> {
    pub reader: &'a GraphifyReader,
    pub seeds: &'a [SeedRef],
    pub candidates: &'a [(String, u8, f64)],
    pub weights: &'a GraphOverlayWeights,
    /// When set, drop candidates not in this community.
    pub community_scope: Option<String>,
}

/// Explainable score breakdown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoreComponents {
    pub search: f64,
    pub proximity: f64,
    pub centrality: f64,
    pub dead_end: f64,
}

/// A ranked node with provenance + explainable components.
#[derive(Debug, Clone)]
pub struct RankedNode {
    pub node_id: String,
    pub label: Option<String>,
    pub fused_score: f64,
    pub components: ScoreComponents,
    pub hops_from_seed: u8,
    pub community: Option<String>,
    /// "structural" for a 0-hop seed/exact node; "overlay" for a re-ranked neighbor.
    pub provenance: &'static str,
}

/// Proximity decays geometrically with hop count: 1.0 at hop 0, 0.5 at hop 1, ...
fn proximity_decay(hops: u8) -> f64 {
    0.5_f64.powi(i32::from(hops))
}

/// Composite re-rank. Read-only over the graph; emits scores in the return value only.
pub fn rank_overlay(input: RankInput<'_>) -> Vec<RankedNode> {
    let w = input.weights;
    let seed_ids: std::collections::HashSet<&str> =
        input.seeds.iter().map(|s| s.node_id.as_str()).collect();

    let mut out: Vec<RankedNode> = input
        .candidates
        .iter()
        .filter(|(id, _, _)| input.reader.contains(id)) // honesty: never rank a non-node
        .filter(|(id, _, _)| match &input.community_scope {
            Some(scope) => input.reader.community_of(id).as_deref() == Some(scope.as_str()),
            None => true,
        })
        .map(|(id, hops, search_score)| {
            let centrality = input.reader.centrality_normalized(id);
            // Dead-end penalty: a degree-0 (isolated/unreachable) node is penalized.
            let dead_end = if input.reader.degree(id) == 0 { 1.0 } else { 0.0 };
            let proximity = proximity_decay(*hops);
            let components = ScoreComponents {
                search: *search_score,
                proximity,
                centrality,
                dead_end,
            };
            let fused = w.search * search_score
                + w.proximity * proximity
                + w.centrality * centrality
                - w.dead_end * dead_end;
            let provenance = if *hops == 0 && seed_ids.contains(id.as_str()) {
                "structural"
            } else {
                "overlay"
            };
            RankedNode {
                node_id: id.clone(),
                label: input.reader.label_of(id),
                fused_score: fused,
                components,
                hops_from_seed: *hops,
                community: input.reader.community_of(id),
                provenance,
            }
        })
        .collect();

    out.sort_by(|a, b| {
        b.fused_score
            .partial_cmp(&a.fused_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.node_id.cmp(&b.node_id)) // deterministic tiebreak
    });
    out
}
```

Run: `cargo test -p vox-search --test graph_overlay` → **expected: `test result: ok. 5 passed`** (2 from C1 + 3 here).

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-search/src/graph_overlay.rs crates/vox-search/tests/graph_overlay.rs crates/vox-search/Cargo.toml
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): composite overlay ranker (proximity/centrality/dead-end) with provenance labels"
```

- [ ] D1 complete

---

# BATCH 3 — `vox_discover` MCP tool (sequential; all touch vox-orchestrator-mcp)

## Task E1 — `discover_tools.rs` handler (TDD) — [SEQUENTIAL] (depends A2, D1, B1)

**Files:** `crates/vox-orchestrator-mcp/src/discover_tools.rs` (new), `crates/vox-orchestrator-mcp/src/lib.rs` (`mod discover_tools;`).

`vox_discover` composes: lexical-seed (`lexical_search_graph` over the corpus graph — Vox Search BM25 seeding is folded in at F3 behind the flag) → resolve (`resolve::resolve_seed`) → BFS expand (`reader.bfs_from_seeds`) → composite rank (`graph_overlay::rank_overlay`). `mode:"structure_only"` skips the lexical-seed step (seed = the query string treated as an exact node id). Persist seeds as `graphify_search_hit` via the existing path (reused for recall).

**Step 1 — failing test.** Create `crates/vox-orchestrator-mcp/src/discover_tools.rs` with the handler stub `todo!()` plus an inline `#[cfg(test)]` module modeled on `graphify_tools.rs`'s tests (reuse `write_registry`, `write_sample_graph`, `test_state_for_repo` — copy them, or factor a shared `pub(crate)` test helper). Test body:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use vox_orchestrator::{
        AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager,
    };
    use vox_repository::{RepoCapabilities, RepositoryContext};
    use vox_skills::new_registry_arc;

    fn write_registry(repo: &Path) {
        let dir = repo.join("contracts/retrieval");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graphify-corpora.v1.yaml"),
            include_str!("../../../contracts/retrieval/graphify-corpora.v1.yaml"),
        )
        .unwrap();
    }

    fn write_graph(repo: &Path) {
        let dir = repo.join(".vox/cache/graphify/repo-code-graph");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graph.json"),
            r#"{"nodes":[
                {"id":"auth","label":"authentication module","community":"sec"},
                {"id":"crypto","label":"crypto lib","community":"sec"},
                {"id":"ui","label":"ui widget","community":"front"}
            ],"links":[{"source":"auth","target":"crypto"}]}"#,
        )
        .unwrap();
    }

    fn test_state_for_repo(root: std::path::PathBuf) -> ServerState {
        // identical to graphify_tools.rs::test_state_for_repo — copy verbatim
        let cfg = OrchestratorConfig::for_testing();
        let orch_cfg = cfg.clone();
        let groups = AffinityGroupRegistry::new(vec![]);
        let session_cfg = SessionConfig {
            persist: false,
            sessions_dir: std::env::temp_dir().join("vox-mcp-discover-test-sessions"),
            ..SessionConfig::default()
        };
        let session_manager = SessionManager::new(session_cfg).expect("session manager");
        let repository = RepositoryContext {
            root,
            git_root: None,
            repository_id: "discover-test".into(),
            origin_url: None,
            capabilities: RepoCapabilities {
                vox_project: false, cargo_workspace: false, cargo_package: false,
                node_workspace: false, python_project: false, go_module: false, git: false,
            },
            has_vox_agents_dir: false,
            vox_toml: None,
        };
        ServerState::test_stub(
            cfg, repository,
            Arc::new(Orchestrator::with_groups(orch_cfg, groups)),
            Arc::new(Mutex::new(session_manager)),
            new_registry_arc(),
        )
    }

    #[tokio::test]
    async fn discover_seeds_then_expands_with_provenance() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        write_graph(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = discover(
            &state,
            DiscoverParams {
                corpus: Some("repo-code-graph".into()),
                query: "authentication".into(),
                radius: Some(1),
                community_scope: Some(false),
                mode: Some("search_seed".into()),
                limit: Some(30),
                persist: Some(false),
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], serde_json::json!(true), "tool error: {json}");
        let data = &parsed["data"];
        // seed = auth (label match), labeled structural.
        let seeds = data["seeds"].as_array().expect("seeds");
        assert!(seeds.iter().any(|s| s["node_id"] == "auth"), "auth must seed: {json}");
        // expansion reached crypto (1 hop) as an overlay result.
        let results = data["results"].as_array().expect("results");
        let crypto = results.iter().find(|r| r["node_id"] == "crypto").expect("crypto in results");
        assert_eq!(crypto["provenance"], serde_json::json!("overlay"));
        assert!(crypto["components"]["proximity"].as_f64().unwrap() > 0.0);
        // every result carries a provenance label.
        assert!(results.iter().all(|r| r["provenance"].is_string()));
    }

    #[tokio::test]
    async fn structure_only_mode_skips_search_lane_and_is_deterministic() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        write_graph(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let call = |q: &str| {
            let q = q.to_string();
            let st = &state;
            async move {
                discover(st, DiscoverParams {
                    corpus: Some("repo-code-graph".into()),
                    query: q, radius: Some(1), community_scope: Some(false),
                    mode: Some("structure_only".into()), limit: Some(30), persist: Some(false),
                }).await
            }
        };
        let a = call("auth").await;
        let b = call("auth").await;
        assert_eq!(a, b, "structure_only must be byte-identical across runs");
        let parsed: serde_json::Value = serde_json::from_str(&a).unwrap();
        // exact seed = the query as a node id; crypto is the 1-hop expansion.
        let seeds = parsed["data"]["seeds"].as_array().unwrap();
        assert_eq!(seeds[0]["node_id"], "auth");
        assert_eq!(seeds[0]["why"], "exact");
    }
}
```

Run: `cargo test -p vox-orchestrator-mcp discover_` → **expected: fails to compile** (`discover`/`DiscoverParams` undefined).

**Step 2 — implement.** Top of `crates/vox-orchestrator-mcp/src/discover_tools.rs`:

```rust
//! Fused graph-RAG discovery (`vox_discover`): search-seed → graph-expand → composite re-rank.
//!
//! HONESTY: reads `graph.json` read-only; overlay scores are returned in the response and
//! never persisted as edges/node fields. `mode:"structure_only"` skips the search lane and
//! is byte-deterministic. Each result is provenance-labeled (`structural` | `overlay`).

use serde::Deserialize;
use std::fs;

use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_config::graphify::{lexical_search_graph, load_graphify_corpora};
use vox_graphify_reader::resolve::{resolve_seed, SeedCandidate};
use vox_graphify_reader::GraphifyReader;
use vox_search::graph_overlay::{rank_overlay, GraphOverlayWeights, RankInput, SeedRef};

const REM_DISCOVER: &str =
    "Ensure the corpus graph.json exists (run `vox graphify rebuild`) and the corpus is registered.";

#[derive(Debug, Deserialize)]
pub struct DiscoverParams {
    pub corpus: Option<String>,
    pub query: String,
    pub radius: Option<u8>,
    pub community_scope: Option<bool>,
    /// "auto" | "search_seed" | "structure_only" (default "auto").
    pub mode: Option<String>,
    pub limit: Option<usize>,
    /// When true (default), persist resolved seeds as graphify_search_hit for recall.
    pub persist: Option<bool>,
}

pub async fn discover(state: &ServerState, params: DiscoverParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_DISCOVER).to_json(),
    };
    let corpus_id = params.corpus.clone().unwrap_or_else(|| reg.default_corpus_id.clone());
    let Some(corpus) = reg.corpora.iter().find(|c| c.id == corpus_id) else {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("unknown corpus: {corpus_id}"), REM_DISCOVER).to_json();
    };
    let graph_path = repo_root.join(&corpus.graph_path);
    let raw = match fs::read_to_string(&graph_path) {
        Ok(s) => s,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("read {}: {e}", graph_path.display()), REM_DISCOVER).to_json(),
    };
    let graph_value: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("parse {}: {e}", graph_path.display()), REM_DISCOVER).to_json(),
    };
    let reader = match GraphifyReader::from_value(graph_value.clone()) {
        Ok(r) => r,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_DISCOVER).to_json(),
    };

    let mode = params.mode.as_deref().unwrap_or("auto");
    let radius = params.radius.unwrap_or(1).min(5);
    let limit = params.limit.unwrap_or(30).max(1);
    let scope = params.community_scope.unwrap_or(false);

    // ── Seed step ────────────────────────────────────────────────────────────
    // structure_only: the query is treated as an exact node id (no search lane).
    let mut seeds: Vec<(String, &'static str, f64)> = Vec::new(); // (node_id, why, score)
    if mode == "structure_only" {
        if reader.contains(&params.query) {
            seeds.push((params.query.clone(), "exact", 1.0));
        }
    } else {
        // lexical-seed first (label overlap over the graph). The embedding lane is F3.
        let lex = lexical_search_graph(&graph_value, &corpus_id, &params.query, limit);
        for h in &lex {
            let cand = SeedCandidate::node_id(h.node_id.clone(), h.score as f64);
            if let Some((id, _why, sc)) = resolve_seed(&reader, &corpus_id, &cand) {
                seeds.push((id, "search", sc));
            }
        }
    }
    if seeds.is_empty() {
        return ToolResult::ok(serde_json::json!({
            "corpus_id": corpus_id, "seeds": [], "results": [],
            "mode": mode, "note": "no seeds resolved",
        })).to_json();
    }

    // ── Expand step (BFS to radius) ──────────────────────────────────────────
    let seed_ids: Vec<&str> = seeds.iter().map(|(id, _, _)| id.as_str()).collect();
    let frontier = reader.bfs_from_seeds(&seed_ids, radius, limit.saturating_mul(2).max(16));

    // Build candidate triples: seeds at hop 0 + BFS frontier at their depth.
    // Seed search_score carries onto its own candidate; neighbors inherit the max seed score.
    let max_seed_score = seeds.iter().map(|(_, _, s)| *s).fold(0.0_f64, f64::max);
    let mut candidates: Vec<(String, u8, f64)> = Vec::new();
    for (id, _, sc) in &seeds {
        candidates.push((id.clone(), 0, *sc));
    }
    for hit in &frontier {
        if !seeds.iter().any(|(id, _, _)| id == &hit.node_id) {
            candidates.push((hit.node_id.clone(), hit.depth, max_seed_score));
        }
    }

    let weights = GraphOverlayWeights::from_env();
    let community_scope = if scope {
        seeds.first().and_then(|(id, _, _)| reader.community_of(id))
    } else {
        None
    };
    let ranked = rank_overlay(RankInput {
        reader: &reader,
        seeds: &seeds.iter().map(|(id, _, s)| SeedRef { node_id: id.clone(), search_score: *s }).collect::<Vec<_>>(),
        candidates: &candidates,
        weights: &weights,
        community_scope,
    });
    let ranked: Vec<_> = ranked.into_iter().take(limit).collect();

    // ── Persist seeds for recall (reuse existing graphify_search_hit path) ────
    if params.persist.unwrap_or(true) {
        persist_seeds(state, &corpus_id, &params.query, &seeds).await;
    }

    let seeds_json: Vec<serde_json::Value> = seeds.iter().map(|(id, why, sc)| serde_json::json!({
        "node_id": id, "label": reader.label_of(id), "why": why, "search_score": sc,
    })).collect();
    let results_json: Vec<serde_json::Value> = ranked.iter().map(|r| serde_json::json!({
        "node_id": r.node_id,
        "label": r.label,
        "fused_score": r.fused_score,
        "components": {
            "search": r.components.search,
            "proximity": r.components.proximity,
            "centrality": r.components.centrality,
            "dead_end": r.components.dead_end,
        },
        "hops_from_seed": r.hops_from_seed,
        "community": r.community,
        "provenance": r.provenance,
    })).collect();

    ToolResult::ok(serde_json::json!({
        "corpus_id": corpus_id,
        "mode": mode,
        "seeds": seeds_json,
        "results": results_json,
    })).to_json()
}

async fn persist_seeds(
    state: &ServerState,
    corpus_id: &str,
    query: &str,
    seeds: &[(String, &'static str, f64)],
) {
    // Best-effort; DB unavailability must not fail discovery.
    let searched_at = chrono::Utc::now().to_rfc3339();
    if let Ok(db) = vox_db::VoxDb::connect_default().await {
        for (node_id, why, _) in seeds {
            let kid = format!("graphify:{corpus_id}:discover:{node_id}");
            let metadata = serde_json::json!({
                "corpus_id": corpus_id, "query": query, "why": why,
                "searched_at": searched_at, "source": "vox_discover_seed",
            }).to_string();
            let _ = db.upsert_knowledge_node(
                &kid,
                node_id,
                &format!("discover seed {node_id} [query: {query}]"),
                Some("graphify_search_hit"),
                Some(&metadata),
                None,
            ).await;
        }
    }
}
```

Add `mod discover_tools;` to `crates/vox-orchestrator-mcp/src/lib.rs` next to `mod graphify_tools;` (grep for it first to place it correctly). **Verify** `vox-orchestrator-mcp/Cargo.toml` depends on `vox-search` and `vox-graphify-reader` (it already uses `vox_graphify_reader` in `graphify_tools.rs`; confirm `vox-search` is present — `grep vox-search crates/vox-orchestrator-mcp/Cargo.toml`; if absent add `vox-search = { path = "../vox-search" }`).

Run: `cargo test -p vox-orchestrator-mcp discover_` → **expected: `test result: ok. 2 passed`**.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/discover_tools.rs crates/vox-orchestrator-mcp/src/lib.rs crates/vox-orchestrator-mcp/Cargo.toml
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(mcp): vox_discover handler — lexical-seed → graph-expand → composite rank with provenance"
```

- [ ] E1 complete

---

## Task E2 — `vox_discover` schema + dispatch arm (TDD) — [SEQUENTIAL] (depends E1)

**Files:** `crates/vox-orchestrator-mcp/src/input_schemas.rs`, `crates/vox-orchestrator-mcp/src/dispatch.rs`.

**Step 1 — failing test.** Add to the existing test module in `input_schemas.rs` (find a `#[test]` that calls the schema fn, e.g. `schema_for("vox_graphify_search")`):

```rust
    #[test]
    fn vox_discover_schema_is_object_with_required_query() {
        let schema = schema_for_tool("vox_discover").expect("schema present");
        assert_eq!(schema["type"], serde_json::json!("object"));
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["query"]));
        assert_eq!(
            schema["properties"]["mode"]["enum"],
            serde_json::json!(["auto", "search_seed", "structure_only"])
        );
    }
```

> Use the exact schema-lookup fn name already in this file (grep for `fn schema_for` / `pub fn` returning the parsed object). Match the neighboring tests' calling convention.

Run: `cargo test -p vox-orchestrator-mcp vox_discover_schema` → **expected: fails** (no arm).

**Step 2 — implement.** In `input_schemas.rs`, add an arm beside the `vox_graphify_*` arms (after line ~485):

```rust
        "vox_discover" => parse_obj(
            r#"{"type":"object","properties":{"query":{"type":"string","minLength":1,"description":"Fuzzy or exact query. In structure_only mode this is treated as an exact node id."},"corpus":{"type":"string","description":"Graphify corpus id; omit for default corpus"},"radius":{"type":"integer","minimum":0,"maximum":5,"description":"BFS expansion depth from resolved seeds (default 1)"},"community_scope":{"type":"boolean","description":"Restrict expansion to the seed's community (default false)"},"mode":{"type":"string","enum":["auto","search_seed","structure_only"],"description":"auto/search_seed run the lexical-seed lane; structure_only skips it and is byte-deterministic (default auto)"},"limit":{"type":"integer","minimum":1,"description":"Max ranked results (default 30)"},"persist":{"type":"boolean","default":true,"description":"Persist resolved seeds as graphify_search_hit for recall"}},"required":["query"],"additionalProperties":false}"#,
        ),
```

In `dispatch.rs`, add an arm after the `vox_graphify_compare` arm (line ~641):

```rust
        "vox_discover" => {
            Ok(crate::discover_tools::discover(state, serde_json::from_value(args)?).await)
        }
```

Run: `cargo test -p vox-orchestrator-mcp vox_discover_schema` → **expected: `test result: ok. 1 passed`**. Then `cargo build -p vox-orchestrator-mcp` → **expected: clean build**.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/input_schemas.rs crates/vox-orchestrator-mcp/src/dispatch.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(mcp): register vox_discover schema + dispatch arm"
```

- [ ] E2 complete

---

## Task E3 — Catalog entry + registry regeneration (TDD) — [SEQUENTIAL] (depends E2)

**Files:** `contracts/operations/catalog.v1.yaml`, plus whatever `vox ci operations-sync --target mcp --write` regenerates.

**Step 1 — add the catalog SSOT entry.** After the `graphify.compare` block (search `id: graphify.compare`), insert:

```yaml
- id: discover
  title: Discover (fused graph-RAG)
  description: Fused search-seed to graph-expand to composite-ranked code discovery (read-only; provenance-labeled structural|overlay).
  description_human: null
  product_lane: platform
  intent_tags:
  - retrieval
  - graph
  side_effect_class: none
  scope_kind: repository
  reversible: true
  requires_repo: true
  preferred_for_models: true
  human_takeover_friendly: true
  mens_planner_visible: null
  canonical_name: null
  latin_aliases: null
  mcp:
    name: vox_discover
    http_read_role_eligible: true
    tier: core
  cli: null
```

**Step 2 — regenerate + verify.** Run:

```
cd /c/Users/Owner/vox-graphify-gui && cargo run -q -p vox-cli -- ci operations-sync --target mcp --write
```

**Expected:** exit 0; any generated MCP registry file under `contracts/operations/` now lists `vox_discover`. Then run the parity check:

```
cd /c/Users/Owner/vox-graphify-gui && cargo run -q -p vox-cli -- ci operations-sync --target mcp
```

**Expected:** exit 0 with no "would change" / drift message (the `--write` already synced). If `vox-cli` fails to build due to the known `db_cli` WIP breakage on this branch, **skip the live regen** and instead assert the SSOT entry is well-formed by parsing it: `cargo test -p vox-cli operations_catalog` if that crate builds; otherwise note in the commit message that regen is deferred to a `vox-cli`-buildable checkpoint and the catalog SSOT entry is the source of truth. (The dispatch arm in E2 is the runtime SSOT regardless.)

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add contracts/operations/catalog.v1.yaml
git -C /c/Users/Owner/vox-graphify-gui add -A contracts/operations/
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(catalog): register vox_discover (tier core, retrieval/graph) + regen mcp registry"
```

- [ ] E3 complete

---

# BATCH 4 — Corpus hook, KG integration, embedding lane (parallel off Batch 3)

## Task F1 — `SearchCorpus::GraphifyNodes` variant (TDD) — [PARALLEL-SAFE] (P3 hook; isolated)

**Files:** `crates/vox-db-types/src/retrieval.rs`.

Adds the corpus variant so P3 (semantic overlay) and the embedding lane have a first-class corpus to target. This task only adds the enum variant + serde round-trip; it does NOT wire execution (that is P3's job) — keeping this isolated and parallel-safe.

**Step 1 — failing test.** In `crates/vox-db-types/src/retrieval.rs`'s test module (or add one):

```rust
#[test]
fn graphify_nodes_corpus_serdes_snake_case() {
    let c = SearchCorpus::GraphifyNodes;
    let s = serde_json::to_string(&c).unwrap();
    assert_eq!(s, "\"graphify_nodes\"");
    let back: SearchCorpus = serde_json::from_str(&s).unwrap();
    assert_eq!(back, SearchCorpus::GraphifyNodes);
}
```

Run: `cargo test -p vox-db-types graphify_nodes_corpus` → **expected: fails to compile**.

**Step 2 — implement.** Add the variant to the enum (after `SymbolProximity`):

```rust
    SymbolProximity,
    /// Structural-index nodes as a retrieval unit (label + kind + module path + doc).
    /// Reserved for the embedding/semantic lanes (Vox Search owns embeddings).
    GraphifyNodes,
```

Check exhaustive `match` sites: `grep -rn "SearchCorpus::SymbolProximity" crates/ | grep -v test`. Any non-exhaustive `match SearchCorpus` that the compiler flags must get a `SearchCorpus::GraphifyNodes => { /* not yet wired; P3 */ }` arm (or fall into an existing default). Build to find them: `cargo build -p vox-db -p vox-search` and fix each reported `non-exhaustive` error with a no-op arm + `// P3: semantic lane` comment.

Run: `cargo test -p vox-db-types graphify_nodes_corpus && cargo build -p vox-db -p vox-search` → **expected: test passes, builds clean**.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-db-types/src/retrieval.rs crates/vox-db crates/vox-search
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-db-types): SearchCorpus::GraphifyNodes corpus variant (P3 semantic-lane hook)"
```

- [ ] F1 complete

---

## Task F2 — KG-corpus relevance integration test (the 0.0 fix, end-to-end) (TDD) — [PARALLEL-SAFE] (depends B1, E2)

**Files:** `crates/vox-search/tests/kg_relevance.rs` (new).

B1 fixed the SQL ordering; this proves the KG corpus, end-to-end through `execute_search_plan`, now yields a relevance-ordered, non-degenerate KG lane (the masked-by-ranker case is separately covered by E1). This is the standalone-corpus half of design fork §6.3.

**Step 1 — write the test.** Create `crates/vox-search/tests/kg_relevance.rs`. Model the harness on existing `vox-search` integration tests that build a `SearchRuntimeContext` with an attached `VoxDb` (grep `crates/vox-search/tests` for the constructor pattern — reuse it verbatim; do NOT invent one). Seed two `knowledge_nodes` (one strong, one weak match), run a `SearchPlan` containing `SearchCorpus::KnowledgeGraph` through `execute_search_plan`, and assert:

```rust
// after collecting unified_hits filtered to source=="knowledge":
assert!(!kg_hits.is_empty(), "KG lane produced hits");
assert!(kg_hits[0].score > 0.0, "top KG score must be non-zero (the 0.0 bug)");
assert!(kg_hits[0].title.as_deref().unwrap_or("").contains("strong")
    || kg_hits[0].path.as_deref().unwrap_or("").contains("strong"),
    "strong match must rank first via bm25 ordering");
```

If `vox-search` integration tests require a feature (e.g. an FTS-enabled DB build), gate the test the same way the neighbors do (`#[cfg(...)]`); if no such harness exists in `vox-search/tests`, place this test in `crates/vox-db/src/store/ops_memory/knowledge.rs` instead (it already has DB access) asserting `query_knowledge_nodes` returns the strong row first — i.e. fold it into B1's module as a second test and SKIP creating a new file (note this choice in the commit message).

Run: `cargo test -p vox-search --test kg_relevance` (or `-p vox-db`) → **expected: passes**.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-search/tests/kg_relevance.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "test(vox-search): KG corpus yields non-zero, relevance-ordered scores end-to-end"
```

- [ ] F2 complete

---

## Task F3 — Embedding-lane seed flag (behind `VOX_SEARCH_DISCOVER_EMBED`) (TDD) — [PARALLEL-SAFE] (depends D1, E1)

**Files:** `crates/vox-orchestrator-mcp/src/discover_tools.rs`.

Per the baked decision: lexical-seed first; the embedding lane is a strict upgrade behind a flag, **off by default, no model call when off**. The seed step gains an optional embedding-backed seeding via Vox Search's `EmbeddingService`; when the flag is unset the code path is identical to E1 (proven by an env-off test).

**Step 1 — failing test.** Append to `discover_tools.rs` tests:

```rust
    #[tokio::test]
    async fn embed_flag_off_matches_lexical_seed_path() {
        // With the flag unset, discover must behave exactly as the lexical-seed path (no model call).
        std::env::remove_var("VOX_SEARCH_DISCOVER_EMBED");
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        write_graph(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = discover(&state, DiscoverParams {
            corpus: Some("repo-code-graph".into()), query: "authentication".into(),
            radius: Some(1), community_scope: Some(false), mode: Some("auto".into()),
            limit: Some(30), persist: Some(false),
        }).await;
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], serde_json::json!(true));
        // seed lane reported as lexical when flag is off.
        assert_eq!(parsed["data"]["seed_lane"], serde_json::json!("lexical"));
    }
```

Run: `cargo test -p vox-orchestrator-mcp embed_flag_off` → **expected: fails** (`seed_lane` absent).

**Step 2 — implement.** In `discover`, add a `seed_lane` decision and report it. Replace the `else` (non-structure_only) seed block with:

```rust
    } else {
        let embed_on = std::env::var("VOX_SEARCH_DISCOVER_EMBED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        seed_lane = if embed_on { "embedding" } else { "lexical" };
        if embed_on {
            // Embedding-backed seeding: reuse Vox Search's EmbeddingService over the
            // GraphifyNodes corpus. Falls back to lexical if no embedder is configured.
            match embedding_seed(state, &graph_value, &corpus_id, &reader, &params.query, limit).await {
                Some(s) if !s.is_empty() => seeds = s,
                _ => { seed_lane = "lexical"; lexical_seed(&graph_value, &corpus_id, &reader, &params.query, limit, &mut seeds); }
            }
        } else {
            lexical_seed(&graph_value, &corpus_id, &reader, &params.query, limit, &mut seeds);
        }
    }
```

Declare `let mut seed_lane = "exact";` before the mode branch (structure_only keeps `"exact"`), factor the lexical loop from E1 into `fn lexical_seed(...)`, and add an `async fn embedding_seed(...) -> Option<Vec<(String, &'static str, f64)>>` that calls `vox_search::embeddings::EmbeddingService` (mirror the construction in `vox-search/src/execution.rs` lines ~294–308: `embedding_config_from_env().map(|cfg| EmbeddingService::new(db, cfg))`); embed the query, kNN against node-corpus embeddings if present, resolve each to a node via `resolve_seed`. When no embedder/config exists it returns `None` → lexical fallback (so the flag is a strict, safe upgrade). Add `"seed_lane": seed_lane` to the final `ToolResult::ok(...)` payload.

Run: `cargo test -p vox-orchestrator-mcp embed_flag_off` → **expected: passes**. Then re-run E1's tests (`cargo test -p vox-orchestrator-mcp discover_`) → **expected: all still pass** (env-off path unchanged).

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/discover_tools.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(mcp): vox_discover embedding-seed lane behind VOX_SEARCH_DISCOVER_EMBED (lexical default, no model when off)"
```

- [ ] F3 complete

---

# BATCH 5 — Final gate (sequential)

## Task G1 — Full per-crate verification + self-review — [SEQUENTIAL] (depends all)

**Step 1 — run the touched-crate test suites:**

```
cd /c/Users/Owner/vox-graphify-gui
cargo test -p vox-graphify-reader
cargo test -p vox-search
cargo test -p vox-db-types
cargo test -p vox-db --lib
cargo test -p vox-orchestrator-mcp discover_ vox_discover_schema
```

**Expected:** every command exits 0, `test result: ok` for each. Record the pass counts.

**Step 2 — honesty smoke check (manual assertion, no code):** confirm by grep that the ranker never persists overlay scores as edges/node fields:

```
cd /c/Users/Owner/vox-graphify-gui
grep -n "upsert_knowledge_node\|knowledge_edges\|insert_edge" crates/vox-search/src/graph_overlay.rs crates/vox-graphify-reader/src/resolve.rs
```

**Expected:** **no matches** (the ranker/resolver touch no persistence). The only persistence is `persist_seeds` in `discover_tools.rs` writing `node_type=graphify_search_hit` log rows — confirm it writes no edge:

```
grep -n "knowledge_edges\|insert_edge\|target" crates/vox-orchestrator-mcp/src/discover_tools.rs
```

**Expected:** no edge writes (only `upsert_knowledge_node` with a `graphify_search_hit` log row).

**Step 3 — self-review checklist** (fill in, then commit this plan's checklist state is N/A — just verify against the spec):
- [ ] `vox_discover` exists end-to-end: schema (E2) + dispatch arm (E2) + handler (E1) + catalog (E3).
- [ ] Lexical-seed ships first (E1); embedding lane behind a flag, off by default (F3).
- [ ] KG-score-0.0 fixed independently in `vox-db` (B1) AND masked by composite ranker (E1) — both, per §6.3.
- [ ] Structural signals wired: proximity (`proximity_decay`), centrality (`centrality_normalized`), dead-end penalty, community scope (D1, E1).
- [ ] `mode:"structure_only"` skips the search lane and is byte-deterministic (E1 test).
- [ ] Every result provenance-labeled `structural`|`overlay`; seeds labeled (E1, D1).
- [ ] Overlay scores query-time only; no edge/node mutation (G1 Step 2 grep).
- [ ] `SearchCorpus::GraphifyNodes` hook for P3 (F1).

**Commit (checklist/notes only if any doc file was updated; otherwise no commit — the work committed per task):**
```
git -C /c/Users/Owner/vox-graphify-gui status --short
```
(If clean, G1 is a verification-only task with no commit. If a fix was needed during verification, commit it with a `fix(...)` message scoped to the touched crate.)

- [ ] G1 complete

---

## Self-Review — Spec coverage

Mapping every requirement in **`2026-06-26-graphify-voxsearch-fusion-design.md`** (the source) + the in-scope rows of the umbrella spec to a task:

| Spec requirement (source §) | Task(s) | Notes |
|---|---|---|
| `vox_discover` composes Search→Graph→re-rank, single agent call (§3.3) | E1, E2, E3 | handler + schema + dispatch + catalog |
| Seed resolution: direct / path-symbol / label resolvers (§3.1) | A2 | pure `resolve.rs`, honesty: never fabricate a seed |
| Direction-B expand: BFS to radius, community scope (§3.2) | E1 (uses A1 `community_of`, reader `bfs_from_seeds`) | radius capped at 5; scope via seed community |
| Composite score `w_search·search + w_prox·prox + w_cent·cent − w_dead·dead` (§3.2) | D1, C1 | exact formula; structural-dominant default weights |
| `centrality` / reachability from reader (§3.2) | A1 (`centrality_normalized`, `degree`) | dead-end = degree-0 penalty |
| Weights are config `VOX_SEARCH_GRAPH_*`, default-off/opt-in (§3.2) | C1 | env getter + clamp; structural-dominant defaults |
| `structure_only` skips search lane, deterministic (§3.2, §5.4) | E1 | byte-identical test |
| KG-score-0.0 fix independently AND via ranker (§6.3, §4.1) | B1 (independent), E1/D1 (masked), F2 (e2e proof) | both, per fork resolution |
| Persist seeds as `graphify_search_hit` for recall (§3.3, §4.2) | E1 (`persist_seeds`) | reuses existing path; log row not edge |
| Provenance label `structural`\|`overlay` on every result (§5.3) | D1, E1 | 0-hop seed = structural, neighbors = overlay |
| `graph.json` read-only to fusion; no edge mutation (§5.1, §5.2) | D1, A2 (pure, `&` only), G1 grep gate | verified by grep in G1 |
| Embeddings owned by Vox Search; lexical-seed first, embed behind flag (§6.1, §6.2) | F3, F1 | `VOX_SEARCH_DISCOVER_EMBED`, `GraphifyNodes` corpus |
| Tool surface joins `vox_search_*` family later (umbrella §3.1) | E3 catalog `tier: core` | added as NEW tool; P0 rename re-keys, no ranker change |
| `GraphifyNodes` corpus hook for semantic overlay P3 (umbrella §2.2) | F1 | isolated enum variant + serde |

**Explicitly out of scope (deferred to other plans, stated to avoid scope creep):** the graphify→Vox Search rename / `vox_graphify_*`→`vox_search_*` (P0); data-flow layer (P1); semantic overlay artifact + `vox_search_semantic_related` (P3); GUI `VoxSearchPanel` pane for discover (P5); `.mcp.json`/steering/code-map injection (P4); CLI `cli:` ingestion (P6). This plan delivers exactly the §3.4 "new code" rows: `resolve.rs`, `graph_overlay.rs`, `vox_discover` tool+schema, and the KG fix — plus the `GraphifyNodes` corpus hook that unblocks P3.

**Risk notes for the executor:**
1. `vox-cli` may not build on this branch (db_cli WIP) — E3's live `operations-sync` regen is conditioned; the dispatch arm (E2) is the runtime SSOT regardless, so `vox_discover` is callable even if regen is deferred.
2. Test harness constructors (`VoxDb::connect_in_memory`, `SearchRuntimeContext`) — **do not invent**; grep the neighboring tests in each crate and reuse the exact constructor. The plan flags this at B1 and F2.
3. `vox-search` → `vox-graphify-reader` dep: verified to be acyclic (reader is a leaf; does not depend on vox-search). D1 verifies and adds the path dep if missing.
