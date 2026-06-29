---
category: "Architecture SSOTs"
title: "Vox Search — Unified Code-Intelligence Service (Design)"
date: 2026-06-26
status: design
---

# Vox Search — Unified Code-Intelligence Service (Design)

> **Umbrella SSOT.** This document absorbs four sibling designs into one service.
> It supersedes the *branding and tool-surface* of the standalone "graphify"
> framing while preserving every engine decision in the sources:
>
> - `2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint-design.md` — structural core (boundary / composition / registry edges, coverage).
> - `2026-06-26-graphify-dataflow-semantic-overlay-design.md` — data-flow / def-use layer + semantic overlay.
> - `2026-06-26-graphify-voxsearch-fusion-design.md` — lexical + vector fusion (`vox_discover`, graph-RAG).
> - `2026-06-26-graphify-agent-tool-surface-design.md` — auto-availability, agent steering, GUI consumption.
> - `docs/agents/cli-gui-governance-audit.md` + `docs/agents/gui-ia-blueprint.md` — CLI→GUI governance gap + ratified IA placement.
>
> Where this doc and a source disagree on a *name* (the brand/CLI/tool prefix),
> **this doc wins**. Where they disagree on *engine behavior*, the source detail
> is authoritative and is preserved verbatim in intent below.

---

## 0. Thesis — one service, N indexes, one honesty contract

Vox has been growing two parallel retrieval systems and a pile of structural
analyses that overlap but never fuse: a deterministic AST/JSX/crate-dep
knowledge graph ("graphify"), a hybrid lexical+vector search engine
("Vox Search"), a planned data-flow/def-use layer, and a planned semantic
overlay. Each was specced separately; each reserved its own CLI verbs and MCP
tool prefix. That fragmentation is the problem this SSOT ends.

**Decision: there is one code-intelligence service — Vox Search.** It owns
**N indexes/layers over the same codebase**, queried through one CLI namespace
(`vox search …`), one MCP tool prefix (`vox_search_*`), and one GUI surface. The
layers are:

| # | Layer | Determinism | Answers | Mutates structural graph? |
|---|---|---|---|---|
| 1 | **Structural index** (the former graphify graph) | deterministic | "who calls / composes / dispatches-to / declares whom" | n/a — *is* the substrate |
| 2 | **Lexical index** (BM25 / FTS5) | deterministic | "where does this term/phrase appear" | never |
| 3 | **Vector index** (embeddings / Qdrant) | non-deterministic (model) | "what is *about* X / semantically near X" | never |
| 4 | **Data-flow / def-use layer** | deterministic (intra-procedural, drop-on-ambiguity) | "is this written value ever *used to decide* anything" — dead signals | never (adds deterministic edge kinds to the same graph family) |
| 5 | **Semantic overlay** | non-deterministic (embeddings + LLM-labeled relations) | "what is *related to* X / what is the auth flow" | **never** — physically separate `semantic-overlay.json` |

One service, two consumers — **agents** (via the MCP dispatcher) and the
**Vox Axis GUI** (via `invoke_mcp_tool` against the *same* dispatcher). No
split-brain: the GUI never re-implements an index in a bespoke Tauri command.

**The single non-negotiable invariant** (inherited identically from every
source): the **deterministic structural core is never mutated by any overlay**.
Layers 1, 2, 4 are deterministic and honest by construction (drop-on-ambiguity,
confidence-labeled, under-report-never-fabricate). Layers 3 and 5 are
query-time, **provenance-labeled** (`layer: "structural"` | `"overlay"`,
`provenance: "structural"` | `"overlay"`), and live in separate artifacts. An
agent (or a CI gate, or the GUI) can always strip the overlay and recover the
pure, byte-reproducible structural truth.

---

## 1. The graphify → Vox Search absorption

The "graphify" brand, the `vox graphify` CLI, and the `vox_graphify_*` MCP
tools are **RETIRED** and rolled into Vox Search. **The engine logic is rehomed,
not rewritten** — `vox-graphify-reader` (built this session: `bfs.rs`,
`cluster.rs`, `reachability.rs`, `coverage.rs`, `compare.rs`, `lens.rs`,
`overlay.rs`, plus the planned `dataflow.rs`, `resolve.rs`, `registry.rs`)
remains intact as the **structural-index library** inside Vox Search. Whether
the internal crate keeps the name `vox-graphify-reader` is a packaging detail
left to the executing plan; the *external* surface is uniformly `vox search` /
`vox_search_*`.

### 1.1 Rename map (SSOT)

**CLI.** `vox graphify <verb>` → `vox search <verb>`. Verbs preserved 1:1:

| Old | New |
|---|---|
| `vox graphify status` | `vox search status` |
| `vox graphify rebuild` | `vox search rebuild` (structural-index rebuild) |
| `vox graphify ingest` | `vox search ingest` (project structural nodes into `knowledge_nodes`) |
| `vox graphify refresh` | `vox search refresh` |
| `vox graphify gc` | `vox search gc` |
| `vox graphify index` | `vox search index` |
| `vox graphify crate-map` | `vox search crate-map` |
| `vox graphify coverage` *(planned)* | `vox search coverage` |
| `vox graphify dataflow` *(planned)* | `vox search dataflow` |
| `vox graphify dead-signals` *(planned)* | `vox search dead-signals` |
| `vox graphify semantic-related` *(planned)* | `vox search semantic-related` |
| — | `vox search discover` *(new: the fused graph-RAG verb)* |

