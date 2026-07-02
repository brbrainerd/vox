---
title: "Vox Urbs — Ludus Codebase Visualizer Rebuild Design"
description: "Full rebuild of the Ludus Sandbox into a Roman-styled, LOD isometric city visualizing the whole workspace and all live harness activity (agents, diagnostics, builds, cost, CI fleet, orchestrator queue, MCP, git)."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Defines the world model, LOD rendering architecture, telemetry mappings, and honesty rules for the rebuilt codebase visualizer."
---

# Vox Urbs — Ludus Codebase Visualizer Rebuild Spec

**Date:** 2026-07-02
**Status:** Design (approved in brainstorming session)
**Supersedes-in-part / builds on:**
[gamified-codebase-representation](2026-06-18-gamified-codebase-representation-design.md) ·
[gamified-engine-and-performance](2026-06-18-gamified-engine-and-performance-design.md) ·
[gamification-surfacing-and-minimap](2026-06-18-gamification-surfacing-and-minimap-design.md) ·
[gamified-full-simulation-and-wiring](2026-06-18-gamified-full-simulation-and-wiring-design.md)

---

## 1. Problem

The shipped `LudusSandbox` (`crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx`) is a
crude partial skeleton of the 2026-06-18 spec suite, which was written but never executed:

- **No pan/zoom.** The only camera movement is an auto-focus on `focusedFile`; there are no
  drag, wheel, or pinch handlers.
- **Cut off and mis-centered.** A fixed 800×500 canvas attribute is CSS-stretched to fill a
  500px container that itself sits inside a 450px parent (`GamifyView`), with hardcoded
  camera offsets (`x: 400, y: 100`) and a hardcoded offscreen center (`1000, 100`).
- **Primitive graphics.** Buildings are 6px circles; warnings are two 4px rectangles;
  scaffolding is two crossed lines.
- **Abstract layout.** Files are placed on a sunflower spiral (`assignPlotCoordinates`)
  with no relationship to directory structure.
- **Mock data.** Hardcoded `treasuryValue={120}`, `energy={90}`, a fake warning injected on
  every `file_edited` event with a hardcoded fallback path, and a speech bubble pinned to
  fixed pixel coordinates. `HudPanels` renders from fake props.

## 2. Goal

Rebuild the visualizer as **Vox Urbs**: one continuous Roman-styled isometric city that
shows the entire workspace and everything happening in the harness, at 60fps, with every
pixel backed by a real data stream.

Decisions locked during brainstorming (2026-07-02):

| Decision | Choice |
|---|---|
| Visual direction | Isometric town (execute the Ludus spec suite properly) |
| Art style | Roman, matching the Limes design language (Cinzel/basalt/gold; basalt flagstones, marble temples, gold pediments, villas/insulae, citizens, denarii) |
| Map scale | Level-of-detail city: whole workspace, one world, districts dissolve into file buildings on zoom |
| Scope | Full spec: map + animated citizens + wired HUD + quest board + minimap surfacing |
| Harness coverage | Full harness world: agents, diagnostics, builds, cost, **plus** CI fleet, orchestrator queue, MCP servers, git branches/PRs |

## 3. World model & layout

