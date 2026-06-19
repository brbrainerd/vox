---
title: "Graphify Capabilities Audit & Vertical-Integration SSOT (2026-06-18)"
description: "How Graphify works, how agents use code graphs to search efficiently, formalized best/worst-case usage, Rust-native status, and a BUILT/PLANNED/GAP audit of the cache lifecycle (caching, expiry, automated rerun, GUI prompting, visualization, retention with learnable history)."
category: "Architecture SSOTs"
status: "audit"
training_eligible: true
training_rationale: "Single audit SSOT agents consult to decide when to trust/rebuild a graphify corpus, how to query it cheaply, and what cache-lifecycle machinery exists vs is still a gap."
---

# Graphify Capabilities Audit & Vertical-Integration SSOT (2026-06-18)

This doc **audits** how Vox consumes Graphify and lays out the **vertical-integration** target for the
cache lifecycle (cache → expire → rerun → surface → retain/discard, with history we can learn from).

It does **not** restate the architecture survey or the open plans — read those first / alongside:

- Architecture survey & tier model: [`graphify-integration-research-2026-06-16.md`](graphify-integration-research-2026-06-16.md)
- Python-free construction roadmap: [`graphify-python-free-findings-2026.md`](graphify-python-free-findings-2026.md)
- Coverage overlay model: [`semantic-test-coverage-graph-strategy-2026-06-07.md`](semantic-test-coverage-graph-strategy-2026-06-07.md)
- Cold-start state: [`../../superpowers/plans/2026-06-16-graphify-integration-HANDOFF-STATE.md`](../../superpowers/plans/2026-06-16-graphify-integration-HANDOFF-STATE.md)
- Open plans: [run-lifecycle](../../superpowers/plans/2026-06-18-graphify-run-lifecycle.md) · [search-map-persistence](../../superpowers/plans/2026-06-18-graphify-search-map-persistence.md) · [native-coverage-overlays](../../superpowers/plans/2026-06-18-graphify-native-coverage-overlays.md)

---

## 0. TL;DR

- **The read/query/recall half is built and Rust-native.** `vox-graphify-reader` reads `graph.json`
  with zero Python, does BFS / shortest-path / god-nodes / community / manifest-diff, runs Leiden
  clustering (`leiden-rs`) and AST extraction (`syn` + tree-sitter) in-process, and is exposed to agents
  through 5 read-only MCP tools. Freshness is modeled (git-drift, TTL, lexical-lag, missing/corrupt).
- **The write/maintain/automate/surface half is mostly plans-not-executed or undesigned.** Graph
  *construction* still shells to `python -m graphify` for the canonical full-corpus build; "automated
  rerun" today only **prints instructions**; and **GUI prompting + visualization + a learnable
  keep-vs-discard retention policy have no design yet.**