`vox search` is a **NEW top-level command group created by RENAMING the
`Graphify` clap variant** — there is no prior `vox search` CLI command to merge
into. The structural verbs above become subcommands of this renamed group; the
lexical/vector query path is reached through the same group's verbs. A
one-release **alias** keeps `vox graphify …` resolving (deprecation warning) so
scripts and the freshness-panel copy-string don't break.

**MCP tools.** `vox_graphify_*` → `vox_search_*`:

| Old | New | Layer |
|---|---|---|
| `vox_graphify_status` | `vox_search_status` | all (freshness) |
| `vox_graphify_search` | `vox_search_structural` | structural (lexical-over-graph) |
| `vox_graphify_query` | `vox_search_neighbors` | structural (BFS) |
| `vox_graphify_path` | `vox_search_path` | structural (shortest path) |
| `vox_graphify_compare` | `vox_search_compare` | structural (corpus delta) |
| *(new — fusion)* | **`vox_discover`** | fusion (search-seed → graph-expand) |
| *(new — data-flow)* | **`vox_search_dataflow`** | data-flow |
| *(new — data-flow)* | **`vox_search_dead_signals`** | data-flow |
| *(new — semantic)* | **`vox_search_semantic_related`** | semantic overlay |

`vox_discover` keeps its working name (it is a verb, not a layer-suffix) and is
the headline fused entry point. The existing user-facing search MCP tools
(`vox_memory_search`, `vox_knowledge_query`, `vox_research_run`,
`vox_search_query` the GUI Tauri command) are unchanged — they already live in
the `vox_search`/retrieval family; the structural tools join them.

**Brand / artifacts.** "Graphify" as a product name is retired in
user-facing copy (CLI help, GUI labels, tool descriptions, docs prose). On-disk
artifacts keep stable paths to avoid a migration: `.vox/cache/graphify/<corpus>/graph.json`
and the contract `contracts/retrieval/graphify-corpora.v1.yaml` remain (renaming
them is a no-value cache-busting churn; they are internal). The corpus registry
is Vox Search's **structural-index corpus registry**. The `graphify-search-log`
virtual corpus becomes the **structural-search recall log**.

### 1.2 What does NOT change

- The deterministic extraction (syn for Rust, tree-sitter for TS/TSX/JS/PY),
  the Leiden communities, the manifest/freshness model in `vox-config`, the
  read-only-to-overlay invariant.
- The `ExtractedNode{ id, label, kind }` / `ExtractedEdge{ source, target,
  confidence }` schema and its `confidence ∈ {resolved, heuristic, declared}`.
- The honesty firewall and provenance labels.
- The on-disk `graph.json` shape and cache layout.

The absorption is a **packaging + naming unification**, not an engine rewrite.

---

## 2. The unified architecture (layer responsibilities + honesty boundary)

### 2.1 Layer 1 — Structural index (the core)

The deterministic substrate. From the general-enhancement design: AST call
edges + **string-dispatch boundary edges** (`invoke('cmd')`→`#[tauri::command]`,
`callTool('tool')`→MCP fn, clap subcommand→impl, `vox://` stream→producer),
**composition edges** (JSX `<Component/>`, hooks, ES imports — collapses the
51%-island problem), and **registry-ingest nodes** (surface registry,
`get_command_catalog`, the clap tree, the MCP tool registry, joined to impls).
Every non-AST edge is `confidence`-labeled `declared`/`heuristic` so no consumer
mistakes a name-match for a proven call.

New node kinds: `command`, `tool`, `surface`, `registry-entry`, plus (§2.4) the
clap `cli:` nodes. Output is the existing `graph.json` + manifest.

**Coverage** (`coverage.rs`, the `vox search coverage` verb): for a chosen
registry node-set, classify each entry as `Surfaced` / `OrphanBackend` /
`DeadEnd` / `CliOnly`, producing the wiring map, command-coverage scorecard, and
orphan-nav report. This is the "registry-vs-impl" capability — deterministic,
trustworthy, the engine's best skill.

**Best at / worst at** (recorded as constraint, from the general-enhancement
design): best at structural recall, tracing string-dispatch, registry-vs-impl
coverage, blast-radius. **Never** asked: semantic intent / "why" / UX judgment
(route to the LLM/audit), dynamic-dispatch (dropped, never fabricated), non-code
(CSS/tokens), fuzzy "related-to" (that is Layers 3/5).

### 2.2 Layers 2–3 — Lexical + Vector indexes (retrieval)

