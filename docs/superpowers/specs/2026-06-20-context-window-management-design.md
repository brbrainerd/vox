---
title: Context Window Management — Unified Spine, Archival, Retrieval, and Graphify Temporal Join
date: 2026-06-20
status: design-approved
audience: implementation (Gemini Flash 3.5 handoff target)
supersedes_partial:
  - docs/superpowers/specs/2026-06-19-dockable-workspace-context-memory-ssot-design.md (extends, does not replace)
  - docs/superpowers/plans/2026-06-17-context-window-meter.md (consumes)
---

# Context Window Management — Design Spec

## 1. Problem & Thesis

Vox today treats "a chat session," "a tab," "an agent's working context," and "an
archived conversation" as four unrelated things. The user wants them unified:
**a tab, a session, an agent's context, and an archived window are the same object — a
`ContextWindow` — observed at different lifecycle stages.** Everything else (GUI,
orchestrator wiring, A2A handoff, API assembly, OpenRouter routing, archival, and a
Graphify temporal model of how the codebase evolved) becomes a *consumer* of that one
object model.

Two non-negotiable constraints frame the whole design:

1. **Scaling / glogg model.** The system must behave like a fast log viewer over an
   arbitrarily large append-only history: index everything, materialize only the
   viewport. There may be tens of thousands of windows and millions of items.
2. **Storage is a blob; retrieval is not.** Content may be stored as opaque
   content-addressed blobs, but retrieval must never rehydrate a window into a model's
   context. Retrieval is a *ranked, deduped, budget-bounded projection* — the boundary
   that prevents context pollution.

## 2. Audit Baseline — What Already Exists (do not rebuild)

This design was audited against the codebase. The following are **BUILT** and are
reused, not reinvented:

| Capability | Location | Use |
|---|---|---|
| Content-addressed store (SHA3-512) + causal DAG + names | `crates/vox-db/src/store/ops_cas.rs`, `schema/domains/cas_codex.rs` (`objects`, `causal`, `names`) | Store every item's content once, deduped, refcount-safe GC |
| Append-only session event log | `crates/vox-orchestrator/src/session/state.rs`; `vox-db` `agent_sessions`, `agent_session_events` (TurnAdded/Compacted/StateChanged) | Event source the window projector folds into items |
| Append-only history/clip store | `crates/vox-db/src/history_store.rs` (`history_entries`, `HistoryCaps`) | Clip/command/chat capture; pin + delete primitives |
| Lineage event log | `vox-db` `orchestration_lineage_events` | Task/agent/session lineage |
| Compaction engine | `crates/vox-orchestrator/src/compaction.rs` | Extend into the tiering engine |
| Context injection + token budget | `crates/vox-orchestrator/src/context/{mod,injection_policy,token_optimization}.rs`, `agentos/context_budget_manager.rs` | Committed-set assembly + budget cap |
| A2A envelope w/ context fields | `crates/vox-orchestrator/src/a2a/envelope.rs` (`context_envelope_json`, `session_id`, `thread_id`, `trace_id`, `parent_task_id`, `span_depth`) | Carry `window_id` + content hashes across mesh |
| OpenRouter cascade + model pool | `crates/vox-actor-runtime/src/llm/{types,cascade}.rs`, `model_resolution.rs`; `crates/vox-gui/src/commands/model_pool.rs` | Per-window model route |
| FTS5 + embeddings tables | `vox-db` `knowledge_nodes_fts`, `search_document_chunks_fts`, `embeddings`, `scientia_embedding_cache` | Retrieval indexes |
| Graphify snapshots + diff + Rust-native build | `crates/vox-graphify-reader/src/{snapshot,compare,rebuild}.rs`; corpus registry `contracts/retrieval/graphify-corpora.v1.yaml` | Temporal snapshots; `git_sha` already in manifest |
| Graphify MCP tools | `crates/vox-orchestrator-mcp/src/graphify_tools.rs` (`status/search/query/path/compare`) | Agent-selectable graph queries |
| GUI docking (dockview) | `crates/vox-gui/ui/src/components/layout/DockShell.tsx` | Panels for the new surfaces |
| GUI virtualization | `crates/vox-gui/ui/src/hooks/useVirtualList.ts` (`@tanstack/react-virtual`) | 100k+ row lists |
| GUI command palette + federated search | `CommandPalette.tsx`, `paletteSources.ts`, `vox_search_query` | Window/item search corpus |
| GUI chat session rail | `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx` | Generalize → window manager |
| Context meter + attach model | `ContextWindowMeter.tsx`, `lib/loquelaContext.ts` (`AttachItem`) | Per-window meter + committed-set chips |
| Surface registry + decorators | `ui/src/generated/surfaceRegistry.generated.ts`, `components/surfaces/decoratorRegistry.ts` | Register new surfaces |
| Reactive event bus | `ui/src/transport.ts` (`vox://…` listeners) | New `vox://context-window-*` topics |

