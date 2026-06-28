---
category: "Architecture SSOTs"
title: "Vox Graph + Omnibar + Task-Monitor Dashboard (2026-06-27)"
date: 2026-06-27
status: design
---

# Vox Graph + Omnibar + Task-Monitor Dashboard

Amendment to the **Vox Search** program (`docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md`). It does three things that share one spine — the structural graph the GUI already produces:

1. **Vox Graph** — finish retiring the word *graphify*; the structural graph engine becomes a named layer (**Vox Graph**) inside Vox Search, plus a callable Vox skill, plus its files.
2. **Omnibar** — one global top-bar search over everything the GUI reveals (surfaces, subsections, on-screen text) + optional docs + the graph's "relates-to" facet, tied to `vox_search`.
3. **Task-Monitor Dashboard** — a Resource-Monitor-style, composable widget grid for things users watch continuously, with purpose-built widgets where they pay and automatic live-mini-render fallback everywhere else.

This spec assumes its base is `origin/main` (GUI honesty merged) and that it rebases onto the Vox Search vs1 rename before execution.

---

## 0. Grounding (measured, not assumed)

Fresh `vox graph rebuild --corpus vox-gui-surface` + `coverage` on this worktree:

| Metric | Value |
|---|---|
| Nodes / edges | 1337 / 1388 |
| Kinds | fn 1006 · command 161 · struct 126 · **surface 40** · tool 4 |
| Surfaces mapped | **40** (33 dirs + 7 alias/registry rows) — all surfaces covered |
| Zero-edge islands | 449 (33%), down from 51% pre-Plan-1 |
| Coverage classes | `surfaced` / `orphan_backend` / `dead_end` |

The engine works and maps the whole GUI. The `dead_end` class (e.g. `cmd:vox_skill_*` — backend commands that reach no surface) plus `graphify-out/gui-coverage/cli-governance.json` already compute **the CLI↔GUI gap** — the list of commands that *need* a GUI surface. That artifact is the data source for the dashboard's coverage widget and the Omnibar's command facet; nothing new is invented to find the gap.

---

## 1. Vox Graph — naming and skill

**Decision:** Vox Search stays the umbrella retrieval service (CLI `vox search`, `vox_search_*` MCP). **Vox Graph** names only the structural graph engine + the callable skill + its files. The vs1 `graphify → search` rename is unchanged; this removes the residual word *graphify* the vs1 plan deliberately left internal.

Rename map (additive to vs1):

| From | To |
|---|---|
| crate `vox-graphify-reader` | `vox-graph-reader` |
| `graphify_tools.rs` / new `search_tools.rs` | (vs1 already moves new tools to `search_tools.rs`; this renames the legacy file `graph_tools.rs`) |
| `.vox/cache/graphify/` | `.vox/cache/vox-graph/` (with one-release back-compat read of the old path) |
| `graphify-corpora.v1.yaml` | `vox-graph-corpora.v1.yaml` |
| CLI group `vox graphify <verb>` | folded into `vox search graph <verb>` (the graph subgroup of search); `vox graphify` kept as a deprecation alias one release |
| `useGraphifyStatus.ts`, `GraphifyStatusPanel.tsx` | `useVoxGraphStatus.ts`, `VoxGraphStatusPanel.tsx` |

**Vox Graph as a Vox skill.** A pinned skill `vox-graph` under `assets/skills/` (the auto-hydrated root) exposes the graph to any agent on any harness: *"to discover how code relates, call `vox_search` / `vox_discover` (graph-first) before grep."* This is the same steering vs5 specs; this spec only fixes the name and guarantees the skill ships graph-first discovery as the default retrieval move. Determinism firewall is unchanged: the structural graph stays deterministic and read-only; overlays remain query-time and provenance-labeled.

`ponytail:` the rename is mechanical; the only non-trivial part is the `.vox/cache` path back-compat read — one fallback branch, one release, then delete.

---

## 2. The hybrid content index (Vox Graph feeds it)

The Omnibar must search "any text revealed in the GUI." Two cooperating sources, merged at query time:

**2.1 Build-time content manifest** — `gui-content-manifest.json`, produced by the **same Vox Graph walk** that builds the surface graph. For each surface it extracts: surface label + route, subsection headings, static on-screen copy, the command names it invokes, and (optional, flag-gated) linked doc titles. Deterministic, ships with the build, **regenerates for free when a surface is added** (it rides the existing walk). Misses values that exist only at runtime.

Two consistency notes the VG-1/VG-2 plans must honor:
- **Headings are keyed off the graph's surface→module edge, not a TSX-filename heuristic.** The Vox Graph already maps each surface to its component module; subsection headings must join on that node id (so multi-word kebab view keys like `sub-agents` / `vox-search` resolve correctly), never on a `stem.replace("view","")` filename guess. The manifest golden test must cover at least one multi-word view key with a real heading.
- **The `docs` field is always emitted (possibly `[]`).** VG-2's `ContentManifestEntry` declares `docs: string[]`, so the manifest artifact must always emit a `docs` array even when empty/flag-off. The Omnibar's live **DOCS facet is sourced from the federated docs index, not this field** — the manifest `docs` is the optional flag-gated linked-title set, kept present so the consuming type matches the artifact.