Vox Search's existing hybrid stack (`SearchCorpus`, `RetrievalMode ∈ {FullText,
Vector, Hybrid}`, real BM25 with status-boost + temporal decay, FTS5⊕embedding
reciprocal-rank fusion, `EmbeddingService` + the `embeddings` table + optional
Qdrant sidecar). **Embeddings are owned by Vox Search** — there is no parallel
embedding stack for "graphify". The structural index becomes a **first-class
corpus** for these lanes via `SearchCorpus::GraphifyNodes` (a node's
`{label, kind, module path, doc-comment, surrounding identifiers}` as the
retrieval unit), computed through the *same* `llm_embed` pipeline into the same
vector store under a distinct collection.

### 2.3 Layer 4 — Data-flow / def-use (deterministic, same graph family)

From the data-flow design, preserved exactly. A new module `dataflow.rs` runs
**intra-procedural def-use** + a fixed set of **local pattern detectors**,
reusing the `syn` visitor (Rust) and tree-sitter walk (TS). It runs **per
function** (no whole-program fixpoint) and **drops on ambiguity** (a missed
defect is acceptable; a false "dead signal" is not).

New deterministic node kinds (`field:<Struct>::<f>`, `binding:<fn>::<n>@<k>`) and
edge kinds (`def-write`; `use-read` sub-typed by `read_kind ∈ {control, consume,
store}` — the `control` vs `store` distinction is the crux). Detectors:
`ignored_result`, `write_only_field`, **`accumulator_never_gates`** (the
canonical swallowed-error-accumulator shape — the one that flags the
frontend-emit `reactive_view_emit_failures` bug the call graph is blind to).
**Load-bearing caveat (from the data-flow sibling):** the
`accumulator_never_gates` / frontend-emit catch holds **only when the
accumulation write and the store-read are in the SAME function** (the
frontend-emit shape). A **cross-function accumulator** — where the value is
accumulated in one function and read in another — is an **accepted
intra-procedural miss** (under-reported, never falsely flagged), consistent with
the general callee-return-miss below.
Output: `DeadSignalReport` via `compute_dead_signals(graph)`, surfaced by
`vox search dead-signals` + a non-blocking CI advisory (promotable to blocking
once false-positive rate is measured).

**Honesty:** this is deterministic and adds to the structural graph family with
the same confidence-labels — it is *not* an overlay. It under-reports
(intra-procedural miss: a write in a callee that flows back via return is
dropped, never falsely flagged).

### 2.4 Layer 5 — Semantic overlay (separate artifact)

From the data-flow design + the fusion design, preserved. Embeddings + optional
LLM-labeled relations, stored in a **physically separate** `semantic-overlay.json`
that references structural node ids but is **never merged into `graph.json`**.
It carries the structural `graph_json_sha256` it was built against; a mismatch
marks it **stale** and queries warn (mirroring the `vox-config` / `lexical_lag`
freshness model). Every result is stamped `layer: "semantic"`, `source`,
`similarity`/`confidence`, `stale: bool`. The capability:
*"find things related to X"* (embedding kNN over the node corpus) and *"what's
the auth flow"* (semantic seeds + 1–2-hop **structural** expansion — the seeds
are guesses, the connective tissue is ground truth).

**The semantic layer ships in two success bars:**

- **P3a — embedding-kNN `vox_search_semantic_related`.** Rides the
  `GraphifyNodes` embedding corpus (no new embedding stack), answers "related to
  X" via kNN, fully overlay-labeled and staleness-stamped. **Shippable** on its
  own — this is the deliverable bar for the semantic layer.
- **P3b — LLM-typed-relation overlay.** Adds LLM-labeled *typed* relations
  (the "what's the auth flow" connective semantics) on top of P3a. A **separate,
  deferrable tail** — not required for the layer to ship.

### 2.5 The honesty boundary (one statement for all five layers)

1. `graph.json` / `knowledge_edges` are **read-only** to Layers 2, 3, 5. No edge
   is created/weighted/deleted by search, embeddings, or re-ranking.
2. Overlay scores (`fused_score` and its `components`) are computed **at query
   time** and returned in the response — never persisted as edge weights or node
   fields. The only persistence is the structural-search recall *log* rows.
3. **Every result is provenance-labeled.** `structural` = deterministic from
   AST/crate extraction; `overlay` = surfaced/re-ranked by a semantic/lexical
   signal. A consumer can strip the overlay and recover the deterministic graph.
4. **Determinism preserved for structural queries.** `mode:"structure_only"`
   (and every Layer-1/4 query) is byte-identical across runs. The semantic lane
   is opt-in; weights default structural-dominant.

This is Vox Search's core promise: *bad/unreal structure never shows up, because
the structural layers are computed not guessed — and you always know which layer
a result came from.*

### 2.6 The fused entry point — `vox_discover` (graph-RAG)

From the fusion design, preserved. One tool composes Search→Graph (resolve the
fuzzy query to seed node ids via direct / path-symbol / label resolvers) →
Graph→Search (BFS expand to radius *r*, optionally community-scoped; composite
re-rank `fused = w_search·search + w_prox·proximity_decay(hops) +
w_cent·centrality − w_dead·dead_end_penalty`). **Layer order honored:**
**lexical-seed fusion ships first** (BM25 + label overlap — proves the plumbing
and the honesty labels), then the **embedding lane behind a flag**
(`VOX_SEARCH_GRAPH_*` weights default-off). The KG-score-0.0 bug is fixed
independently *and* masked by the composite ranker. `mode:"structure_only"` skips
the search lane entirely (deterministic).

---

## 3. The unified agent tool surface (`vox_search_*`)

One tool layer (the MCP dispatch), two consumers (agents + GUI). All tools are
`tier: core`, unconditional in the dispatcher, and every response carries a
`layer` field. Schemas land in `vox-orchestrator-mcp/src/input_schemas.rs`;
handlers in `graphify_tools.rs` (or sibling `*_tools.rs`); one dispatch arm each.

### 3.1 The surface

| Tool | Layer | Input (sketch) | Output | Provenance |
|---|---|---|---|---|
| `vox_search_status` | all | `{ corpus?, summary? }` | freshness + (optional) code-map summary | structural |
| `vox_search_structural` | structural | `{ corpus, query, intent? }` | ranked node ids (lexical-over-graph) | structural |
| `vox_search_neighbors` | structural | `{ corpus, node_ids, max_depth ≤ 5 }` | BFS frontier | structural |
| `vox_search_path` | structural | `{ corpus, from, to }` | shortest path (`reachable:false` shown honestly) | structural |
| `vox_search_compare` | structural | `{ corpus_a, corpus_b }` | node/edge/community delta | structural |
| **`vox_discover`** | fusion | `{ query, corpus?, radius=1, community_scope?, mode:"auto"\|"search_seed"\|"structure_only", limit=30 }` | `{ seeds[], results[{node_id, fused_score, components, hops, community, reachability_class, provenance}] }` | **mixed** (each labeled) |
| **`vox_search_dataflow`** | data-flow | `{ corpus, node_id }` (fn/field/binding) | def-use edges (`def-write` / `use-read` w/ `read_kind`) | structural |
| **`vox_search_dead_signals`** | data-flow | `{ corpus, detector?, min_confidence? }` | `DeadSignalReport.findings[]` | structural |
| **`vox_search_semantic_related`** | semantic | `{ corpus, query\|node_id, k?, min_similarity? }` | ranked node ids + `similarity` + `source` + `stale` | **overlay** |

(Reserved for later layer work, names fixed so layers don't collide:
`vox_search_callers`, `vox_search_callers_ignoring_result`, `vox_search_defuse`,
`vox_search_explain`.)

### 3.2 Auto-availability — every Vox-hosted agent gets Vox Search, no setup

From the agent-tool-surface design, with names updated. Two cases:

- **In-process agents** (orchestrator-hosted, GUI chat, VoxMens, deployed/
  headless) already dispatch through the same `handle_tool_call → match name`
  path; the `vox_search_*` arms are unconditional → **they already have every
  tool**. Add a CI assertion that the full set is present in the default tier so
  a tiering change can't silently drop them.
- **External harnesses** (Claude Code, Gemini/Antigravity, third-party MCP):
  1. **Ship a repo-root `.mcp.json`** registering `{ "mcpServers": { "vox": {
     "command": "vox", "args": ["mcp"] } } }`, generated from the catalog SSOT by
     `vox ci mcp-client-config --write` so transport/binary are SSOT-derived.
     Any agent run in a Vox checkout gets all Vox MCP tools, zero setup.
  2. **`vox mcp install <harness>`** writes the equivalent entry into each
     harness's own config (one generator, multiple emitters). Global writes
     (`vox mcp install --all`) are **explicit opt-in** — Vox never mutates a
     user's global harness config without consent (design fork F1, recommended
     resolution = ship `.mcp.json` always, global install opt-in).

### 3.3 Agent steering — graph-first discovery (always-on)

From the agent-tool-surface design, **resolved to always-on** per the baked
decisions:

- **Tool descriptions that route** ("PREFER THIS over grep/Glob for 'where is X'")
  — comparative/imperative copy, kept in the catalog `description`/`agent_hint`
  SSOT (descriptions are GENERATED; never hand-edit the canonical YAML).
- **A pinned `graph-first-discovery` skill** shipped under `assets/skills/`
  (the auto-hydrated skill root — shipping it under `crates/vox-skills/skills/`
  would drop it where it is **not** loaded), **pinned-by-default** so its body (the
  `search→neighbors→path` call-order playbook) is injected, **size-gated** to
  keep the cache prefix stable. Its frontmatter description is itself a steering
  one-liner so even unloaded it nudges graph-first via the Tier-1 catalog.
- **Always-on code-map system-prompt injection.** A compact, size-capped
  `## Repository code map (Vox Search)` block (top god-nodes, community labels,
  node/edge counts + freshness line, a "drill in with `vox_search_*`" pointer)
  injected in `build_system_prompt_with_skill` **immediately after the MEMORY.md
  block**, sourced from the reader over the `repo-code-graph` corpus, capped
  (~1–2 KB). Gives every agent — including prompt-only models — a baseline
  mental model and primes the tools. (Design fork F2 resolved to **always-on**.)
