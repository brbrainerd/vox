---
category: "Architecture SSOTs"
title: "Graphify ↔ Vox Search Fusion — Graph-Augmented Retrieval Design"
date: 2026-06-26
status: design
---

# Graphify ↔ Vox Search Fusion — Graph-Augmented Retrieval Design

## 0. Summary

Vox already has two retrieval systems that overlap at exactly one table
(`knowledge_nodes`) but do not yet *fuse*:

- **Vox Search** — a hybrid lexical + embedding retrieval engine over six
  corpora (memory, knowledge graph, document chunks, repo inventory, web,
  symbol proximity). It is good at *fuzzy / natural-language* queries and
  ranking, but blind to structure.
- **Graphify** — a deterministic structural knowledge graph
  (`nodes` + `edges` + communities + coverage) built from AST/JSX/crate-dep
  extraction. It is good at *structural* queries (callers/callees,
  reachability, communities) but cannot resolve a fuzzy query like
  "the auth flow".

This document designs **graph-augmented retrieval (graph-RAG)**: a
bidirectional fusion in which Vox Search *resolves* fuzzy queries into seed
nodes that Graphify then *expands* structurally, and Graphify's structure
*re-ranks and filters* Vox Search results — **without ever mutating the
deterministic structural graph**. The structural edges remain the trustworthy
substrate; all search/semantic signals are a clearly labeled, separable
overlay.

The good news: the seam already exists. `vox graphify ingest` projects graph
nodes into `knowledge_nodes`; `vox_graphify_search` scores them lexically and
persists hits; and `vox-graphify-reader` already provides BFS, reachability,
clustering, and compare. The fusion is mostly *wiring* + one new combined
tool, not new infrastructure.

---

## 1. Vox Search — Current State

### 1.1 Corpora indexed today

`SearchCorpus` (`crates/vox-db-types/src/retrieval.rs`):

| Variant | What it indexes | Backend(s) |
|---|---|---|
| `Memory` | Markdown memory docs | BM25 (in-process) + optional vector |
| `KnowledgeGraph` | `knowledge_nodes` rows (incl. Graphify-projected nodes) | FTS5 / LIKE |
| `DocumentChunks` | Ingested doc chunks (RAG) | FTS5 + vector |
| `RepoInventory` | File-path inventory | WalkDir token-overlap (no index) |
| `WebResearch` | Web search | SearXNG → DuckDuckGo → Tavily |
| `SymbolProximity` | Code-symbol proximity heuristic | lexical |

Plus a **virtual corpus** `graphify-search-log` (not a `SearchCorpus` enum
variant — a contract entry in `contracts/retrieval/graphify-corpora.v1.yaml`
with `is_virtual: true`). It has no disk footprint; its content is the
`graphify_search_hit` rows persisted into `knowledge_nodes` by
`vox_graphify_search`. `assess_corpus_status()` short-circuits virtual corpora
to "fresh" so agents query Turso, not disk.

### 1.2 Query model — genuinely hybrid

`RetrievalMode` ∈ `{FullText, Vector, Hybrid}`. Scoring is corpus-specific:

- **Memory** (`crates/vox-search/src/memory_hybrid.rs`): real **BM25**
  (`idf = ln(1 + (N−df+0.5)/(df+0.5))`, `k1=1.2`, `b=0.75`, env-tunable) with a
  status boost (`current` 1.2× … `deprecated` 0.2×) and a 180-day exponential
  temporal decay; optionally fused with vector similarity at `vector_weight`
  (default 0.55).
- **Document chunks** (`crates/vox-db/src/store/ops_memory/search.rs`):
  FTS5 reciprocal-rank fused with brute-force L2 embedding similarity
  (`vector_weight` 0.60).
- **KnowledgeGraph** (`crates/vox-search/src/execution.rs`): FTS5 or LIKE
  fallback over `knowledge_nodes(label, content)`, scored `1/(1+rank)`.
  **Known bug (omni-search audit):** score collapses to 0.0 when no FTS table
  exists — relevant to fusion (§6).
- **Repo inventory**: O(files×tokens) WalkDir token overlap, 20k-file cap,
  +1.3× for exact path-segment match. No persistent index.

