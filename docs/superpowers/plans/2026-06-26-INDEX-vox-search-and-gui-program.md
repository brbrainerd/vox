---
category: "Architecture SSOTs"
title: "HANDOFF INDEX — Vox Search + GUI Reorg Program (2026-06-26)"
date: 2026-06-26
status: index
---

# HANDOFF INDEX — Vox Search + GUI Reorg Program

This index is the **single entry point** for the 2026-06-26 program: the unified
**Vox Search** code-intelligence service (absorbing the former "graphify" framing)
plus the **GUI reorg / honesty** trilogy and its CLI-governance + frontend-emit
companions. It lists every spec and plan, the execution-order DAG (which plans gate
which), and each plan's `[PARALLEL-SAFE]` fan-out batch count so a workflow runner
can dispatch concurrently.

All work is on branch **`claude/graphify-general-gui-ia`** in worktree
`C:/Users/Owner/vox-graphify-gui`. Sub-agents use **add+commit only** — never
`checkout`/`reset`/`clean`/`push`/`rebase`. Plans are workflow-ready: every task is
tagged `[PARALLEL-SAFE]`/`[SEQUENTIAL]`, grouped into explicit fan-out batches, and
ends in its own commit.

---

## 1. Specs (design SSOTs — read-only w.r.t. code)

| Spec (path) | One-line purpose |
|---|---|
| `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md` | **Umbrella master spec.** Absorbs the four sibling graphify designs into one "Vox Search" service; defines P0–P6, the 5 retrieval layers, the honesty firewall, and §10.4 frontend-emit success criterion. |
| `docs/superpowers/specs/2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint-design.md` | The executed general-enhancement + GUI-IA blueprint (P0 structural core + coverage); the foundation the master spec builds on. |
| `docs/superpowers/specs/2026-06-26-graphify-dataflow-semantic-overlay-design.md` | Data-flow / def-use layer (L4) + semantic overlay (L5): new node/edge kinds, `accumulator_never_gates` detector, separate `semantic-overlay.json`. |
| `docs/superpowers/specs/2026-06-26-graphify-voxsearch-fusion-design.md` | Graph-augmented retrieval: `vox_discover` (search-seed → graph-expand → composite re-rank), structural-overlay ranking, KnowledgeGraph corpus fix. |
| `docs/superpowers/specs/2026-06-26-graphify-agent-tool-surface-design.md` | Auto-availability + agent steering (graph-first over grep) + GUI consumption of the one MCP tool layer; layer-tool registry pattern. |
| `docs/superpowers/specs/2026-06-26-settings-consolidation-policies-unification-design.md` | Settings consolidation + Settings/Policies co-location (drives GUI Plan 3C). |
| `docs/superpowers/specs/2026-06-26-voxmens-gui-cli-parity-design.md` | VoxMens / Populi GUI ↔ CLI parity surface (drives GUI Plan 3B). |

---

## 2. Plans (workflow-ready, TDD, per-task commits)

### 2.1 Vox Search layers (vs1 → vs5)