- **Tiered freshness → self-healing.** `assess_corpus_status` pre-check on the
  read tools: fresh → proceed; stale-cheap (`lexical_lag`, `ttl_expired` small
  corpus) → **regenerate before answering**, tag `regenerated_at`; stale-expensive
  (`git_drift` full repo) → **answer on last build**, stamp `stale:true` +
  reasons + rebuild command, **enqueue** a debounced single-flight background
  rebuild. Event-driven invalidation hooks the post-commit/HEAD-change signal.
  (Design fork F3 resolved to **tiered block-cheap / answer-stale-expensive**;
  cheap/expensive corpus-size cutoff is a ratification knob.)

### 3.4 Uniform "add a layer-tool" recipe (mechanical, 5 steps)

Preserved from the agent-tool-surface design (names updated): (1) add a catalog
`mcp:` block in `contracts/operations/catalog.v1.yaml` (`name: vox_search_<x>`,
`http_read_role_eligible: true`, `tier: core`, `product_lane: platform`,
`intent_tags: [retrieval, graph, <layer>]`, optional `agent_hint`); (2)
`vox ci operations-sync --target mcp --write`; (3) inline JSON schema arm in
`input_schemas.rs`; (4) handler + one dispatch arm; (5) optional GUI pane calling
`invokeMcpTool('vox_search_<x>', …)`. Because availability is unconditional and
the registry is generated from one SSOT, a new layer-tool is automatically
available to every agent and harness on next build.