So Vox Search is **hybrid (lexical BM25/FTS + optional embeddings)** — *not*
lexical-only. Embedding infrastructure is present and active:
`EmbeddingService` in vox-search, an `embeddings` table (BLOB vectors) in
vox-db, brute-force `search_embeddings()`, and an optional Qdrant sidecar
behind the `qdrant-vector` feature.

### 1.3 How the GUI / agents call it

- GUI: Tauri command `vox_search_query(query, scope, kinds, path_glob, limit,
  offset)` in `crates/vox-gui/src/commands/search.rs`; `scope_to_corpus()` maps
  `["memory","knowledge","chunk","repo","web","symbol"]` → corpora; returns
  `SearchResponseDto { UnifiedHitDto[] }`.
- Agents (MCP): `vox_memory_search` (full bundle), `vox_knowledge_query`
  (KG only), `vox_research_run` (web+synthesis), and `vox_graphify_search`
  (the Graphify lexical path).

---

## 2. Current Graphify ↔ Search Touchpoints

There is exactly **one** data-flow seam today, and it runs through
`knowledge_nodes`. Two directions exist on it:

### 2.1 Graph → store (ingest projection)

`vox graphify ingest --corpus <id>`
(`crates/vox-cli/src/commands/graphify/mod.rs::run_graphify_ingest`):
reads `corpus.graph_path` (`graph.json`), calls
`project_graph_nodes_for_ingest()` (`crates/vox-config/src/graphify.rs`), and
upserts each node:

| `knowledge_nodes` field | Source |
|---|---|
| `id` | `graphify:{corpus_id}:node:{node_id}` |
| `label` | node `label` / `id` / `name` |
| `content` | full node JSON, serialized |
| `node_type` | node `type` or `"graph_node"` |
| `metadata` | `{corpus_id, source:"graphify_lexical_ingest"}` |

Freshness is tracked by stamping `manifest.lexical_ingest_sha256`; a mismatch
vs `graph.json` is "lexical lag" surfaced by `assess_corpus_status()` /
`vox graphify refresh`.

### 2.2 Search → store (hit persistence)

`vox_graphify_search` (`crates/vox-orchestrator-mcp/src/graphify_tools.rs`)
does **lexical token-overlap** scoring over node labels
(`lexical_search_graph`: `score = |query_tokens ∩ label_tokens|`), reading the
on-disk `graph.json` (read-only against the structural graph). When
`persist=true` it upserts each hit as a `graphify_search_hit` node:
`id = graphify:{corpus}:search:{query_slug}:{node_id}`,
`metadata = {corpus_id, query, searched_at, git_sha, source}`. These rows are
the content of the `graphify-search-log` virtual corpus — a queryable trail of
"what was found when".

### 2.3 Structural reader (already built)

`crates/vox-graphify-reader/` already exposes the graph algorithms fusion
needs: `bfs.rs` (neighbor expansion), `reachability.rs` (coverage /
Surfaced·OrphanBackend·DeadEnd), `cluster.rs` (communities), `compare.rs`
(manifest diff), `lens.rs` / `overlay.rs`. MCP tools `vox_graphify_query`
(BFS), `vox_graphify_path` (shortest path), and `vox_graphify_compare` already
wrap it. **The structural muscle is present; it is simply not wired to
search.**

### 2.4 The gap

The two directions on the seam never *meet*. Ingest pushes nodes into the
search store but search results are never expanded structurally; lexical hits
are persisted but never re-ranked by graph centrality; and there is no single
tool that does "search-seed → graph-expand → ranked structured results". Vox
Search treats Graphify nodes as flat text rows (`KnowledgeGraph` corpus), and
the rich edge/community/coverage structure sitting in `graph.json` is invisible
to the ranker.

---

## 3. Graph-Augmented Retrieval (Graph-RAG) Design

### 3.1 Direction A — Search → Graph (resolve the fuzzy query)

The structural graph cannot answer "the auth flow"; Vox Search can. So Vox
Search becomes the **resolver / seeder**:

1. Run the fuzzy/NL query through Vox Search (memory + chunks + KG +,
   optionally, the embedding lane) → top-k `UnifiedHit`s.