### 2.1 Keystone Gaps (the only true net-new foundations)

1. **session→commit linkage is MISSING.** No `git_sha` on `agent_sessions` / windows.
   Without it the Graphify join (layer E) is impossible. **Highest priority.**
2. **Compaction is lossy at storage.** Dropped turns are discarded transiently; the
   `Compacted` event carries an opaque summary string but raw turns are not persisted.
   Lossless archival requires writing dropped turns to CAS *before* dropping.
3. **No `context_windows` index / projector.** The unifying SSOT must be created.
4. **No interactive graph in the GUI.** Only a standalone `graphify-out/graph.html`
   (vis-network) and a canvas `AgentFlow.tsx`. A navigable graph surface is net-new.
5. **No generic cross-surface window manager** (only chat-scoped `ChatSessionRail`).
6. **Snapshot metadata lives off-DB** (filesystem manifest). Needs a queryable index.
7. **CAS has no refcount and no GC** (`ops_cas.rs` is store/get/bind only). Refcounting
   + sweep is net-new.
8. **GUI chat is a separate store** (`conversations`/`conversation_messages`), NOT
   `agent_session_events` — the projector must adapt three producers.
9. **No per-call model override** in `RouteResolutionInput`; per-window routing is net-new.
10. **`compact()` discards dropped turns and has no live caller** — must be refactored to
    return them and be wired before lossless tiering is possible.
11. **`embeddings` table is dormant** — semantic retrieval is a later phase, not v1 reuse.

## 3. Layered Architecture

```
E · Graphify temporal JOIN      window → commit → code-graph delta (NEW joins on built snapshots)
D · Archival & tiering          Hot→Warm→Cold→Frozen; lossless to CAS (EXTEND compaction.rs)
R · Retrieval layer             FTS5 + embeddings + graphify → rank·dedup·budget (NEW, the anti-pollution gate)
C · Consumers / wiring          API assembly · orchestrator · A2A · OpenRouter (EXTEND)
B · GUI surfaces                window manager · bonsai trim · context editor · graph navigator (EXTEND/NEW)
A · ContextWindow spine         context_windows + context_window_items over CAS + event log (NEW, small)
S · Storage (built)             objects/causal/names (CAS) · agent_session_events · history_entries
```

## 4. Layer A — The ContextWindow Spine (SSOT)

### 4.1 Data model (new tables in `vox-db`, added via the existing migration manifest)

`context_windows`:

| column | type | notes |
|---|---|---|
| `id` | TEXT PK | uuid |
| `repo_id` | TEXT | scopes everything (matches `repository_id` elsewhere) |
| `title` | TEXT | user/agent label |
| `kind` | TEXT | `chat` \| `task` \| `agent` \| `a2a` \| `archived` |
| `tier` | TEXT | `hot` \| `warm` \| `cold` \| `frozen` |
| `parent_window_id` | TEXT NULL | fork parent → the bonsai tree |
| `root_window_id` | TEXT | tree root for fast subtree queries |
| `agent_id` | TEXT NULL | owning agent if any |
| `thread_id` / `trace_id` | TEXT NULL | mirror A2A envelope identity |
| `model_route` | TEXT NULL | per-window OpenRouter/model override |
| `git_sha_at_open` | TEXT NULL | **keystone**: commit at window start |
| `git_sha_at_close` | TEXT NULL | **keystone**: commit at window archive |
| `token_estimate` | INTEGER | cached rollup |
| `pinned` | INTEGER | survives auto-tiering/GC |
| `created_at` / `updated_at` | INTEGER | ms epoch |
| `deleted_at` | INTEGER NULL | soft delete (bonsai trim at window granularity) |