---

## 4. The GUI surface — Vox Search / code-intelligence in Vox Axis

**Single tool layer, two consumers.** The GUI calls the **same MCP tools** via
the proven `voxTransport.invokeMcpTool(tool, args)` (`transport.ts`), used today
for `vox_pending_approvals` etc. It must **not** re-implement graph logic in a
Tauri command — the existing `getGraphifyStatus()` split-brain (`crates/vox-gui/
src/commands/graphify.rs`) is retired in favor of `invokeMcpTool('vox_search_status')`.

### 4.1 Panes (each backed 1:1 by an MCP tool)

- **Code map / overview** — communities + god nodes + counts + freshness banner
  (`vox_search_status`, reusing the §3.3 summary). Default landing pane.
- **Search** — query box → ranked hits (`vox_search_structural` / `vox_discover`),
  each hit clickable to seed the neighborhood pane; surfaces `knowledge_id` for
  pin-to-memory.
- **Neighborhood** — node + depth → BFS node-link view (`vox_search_neighbors`).
- **Path** — from/to pickers → structural route (`vox_search_path`), `reachable:
  false` shown honestly.
- **Coverage / communities** — community list + orphan/dead-end/zero-edge counts
  (`coverage`); the honesty surface (shows what the graph does *not* cover).
- **Dead signals** — `vox_search_dead_signals` findings, confidence-labeled.
- **Related** — `vox_search_semantic_related`, every row stamped `overlay` +
  `stale` so the human knows it is a guess.
- **Compare** (secondary) — corpus-A vs corpus-B deltas.

### 4.2 Placement (per the ratified IA)

The ratified blueprint (`docs/agents/gui-ia-blueprint.md`, RATIFIED 2026-06-26)
retires the free-text `search` parent into **Knowledge** (de-Latinized
`scientia`) and merges claims+knowledge into one Knowledge surface. **Vox Search
is code-intelligence → it lives under Knowledge** as a first-class child,
co-located with the retired free-text search it supersedes. Fix the existing
`graphify` orphan (present in `surfaceComponents.tsx` `case 'graphify'`, absent
from `navigation.ts` + `surfaceRegistry.generated.ts`): re-key it to the Vox
Search surface, add it to `navigation.ts` under Knowledge, regenerate
`surfaceRegistry.generated.ts` via `vox ci gui-surface-registry --write`, and
promote `GraphifyStatusPanel` into a tabbed `VoxSearchPanel` hosting the §4.1
panes. New layer-tools appear in the GUI for free by adding a pane — no backend
duplication.

---

## 5. CLI governance — clap-tree `cli:` node ingestion for unified coverage

From the CLI→GUI governance audit: **71% of 549 leaf CLI commands across 74
groups are ungoverned** (no GUI path); the audit corrects to ~29.1% governed as
an *upper bound* (governed groups like `mens`/`populi`/`oratio` are shallow). Two
moves, both inside Vox Search's coverage capability:

### 5.1 Ingest the clap tree as `cli:` nodes

The structural graph today has `cmd:`/`tool:`/`surface:` nodes from the Rust/TSX
walk but **not** the clap derive enums. Add a **registry adapter** that ingests
`vox commands --format json --include-nested` (the compile-time `VoxCliRoot::
command()` reflection, gated-corrected for `mens`/`populi`/`oratio` from the
`vox-ml-cli` enums) as `cli:<group>:<command>` nodes, joined to their `impl` fn
nodes where resolvable. Then `vox search coverage` computes a **unified** matrix
over (clap `cli:` ∪ MCP `tool:` ∪ `cmd:` ∪ surfaces), classifying each leaf as
`Surfaced` / `OrphanBackend` / `DeadEnd` / **`CliOnly`** — with **honest
"not-in-GUI"** for genuinely CLI-only commands (`completions`, `lsp`, `grammar`,
`wasm`, `play`, `repl`/`shell`/`term`, `visus`, and the dangerous-admin set kept
behind confirms). This is the missing CLI-side enumeration the audit flagged.

### 5.2 The governance surfaces to add (under the ratified nav)

Per the audit's recommendations (no new top-level group required):

- **`Develop > CI`** for `ci` (157 cmds) — read-only gate dashboard + run actions
  behind confirm. Biggest single win.