2. **Resolve hits to graph node IDs.** Three resolvers, in priority order:
   - *Direct*: hit is already a Graphify node (`id` starts `graphify:…:node:`
     or `source=graphify_lexical_ingest`) → use it.
   - *Path/symbol*: hit carries a file path or symbol → map to the graph node
     whose `file`/`symbol` matches (reuse repo-inventory + symbol-proximity
     locators).
   - *Label*: fall back to `lexical_search_graph` over labels for the residual
     query terms.
3. The resolved IDs are the **seed set** for graph expansion.

This is the bridge to the semantic overlay: embeddings/BM25 do the
"understand what the human meant" step; the graph does the "and everything
structurally connected to it" step.

### 3.2 Direction B — Graph → Search (expand & re-rank)

Given seeds, use `vox-graphify-reader`:

- **Expand**: `bfs` to radius *r* (default 1–2) → callers/callees / neighbors;
  optionally restrict to the seeds' **community** (`cluster.rs`) to keep
  expansion subsystem-local.
- **Score (overlay layer)**: each candidate node gets a *composite* score:
  ```
  fused = w_search · search_score            (BM25/vector — semantic relevance)
        + w_prox   · proximity_decay(hops)    (graph distance from a seed)
        + w_cent   · centrality(node)         (degree / god-node-ness)
        − w_dead   · dead_end_penalty(node)   (orphan / unreachable)
  ```
  `centrality` and reachability classes come from `reachability.rs`; weights
  are config (`VOX_SEARCH_GRAPH_*`), default-off so behavior is opt-in.
- **Structure-only queries** ("find X and everything that calls it") skip the
  search lane entirely: seed = exact node, return the BFS frontier ranked by
  proximity + centrality. Deterministic, no overlay needed.

### 3.3 The combined agent tool

Add **one** MCP tool, `vox_discover` (working name), that composes the two
directions so an agent makes a single call:

```
vox_discover {
  query: String,            // fuzzy or exact
  corpus: Option<String>,   // graphify corpus; default = repo graph
  radius: u32 = 1,          // BFS depth for expansion
  community_scope: bool,    // restrict expansion to seed community
  mode: "auto" | "search_seed" | "structure_only",
  limit: usize = 30,
}
→ {
  seeds: [{ node_id, label, why: "search"|"exact", search_score }],
  results: [{
    node_id, label, node_type,
    fused_score,
    components: { search, proximity, centrality, dead_end },  // explainable
    hops_from_seed, community, reachability_class,
    provenance: "structural" | "overlay"      // honesty label, §5
  }],
  searched_at, corpus_id, git_sha
}
```

Internally: `vox_search_query` (seed) → resolve → `vox-graphify-reader::bfs`
(+ `cluster`/`reachability`) → composite rank → persist seeds as
`graphify_search_hit` (existing path) for recall. It is a *composition* of
parts that already exist; the new code is the resolver and the composite
ranker.

### 3.4 Where the work lands

| Piece | Home | Status |
|---|---|---|
| Seed resolution (hit → node_id) | new `resolve.rs` in vox-graphify-reader (pure, deterministic) | new |
| BFS / community / reachability | vox-graphify-reader (`bfs`,`cluster`,`reachability`) | exists |
| Composite ranker (overlay) | vox-search (new `graph_overlay.rs`) | new |
| `vox_discover` MCP tool + schema | vox-orchestrator-mcp | new |
| GUI scope `"graph"` + result lane | vox-gui `search.rs` / SearchView | new (later) |

---

## 4. Mutual Improvements

### 4.1 What Graphify gives Vox Search

- **Structure-aware ranking**: re-rank flat KG/repo hits by centrality +
  proximity, so a god-node match outranks a leaf with the same lexical score.
- **Dead-code / orphan filtering**: `reachability.rs` already classifies
  `OrphanBackend` / `DeadEnd`; expose a `exclude_dead: bool` search flag so
  agents searching for "live" code don't surface unreachable nodes.