`context_window_items`:

| column | type | notes |
|---|---|---|
| `id` | TEXT PK | uuid |
| `window_id` | TEXT FK | → context_windows |
| `ordinal` | INTEGER | order within window |
| `role` | TEXT | `user` \| `assistant` \| `system` \| `tool` |
| `item_kind` | TEXT | `message` \| `pin` \| `attachment` \| `summary` \| `tool_call` |
| `content_hash` | TEXT FK | → `objects.hash` (CAS). Content stored ONCE, deduped |
| `token_estimate` | INTEGER | per-item |
| `pinned` | INTEGER | promoted into committed set, survives compaction |
| `committed` | INTEGER | currently in the committed set (what the model sees) |
| `redacted` | INTEGER | secret-scrubbed variant flag |
| `created_at` | INTEGER | ms epoch |
| `trimmed_at` | INTEGER NULL | soft delete (bonsai trim at item granularity) |

Indexes (covering, for glogg-scale): `(repo_id, tier, updated_at)`,
`(root_window_id, parent_window_id)`, `(window_id, ordinal)` on items,
`(content_hash)` for refcount, partial index `WHERE deleted_at IS NULL` / `trimmed_at IS NULL`.

### 4.2 The projector

A `WindowProjector` folds the existing append logs into `context_window_items`.
**Correction (verified):** there is NOT a single event source. There are **three**
producers and the projector must adapt all three behind one `WindowSource` trait:

1. **Orchestrator/agent sessions** — `agent_session_events` (`TurnAdded`/`Compacted`/…).
2. **GUI chat** — a *separate* store: `conversations` + `conversation_messages`
   (`crates/vox-db/src/codex_chat.rs`, written by `crates/vox-gui/src/commands/chat.rs`).
   This is the split-brain risk; the projector unifies it at the index, it does NOT
   merge the underlying tables.
3. **History/clips** — `history_entries` (clip/command/chat capture).

The single SSOT is `context_windows`/`context_window_items` (the *index*), fed by a
`WindowSource` adapter per producer. Rules (per `TurnAdded`-equivalent event):

- `TurnAdded` → write content blob to CAS (zstd-compress, get hash), insert an item
  row referencing the hash.