- **`Knowledge > Database`** for `db` (77 cmds) — read-only query/table browser;
  destructive admin behind confirm.
- **Build-spine actions** (`build`/`check`/`compile`/`dev`/`run`/`test`/`fmt`/
  `fabrica`/`emit`/`new`/`init`/`generate`/`component`/`bundle`/`snippet`) folded
  into **Develop > Workspace / Console** (Repository already proves the
  `execute_command` pattern).
- **Typed secret/auth wrappers** — `secrets`, `auth`, `login`/`logout`, `config`
  writes get `#[tauri::command]` wrappers with structured args (never shelled
  through `execute_command`, so credentials never transit a shell string) under
  **System > Settings** (Account / Secrets / Telemetry / Audits sub-panels).

CI + Database alone convert 234 of 389 ungoverned commands (60%) into reachable
surfaces. Each new governance surface, like every other GUI pane, calls the MCP
dispatch / typed wrappers — never a re-implementation.

---

## 6. Sequencing & layer order

Baked decision: **data-flow first → fusion (`vox_discover`) → semantic overlay**,
with the structural-core enrichment + absorption as the prerequisite spine.

1. **Structural core + absorption** — enrich the structural index (boundary /
   composition / registry edges, edge-confidence), add `coverage`, and perform
   the graphify→Vox Search rename (CLI alias, MCP tool rename, GUI re-key). This
   is the foundation every later layer rides on.
2. **Data-flow / def-use** — `dataflow.rs` (def-use edges + 3 detectors +
   `compute_dead_signals` + `vox_search_dataflow`/`vox_search_dead_signals` + CI
   advisory). Concrete, self-contained, catches the frontend-emit class. **First
   of the new layers** (deterministic, proves two-layer discipline, no embedding
   dependency).
3. **Fusion** — `vox_discover` over **lexical-seed first** (resolver + composite
   ranker + the `GraphifyNodes` corpus hook), embedding lane behind a flag.
4. **Semantic overlay** — `semantic-overlay.json` writer/reader + freshness sha +
   `vox_search_semantic_related` + mixed seed-then-expand query. **Last**: it
   depends on the `GraphifyNodes` Vox-Search corpus (step 3) existing, so it
   reuses one embedding pipeline rather than duplicating infra.
5. **Auto-availability + steering + GUI** — `.mcp.json` + `vox mcp install`,
   pinned skill + always-on code-map injection + tiered freshness, the
   `VoxSearchPanel`, and the CLI-governance `cli:` ingestion + CI/Database
   surfaces. Sequenced alongside as each layer's tools land.

---

## 7. Honesty / error handling (consolidated)

- Deterministic extraction with **drop-on-ambiguity** across Layers 1 & 4; every
  non-AST / non-control edge `confidence`-labeled (`declared`/`heuristic`/
  `resolved`) so a name-match is never mistaken for a proven call, and a dead
  signal is never falsely raised (intra-procedural miss is acceptable).
- Overlays (Layers 3, 5) are **physically separate**, **query-time**,
  **provenance-labeled**, and carry the structural `graph_json_sha256` for
  staleness; they **never** mutate `graph.json`.
- Coverage / dead-signal reports cross-checked against the canonical command/tool
  registries so a "surfaced→nonexistent command" or false "wired" claim cannot
  survive (a permanent regression gate, complementary to `vox ci gui-honesty`).
- Freshness respected and **self-healing** (tiered); a stale structural graph
  fails-or-stamps rather than misleading.

---

## 8. Non-goals

- **No engine rewrite** — `vox-graphify-reader` is rehomed intact as the
  structural-index lib; this is naming + packaging unification.
- **No second embedding stack** — Vox Search owns embeddings; the structural
  index rides on `EmbeddingService`/Qdrant/`llm_embed`.
- **No LLM-guessed edges in the structural graph** — overlays stay separate.
- **No inter-procedural data-flow** in the first cut (intra-procedural +
  detectors only; accept the known miss for zero false positives).
- **No GUI-specific extractor fork** — all extraction lives in the general
  engine; the GUI is a lens/config + adapters.
- **No reorg code changes before ratification** — the GUI IA blueprint is
  already RATIFIED (2026-06-26); its execution (Plan 3) is the separate program.

---

## 9. Plans index (workflow-ready, with dependency DAG)

The implementation decomposes into the plans below. Several already exist on this
branch (noted); the new umbrella adds the rename/absorption + the cross-cutting
auto-availability/governance work. Each plan produces working software and is
authored/executed via `superpowers:writing-plans` → `subagent-driven-development`.

### 9.0 Canonical plan-ID crosswalk (SSOT)

This table is the **single source of truth** mapping the master spec's `P-id`s to
the layer/track plan ids (`vs*` / `3*`), the on-branch plan files, and the sibling
spec each was distilled from. **The index references this table; do not re-derive
the mapping** anywhere else.