- **Subsystem scoping**: scope a search to a **community** ("search only the
  auth subsystem") — a structural filter Vox Search cannot express today.
- **Fixes the KG-score 0.0 bug naturally**: once KG hits flow through the
  composite ranker, proximity/centrality give them a real, non-zero score even
  when the FTS table is absent (§1.2).

### 4.2 What Vox Search gives Graphify

- **Semantic seeding**: Graphify's lexical `lexical_search_graph` is pure
  token overlap — it cannot match "auth" to "login"/"credentials". Routing the
  seed step through Vox Search's BM25 + **embeddings** is the semantic layer
  Graphify lacks, and the path to "relate arbitrary features" across
  communities.
- **Fuzzy entry resolution**: turns NL queries into structural entry points —
  the precondition for every graph traversal an agent wants to start from a
  human description.
- **Richer recall trail**: seeds resolved semantically get persisted into
  `graphify-search-log`, so the virtual corpus records *why* a node was an
  entry point, not just that a token matched.

---

## 5. Honesty Boundary (Non-Negotiable)

**The structural graph is deterministic and trustworthy; search/semantic
signals are a separate, labeled overlay that never mutates structural edges.**

Concretely:

1. **`graph.json` / `knowledge_edges` are read-only to the fusion layer.**
   No edge is ever created, weighted, or deleted by search, embeddings, or
   re-ranking. `vox_graphify_search` already reads the graph read-only; that
   invariant extends to `vox_discover`.
2. **Overlay scores live beside, not inside, the graph.** `fused_score` and
   its `components` are computed at query time and returned in the response;
   they are *not* persisted as edge weights or node fields on the structural
   graph. The only persistence is the `graphify_search_hit` *log* rows (a
   query trail, explicitly `node_type=graphify_search_hit`, never an edge).
3. **Every result is provenance-labeled.** `provenance: "structural"` = the
   node/edge came deterministically from AST/crate extraction;
   `provenance: "overlay"` = this node surfaced or got re-ranked because of a
   semantic/lexical signal. A consumer can always strip the overlay and recover
   the pure deterministic graph.
4. **Determinism is preserved for structural queries.** `mode:"structure_only"`
   produces byte-identical results across runs (no embeddings, no model). The
   semantic lane is opt-in (`mode:"auto"`/`search_seed`) and weights default to
   structural-dominant.

This keeps Graphify's core promise — *"bad/unreal structure doesn't show up
because the graph is computed, not guessed"* — while letting fuzzy discovery
ride on top.

---

## 6. Key Design Forks for the Human

1. **Where does the semantic/embedding layer live — Vox Search or Graphify?**
   Recommendation: **Vox Search owns embeddings** (it already has
   `EmbeddingService`, the `embeddings` table, and fusion math). Graphify stays
   purely structural and *calls* Vox Search for the seed step. The alternative
   (give Graphify its own embedding index per corpus) duplicates infra and
   blurs the honesty boundary. Decision needed before building the resolver.

2. **Build embeddings into the seed path now, or ship lexical-seed first?**
   `vox_discover` works end-to-end with *lexical* seeding alone (BM25 + label
   overlap) and is a clean, low-risk first cut. Embedding-seeding is a strict
   upgrade to the resolve step. Recommendation: **ship lexical-seed +
   graph-expand first** (proves the fusion plumbing and the honesty labels),
   then turn on the embedding lane behind a flag. Need a yes/no.

3. **Fix the KG-score 0.0 bug independently, or only via the composite
   ranker?** The bug (§1.2) makes KG hits unrankable today. Fusing through the
   composite ranker masks it, but the standalone KG corpus stays broken.
   Recommendation: fix the standalone `1/(1+rank)` path *and* add the overlay —
   they are independent. Confirm scope.

4. **GUI surfacing timing.** Backend-first (`vox_discover` MCP tool + agents)
   vs. simultaneously adding a `"graph"` scope + a structural result lane to
   SearchView. Recommendation: **backend-first**; GUI is a follow-on once the
   ranker is proven. Confirm.

---

## 7. Relationship to Prior Plans

This design *composes and supersedes the deferred fusion tail* of two existing
plans, without duplicating their landed work:

- `2026-06-18-graphify-search-map-persistence.md` — landed the seam
  (`is_virtual`, `lexical_ingest_sha256`, `persist`, `vox-graphify-reader`,
  `vox_graphify_query/path/compare`). This design **consumes** those.
- `2026-06-18-graphify-search-fusion-plan-F.md` — intent routing
  (`select_corpus_for_intent`). Complementary: intent routing picks *which
  corpus*; `vox_discover` fuses *within* a corpus. They stack.

Next step after ratification: a `writing-plans` pass to decompose §3.4 into
TDD tasks (resolver → composite ranker → `vox_discover` → GUI), gated on the
four forks in §6.