| Plan (path) | Master-spec scope | One-line purpose |
|---|---|---|
| `docs/superpowers/plans/2026-06-26-vox-search-absorption-and-cli-ingest.md` | **vs1 = P0 + P6 §5.1** | Rename `vox graphify` → `vox search` + `vox_graphify_*` → `vox_search_*` (1:1, deprecation alias); retire `getGraphifyStatus` split-brain; ingest the 549-leaf clap CLI tree as `cli:` nodes + `CliOnly` coverage. **Prerequisite spine for all later Vox Search plans.** |
| `docs/superpowers/plans/2026-06-26-vox-search-dataflow-layer.md` | **vs2 = P1** | Layer-4 def-use index in `vox-graphify-reader`; `vox_search_dataflow` / `vox_search_dead_signals`; the `accumulator_never_gates` detector + mandatory e2e test reproducing the frontend-emit bug class. |
| `docs/superpowers/plans/2026-06-26-vox-search-fusion-discover.md` | **vs3 = P2** | Fused graph-RAG entry point `vox_discover` (lexical-seed-first; embedding behind a flag); structural-overlay ranking as a query-time, provenance-labeled overlay; `SearchCorpus::GraphifyNodes` hook. |
| `docs/superpowers/plans/2026-06-26-vox-search-semantic-overlay.md` | **vs4 = P3** | Semantic overlay (L5): embedding-backed, LLM-relation-labeled, stored in separate `semantic-overlay.json`; `vox_search_semantic_related`; mixed seed-then-structural-expand; rides vs3's `GraphifyNodes` corpus (no second embedding stack). |
| `docs/superpowers/plans/2026-06-26-vox-search-agent-tool-surface.md` | **vs5 = P4 + agent/GUI surface** | Generated repo-root `.mcp.json` + `vox ci mcp-client-config` gate; graph-first agent steering; GUI consumption of the same tool layer. Parallel to vs2–vs4. |

### 2.2 GUI reorg / honesty + companions (3A–3F)

| Plan (path) | One-line purpose |
|---|---|
| `docs/superpowers/plans/2026-06-26-gui-reorg-execution-plan3a.md` | **3A** — ratified GUI moves/merges/renames/cuts + nav skeleton. First of the reorg trilogy; gates 3C/3D/3F. |
| `docs/superpowers/plans/2026-06-26-voxmens-gui-full-plan3b.md` | **3B (FULL, no-deferral — the ratified plan)** — complete `mens` ("Model Lab") + `populi` ("Mesh") GUI: launch + monitor + streaming Tauri wrappers + opencode-style no-nag cost UI + gamification, admin ops confirm-gated, keys central in Settings/Secrets. (The earlier monitor-only `voxmens-gui-v1` plan is **retired/superseded** by this one.) |
| `docs/superpowers/plans/2026-06-26-settings-consolidation-plan3c.md` | **3C** — Settings consolidation + Settings/Policies co-location (TDD). Depends on 3A. |
| `docs/superpowers/plans/2026-06-26-gui-caveat-completions-plan3d.md` | **3D** — GUI honesty-audit caveat completions over surviving surfaces. Depends on 3A. |
| `docs/superpowers/plans/2026-06-26-frontend-emit-validation-gate-plan3e.md` | **3E** — runtime `strict_view_validation` gate making "bad UI doesn't compile" true at `vox build --target client`. Independent; cross-refs vs2 (structural-detection complement of the same bug class). |
| `docs/superpowers/plans/2026-06-26-gui-cli-governance-surfaces-plan3f.md` | **3F = P6** — GUI CLI-governance surfaces (Develop>CI, Knowledge>Database, build-spine, typed secret/auth wrappers, honest "not-in-GUI"). Depends on 3A. |

---

## 3. Execution-order DAG

### 3.1 Vox Search track

```
vs1 (P0 absorption+rename+CLI-ingest)  ── PREREQUISITE SPINE ──┐
   │  (tool names final once Batch 1 merged)                    │
   ▼                                                            │
 ┌────────────┬───────────────┬───────────────────────────┐    │
 ▼            ▼               ▼                            ▼    │
vs2 (P1      vs3 (P2          vs5 (P4 agent/GUI            (all gated on
data-flow)   fusion           surface) — PARALLEL          vs1 tool names)
             vox_discover)    to vs2/vs3/vs4
                  │
                  ▼
              vs4 (P3 semantic overlay)
              rides vs3's GraphifyNodes corpus
```