**2.2 Thin runtime registry** — a `useSearchable(entries)` hook. A surface that has watch-worthy *dynamic* text (live counts, fetched rows, streamed status) registers those strings while mounted; the registry is a plain in-memory map keyed by surface. The Omnibar queries the registry alongside the manifest.

`ponytail:` the registry starts as a no-op map. Only surfaces with genuinely dynamic, searchable text opt in — do **not** instrument all 33 up front. A surface with nothing dynamic contributes only its manifest rows.

**2.3 Query path.** Omnibar query → `vox_search` (corpora + the manifest as a corpus) **and** the runtime registry **and** `vox_discover` for the graph facet → merge → rank → faceted results. The graph facet ("relates to: …") is the one thing only Vox Graph can answer and is labeled as such.

> **`vox_discover` contract (canonical = master spec §2.6/§3.1).** The GRAPH facet must consume the real tool shape, **not** an invented `result.neighbors[]`/`view_key` shape:
> - **Output:** `{ seeds[], results[{ node_id, fused_score, components, hops, community, reachability_class, provenance }] }`. There is **no `result.neighbors`, no `view_key`, no `label`** — map `node_id` → id and derive the view key from the `surface:<vk>` node-id prefix.
> - **Input:** `{ query, corpus?, radius=1, community_scope?, mode: "auto"|"search_seed"|"structure_only", limit }`. There is **no `seed` param and no `mode:'expand'`** — neighbor expansion (the ⌥→ affordance) uses **`vox_search_neighbors({ corpus, node_ids, max_depth })`**, the actual neighbor primitive, not a re-seeded `vox_discover`.
>
> The VG-2 plan's GRAPH-facet parser and ⌥→ expansion are reconciled to this contract in the apply phase.

---

## 3. The Omnibar (top bar, global)

**3.1 Placement — top bar, not sidebar (design-principle grounded).** Search is a *query* affordance; the sidebar is a *navigation* affordance — putting a query field in the sidebar conflates them and fights the sidebar's collapse. Search wants a wide target (results, facets, preview); the top bar is wide, the sidebar narrows. Convention (Linear, VS Code, GitHub, Raycast) is top-anchored + ⌘K; meeting it is the lazy-correct choice. The Omnibar lives in the top bar on **every** view.

**3.2 Consolidation.** Today there are three overlapping entry points — `CommandPalette.tsx` (⌘K, already global, already seeds `vox_search`), the dedicated `Search` *surface* (`SearchView.tsx`, the "awkward placement"), and per-view inputs. The Omnibar **replaces all three**: delete the `Search` surface (migration-ledger redirect `#view=search` → open Omnibar), fold `CommandPalette` into the Omnibar, and leave per-surface filters where they are (they filter one surface; the Omnibar searches everything).

**Final surface-key ownership.** Three surface keys touch this work — `graphify`, `search`, `vox-search`. To prevent a double-rekey, ownership is fixed: **`graphify` → `vox-search` is owned by vs1** (the rename); the transitional `case 'graphify'` arm is removed once vs1 re-keys, and VG-1 G10 **consumes** that re-key rather than re-keying again. **`search` → Omnibar redirect (`#view=search`) is owned by VG-2** (§3.2). `vox-search` is the live unified surface key from vs1. This table is mirrored in INDEX §2.3; if they disagree, INDEX §2.3 + vs1 win.

**3.3 Behaviour.** A slim persistent field, top-right; ⌘K (or click) expands it into a faceted palette:

```
SURFACES    › Approvals · Needs-You                      → navigate
COMMANDS    › vox_resolve_approval · vox_pending_…       → run / route
ON SCREEN   › "3 pending approvals" (Activity)           → navigate + scrollIntoView
GRAPH       › relates to: Chat rail, doubt_task   ⌥→     → expand neighbors (Vox Graph)
DOCS        › approvals-workflow.md                      → open doc (optional facet)
```

`Enter` activates the top hit (navigate / run / scroll-to); `⇧Enter` sends the raw query to chat. Facets come straight from §2. Each facet is independently capped and labeled with provenance (corpus vs manifest vs runtime vs graph).

**3.4 Audit of the existing search (capabilities vs needs).** `searchController.ts` → `vox_search_query` with scope chips is real and reused. Gaps the Omnibar closes: (a) it searched backend corpora only — now also manifest + runtime revealed text; (b) no graph facet — now `vox_discover`; (c) no docs facet — now optional; (d) three entry points — now one.

---

## 4. The Task-Monitor Dashboard