- **The four headline gaps** (vs the user's goals) are, in priority order:
  1. **No autonomous rerun** — staleness is detected but nothing rebuilds without a human typing a command. (§5.3)
  2. **No GUI surface** — corpus health/staleness is invisible in `vox-gui`; the user is never prompted. (§5.4)
  3. **No retention/GC policy and no usage-driven learning** — every rebuild overwrites in place; we keep no
     snapshot history and score nothing, so "which graphs to maintain vs discard" is unanswered. (§5.6, §7)
  4. **Construction is still Python** — the dev-loop friction (Windows MSVC / file locks) the rest of the
     program is trying to delete still lives in the rebuild path. (§4)

---

## 1. What Graphify is and how it actually works

**Graphify** (`graphifyy` on PyPI, [safishamsi/graphify](https://github.com/safishamsi/graphify)) is a Python
pipeline that turns *any* folder (code, docs, papers, images, audio/video) into a persistent, queryable
**knowledge graph**. The full pipeline (from the `/graphify` skill at `~/.claude/skills/graphify/SKILL.md`):

| Step | What happens | Cost | Determinism |
| --- | --- | --- | --- |
| **Detect** | `graphify.detect` walks the tree, classifies files (code/doc/paper/image/video), counts words, flags sensitive files. | Free | Deterministic |
| **AST extract (Part A)** | `graphify.extract` parses code → structural nodes (functions/structs) + call/import edges. | Free | Deterministic |
| **Semantic extract (Part B)** | LLM reads docs/papers/images in 20–25-file chunks → entity/relationship nodes with an **EXTRACTED/INFERRED/AMBIGUOUS** confidence tag. On this machine routed through OpenRouter (`google/gemini-2.5-flash`); else Claude subagents. | **Tokens** | Non-deterministic |
| **Build** | `graphify.build.build_from_json` → NetworkX graph (undirected by default, `--directed` opt-in). | Free | Deterministic |
| **Cluster** | `graphify.cluster` → Leiden community detection + cohesion scores. | Free | Deterministic |
| **Analyze** | god-nodes (high-degree hubs), "surprising connections" (cross-community bridges), suggested questions. | Free | Deterministic |
| **Report/Export** | `GRAPH_REPORT.md` (audit trail) + `graph.json` (GraphRAG-ready) + opt-in `graph.html` / Obsidian vault / Neo4j cypher / SVG / GraphML / agent wiki / MCP server. | Free | Deterministic |

**The three properties that make it useful for codebase automation:**

1. **Persistent across sessions** — built once, the graph is a durable artifact; agents query it without
   re-reading source.
2. **Honest audit trail** — every edge is `EXTRACTED` (from AST/structure), `INFERRED` (LLM hypothesis),
   or `AMBIGUOUS`. Agents can refuse to trust low-confidence edges (the same honesty model the
   semantic-coverage overlay inherits so it never invents a `proves` edge).
3. **Community detection surfaces what you wouldn't think to ask** — Leiden clusters + god-nodes + bridge
   nodes give an agent a *map* (where are the hubs, where do subsystems connect) before it asks a single
   targeted question.

**Graphify's own caching internals** (relevant because Vox mirrors them): a per-file semantic cache keyed
by content (`check_semantic_cache` / `save_semantic_cache`) so `--update` only re-extracts changed files;
a `.graphify_manifest` for incremental rebuilds; and a cumulative `cost.json` token ledger.

---

## 2. How LLMs / agents use code graphs to search efficiently

The value is **token economics + structure-awareness**. A code graph changes search from "grep + read
files into the context window" into "traverse a pre-computed index and pull only the named nodes."

- **Cheap structural recall.** `vox_graphify_query` does a BFS from seed node IDs and returns
  `{node_id, label, depth, path}` — an agent learns *what connects to what* without reading a single file
  body. Source: `crates/vox-orchestrator-mcp/src/graphify_tools.rs` (`vox_graphify_query`, BFS via
  `vox_graphify_reader::bfs_from_seeds`).
- **Lexical entry points.** `vox_graphify_search` tokenizes a natural-language query, scores nodes by
  token-overlap against labels, and returns the top hits — the agent's *entry seeds* for a subsequent BFS.
- **"How does A reach B?"** `vox_graphify_path` returns the shortest path between two node IDs — the
  call/dependency chain, not a guess.
- **"What changed?"** `vox_graphify_compare` diffs two corpus manifests (node/edge/community deltas) — the
  agent sees drift without diffing the whole repo.
- **Map-first navigation.** God-nodes + communities let an agent orient (which crate is the keystone, which
  subsystems are coupled) before drilling — the exact pattern MEMORY records as `vox-db = #1 blast-radius`
  was *found via graphify*, not by reading 106 crates.

**Why this beats raw retrieval for code:** code's signal is in its *edges* (calls, imports, dependencies),
which embedding/BM25 retrieval flattens away. A graph keeps the edges, so traversal answers reachability
and blast-radius questions that similarity search cannot, at a fraction of the context-window cost.

---

## 3. Best-case / worst-case usage (decision framework)

This is the policy the rest of the program implies but never wrote down. **Use it to decide whether to
query a cached graph, rebuild first, or skip graphify entirely.**

### 3.1 Best case — graphify is the right tool when ALL hold

| Condition | Why |
| --- | --- |
| **Large, stable corpus** (100s–1000s of files) | Amortizes build cost over many queries; the whole point is *not* re-reading source. |
| **Edge/structure questions** ("what calls X", "blast radius of Y", "how does A reach B", "which subsystems couple") | Traversal answers these; grep/embeddings can't cheaply. |
| **Repeated/agentic access** | A persistent map pays off when many agents/turns hit it. |
| **A fresh graph exists** (or rebuild is cheap relative to the question's value) | Querying stale structure is worse than not querying. |
| **Structural (code) content dominates** | AST extraction is free + deterministic; the expensive lane is LLM semantic extraction of docs/media. |

### 3.2 Worst case — avoid or de-prioritize graphify when ANY hold

| Anti-pattern | Do instead |
| --- | --- |
| **Small/one-off corpus** (a handful of files) | Just read the files — build cost dominates. |
| **Single exact-string lookup** ("where is `FOO_CONST` defined") | `Grep`/`rg` — a graph round-trip is pure overhead. |
| **Volatile corpus churning faster than it's queried** | Either accept a known-stale snapshot for orientation only, or skip — you'll spend more rebuilding than querying. |
| **Freshness-critical correctness** against a stale graph | **Never** answer a correctness question from a `git_drift`/`ttl_expired` graph without flagging it; rebuild or fall back to live tools. |
| **Multimodal/doc semantic extraction at scale in Rust** | Keep in the Python/orchestrator lane — see §4; it's the expensive, non-deterministic, token-billed path. |
| **Pushing a 100MB+ graph into Turso** | Tier D (disk) only; Turso holds the lexical projection, not the full graph (per the tier model). |

### 3.3 The cost asymmetry to remember

Structural extraction is **free and deterministic**; LLM semantic extraction is the **only token-billed,
non-deterministic** phase, tracked today in an ad-hoc `cost.json` (flagged for `vox-telemetry`
unification). Best/worst-case reasoning is mostly "am I paying the semantic lane, and is the graph fresh?"

---

## 4. Rust-native status — the hybrid line

**Decision (firm, across all prior docs): hybrid, not a full rewrite.** Rust owns *structural + query +
freshness*; the LLM semantic/multimodal lane stays in the Python/orchestrator layer ("full Rust rewrite is
NOT cost-effective").

| Capability | Lane | Status |
| --- | --- | --- |
| Read `graph.json` (NetworkX JSON, `links` or `edges`) | **Rust** `vox-graphify-reader::GraphifyReader::from_value` | **BUILT** |
| BFS / shortest-path / god-nodes / community members | **Rust** `bfs.rs`, `lib.rs` | **BUILT** |
| Manifest diff (node/edge/community deltas) | **Rust** `compare.rs::diff_manifests` | **BUILT** |
| AST extraction (Rust via `syn`; TS/JS via tree-sitter) | **Rust** `ast.rs::extract_ast` | **BUILT** |
| Leiden clustering | **Rust** `cluster.rs` (`leiden-rs` FFI) | **BUILT** |
| File-level incremental cache (BLAKE3 content hash) | **Rust** `cache.rs::CacheManager` | **BUILT** |
| Rebuild orchestration (`vox graphify rebuild`) | **Rust** `rebuild.rs::rebuild_graph` | **BUILT (per-corpus AST graph)** |
| Test-targeting overlay / LCOV reachability | **Rust** `overlay.rs`, `reachability.rs` | **PLANNED** (built-but-not-integrated; coverage-overlays plan) |
| **Canonical full-corpus build incl. doc/media semantic nodes** | **Python** `python -m graphify` via `scripts/graphify-refresh.vox` | **STILL PYTHON** (python-free roadmap = research only) |
| HTML / Obsidian / Neo4j / SVG exporters | **Python** (upstream) | **STILL PYTHON** (not Vox-integrated) |

**The gap:** `vox graphify rebuild` builds the *structural* AST graph natively, but the registry's refresh
path for a full corpus still invokes Python. The 4-phase python-free roadmap (`leiden-rs` + `syn`/
`tree-sitter` + blake3 cache + removal of 19 `scripts/coverage-graph/*.py`) is **roadmap only**.

---

## 5. Vertical-integration audit — the cache lifecycle

The user's core ask: cache prior searches, know when they expire, automate rerun, decide whether to,
prompt through the GUI, visualize, and decide which graphs to keep vs discard with learnable history.
Here is each capability, with status and file anchors.

### 5.1 Where graphs live (registry + tiers) — **BUILT**

- SSOT registry `contracts/retrieval/graphify-corpora.v1.yaml`: 4 corpora — `repo-code-graph` (default),
  `vox-gui-surface`, `config-audit`, and the **virtual** `graphify-search-log`.
- Per-corpus fields (`vox-config/src/graphify.rs`): `id`, `title`, `scope_path`, `graph_path`,
  `manifest_path`, `extraction_mode`, `default_for_intents`, `is_virtual`.
- Canonical disk path: `.vox/cache/graphify/<corpus_id>/` (`vox-config/src/paths.rs`). **Tier D** = full
  graph on disk; **Tier A** = lexical projection in Turso `knowledge_nodes`.
- ⚠️ **Hygiene blocker C1** (open): ~108 stale `COVERAGE_BEHAVIORS_*` files remain tracked under
  `graphify-out/` despite the `.gitignore` entry — the only "discard" decision currently pending, unexecuted.

### 5.2 Caching prior **searches** + expiry detection — **BUILT (model) / PARTIAL (wiring)**

- **Search persistence:** `vox_graphify_search` with `persist: true` (serde default) upserts each hit into
  Turso `knowledge_nodes`, ID `graphify:{corpus_id}:search:{query_slug}:{node_id}`, metadata
  `{corpus_id, query, searched_at (RFC3339), git_sha, source:"graphify_search_hit"}`. `query_slug` =
  32-char normalized prefix + FNV-64 suffix (collision-safe, std-only). DB failure is non-fatal
  (`tracing::warn!`). This is the **agent-recall / learnable-history seed**.
- **Expiry model** (`assess_corpus_status`, `vox-config/src/graphify.rs`):
  - **Stale (blocks freshness):** `graph_missing`, `graph_corrupt`, `git_drift` (manifest `git_sha` ≠ HEAD),
    `ttl_expired` (`now - built_at > ttl_days`), `lexical_lag` (`lexical_ingest_sha256` ≠ `graph_json_sha256`).
  - **Warnings (non-blocking):** `manifest_missing`, `node_count_drift`, `edge_count_drift`, `virtual_corpus`.
  - TTL default 30d; override `VOX_GRAPHIFY_TTL_DAYS`. Recalled search hits carry `git_sha` so consumers
    **must** compare against HEAD to detect a stale recall.
- ⚠️ Per HANDOFF-STATE, **`lexical_lag` wiring + `VOX_GRAPHIFY_TTL_DAYS` env are in the run-lifecycle plan,
  not yet executed** — the *model* is richer than the *wired* behavior.

### 5.3 Automated rerun — **WEAKEST AREA / mostly GAP**

- Today "rerun" = `vox graphify status --strict` (exit 1 on stale) + `scripts/graphify-refresh.vox`, which
  **prints rebuild instructions** and runs `vox graphify ingest` for `lexical_lag` corpora. The actual
  *rebuild* of a full corpus still shells to Python.
- CI gate is `continue-on-error: true` (a **warning**, not blocking) because Tier-D graphs aren't committed,
  so CI checkouts always show `graph_missing`.
- **No autonomous trigger** rebuilds on git-drift; **no scheduler**; **no "decide whether to rerun" cost
  policy.** This is the single biggest delta to "automate their rerunning, decide if we want to."

### 5.4 Prompting the user through the GUI — **GAP (no design)**

- Freshness is reachable only via CLI `vox graphify status` and MCP `vox_graphify_status`. **There is no
  `vox-gui` surface** — no corpus-health panel, no "your code graph is 12 days stale, rebuild?" prompt,
  no accept/decline of a fresh build. Every prior plan explicitly defers GUI to a separate sub-project.
  This is **greenfield** relative to the user's "prompt the user through to the GUI."

### 5.5 Visualization — **GAP (upstream-only, not integrated)**

- Graphify can emit `graph.html` / Obsidian / Neo4j / SVG, but those are **upstream Python exporters, not
  wired into Vox**. The only Vox-side "viz" lever is `VOX_GRAPHIFY_VIZ_NODE_LIMIT` capping BFS result size.
  No embedded graph explorer in `vox-gui`.

### 5.6 Retention / discard with learnable history — **GAP (design)**

- **Retention:** rebuild **overwrites `graph.json` in place**. No snapshot history, no versioning, no GC
  policy. Turso `knowledge_nodes` is append/upsert with **no TTL purge** — history accumulates indefinitely.
- **Learning:** nothing scores a corpus's *value* or *usage*, so "which graphs to maintain vs discard" has
  no signal to learn from. The raw materials exist (search-log = usage history; `diff_manifests` +
  community Jaccard drift = change signal) but **no loop consumes them.**

### 5.7 Capability matrix (summary)

| Capability (user's ask) | Status | Anchor / gap |
| --- | --- | --- |
| Cache prior **searches** | **BUILT** | `graphify_tools.rs` persist path → Turso |
| Know when they **expire** | **BUILT (model), PARTIAL (wiring)** | `assess_corpus_status`; lexical-lag/TTL-env unwired |
| **Automate rerunning** | **GAP** | refresh script only prints; rebuild still Python; no trigger |
| **Decide if we want to** rerun (cost policy) | **GAP** | no cost/value gate (§3 is the missing policy) |
| **Prompt user through GUI** | **GAP** | no `vox-gui` surface at all |
| **Visualize** graphs | **GAP** | upstream-only exporters; not integrated |
| Decide **maintain vs discard** | **GAP** | overwrite-in-place; no GC; C1 stale files tracked |
| **History we can learn from** | **PARTIAL** | search-log + manifest diff exist; no learning loop |
| Rust-native | **BUILT (query), GAP (construction)** | reader native; full build still Python |

---

## 6. Target lifecycle state machine (the convergence picture)

The vertical integration the user is describing is a single loop the pieces should snap into:

```
                 ┌────────────────────────────────────────────────┐
                 │                 corpus registry                 │
                 │        (which graphs are authoritative)         │
                 └───────────────────┬────────────────────────────┘
                                     │
              assess_corpus_status   ▼
   ┌─────────── FRESH ───────────────┴───────────── STALE ───────────┐
   │ agents query freely             (git_drift│ttl│lexical_lag│      │
   │ (MCP query/path/search)          missing│corrupt)               │
   │                                          │                      │
   │                         ┌────── cost/value gate (§3) ───────┐   │
   │                         │  worth rebuilding now?            │   │
   │                         ▼                                   ▼   │
   │                  AUTO-REBUILD                       SURFACE/PROMPT
   │              (native, no Python)                   (GUI: "stale, rebuild?")
   │                         │                                   │   │
   │                         ▼                                   │   │
   │                 snapshot + retain                           │   │
   │              (keep N, GC by usage score) ◀──────────────────┘   │
   └───────────────────────────┬────────────────────────────────────┘
                               ▼
                  search-log + manifest-diff history
                  (the signal a retention/value learner consumes)
```

Of this loop, the **left arm (assess → query)** is built. The **cost/value gate, auto-rebuild trigger,
GUI prompt, snapshot/GC, and the learner** are the gaps from §5.

---

## 7. Learnable retention — which graphs to keep vs discard

A concrete, non-LLM policy (deterministic, cheap) the program could adopt — **proposed, not built**:

- **Per-corpus value score** from signals we already emit:
  - *usage* = count of `graphify_search_hit` rows referencing the corpus over a window (search-log),
  - *recency* = time since last query,
  - *churn* = manifest-diff magnitude + community-Jaccard drift since last rebuild,
  - *cost* = tokens/seconds last rebuild took (from the build manifest / `cost.json`).
- **Maintain** corpora with high usage and tolerable churn; **let TTL-expire** (don't auto-rebuild) corpora
  with zero usage in the window; **discard** (GC the snapshot) corpora unused past a retention horizon.
- **Snapshot retention:** keep the last *N* `graph.json` snapshots per corpus (cheap, Tier D) so
  `diff_manifests` can answer "what changed since last week" — today there's only one snapshot, so history
  is one step deep.
- **The learning loop** is just: log every query + rebuild outcome, recompute value scores on a schedule,
  and feed the keep/expire/discard decision — exactly the history the search-log was designed to hold but
  nothing yet reads back.

---

## 8. Prioritized recommendations (mapping to the user's goals)

1. **Wire the freshness model that's already written** (run-lifecycle plan): `lexical_lag` into
   `assess_corpus_status`, `VOX_GRAPHIFY_TTL_DAYS` env. Cheapest, highest-correctness win. *(expire)*
2. **Make rebuild fully native** (python-free roadmap): so auto-rerun doesn't reintroduce the Windows/
   Python friction the program is deleting. Prereq for trustworthy automation. *(Rust-native + rerun)*
3. **Add an auto-rerun trigger with a cost/value gate** using §3 as the policy: rebuild on `git_drift`
   when the corpus has recent usage; otherwise expire quietly. *(automate + decide-if-we-want-to)*
4. **Design the `vox-gui` corpus-health surface**: staleness panel + "rebuild?" prompt + accept/decline +
   an embedded graph explorer (the genuinely greenfield ask). *(GUI prompt + visualize)*
5. **Add snapshot retention + a deterministic value-score GC** (§7), reading the search-log/manifest-diff
   history. *(maintain-vs-discard + learnable history)*
6. **Execute hygiene blocker C1** (untrack stale `graphify-out/COVERAGE_BEHAVIORS_*`) and **C2** (path
   migration) so retention starts from a clean tree.

Items 1–2 are *execute existing plans*; 3–5 are *new design* (the user's real frontier); 6 is *cleanup*.

---

## 9. Anchors

- Reader crate: `crates/vox-graphify-reader/{lib,bfs,ast,cache,cluster,rebuild,compare,overlay,reachability}.rs`
- Config/freshness: `crates/vox-config/src/graphify.rs`, `crates/vox-config/src/paths.rs`
- CLI: `crates/vox-cli/src/commands/graphify/mod.rs` (`status` · `ingest` · `rebuild`)
- MCP tools: `crates/vox-orchestrator-mcp/src/graphify_tools.rs` (`status`/`search`/`query`/`path`/`compare`), dispatched in `dispatch.rs`
- Registry SSOT: `contracts/retrieval/graphify-corpora.v1.yaml`
- Automation: `scripts/graphify-refresh.vox`, `scripts/graphify-coverage.vox`
- Upstream skill (pipeline reference): `~/.claude/skills/graphify/SKILL.md`
