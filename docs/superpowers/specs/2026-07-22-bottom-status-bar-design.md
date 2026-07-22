---
title: Bottom status bar — move TopHud's configurable tile system down, VS-Code style
status: approved
---

# Bottom status bar — design

## Context

Vox Axis's header currently stacks `TopHud` (KPI tiles + command palette trigger), `BreadcrumbBar`, and `StatusBar` (a single-row KPI strip: Agents/Queue/Budget/Mesh/Model + freshness pill + the Chat surface's portaled "Panels ▾" trigger), all in `AppShell.tsx`'s header block. The user wants the "dashboard" moved to a VS-Code-style **bottom** bar: one line, configurable (add/remove items via a menu, same interaction as the Chat surface's existing "Panels ▾" checkbox dropdown), mesh-aware, and — since `StatusBar`'s current trailing slot (Chat's Panels ▾ portal target) would lose its home if `StatusBar` itself moves — that trigger needs a new home too.

Investigation found more reusable infrastructure than expected:
- `TopHud` already has a real, working add/remove/reorder configuration system — `useHudTiles.ts`'s `HudTilesConfig`/`toggleHudTile`/`reorderHudTile`, persisted via `useLocalStorage` under `gui.hud.tiles.v1` (`shellPersistence.ts:11`), edited today via a `HudTilesEditor` component buried in Settings (`SettingsView.tsx:953-957`). This is exactly the "add or remove from" mechanism asked for — it's just not exposed at the point of use.
- `StatusBar`'s Mesh figure (`kpis.mesh.peers`) is real live data (`meshKpiFromStatus`, sourced from the orchestrator daemon's actual peer list), not a placeholder — but it's a bare count, while `MeshView.tsx` already polls richer per-node data (online/maintenance/quarantined status, queue depth) every 5s via `vox_mesh_nodes`/`vox_mesh_queue_stats`.
- The Chat surface's "Panels ▾" menu (`ChatSurface.tsx:950-1038`) is a proven template: trigger button → absolutely-positioned dropdown of checkboxes bound directly to live state, live-apply on every click (no separate "Apply" step), stays open across multiple toggles, closes only on outside-click/Escape/an explicit reset action. It's *also* already portaled into shared shell chrome (`StatusBar.tsx`'s `#workbench-tabbar-trailing-slot`) rather than rendered inline — precedent for putting a similarly cross-cutting trigger into new shared chrome.
- A real, unrelated latent bug was found in passing: `TopHud.tsx:198`'s mesh tile navigates to `'compute'` on click, but no `'compute'` top-level nav parent exists in `lib/navigation.ts` (mesh actually lives under `agents` → `mesh`). This is fixed as part of this spec since the mesh tile is central to the new bar's mesh-awareness requirement.

Per the user's approved choice: **TopHud's tile-config system moves to the bottom and absorbs StatusBar's data**, rather than StatusBar moving down as-is. The mesh indicator should show richer status (an online/total count or queue-depth figure), not just a bare peer count.

## Approach

### 1. New `BottomStatusBar` component, anchored on the existing `useHudTilesConfig`

A new `crates/vox-gui/ui/src/components/layout/BottomStatusBar.tsx` renders as a single, fixed, non-scrolling row at the bottom of the app shell (sibling to `<main>` in `AppShell.tsx`, not nested inside the per-surface scroll area). It consumes the *same* `hudTilesConfig`/`onHudTilesChange` (from `useHudTilesConfig()`, already wired in `App.tsx`) that today's Settings-page `HudTilesEditor` edits — no new config state, no new persistence key. `StatusBar.tsx`'s five KPI segments (Agents/Queue/Budget/Mesh/Model) become `HudTileKind` entries in the same config system (today, `STATUS_BAR_TILE_KINDS` explicitly *excludes* these from `TopHud`'s own render — that carve-out is removed since there's no more separate `StatusBar` to own them). `TopHud.tsx` and `StatusBar.tsx` are both retired; their render logic (the actual per-tile JSX: `renderTile()`'s cases, `Segment`'s layout) is consolidated into `BottomStatusBar.tsx`, condensed to a compact one-line "label: value" form factor matching `StatusBar`'s existing visual density (not `TopHud`'s larger tile-card style) — density comes from `StatusBar`, configurability comes from `TopHud`'s system.

