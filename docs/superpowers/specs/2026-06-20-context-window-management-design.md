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

A `WindowProjector` folds the existing `agent_session_events` append log into
`context_window_items`. **One event source — no split-brain.** Rules:

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
- **Frozen** — deep archive; index row + summary kept light; CAS blobs already
  compressed+deduped; optionally zstd-pack cold blob groups.

**GC is refcount-safe.** A CAS blob is purged only when no live (non-trimmed) item in
any window references its `content_hash` — refcount derived from `content_hash` index
+ `causal`/`names`. Pinned windows/items are never auto-GC'd. Hard purge (bonsai) is
the only path that removes content, and only when unreferenced.

Persisted summaries and snapshot pointers also fix the audit finding that compaction
rationale is currently un-queryable.

## 6. Layer R — Retrieval (the anti-pollution boundary)

**Storage is a blob; retrieval is structured.** A `RetrievalRouter` fans a query to:

- **FTS5** — exact/lexical over item text (new FTS virtual table mirroring
  `context_window_items`, with the established trigger pattern).
- **embeddings** — semantic recall over the existing `embeddings` table.
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
  window; the existing model-pool DTO gains a per-window binding; the budget meter is
  per-window.

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

**Graph navigator surface (net-new):** an interactive graph component. **Reuse
vis-network** (already a dependency via `graph.html`) wrapped in React, rather than
introducing react-flow. Left rail selects the corpus (code-graph, evolution-join,
window-provenance, agent-mesh) and lens (clusters/god-nodes/blast-radius). A time
scrubber replays codebase evolution. Click a code node → trace back to the windows +
items that produced it; scrub time → watch a subsystem grow.

Register both surfaces in `decoratorRegistry`; subscribe to new
`vox://context-window-changed`, `vox://context-tier-changed`, `vox://graph-snapshot-added`
events.

## 9. Layer E — Graphify Temporal Join

The chosen model is **the join** (windows are the edit events that link to commits that
mutate code-graph nodes), with **arbitrarily expandable corpora** so agents/LLMs choose
what to search.

- **Enable the join:** populate `git_sha_at_open/close` on windows (keystone #1). On
  window close (or commit), record the SHA.
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
  the graph component; mitigate by reusing vis-network.

## 11. Scope Decomposition for Planning

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