- **Deterministic treemap on the iso grid.** The directory tree drives a slice-and-dice
  treemap: each crate gets a district plot sized by file count; files get building plots
  inside their district via the same recursion. Tie-breaking is seeded by path hash, so
  **placement is stable across sessions and rebuilds** — a file keeps its address. New
  files created mid-session take the nearest open plot in their district (per the original
  spec's dynamic-assignment rule).
- **Landmarks.** Graphify god nodes (read from `graphify-out/` when present; when absent,
  fall back to the largest-degree crates by workspace dependency count) render as landmark
  temples with gold pediments sized by crate size. The current worktree's focused crate
  gets a subtle gold district boundary.
- **Roads** are generated along district boundaries and form the graph citizens path over.
- The spiral `assignPlotCoordinates` is **deleted**.

### 3.1 Harness landmarks (fixed positions at the city edge)

| Landmark | Represents | Visual encoding |
|---|---|---|
| **CASTRVM** (fort, outside the walls) | CI runner fleet | Tents = registered runners; red tents = busy; legion standard raised when a run is in progress |
| **PORTVS** (harbor, on the sea) | Orchestrator queue | Ships waiting = queued tasks; ship docks/departs on dequeue/complete |
| **AQVAE** (aqueduct entering the city) | MCP servers | One arch span per connected server; water-flow pulses = traffic; dry/cracked span = server down |
| **Gates & roads** | Git | Dashed "via nova" roads under construction = local branches, with real ahead/behind from `git for-each-ref %(upstream:track)` shown as construction progress; caravans arriving at the gate = open PRs. (Worktree enumeration deferred.) |

## 4. Rendering architecture

Keeps the specced hybrid (see engine spec), extended with LOD:

1. **Canvas 2D offscreen buffer** for the world (terrain, districts, buildings, overlays);
   re-rendered only on data change or LOD band change — never per frame.
2. **DOM overlay** for citizens, speech bubbles, radial menus, tooltips — absolutely
   positioned, `zIndex = floor(x + y)` for depth ordering.
3. **Zustand store** (`vox-gamify-store`) with direct `store.subscribe` + `ref.current.style`
   updates in a rAF loop — no React re-render per tick.
4. **LOD threshold on zoom.** Below the threshold, each district renders as a single
   aggregate landmark (~109 draw calls for the whole workspace). Above it, every file
   building is painted **once into the full-world offscreen buffer**; the per-frame cost
   is a single camera-transformed blit, so pan/zoom never re-draws buildings. (Deliberate
   deviation from per-frame viewport culling: the buffer amortizes better and is simpler.
   The buffer is clamped via a render-scale factor to bound memory, and animations —
   fires — draw in the blit pass, never into the buffer, so buffer repaints stay rare.)
5. **Procedural-parametric Roman sprites.** Building art (columns, pediments, roofs,
   weeds, fire frames) is drawn parametrically from file metrics into a **runtime sprite
   atlas** once, then stamped onto the buffer. No checked-in image assets; crisp at every
   zoom because atlases are regenerated per LOD band / DPR.

### 4.1 Visual encodings (all from real data)

| Signal | Source | Visual |
|---|---|---|
| File size | line count from the workspace scan (quartile within its district) | Building tier: hut → villa → insula → temple |
| Warnings | file diagnostics | Weeds/ivy on the plot |
| Errors | file diagnostics | Animated fire sprite on the building |
| Active agent task | agent task stream | Citizen at the building + wooden scaffolding |
| Churn (recent edits) | edit events | Warm torchlight tint (**deferred** — requires an edit-recency store) |
| FSRS overdue actions | discovery ledger (`gamify_due_actions`) | SENATVS panel due list (per-building glow **deferred** — `DueActionDto` lacks a file path) |
| Cost | `get_llm_spend` via the `useLlmSpend` hook — same source as the Office cost widget | AERARIVM treasury count; drain animates on `CostIncurred` |
| Build progress | `AgentEventKind::BuildStage` on `vox://agent-events` | FABRICA HUD chip naming the active stage (lex/parse/hir/typecheck/codegen) |

## 5. Camera (the bug fix)

A proper camera controller replaces the hardcoded offsets:

- Drag-to-pan, wheel-zoom **centered on the cursor**, pinch support.
- Zoom and pan clamped to world bounds.
- Canvas sized to its container via **ResizeObserver** with `devicePixelRatio` scaling —
  eliminates the fixed-attribute stretch, the blur, the mis-centering, and the 450px/500px
  container mismatch in `GamifyView`.
- Double-click a district → zoom-fit it. A "fit world" home button.
- Auto-focus animation when an agent starts a task — **interruptible; user input always
  wins over auto-camera**.
- Screen↔world math is a single pure module shared by rendering and hit-testing (the
  current duplicated `centerOffsetX = 1000` constants are removed).

## 6. Citizens

- Agents and the developer render as citizens (Roman dress, existing mood system).
- **A\* pathfinding over the road grid**; states: `Idle` → `Commuting` → `Working`
  (hammer/scroll animation) → `Exhausted`. (The budget-lockout *trigger* for `Exhausted`
  is deferred until a lockout signal exists in the GUI; the state machine ships.)
- Moods from task phase via the existing `moodFromPhase` mapper.
- Speech bubbles anchor to the citizen's projected position (the hardcoded
  `top-[180px] left-[380px]` bubble is deleted).

## 7. Telemetry

### 7.1 Existing feeds (wire directly, no new backend)

- Agent tasks/phases (`vox://agent-events`) → citizens, moods, scaffolding, auto-focus.
- File diagnostics (`FileDiagChanged`) → weeds/fire/cracks.
- Cost: `get_llm_spend` (via the existing `useLlmSpend` hook — the same source as the
  Office cost widget, so AERARIVM is never an independent sum); `CostIncurred` events
  animate the drain. (`budget_get`/`vox://cost-changed` **do not exist** — they are
  unexecuted items of the 2026-06-18 budget-SSOT spec; migrate to them when that lands.)
- Quests + FSRS due (`gamify_due_actions`) → SENATVS quest board.
- Orchestrator queue (existing `orchestrator.rs` commands) → PORTVS ships.
- MCP: **no command surface exists** (`mcp.rs` has only `invoke_mcp_tool`, and
  `get_orchestrator_status` serializes a closed struct that can never carry an MCP field)
  → AQVAE renders **unconditionally unlit** until a dedicated server-list command lands.
- Build progress: `AgentEventKind::BuildStage` (already on the consumed
  `vox://agent-events` channel) → FABRICA HUD chip.
- Scientia/discovery activity (`vox://scientia-queue`, `vox://scientia-discovery-surfaced`)
  is explicitly **out of scope** for this rebuild — it has its own surface.

### 7.2 New thin taps (the only new backend)