- **vs1 → vs2 → vs3 → vs4** is the strict serial backbone for the engine layers
  (each consumes the prior layer's names/corpus). In practice only the *names* from
  vs1 must be final (vs1 Batch 1 merged) before vs2/vs3 start; vs4's **Phases A/B/D**
  depend only on P0 and can begin in parallel with vs3, while vs4 **Phase C**
  (embedding-seed wiring) hard-blocks on vs3's `SearchCorpus::GraphifyNodes`.
- **vs5 is parallel** to vs2/vs3/vs4 — it only needs vs1's final tool names; it adds
  config/steering/GUI plumbing, no engine-layer dependency.
- Critical path: **vs1 → vs3 → vs4**.

### 3.2 GUI track

```
3A (reorg skeleton — Bundle 7 gate, Task 7.1)
   │   do NOT dispatch 3C/3D/3F until 3A Bundle 7 green
   ▼
 ┌──────────┬──────────┬──────────┐
 ▼          ▼          ▼
3C         3D         3F (P6)
(settings) (caveats)  (CLI-governance)

3B  ── independent (VoxMens/Populi GUI; no 3A dependency)
3E  ── independent (frontend-emit runtime gate)
```

- **3A → {3C, 3D, 3F}** — 3A is the first reorg plan, depends on nothing; its
  Bundle 7 gate (Task 7.1) must be green before 3C/3D/3F dispatch.
- **3B and 3E are independent** — they touch disjoint surfaces and gate nothing in
  the reorg trilogy.

### 3.3 Cross-track reference

- **3E ↔ vs2** — same bug class (swallowed `reactive_view_emit_failures` accumulator
  that never gates the build). **3E is the runtime fix** (land now, self-contained);
  **vs2 is the structural-detection complement** (`vox_search_dead_signals` /
  `accumulator_never_gates`) that finds the class across the whole codebase and, per
  master spec §10.4, proves the detector against this very fixture. Sibling plans,
  not a hard dependency — either can land first.

---

## 4. `[PARALLEL-SAFE]` fan-out batch counts

| Plan | `[PARALLEL-SAFE]` tasks | `[SEQUENTIAL]` tasks | Fan-out batches |
|---|---:|---:|---|
| vs1 — absorption + CLI-ingest | 7 | 12 | Batches 0–4 (5) |
| vs2 — data-flow / def-use | 5 | 21 | sequential-dominant (def-use core serial; parallel siblings within phases) |
| vs3 — fusion / `vox_discover` | 9 | 5 | Batches 1–5 (5) |
| vs4 — semantic overlay | 13 | 12 | phased (A/B/D P0-only run parallel to vs3; C blocks on vs3) |
| vs5 — agent tool surface | 7 | 5 | Batches A–F (6) |
| 3A — GUI reorg execution | 12 | 24 | Batches A–D (4) + Bundle 7 gate |
| 3B — VoxMens/Populi GUI (FULL) | 24 | 11 | multi-batch fan-out (largest parallel surface) |
| 3C — settings consolidation | 8 | 9 | Batches A–D (4) |
| 3D — GUI caveat completions | 15 | 18 | Batches 1–6 (6) |
| 3E — frontend-emit validation gate | 4 | 1 | Batches A–D (4) |
| 3F (P6) — CLI-governance surfaces | 15 | 7 | Batches 1–4 (4) |

> Counts are raw `[PARALLEL-SAFE]` / `[SEQUENTIAL]` task tags per plan; read each
> plan's **Workflow Batch Plan** table for the exact per-batch task membership before
> dispatching concurrently.

---

## 5. Dispatch summary

1. **Start vs1** (engine spine) and, in the GUI track, **3A** — both have no
   predecessors. 3B and 3E may also start immediately (independent).
2. Once **vs1 Batch 1** (tool names final) merges → fan out **vs2, vs3, vs5** in
   parallel; begin **vs4 Phases A/B/D** alongside vs3.
3. Once **vs3** lands `SearchCorpus::GraphifyNodes` → unblock **vs4 Phase C**.
4. Once **3A Bundle 7 gate (Task 7.1)** is green → fan out **3C, 3D, 3F**.
5. Land **3E** any time; pair with **vs2** for the frontend-emit class (§10.4).