- `Compacted` → persist the dropped turns to CAS first (fixes keystone #2), then write
  a `summary` item; mark superseded items `committed = 0` (not trimmed — still
  retrievable).
- `StateChanged`/idle → drives tier transitions (layer D).

Sessions/tabs/agent-contexts continue to emit session events as today; the projector
makes them all materialize as `ContextWindow`s. New GUI-initiated windows emit the same
event shape.

## 5. Layer D — Archival & Tiering

Extend `compaction.rs` into a `TierManager` (background, idempotent):

- **Hot** — active window; items hot in DB, content in CAS.
- **Warm** — recently idle; unchanged storage, dropped from in-memory working set.
- **Cold** — compaction runs; **dropped turns persisted to CAS before removal**;
  a persisted summary item is created; raw items remain retrievable (lossless).
  **Correction (verified):** `compaction.rs::compact()` currently returns only *counts*
  (`dropped_turns`) and discards the dropped `Turn` objects inside its trim strategies,
  and has **no live caller found** — so two prerequisite fixes precede tiering: (a)
  refactor `compact()` to *return* the dropped `Turn`s, (b) confirm/establish its call
  site. Treat (a)+(b) as their own atomic task before any CAS-persist step.
- **Frozen** — deep archive; index row + summary kept light; CAS blobs already
  compressed+deduped; optionally zstd-pack cold blob groups.

**GC is refcount-safe — and refcounting is NEW work (verified).** `ops_cas.rs` today
has NO refcount column and NO delete/GC path (`store`/`get`/`bind_name` only); `causal`
is lineage, not a refcount. So this design adds a `content_hash` refcount derived from
live `context_window_items` (a covering index + a sweep query, or a maintained
`cas_refcount` row). A CAS blob is purged only when no live (non-trimmed) item
references its `content_hash`. Pinned windows/items are never auto-GC'd. Hard purge
(bonsai) is the only path that removes content, and only when unreferenced.

Persisted summaries and snapshot pointers also fix the audit finding that compaction
rationale is currently un-queryable.

## 6. Layer R — Retrieval (the anti-pollution boundary)

**Storage is a blob; retrieval is structured.** A `RetrievalRouter` fans a query to:

- **FTS5** — exact/lexical over item text (new FTS virtual table mirroring
  `context_window_items`, with the established trigger pattern). **Must degrade
  gracefully:** FTS5 is runtime-detected (`has_fts5_support`) and absent on some SQLite
  builds; reuse the existing `LIKE`-fallback path from
  `ops_memory/search.rs::query_search_document_chunks`. v1 retrieval is correct with or
  without FTS5.
- **embeddings (semantic) — OPT-IN / PHASE 2, not v1.** The `embeddings` table exists
  but is **dormant**: nothing populates it unless an `EmbeddingService` is explicitly
  wired (verified — only research/memory paths write vectors, gated on a service). v1
  retrieval is lexical (FTS5/LIKE) + graphify only. Semantic recall is a later phase
  that first wires embedding generation for window items; the router must treat the
  semantic lane as optional and skip it when no service is attached.
- **graphify** — structural ("which windows touched symbol X") via the MCP tools.

Results are merged → **ranked → deduped by `content_hash` → token-budget-capped** →
returned as **references + snippets + summaries**, never raw blobs. Two audiences,
one index:

- **Human (GUI):** may receive large virtualized result lists.
- **LLM/agent:** top-k only, token-budgeted; **blobs are fetched by hash only when a
  specific reference is explicitly opened or promoted into a committed set.** Nothing
  auto-injects raw content.

The committed set (`context_window_items.committed = 1`) is the SSOT for what reaches
the model and is the single enforcement point for the token budget (reuse
`context_budget_manager` / AttentionBudget).

### 6.1 Byte-native storage, per-model token estimation at the boundary (reviewed)

Content is stored as raw bytes (CAS `objects.data`, already a `Vec<u8>`), and each item
records an exact `byte_len` — the one **model-invariant** size fact. Token counts are NOT
a storage property: every model family tokenizes differently (a chunk that is 4k tokens
for one model is 5.5k for another), so token accounting is computed **at the committed-set
/ API-assembly boundary against the window's current `model_route`** via a `TokenEstimator`
trait (backed by `tiktoken-rs`/`tokenizers` where available, else a per-family
bytes/token ratio). Because a window can switch models mid-session, the budget meter
recomputes when `model_route` changes.

**Rejected as unsafe (review of a proposed heuristic):** a "max bytes = token_limit ×
3.5" rule is an *average-case* ratio, NOT a hard guarantee — dense content (code, CJK,
base64) runs well under 3.5 bytes/token and would overflow. A true cap either tokenizes
exactly or uses a *conservative lower-bound* bytes/token with margin. Any byte-size
chunking threshold (e.g. 128 KB) is a tunable for degradation-avoidance ("lost in the
middle"), never a correctness guarantee. The estimator must also handle non-UTF-8 bytes
explicitly (never silently treat invalid UTF-8 as empty/zero tokens).

### 6.2 Model-relative projections & sub-agent handoff (approved)

A `ModelProfile { model_id, max_context_tokens, tokenizer_ratio, tool_capable,
reasoning_tier }` (sourced from the OpenRouter pool / `model_resolution`) is the SSOT for
"what fits." A `Projection(window, model)` is an ordered list of **item references** plus a
per-item fate (`included`/`summarized`/`dropped`/`on_demand`) computed by the
`TokenEstimator` against the profile. **Storage stays model-invariant (bytes, one CAS
copy); only the projection is model-relative** — never store content per model.

**Sub-agent detach = retrieval-on-demand seed (default) with pre-pack fallback.** A
sub-agent is a **child window** (`parent_window_id`). Its seed = pins + task brief + a
compact summary, sized to a fraction of the sub-agent's window (reserve headroom). The
A2A envelope carries `child_window_id` + `parent_window_id` + seed hashes + a **retrieval
grant scoped to the parent's `root_window_id` lineage**; the sub-agent pulls items by hash
through the retrieval router into *its own* committed set, budget-enforced by *its*
`ModelProfile`. The seed snapshots CAS hashes at handoff → immutable even if the parent
advances. **Critique-driven guardrails:** retrieval-on-demand requires `tool_capable` —
non-tool models auto-fall-back to a pre-packed projection (so both paths exist, on-demand
is the principled default); cap pulls/turn + budget to prevent thrash; the summary is
lossy but raw stays retrievable in CAS with provenance.

## 7. Layer C — Consumers / Wiring

- **API assembly:** `context_get` / `context_set` Tauri commands (already planned in the
  dockable-workspace spec) read/write the committed set of the active window → emit the
  serialized API payload. This is the SSOT for "what goes to the model."
- **Orchestrator:** `context/injection_policy.rs` reads the committed set instead of
  ad-hoc assembly.
- **A2A:** `RemoteTaskEnvelope.context_envelope_json` carries `window_id` + the list of
  `content_hash`es (not the bytes). The receiver pulls from the **shared CAS** →
  zero-copy, dedup-by-construction handoff.
- **OpenRouter "fully wired":** `context_windows.model_route` overrides the cascade per
  window. **Correction (verified):** the cascade has NO per-call override today —
  `RouteResolutionInput` resolves from env (`VOX_SELECTOR_MODEL`/`VOX_MODEL`) then global
  config; `manual_model` is the closest hook. This design adds an `override_model:
  Option<String>` field to `RouteResolutionInput`, checked *before* the global cascade,
  threaded from the active window's `model_route`. The model-pool DTO gains a per-window
  binding; the budget meter is per-window.

## 8. Layer B — GUI Surfaces

A new **Context Workspace** surface (generalize `ChatSessionRail`), three dockview
panels, all `useVirtualList`:

1. **Window bonsai (left):** virtualized tree of windows by `parent/root`, tier badges,
   token size, last-touched, agent. FTS+vector search box. Batch-select. Actions:
   *Batch trim*, *Merge→new* (compose a window from items selected across windows),
   *Recover* (rehydrate index, lazy-pull blobs).
2. **Items (center):** the selected window's items, virtualized; filters for
   junk/dupes/tool-noise; individual + shift-range multiselect; *Trim* (soft-delete
   `trimmed_at`) and *Promote →* into the committed set.
3. **Committed set (right):** the anti-pollution gate — exactly what the model sees;
   dupes collapsed by hash; per-window `ContextWindowMeter`; per-window model route
   selector.

**Bonsai trimming:** soft-delete at window and item granularity; individual, batch,
range, or filter-based ("all tool-noise older than 90d"); a separate *Hard purge* step
GC's unreferenced CAS blobs.

**Graph navigator surface (net-new):** an interactive graph component. **Correction
(verified):** `vis-network` is NOT a vox-gui dependency (it is CDN-loaded only by the
standalone `graphify-out/graph.html`). The GUI already depends on **`@xyflow/react`
(React Flow)** — build the navigator on that, not vis-network. Left rail selects the
corpus (code-graph, evolution-join,
window-provenance, agent-mesh) and lens (clusters/god-nodes/blast-radius). A time
scrubber replays codebase evolution. Click a code node → trace back to the windows +
items that produced it; scrub time → watch a subsystem grow.

Register both surfaces in `decoratorRegistry`; subscribe to new
`vox://context-window-changed`, `vox://context-tier-changed`, `vox://graph-snapshot-added`
events.

### 8.1 Sub-agent activity visualization (nested · editable · skill-aware · controllable)

Sub-agent activity must be a first-class **nested** surface, not one line in a chat. It
renders the live tree of agent/sub-agent **windows** (from A2A lineage: `parent_task_id` /
`span_depth` / `trace_id`, joined to `context_windows.parent_window_id`). Each node:

- **Skill-aware:** shows the running skill (badge), and its context can be grouped/filtered
  by skill (a skill lens over the committed set).
- **Model-relative meter:** the node's `ModelProfile` + token meter; the committed-set view
  reflows when the node's model changes (per §6.2).
- **Editable at every level:** the committed-set editor (§8 right pane) operates on ANY
  node — add/remove/pin/reorder items, adjust budget — including deeply nested sub-agents,
  live. Edits write through the same `context_set` SSOT scoped to that node's window.
- **Visualizable:** an expandable tree (bonsai) + a graph view (`@xyflow/react`,
  `agent-mesh` corpus) + a live event stream per node (`vox://agent-events`), including the
  **retrieval pulls** a sub-agent makes (which parent items it materialized on demand).
- **Controllable:** per-node actions — pause/resume, overrule (reuse soft-HITL
  `overrule_task` / `FeedbackStore`), adjust budget/model, inject/remove context, kill.

Reuse: `@xyflow/react`, `DockShell`, `useVirtualList`, `ContextWindowMeter`,
`loquelaContext`, `decoratorRegistry`/`surfaceRegistry`, `vox://agent-events`, and the
existing `AgentFlow.tsx` as a starting point. Frontend is testable in isolation against
mocked transport clients (the established GUI test pattern); real Tauri wiring of
`context_get/set`, lineage, and control commands is a named backend follow-on (chunks
6–7).

## 9. Layer E — Graphify Temporal Join

The chosen model is **the join** (windows are the edit events that link to commits that
mutate code-graph nodes), with **arbitrarily expandable corpora** so agents/LLMs choose
what to search.

- **Enable the join:** populate `git_sha_at_open/close` on windows (keystone #1).
  **Verified:** these fields exist nowhere today and there is no session→commit
  association anywhere in the codebase — this is entirely net-new. On window close (or
  on commit), record the SHA via `git rev-parse HEAD` in the repo scope.
- **Scope the join honestly:** many windows (research, brainstorm, agent chatter)
  produce no commit. `git_sha_*` is nullable; such windows participate in the
  provenance graph but contribute no code-delta edge. The evolution-join only draws
  `window → commit → code-delta` edges for windows whose SHA actually advanced.
- **Queryable snapshots:** new `context_graph_snapshots` table indexing the existing
  filesystem snapshots (`corpus_id`, `stamp`, `git_sha`, `node/edge/community counts`,
  path) so snapshots join to windows/commits in SQL rather than only on disk.
- **Evolution-join corpus:** a new graphify corpus built by a `vox graphify evolution`
  command that reads `window → git_sha → diff_manifests(prev, cur)` and emits a temporal
  graph: `ContextWindow → commit → {file/symbol delta}`.
- **Agent-facing:** extend the graphify MCP tools with a window-join + temporal mode so
  LLMs "decide what aspects of which graphs to search." Corpora remain registry-driven
  (`graphify-corpora.v1.yaml`) ⇒ arbitrarily expandable, dynamically teased apart.
- **Global vs local traversals on one graph:** "how did the whole codebase evolve" =
  time-scrub the evolution-join; "what context produced this function" = node traceback.

## 10. Critique & Codebase Audit (design:design-critique pass)

- **Do not build a new CAS** — `ops_cas.rs` (SHA3-512 `objects`/`causal`) already
  provides lossless + dedup. A naive plan would have duplicated it.
- **Do not fork session persistence** — build `context_windows` as a projection over the
  existing `agent_session_events`. A second event source would create the split-brain
  pattern this codebase has repeatedly suffered.
- **session→commit linkage is the true keystone** — sequence it first; layer E is dead
  without it.
- **Compaction must become lossless-at-storage** — persist dropped turns to CAS before
  dropping; otherwise "observe the codebase over time" replay is untrustworthy.
- **Retrieval must never be a blob dump** — the `RetrievalRouter` + committed-set gate is
  mandatory, not optional polish; it is the difference between an archive and context
  pollution.
- **Scaling discipline** — every list virtualized; never `SELECT *` a window; covering
  indexes + FTS/vector drive search; show counts not contents.
- **GC must be refcount-safe** — never purge a CAS blob another window references.
- **GUI reuse** — `useVirtualList`, `DockShell`, `CommandPalette`, `ContextWindowMeter`,
  `loquelaContext`, surface decorators, `vox://` events. The single largest net-new UI is
  the graph component; mitigate by building on `@xyflow/react` (already a dependency).

## 11. Scope Decomposition for Planning

> **Superseded by §14.2.** The list below is the conceptual grouping; the
> *authoritative, audit-corrected* execution order (which splits out CAS refcount, the
> 3-source projector, the `compact()` refactor, and the per-window override as their own
> chunks, and defers semantic retrieval to Phase 2) is **§14.2**. Plan against §14.2.

The plan should sequence by the keystones, each independently testable:

1. **Spine:** `context_windows` + `context_window_items` tables + `WindowProjector` over
   the existing event log (writes content to CAS).
2. **session→commit linkage:** `git_sha_at_open/close` capture.
3. **Lossless compaction → TierManager:** persist dropped turns to CAS; tier transitions;
   refcount-safe GC.
4. **Retrieval layer:** FTS over items + embeddings + graphify merge → rank/dedup/budget;
   committed-set SSOT.
5. **Wiring:** `context_get/set`, injection policy, A2A hash handoff, per-window
   OpenRouter route.
6. **GUI window manager + bonsai trim + committed-set editor.**
7. **Graph navigator surface + `context_graph_snapshots` index.**
8. **Graphify evolution-join corpus + temporal MCP query mode.**

## 12. Non-Goals (YAGNI)

- No external vector DB / VSS plugin (use existing `embeddings` BLOB + FTS hybrid).
- No lossy cold tier in v1 (lossless chosen; a `RetentionPolicy` registry is a later
  follow-on if opt-in lossy is ever wanted).
- No new docking framework (dockview stays).
- No rewrite of graphify construction (Rust-native build stays; we add a corpus + index).

## 13. Adversarial Audit — Verified Corrections (AUTHORITATIVE)

This section overrides any earlier statement it conflicts with. Each row was checked
against the code. The lesson from the Antigravity ledger is that **plan-side false
assumptions, not the agent, cause hollow-green failures** — so these are corrected
*before* a single task is handed off.

| # | Original assumption | Verdict | Reality | Plan consequence |
|---|---|---|---|---|
| 1 | Single event source (`agent_session_events`) | **FALSE** | GUI chat is `conversations`/`conversation_messages` (`codex_chat.rs`); history is `history_entries` | Projector = a `WindowSource` trait with 3 adapters; index unifies, tables stay separate |
| 2 | CAS gives refcount-safe GC for free | **FALSE** | `ops_cas.rs` has no refcount, no delete/GC; `causal` ≠ refcount | Add refcount + sweep as an explicit task; never "free" |
| 3 | FTS5 always available | **PARTIAL** | Runtime-detected; `LIKE` fallback exists | Router must degrade; reuse `ops_memory/search.rs` fallback; v1 correct without FTS5 |
| 4 | Reuse `vis-network` (a dep) | **FALSE** | Not in `package.json`; CDN-only in `graph.html`. `@xyflow/react` IS a dep | Build navigator on `@xyflow/react` |
| 5 | Hook compaction to persist dropped turns | **PARTIAL** | `compact()` returns counts only, discards `Turn`s, no live caller | Refactor signature + establish caller first (own task) |
| 6 | Per-window model override exists | **FALSE** | `RouteResolutionInput` is env/global only | Add `override_model` field before the cascade |
| 7 | `embeddings` powers semantic retrieval | **PARTIAL** | Table dormant; nothing populates it by default | Semantic = Phase 2 behind an `EmbeddingService`; v1 lexical+graph |
| 8 | `git_sha_at_open/close` links windows→commits | **FALSE** | Fields exist nowhere; no session→commit link anywhere | Entirely net-new; nullable; only commit-advancing windows get code-delta edges |

**Net effect on scope:** the design is still sound, but four items move from "reuse" to
"net-new, sequence first": (a) CAS refcount+GC, (b) 3-source projector, (c)
`compact()` refactor, (d) per-window model override. None is large; all are now explicit.

## 14. Flash Handoff Strategy — Pilot-First, Pathway-Validated

The plan will be executed by **Gemini Flash 3.5 in Antigravity** via the existing
pathway: `vox_agy_pipeline` runs each task's gates inside a **jailed git worktree**
(`agy_exec.rs`/`agy_gates.rs`/`agy_worktree.rs`), an adversarial `code-reviewer` pass
hunts hallucination + hollow-green, and `vox_agy_review` records the verdict + lessons
to `docs/superpowers/antigravity-handoff-ledger.md` (next AGH id). Flash's hard limits
(~48% unaided in-IDE completion, no mid-task checkpoint, weak long-context recall,
hard quota cutoff, repeats failures) dictate the plan *shape*, per
`docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`.

**Every task** carries the standard Operating Rules + Flash Execution Addendum: atomic +
green + committed; a BLOCKING pre-flight `rg` gate that pastes on-disk reality and STOPs
on mismatch; two-strike circuit breaker; split-on-overrun; `[PARALLEL-SAFE]`/
`[SEQUENTIAL]` tags by file-write disjointness; prove EFFECT not SHAPE (route fixtures
through real compilers/type-checkers, no substring-only tests).

### 14.1 Pilot chunk first (validate the pathway before committing the full plan)

Do **not** hand off all 8 chunks at once. Ship one **vertical, low-blast-radius pilot**
through the *entire* pathway, read the result, and only then continue. The pilot is
**Chunk 1 (the spine), narrowed**:

- **Pilot task P0** — add `context_windows` + `context_window_items` tables via the
  existing migration manifest + a `cas_refcount` helper; pure `vox-db`, no consumers.
  Gates: `cargo build -p vox-db`, `cargo test -p vox-db`, `cargo clippy -p vox-db -D
  warnings`. This exercises the riskiest *process* questions (Does Flash respect the
  migration manifest? Does it invent schema APIs? Does the worktree jail + gates report
  cleanly?) with the *lowest* code risk.

**Go/No-Go after the pilot:** inspect the worktree diff + the `code-reviewer` verdict +
the ledger entry. Useful answer (green, faithful, no hallucinated API, schema matches
`BASELINE_VERSION` conventions) ⇒ proceed to the next chunk. Hollow/false ⇒ fix the
*plan/prompt* (not just re-run), capture the lesson in §B of the ledger, re-issue once
(two-strike), and only widen scope once the pathway is proven on something small.

### 14.2 Sequencing (each chunk = one handoff, gated by the prior chunk's review)

1. **P0 spine + refcount** (pilot) → 2. **3-source projector** → 3. **session→commit
capture** → 4. **`compact()` refactor + TierManager + GC sweep** → 5. **retrieval router
(lexical+graph, FTS5-degrading)** → 6. **wiring: `context_get/set` + injection +
`override_model` + A2A hash handoff** → 7. **GUI window manager + bonsai + committed-set
(on `@xyflow/react` where graph is involved)** → 8. **graph navigator +
`context_graph_snapshots`** → 9. **graphify evolution-join + temporal MCP mode** →
(Phase 2) **embeddings/semantic lane**.

Chunks 2–9 are authored only after the pilot proves the pathway returns useful answers.
Each chunk gets its own plan file + Flash Execution Addendum + a fresh ledger entry, and
must branch off **current** `origin/main` with ONLY that chunk's commits and a full
delivery manifest (so review detects undisclosed shared-config edits).

### 14.3 Pathway gaps to build out (only if the pilot reveals them)

The pathway exists and is proven on 4 prior handoffs, but watch for: (a) Rust-side gates
are well-trodden, GUI/TS gates (`pnpm`, `npx tsc --noEmit`, `npx vitest run`) less so —
the GUI chunks (7–8) may need the addendum's house-rules tightened; (b) the evolution
graph build (chunk 9) is the only step with an open-ended feel — pre-decompose it into
single-decision steps before handoff, since Flash reasons poorly on open-ended "design
X" tasks. No new pathway crates are anticipated; if the pilot shows the jail/gates can't
express a needed check, extend `agy_gates.rs` rather than loosening the gate.