| P-id | vs/3x-id | Plan file | Sibling spec |
|---|---|---|---|
| **P0** | **vs1** | `2026-06-26-vox-search-absorption-and-cli-ingest.md` | `2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint-design.md` |
| **P1** | **vs2** | `2026-06-26-vox-search-dataflow-layer.md` | `2026-06-26-graphify-dataflow-semantic-overlay-design.md` |
| **P2** | **vs3** | `2026-06-26-vox-search-fusion-discover.md` | `2026-06-26-graphify-voxsearch-fusion-design.md` |
| **P3** | **vs4** | `2026-06-26-vox-search-semantic-overlay.md` | `2026-06-26-graphify-dataflow-semantic-overlay-design.md` |
| **P4** | **vs5** | `2026-06-26-vox-search-agent-tool-surface.md` | `2026-06-26-graphify-agent-tool-surface-design.md` |
| **P5** | **3A / 3D** *(split)* | `2026-06-26-gui-reorg-execution-plan3a.md` + `2026-06-26-gui-caveat-completions-plan3d.md` | `docs/agents/gui-ia-blueprint.md` |
| **P6** | **3F** | `2026-06-26-gui-cli-governance-surfaces-plan3f.md` | `docs/agents/cli-gui-governance-audit.md` |
| **P7** | **3B** | `2026-06-26-voxmens-gui-full-plan3b.md` | `2026-06-26-voxmens-gui-cli-parity-design.md` |
| **P8** | **3C** | `2026-06-26-settings-consolidation-plan3c.md` | `2026-06-26-settings-consolidation-policies-unification-design.md` |

> **Note:** P5 (GUI Vox Search surface) is **split across 3A and 3D** — 3A lands the
> reorg skeleton + nav placement, 3D completes the honesty-caveat panes. P7/P8 are
> **related programs**, mapped here only for traceability; they are **not** part of
> the Vox Search service (§ appendix).

### 9.1 Plan table

| Plan | Title | Scope | Depends on |
|---|---|---|---|
| **P0** | **Absorption + structural-core enrichment** | graphify→Vox Search rename map (CLI alias, MCP tool rename, GUI re-key, brand copy); boundary + composition + registry edges; edge-confidence; `vox search coverage`. *(Extends the existing `2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint` Plan 1 / Phases A0–F with the rename.)* | — |
| **P1** | **Data-flow / def-use layer** | `dataflow.rs`: def-use edges + 3 detectors + `compute_dead_signals` + `vox_search_dataflow`/`vox_search_dead_signals` + CI advisory; frontend-emit fixture e2e. | P0 (structural node/edge schema + confidence) |
| **P2** | **Fusion — `vox_discover` (lexical-seed first)** | `resolve.rs` (hit→node_id), `graph_overlay.rs` composite ranker, `GraphifyNodes` Vox-Search corpus, `vox_discover` tool+schema, KG-score-0.0 fix; embedding lane behind a flag. | P0 (structural index + coverage); Vox-Search corpus model |
| **P3** | **Semantic overlay** | **P3a** (shippable): `semantic-overlay.json` writer/reader, freshness sha, embedding-kNN `vox_search_semantic_related` over `GraphifyNodes`, overlay staleness GUI warn. **P3b** (deferrable tail): LLM-typed-relation overlay + mixed seed-then-structural-expand "auth flow" query. | **P2** (the `GraphifyNodes` embedding corpus must exist) |
| **P4** | **Auto-availability + agent steering** | repo-root `.mcp.json` + `vox ci mcp-client-config`; `vox mcp install`; pinned `graph-first-discovery` skill; always-on code-map injection; tiered self-healing freshness; CI tier-presence assertion. | P0 (tool names final) |
| **P5** | **GUI Vox Search surface** | retire `getGraphifyStatus` split-brain; `VoxSearchPanel` tabbed panes (all via `invokeMcpTool`); fix the `graphify` orphan; place under Knowledge per ratified IA; regen surface registry. | P0 (tools), P1/P2/P3 panes land incrementally |
| **P6** | **CLI governance — `cli:` ingestion + coverage surfaces** | clap-tree `cli:` registry adapter + unified coverage matrix (+`CliOnly`, honest not-in-GUI); `Develop > CI`, `Knowledge > Database`, build-spine actions, typed secret/auth wrappers. | P0 (coverage capability + registry-adapter pattern) |

