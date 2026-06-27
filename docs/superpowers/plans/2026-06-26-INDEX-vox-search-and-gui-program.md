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
| `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md` | **Umbrella master spec.** Absorbs the four sibling graphify designs into one "Vox Search" service; defines P0–P8 (Vox Search = P0–P6; P7/P8 are related programs), the 5 retrieval layers, the honesty firewall, and the frontend-emit success criterion #4. The master spec §9.0 holds the **canonical plan-ID crosswalk** (P-id ↔ vs/3x-id ↔ plan-file ↔ sibling-spec) — this index references that table; do not re-derive the mapping. |
| `docs/superpowers/specs/2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint-design.md` | The executed general-enhancement + GUI-IA blueprint (P0 structural core + coverage); the foundation the master spec builds on. |
| `docs/superpowers/specs/2026-06-26-graphify-dataflow-semantic-overlay-design.md` | Data-flow / def-use layer (L4) + semantic overlay (L5): new node/edge kinds, `accumulator_never_gates` detector, separate `semantic-overlay.json`. |
| `docs/superpowers/specs/2026-06-26-graphify-voxsearch-fusion-design.md` | Graph-augmented retrieval: `vox_discover` (search-seed → graph-expand → composite re-rank), structural-overlay ranking, KnowledgeGraph corpus fix. |
| `docs/superpowers/specs/2026-06-26-graphify-agent-tool-surface-design.md` | Auto-availability + agent steering (graph-first over grep) + GUI consumption of the one MCP tool layer; layer-tool registry pattern. |
| `docs/superpowers/specs/2026-06-26-settings-consolidation-policies-unification-design.md` | Settings consolidation + Settings/Policies co-location (drives GUI Plan 3C). |
| `docs/superpowers/specs/2026-06-26-voxmens-gui-cli-parity-design.md` | VoxMens / Populi GUI ↔ CLI parity surface (drives GUI Plan 3B). |
| `docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md` | **Vox Graph + Omnibar + Task-Monitor Dashboard** — amendment to the master spec. Finishes the *graphify → Vox Graph* naming (crate `vox-graphify-reader → vox-graph-reader`, `.vox/cache/graphify → vox-graph`, a pinned `vox-graph` skill); defines the hybrid content index (`gui-content-manifest.json` build artifact + `useSearchable()` runtime registry); the global top-bar Omnibar (5 provenance facets, `vox_discover` GRAPH facet); and the registry-driven Task-Monitor Dashboard (config in Settings/3C). Drives the VG-1→VG-3 plans below. |

---

## 2. Plans (workflow-ready, TDD, per-task commits)

> **Plan-ID mapping is owned by the master spec §9.0 crosswalk** (P-id ↔ vs/3x-id
> ↔ plan-file ↔ sibling-spec). The "Master-spec scope" labels below are a
> convenience echo; if they ever disagree with §9.0, **§9.0 wins** — do not
> re-derive the mapping here. (Recall P5 = split across 3A/3D; P7↔3B and P8↔3C are
> *related programs*, not Vox Search service plans.)

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

### 2.3 Vox Graph GUI extensions (VG-1 → VG-3)

Amendments to the program from spec `2026-06-27-vox-graph-omnibar-dashboard-design.md`. All three land **on top of vs1's `graphify → search` rename** and **coordinate with the 3A/3F→3C registry chain** (no concurrent `surfaceRegistry.generated.ts` regen — see §3.2 / §3.4).

| Plan (path) | One-line purpose |
|---|---|
| _(plan file NOT YET AUTHORED)_ | **VG-1 — Vox Graph rename + skill + content-manifest emission.** **Extends vs1's rename** (crate `vox-graphify-reader → vox-graph-reader`, `.vox/cache/graphify → vox-graph` with one-release back-compat read, `graphify-corpora.v1.yaml → vox-graph-corpora.v1.yaml`, `vox graphify → vox search graph`); ships a pinned `vox-graph` skill (graph-first discovery); emits the build-time `gui-content-manifest.json` + a Tauri reader (`voxContentManifest`, modeled on `vox_docs_index`). **The manifest is the new capability VG-2 consumes.** ⚠ **No plan file on disk yet** — only the spec describes it; author before dispatch. |
| `docs/superpowers/plans/2026-06-27-omnibar-plan-vg2.md` | **VG-2 — Top-bar Omnibar.** Global faceted palette (SURFACES/COMMANDS/ON-SCREEN/GRAPH/DOCS), provenance-labeled, facets fail independently; merges `useSearchController` (`vox_search_query`) + VG-1's `gui-content-manifest.json` (via `useContentManifest`, defaults `[]` pre-VG-1) + the new no-op `useSearchable()` runtime registry + `vox_discover` (GRAPH). Consolidates the orphaned Search surface + `CommandPalette`. Registry touch is one `notes:`/redirect row authored in `surface-registry.v1.yaml` then regenerated — **never hand-edits the generated TS**. |
| `docs/superpowers/plans/2026-06-27-task-monitor-dashboard-plan-vg3.md` | **VG-3 — Task-Monitor Dashboard.** Registry-driven composable widget grid: purpose-built compact widgets for the five high-value monitorables (agents/cost/mesh/approvals/coverage) **else** an auto-fallback mini-render of any `SURFACE_REGISTRY` surface; adds `pending_approvals` to the minimized HUD strip (**config in Settings via 3C — no bespoke settings island**); error boundary → compact error tile; sections derived from registry `navGroup`. **Reads `SURFACE_REGISTRY` only — never writes the generated TS.** |