### 2. Configurability menu — reuses the Panels ▾ pattern exactly

A trigger button at the bottom bar's right edge (mirroring VS Code's own status-bar right-click-for-options convention, but as a persistent visible trigger rather than a right-click context menu, since a discoverable click target is more consistent with this app's existing left-click "Panels ▾" convention than a hidden right-click) opens a dropdown: one checkbox per `HudTileKind`, bound directly to `hudTilesConfig`, live-apply on toggle (calls `toggleHudTile`/`onHudTilesChange` immediately), stays open across multiple toggles, closes on outside-click/Escape — structurally identical to `ChatSurface.tsx`'s existing Panels ▾ implementation, reusing that component's actual interaction code where practical (extract a shared `LiveApplyCheckboxMenu` primitive if the implementation plan finds enough literal duplication to justify it; a thin copy is also acceptable per YAGNI if extraction would touch Chat's already-hardened implementation unnecessarily — implementation-time judgment call, not a design requirement either way).

The Settings-page `HudTilesEditor` is **not removed** — it remains as the "full" editor (with reordering, which the compact dropdown does not need to support at menu-open time) but now reads/writes the same underlying config the bottom bar's own menu does, so changes in either place are immediately reflected in both.

### 3. Chat's "Panels ▾" trigger gets a new home

Since `StatusBar.tsx` (today's home for the `#workbench-tabbar-trailing-slot` portal target) is retired, the Panels ▾ trigger's portal target moves to `BottomStatusBar.tsx` — same portal mechanism (`document.getElementById('workbench-tabbar-trailing-slot')`), just a different host component owning that DOM id. This preserves everything the earlier "move Panels trigger out of the wrapping tab bar" fix (`0b3e7b06fc`, done earlier this session) established: a persistent, non-wrapping, always-reachable home. This spec's `BottomStatusBar` becomes that persistent home instead of the (now also being retired, per the separate nav-shell-redesign effort) top area.

### 4. Mesh indicator uses richer real data

The `mesh_peers` tile's rendering (wherever it lands post-consolidation) is changed from a bare `${peers} peers` string to an online/total figure or queue-depth summary sourced the same way `MeshView.tsx` already gets its data (`vox_mesh_nodes`/`vox_mesh_queue_stats`) — polled at a cadence appropriate for a persistent one-line bar (likely reusing `MeshView`'s existing 5s `REFRESH_MS`, not inventing a different cadence, to avoid two independently-polling mesh-data sources drifting out of sync with each other visually). Clicking the mesh figure navigates to the real `agents` → `mesh` view (fixing the `'compute'` dead-route bug found during investigation), not the current broken target.

## What this does not include

- No change to the underlying mesh RPCs (`vox_mesh_nodes`/`vox_mesh_queue_stats`) themselves — this only changes which existing data feeds the bar.
- No removal of the Settings-page tile editor — it stays as the full-featured configuration surface.
- No mobile-specific layout for the bottom bar (consistent with this session's broader "not needed now" mobile stance established during the navigation-shell redesign work).
- Reordering support in the compact dropdown menu (only in the full Settings editor) — the dropdown is add/remove (checkbox) only, matching Panels ▾'s own scope.

## Testing

Component tests for `BottomStatusBar` cover: tile visibility reflects `hudTilesConfig`, toggling a checkbox in the dropdown immediately shows/hides that tile (live-apply, no separate test for a nonexistent "Apply" step), the dropdown stays open across multiple toggles, mesh figure renders the real online/total or queue-depth value (mocked `vox_mesh_nodes`/`vox_mesh_queue_stats` response, not a hardcoded string), and clicking the mesh figure navigates to `agents`/`mesh` (regression-guarding the bug fix). Live CDP verification (this session's established practice) confirms the bar renders at the bottom, doesn't get clipped by the app-shell scroll fixes from earlier work, and the Panels ▾ trigger is still reachable/functional from its new portal home.