**Vox Search = P0–P6.** P7 (VoxMens GUI) and P8 (Settings/Policies) are **related
programs, not part of the Vox Search service** — see the
[Related programs](#related-programs-not-part-of-vox-search) appendix.

### Dependency DAG

```
P0 ──┬─► P1 ─────────────────────────────► P5 (panes)
     ├─► P2 ─► P3 ─────────────────────────► P5 (panes)
     ├─► P4
     └─► P6

P5 consumes tools from P0/P1/P2/P3 as they land (incremental panes).
(Related programs P7/P8 ride on P6's governance/wrapper foundation — see appendix.)
```

**Critical path:** `P0 → P2 → P3` (semantic overlay is gated on the fusion
corpus). **Parallelizable off P0:** P1, P4, P6. **GUI (P5)** trails its tools.
The **related programs** P7 (VoxMens FULL launch + cost) and P8 (Settings/
Policies co-located) — **not part of the Vox Search service** — ride on P6's
governance/wrapper foundation; see the
[Related programs](#related-programs-not-part-of-vox-search) appendix.

---

## 10. Success criteria

1. One service: `vox search …` + `vox_search_*` are the only external surface;
   `vox graphify`/`vox_graphify_*` resolve via deprecation alias for one release.
   The engine (`vox-graphify-reader`) is rehomed intact, not rewritten.
2. The enriched structural index connects the GUI graph (zero-edge share drops
   sharply from 51%; TS↔Rust linked via labeled boundary edges) and runs on a
   non-GUI corpus (generality, no fork).
3. `vox search coverage` produces the unified wiring/command-coverage matrix over
   clap `cli:` ∪ MCP `tool:` ∪ `cmd:` ∪ surfaces, cross-checked against the
   registries with no false "wired" claims and honest `CliOnly`.
4. The data-flow layer flags the frontend-emit `accumulator_never_gates` class
   deterministically **when the accumulation write and the store-read are in the
   same function** (the frontend-emit shape) — cross-function accumulators are an
   accepted intra-procedural miss; the general callee-return-miss statement still
   holds. `vox_discover` fuses lexical-seed→graph-expand with provenance labels;
   the semantic overlay answers fuzzy queries from a separate, staleness-stamped
   artifact, split into **P3a** (`vox_search_semantic_related` embedding-kNN over
   `GraphifyNodes` — shippable) and **P3b** (LLM-typed-relation overlay — a
   separate, deferrable tail).
5. Every Vox-hosted agent has the full `vox_search_*` set with zero setup;
   external harnesses get it via shipped `.mcp.json` / `vox mcp install`; the
   always-on code-map + pinned skill steer graph-first discovery.
6. The Vox Search GUI surface (under Knowledge) calls only the shared MCP
   dispatch; the `getGraphifyStatus` split-brain is retired; the `graphify`
   orphan is fixed.
7. CLI governance: the new CI/Database/build-spine/secret surfaces convert ≥60%
   of the ungoverned command set into reachable surfaces, with the rest honestly
   labeled CLI-only.
8. Every result on every layer carries a `layer`/`provenance` label; no overlay
   ever mutates `graph.json`; structural queries are byte-reproducible.

---

## 11. Relationship to prior specs/plans

### 11.0 Base / rebase

- **Base = `origin/main` @ `063a3c3235`.** The GUI honesty/wiring work is
  **MERGED to `main`** — `main` now compiles. (Supersedes the earlier note that
  this project was based on the compiling honesty branch because `main` did not
  build; that is no longer true.)
- **Rebase `claude/graphify-general-gui-ia` onto `main` before executing.**
- **NOTE: the `GraphifyStatusPanel → voxTransport` seam ALREADY landed on
  `main`** (commit `30a46cc88d`). Plan **P5 / vs5** and the 3D panes must
  **CONSUME** that seam, **not redo it** — the GUI re-key/retire work builds on
  the already-routed transport, not a fresh re-implementation.

### 11.1 Prior specs/plans

- **Composes & renames** the four `2026-06-26-graphify-*` designs (general
  enhancement, data-flow/semantic, voxsearch-fusion, agent-tool-surface) under
  one brand; their engine decisions are authoritative and preserved.
- **Consumes** the landed seam (`is_virtual`, `lexical_ingest_sha256`, `persist`,
  `vox-graphify-reader`, the existing structural MCP tools) from
  `2026-06-18-graphify-search-map-persistence.md`.
- **Stacks with** `2026-06-18-graphify-search-fusion-plan-F.md` (intent routing
  picks *which* corpus; `vox_discover` fuses *within* a corpus).
- **Ratifies-into** `docs/agents/gui-ia-blueprint.md` (the GUI placement under
  Knowledge) and **fills** `docs/agents/cli-gui-governance-audit.md` (the `cli:`
  ingestion + governance surfaces).
- **References** (does not own) the two amendment sibling specs
  (`voxmens-gui-cli-parity`, `settings-consolidation-policies-unification`) as the
  **related programs** P7/P8 — see the appendix below.

---

## Related programs (NOT part of Vox Search)

**Scope guard.** The Vox Search service is **P0–P6**. The two programs below are
**adjacent GUI-track programs**, distilled from their own sibling specs and
executed by their own plans. They ride on P6's governance/wrapper foundation but
are **out of the Vox Search service scope**; they are listed here only for
traceability (and appear as GUI-track peers in the program index).

| Program | vs/3x-id | Plan file | Sibling spec |
|---|---|---|---|
| **P7 — VoxMens GUI (FULL launch + cost)** | **3B** | `2026-06-26-voxmens-gui-full-plan3b.md` | `2026-06-26-voxmens-gui-cli-parity-design.md` |
| **P8 — Settings / Policies co-located** | **3C** | `2026-06-26-settings-consolidation-plan3c.md` | `2026-06-26-settings-consolidation-policies-unification-design.md` |

- **P7 (VoxMens GUI):** streaming Tauri wrappers (`mens_train`/`serve`,
  `populi_up`/`down` emitting `vox://` progress), opencode-style no-nag cost
  tracking, gamification; keys central in Settings/Secrets. Depends on P6 (typed
  wrappers + `cli:` parity map) and P8 (Settings IA for key placement).
- **P8 (Settings/Policies):** Settings + Policies co-located, **distinct**, under
  one "Configuration & Governance" area; central secret/key store. Depends on P6
  (secret/auth wrappers).
</content>
</invoke>