**DAG:** **VG-1 → VG-2** (the Omnibar needs the manifest; VG-2 is independently testable/landable before VG-1 via the `[]`-default `useContentManifest` hook, with the live ON-SCREEN facet lighting up once VG-1 ships). **VG-3 is independent of VG-2** — it shares **only** the surface registry and depends on neither VG-1's manifest nor VG-2's Omnibar.

```
vs1 (graphify→search rename) ──┐
                               ▼
                         VG-1 (Vox Graph rename + skill + manifest)
                               │  (gui-content-manifest.json)
                               ▼
                         VG-2 (Omnibar) ── consumes manifest

VG-3 (Task-Monitor Dashboard) ── independent; shares only SURFACE_REGISTRY
```

**Registry-chain coordination:** VG-1 **extends vs1's rename** (does not redo it). VG-2's single registry change and any VG-x registry touch must **serialize with the 3A → 3F → 3C chain** — `surfaceRegistry.generated.ts` is re-sorted on every `--write` (not append-only, see §3.2), so two concurrent regens collide. Edit the `surface-registry.v1.yaml` SSOT and regenerate; **never run a VG registry regen concurrently with 3F's or 3C's** (rebase the YAML edit and re-run the generator instead).

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
 ┌──────────┬───────────────────────────┐
 ▼          ▼                            ▼
3D         3F (P6)  ───────────────►  3C
(caveats)  (CLI-governance)           (settings)
           adds CI/Database rows      Phase 5 regen ON TOP

3B  ── independent (VoxMens/Populi GUI; no 3A dependency)
3E  ── independent (frontend-emit runtime gate)
```

- **3A → 3F → 3C** — 3F and 3C are **mutually SEQUENTIAL on the generated registry**,
  not parallel. Both regenerate the *single* re-sorted
  `surfaceRegistry.generated.ts` (which is **not** append-only — the generator
  re-sorts by `(cli_group, view_key)` on every `--write`), so two concurrent
  regens are a **guaranteed collision**. Order: **3F first** (adds the CI/Database
  + secrets/auth/cli-only rows), then **3C** (its Phase 5 reparents the `policies`
  row and regenerates on top of 3F's rows). Run 3C's Phase 5 only after 3F's
  registry writes have landed.
- **3A → 3D** stays **parallel-after-3A** — 3D's Workstream C touches surface
  markup but does **not** regenerate the surface registry, so it does not collide
  with the 3F→3C registry chain and may run alongside it.
- **3B and 3E are independent** — they touch disjoint surfaces and gate nothing in
  the reorg trilogy.

### 3.3 Cross-track reference

- **3E ↔ vs2** — same bug class (swallowed `reactive_view_emit_failures` accumulator
  that never gates the build). **3E is the runtime fix** (land now, self-contained);
  **vs2 is the structural-detection complement** (`vox_search_dead_signals` /
  `accumulator_never_gates`) that finds the class across the whole codebase and, per
  master spec success criterion #4, proves the detector against this very fixture. Sibling plans,
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

**Shared-file merge-contention points (serialize even when handler work is parallel):**

- **`dispatch.rs` + `input_schemas.rs` are appended by vs2, vs3, vs4, AND vs5.**
  Their handler/schema bodies are independent and parallel-safe, but each plan's
  **final dispatch-arm + schema-arm commit** touches these two shared files —
  **serialize those specific commits** (land one plan's dispatch/schema arm before
  the next plan's) to avoid guaranteed merge collisions, even while the rest of
  each plan runs concurrently.
- **vs4 phase split vs vs3:** vs4 **Phases A/B/D depend only on P0** and run in
  **parallel with vs3**; only **vs4 Phase C** needs vs3's `GraphifyNodes` corpus
  (see §3.1). Dispatch A/B/D alongside vs3; hold C until vs3's corpus lands.
- **P4 ↔ P5 is a (non-)edge:** **P5 uses the existing `invokeMcpTool` seam, not
  P4's new `.mcp.json`.** **P4 and P5 are independent** — neither gates the other;
  P5's dependency is on vs1's final tool names + the already-landed `voxTransport`
  seam (commit `30a46cc88d`), not on P4's harness-registration work.

---

## 5. Dispatch summary

> **Base / rebase.** Base = `origin/main` @ `063a3c3235` — the GUI honesty work is
> **MERGED to `main`** and `main` now compiles. **Rebase
> `claude/graphify-general-gui-ia` onto `main` before executing.** NOTE: the
> `GraphifyStatusPanel → voxTransport` seam **ALREADY landed on `main`** (commit
> `30a46cc88d`) — plan **P5 / vs5** and 3D must **CONSUME** it, **not redo it**.

1. **Start vs1** (engine spine) and, in the GUI track, **3A** — both have no
   predecessors. 3B and 3E may also start immediately (independent).
2. Once **vs1 Batch 1** (tool names final) merges → fan out **vs2, vs3, vs5** in
   parallel; begin **vs4 Phases A/B/D** alongside vs3.
3. Once **vs3** lands `SearchCorpus::GraphifyNodes` → unblock **vs4 Phase C**.
4. Once **3A Bundle 7 gate (Task 7.1)** is green → dispatch **3D** (parallel) and the
   **3F → 3C** registry chain in series (3F adds CI/Database rows; 3C Phase 5
   regenerates the registry on top — never run 3F and 3C's registry regens
   concurrently).
5. Land **3E** any time; pair with **vs2** for the frontend-emit class (success criterion #4).