| Command | Impl | Feeds |
|---|---|---|
| `harness_ci_fleet_status` | `gh api repos/<slug>/actions/runners` + queued-runs count — the same source `vox-cli`'s `runner_scale.rs` reads | CASTRVM |
| `vcs_town_status` | Local git: `git for-each-ref` with `%(upstream:track)` (branches + real ahead/behind); open PRs via `gh` **when available**, gracefully absent offline. Worktree enumeration deferred. | Roads, caravans |

Both poll at slow cadence (15–30s), no filesystem watchers. Spawned processes must use the
existing `CREATE_NO_WINDOW` quiet-command helper on Windows.

### 7.3 Deleted mocks

The `file_edited` handler's hardcoded fallback path (`crates/vox-db/src/lib.rs`), the fake
`warnings: 1` injection, `treasuryValue={120}`, `energy={90}`, and the no-op `onSetSpeed`
are all removed.

## 8. HUD & gamify wiring

- `HudPanels` already renders from props (the surfacing spec's "returns null" is stale);
  the defect is the **mock props at the call site** (`treasuryValue={120}`, `energy={90}`)
  — rewire to real spend and profile energy.
- The energy persist-back bug is **already fixed**: `get_ludus_profile_impl` regens and
  upserts in `crates/vox-gui/src/commands/gamify.rs`, guarded by the existing
  `energy_regen_persists_to_db` round-trip test. Not rebuild scope.
- Sim speed (Ⅰx / Ⅲx / pause) affects **animation speed only** — Vox Urbs is a view of
  real telemetry, not a simulation that can be fast-forwarded.
- Building click → radial menu with **real actions only**: open file, focus agent, view
  diagnostics. The spec'd "dispatch refactor subagent" action ships only if the existing
  orchestrator dispatch command supports it end-to-end; otherwise the menu item is absent
  (not disabled, not stubbed).

## 9. Surfacing

One component, two render modes (existing `EmbeddedSurfaceContext` mechanism):

- **Embedded mini-map** — dashboard widget and the Gamify surface panel; single initial
  fetch, no poll loop (already implemented), reduced LOD (always aggregate landmarks).
- **Immersive full surface** — the deep-link target; full LOD, full HUD, quest board.

## 10. Honesty & error handling

Per the project anti-stub rule (**only surface what is real**):

- A landmark whose data tap fails renders **unlit** (visually distinct) with a tooltip
  stating why (e.g., "gh unavailable/unauthenticated", "git unavailable") — never fake
  numbers, and never a fabricated placeholder value in a real-sounding field.
- Event channel dropout → "SIM PAVSED" banner, last state retained, auto-reconnect.
- Empty workspace → empty plaza; no agents → idle city; no crash paths.
- Energy upsert failure → log and return the in-memory value (don't fail the fetch).

## 11. Performance targets

- 60fps pan/zoom on the full workspace (~109 crates, ~4–5k files) — enforced by LOD
  banding + the full-world offscreen buffer (repainted only on data/LOD change; fires
  animate in the blit pass) + render-scale clamping of the buffer.
- Layout assignment for 5,000 files < 5ms (extends the engine spec's 1,000-file target).
- Buffer redraw only on data/LOD change; overlay updates via rAF-clamped direct DOM writes.

## 12. Testing

- **Rust:** `harness_ci_fleet_status` / `vcs_town_status` / `workspace_town_scan` parsing
  and DTO mapping against fixtures (no live network in tests). (The energy persist
  round-trip already exists: `energy_regen_persists_to_db`.)
- **TS (vitest):** treemap determinism, stability under file add/remove, and
  **completeness under skewed crate sizes** (no silently dropped files); screen↔world
  camera math round-trips at arbitrary zoom/DPR; LOD band selection; redraw-key
  discipline (camera and animation frames excluded); existing `moodFromPhase` /
  `integrityFromDiag` mappers; `HudPanels` renders real props; honesty states (unlit
  landmark on tap failure).
- **Perf guards:** 5,000-file layout < 5ms; buffer redraw count assertions (no redraw on
  camera-only change).
- **Manual:** edit a file → citizen commutes and scaffolding appears; introduce a compile
  error → fire on the right building; kill the `gh` path → caravan gate unlit with tooltip.

## 13. Decomposition preview (plan tasks)

1. Camera + canvas sizing module (pure math + ResizeObserver/DPR) — fixes cut-off/centering/pan/zoom.
2. Treemap layout engine + stability tests (replaces spiral).
3. Procedural Roman sprite atlas + building tiers + quality overlays.
4. LOD bands + full-world offscreen buffer + render-scale clamp.
5. Citizens: road graph, A\*, state machine, DOM overlay ticks.
6. HUD: real props at both call sites, treasury from `get_llm_spend`, FABRICA build chip.
7. New taps: `harness_ci_fleet_status` + `vcs_town_status` (Rust) + CASTRVM/PORTVS/AQVAE/gates rendering.
8. Quest board (SENATVS = the existing quests + DueNudge surfaces) + radial menu (real actions).
9. Surfacing: embedded vs immersive modes, delete mocks, wire `GamifyView`.

Rust and TS tracks are file-disjoint and parallel-safe where marked in the plan.