Use-case (user's words): like Windows Resource Monitor / Task Manager — what you watch continuously or over time, including the back-and-forth feedback of background work. Reuses the existing dockable-workspace tiling.

**4.1 Two render paths with automatic fallback.** Each widget slot renders either a **purpose-built compact widget** (if one is registered for that surface/metric) or a **live mini-render** of the real surface component in a `compact` mode. A newly added GUI surface **auto-appears** as a mini-render fallback — driven off the surface registry, no dashboard edit. This is the "dynamically expandable" requirement: the registry is the single source; the dashboard subscribes to it.

**4.2 What earns a purpose-built widget** (by info-scale × thumbnail legibility × real-time-watch value):

| Purpose-built | Why | Falls back to mini-render |
|---|---|---|
| Agent runs / streams | high-rate background feedback to watch live | Settings, Policies (static config) |
| Cost / spend | one number + sparkline, legible at thumbnail | reference / doc surfaces |
| Mesh | peer count + health over time | one-shot action surfaces |
| Approvals / doubt feedback | the operator back-and-forth | low-change list surfaces |
| Coverage / build-spine | trend over time | — |

Everything not in the left column falls back automatically and gracefully.

**4.3 Core monitorables + minimized strip.** A always-available set — mesh peers · agents running · OpenRouter spend · queue depth · pending approvals — rendered as a thin **minimized strip** that can sit atop any view (the task-monitor row). Every core widget is **disableable**; a hidden one drops out of the strip too. All of this is configurable and the config lives in Settings (per the Settings-consolidation plan 3C), not a bespoke dashboard settings island.

**4.4 Subdivision.** The dashboard is sectioned (e.g. *Operations* / *Cost* / *Knowledge* / *Surfaces*) so the minimized strip can pull one row per section without losing the expanded grid. Sections are derived from the surface registry's groups, so they stay in sync as surfaces move.

---

## 5. Architecture & isolation

| Unit | Does | Depends on |
|---|---|---|
| `vox-graph-reader` (renamed) | structural graph + manifest emission | tree-sitter, syn (unchanged) |
| `gui-content-manifest.json` (build artifact) | searchable static text per surface | the Vox Graph walk |
| `useSearchable()` hook + registry | live dynamic text registration | none (in-memory) |
| Omnibar component | faceted query UI in the top bar | `vox_search`/`vox_discover`, registry, manifest |
| Dashboard registry subscriber | maps surfaces → widget (purpose-built or mini-render) | surface registry |
| Widget gallery + layout | add/remove/arrange, persisted | Settings (3C) |

Each is independently testable: the manifest is a pure function of the walk; the registry is a map; the Omnibar is a component over three injected sources; the dashboard is a function of the registry + a layout config.

---

## 6. Error handling & honesty

- Omnibar facets fail **independently** — if `vox_discover` errors, the GRAPH facet shows an honest empty/error row; SURFACES/ON-SCREEN still work. No facet's failure blanks the bar.
- Mini-render fallback wraps each widget in an error boundary; a broken surface renders a compact error tile, not a crashed dashboard.
- No honesty-gate regression: the Omnibar's ON-SCREEN/GRAPH results are real (manifest/registry/graph), never fabricated; the dashboard widgets render real data or an explicit empty state (subject to the existing `vox ci gui-honesty` scanner).
- **Provenance-axis mapping (firewall consistency).** The Omnibar's facet badges name a *UI source* (`corpus` / `manifest` / `runtime` / `graph` / `docs`); the master firewall classifies determinism on a *structural vs overlay* axis. They must agree: `manifest`, `corpus`, and `runtime` rows are deterministic/static reads → **structural**; the `graph` facet (`vox_discover` fusion) is a query-time overlay → it must be labeled **overlay / derived**, never implied deterministic. The Omnibar's GRAPH badge therefore renders as an explicitly derived result, consistent with the master spec's `structural | overlay` provenance and `declared | heuristic | resolved` confidence vocabulary.

---

## 7. Testing

- **Vox Graph rename:** the existing graph tests pass under new names; a back-compat test reads the old `.vox/cache/graphify/` path one release.
- **Manifest:** golden test — add a fixture surface, assert its label/headings/commands appear in `gui-content-manifest.json` after a walk.
- **Omnibar:** vitest — query returns merged faceted results from injected manifest + registry + a mocked `vox_discover`; `Enter`/`⇧Enter`/`⌥→` routing; deleting the `Search` surface redirects.
- **Dashboard:** a new surface auto-appears as a mini-render fallback (registry-driven); a purpose-built widget overrides the fallback; disabling a core widget removes it from the strip; error boundary renders the compact error tile.

---

## 8. Scope / decomposition

Three shippable plans, in dependency order:

1. **VG-1 — Vox Graph rename + skill + content-manifest emission** (extends vs1; the manifest is the new capability).
2. **VG-2 — Omnibar** (top-bar, faceted, consolidates Search surface + CommandPalette; hybrid index from VG-1).
3. **VG-3 — Task-Monitor Dashboard** (registry-driven widgets + purpose-built shortlist + fallback; config in Settings/3C).

VG-1 → VG-2 (Omnibar needs the manifest). VG-3 is independent of VG-2 (shares only the registry). All three land as amendments to the Vox Search program and rebase onto its vs1 rename.
