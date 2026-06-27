---
category: "Architecture SSOTs"
title: "VG-3 — Task-Monitor Dashboard (registry-driven widgets + mini-render fallback) (TDD)"
date: 2026-06-27
status: plan
---

# VG-3 — Task-Monitor Dashboard

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Every task is **write-through-workflow**: it ends in a concrete `git -C /c/Users/Owner/vox-graphify-gui add … && git -C /c/Users/Owner/vox-graphify-gui commit …` (add + commit only — **never** `push`, `reset`, `rebase`, `checkout --`, `clean`, or `commit --amend`). The workflow performs the final integration commit; do not branch or merge here. On a `.git/index.lock` collision, wait ~20s and retry the `git` command once.

**Goal:** Turn the existing single-purpose Dashboard surface into a **registry-driven, composable Task-Monitor**: a widget grid where each slot renders **either** a purpose-built compact widget (for the five high-value monitorables) **or** a live mini-render of the real surface component in a new `compact` mode — with a brand-new GUI surface **auto-appearing** as a mini-render fallback off `SURFACE_REGISTRY` (no dashboard edit). Add `pending_approvals` to the always-available **minimized strip** (the existing top-HUD tiles, config in Settings per Plan 3C), wrap every widget in an **error boundary** → compact error tile, and **section** the gallery (Operations / Cost / Knowledge / Surfaces) derived from the surface registry's `navGroup`. No `vox ci gui-honesty` regression.

**Architecture:** Pure GUI-frontend, additive over already-shipped infra. The existing `DashboardGrid` (`lib/dashboardLayout.ts` + `components/dashboard/DashboardGrid.tsx`), `WidgetPickerDrawer`, and the dockview-based `DockShell` tiling are **reused** — VG-3 does not re-implement tiling. The load-bearing new piece is a **widget registry** (`dashboardWidgetRegistry.ts`) that maps a slot's `surfaceKey` to one of two render paths:

1. **Purpose-built** — a registered compact widget component, for exactly: agent runs/streams · cost/spend (reuses `useLlmSpend` → `get_llm_spend`) · mesh · approvals/doubt feedback · coverage/build-spine.
2. **Mini-render fallback** — for every other `SURFACE_REGISTRY` row, render the real surface component via the existing `childRenderer(props, viewKey)` (the single home that maps `viewKey → surface component` in `surfaceComponents.tsx`) inside a `compact`-scaled, scroll-clipped, **non-interactive** wrapper. Because the catalog is **derived from `SURFACE_REGISTRY`** at render time (not a hard-coded `switch`), a new surface row auto-appears with zero dashboard edits.

The new dashboard kind family (`surface_widget`) carries a `surfaceKey` in `config`, so the existing `validateDashboardLayout` + persistence keep working unchanged; the legacy fixed kinds (`stream`/`agents`/`alerts`/…) stay as-is for back-compat. Sectioning reads `SURFACE_REGISTRY[].navGroup` and folds the registry's groups into the four dashboard sections via a pure `sectionForNavGroup()` map. The strip is the **already-shipped HUD tile system** (`useHudTiles` / `useHudTilesConfig` / `HudTilesEditor` in Settings / `TopHud`); VG-3 only **adds the `pending_approvals` kind** to that SSOT (and its render arm) — it does **not** build a bespoke dashboard settings island. Every widget render is wrapped in a small class-component `WidgetErrorBoundary` that catches a thrown child and renders a compact error tile (honest: shows the surface key + the error message, never a fabricated value).

**Tech Stack:** TypeScript/React 18, vitest + @testing-library/react (the GUI test stack already in `crates/vox-gui/ui`); `@dnd-kit/*` (already a dep, used by `DashboardGrid`); `dockview` (already a dep, `DockShell`). Strip/HUD config rides the existing `contracts/gui/hud-tiles.v1.yaml` SSOT; widget kinds ride `contracts/gui/dashboard-layout.v1.yaml`. No new npm deps. GUI package manager is **pnpm** (never `npm`). Surface-registry edits (none required for VG-3, but if a kind is added it is via `contracts/gui/*.v1.yaml` → regenerate, never hand-edit generated TS).

**Spec:** Source design — `docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md` §4 (the dashboard), §5 (architecture/isolation), §6 (error handling/honesty), §7 (testing — the four dashboard cases). Read §4–§7 first. VG-3 is **independent of VG-2** (the Omnibar) and shares only `SURFACE_REGISTRY`; it does **not** depend on VG-1's content manifest.

**Dependencies (cross-plan):**
- **Requires nothing from VG-1/VG-2.** It reads `SURFACE_REGISTRY` (already generated, on this branch) and `useLlmSpend` (already shipped). The only soft coupling to Plan 3C is the **strip config home**: the HUD-tile editor already lives in Settings (`HudTilesEditor` is threaded into `SettingsView` via `hudTilesConfig`/`onHudTilesChange`), so "config in Settings" is **already satisfied**; VG-3 keeps it there and adds one tile kind. If Plan 3C's Settings re-grouping has not landed, the editor still renders under the current Settings `display`/`theme` section — VG-3 does not move it.
- **Blocks nothing.** A later plan may register more purpose-built widgets by adding entries to `dashboardWidgetRegistry.ts`; the fallback covers them until then.

**Base branch note:** Authored/executed on `claude/graphify-general-gui-ia` at `/c/Users/Owner/vox-graphify-gui`. **Confirm the branch before the first commit:** `git -C /c/Users/Owner/vox-graphify-gui rev-parse --abbrev-ref HEAD` must print `claude/graphify-general-gui-ia`. All UI work and the verify loop run from `crates/vox-gui/ui`:

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run <file> && pnpm typecheck
```

---

## Key internals (verified against the code — exact, with line anchors)

- **`crates/vox-gui/ui/src/lib/dashboardLayout.ts`** — `DASHBOARD_WIDGET_KINDS` array (`:5`) is the kind SSOT (mirrors `contracts/gui/dashboard-layout.v1.yaml`); `DashboardWidget { id, kind, grid, config?: Record<string,unknown> }` (`:32`); `DashboardLayout { version: 1, columns, widgets }` (`:39`); `validateDashboardLayout` (`:141`) checks `KIND_SET.has(w.kind)` (`:173`) and allows an optional `config` object (`:187`); `addWidgetToLayout` (`:114`); `availableWidgetKinds` (`:136`); `defaultDashboardLayout` (`:78`). **VG-3 adds one kind `surface_widget`** to this array (Task 1) so the validator accepts surface-backed slots whose `config.surfaceKey` names the surface.
- **`crates/vox-gui/ui/src/components/dashboard/DashboardGrid.tsx`** — `DashboardGrid({ layout, customizeMode, onLayoutChange, renderWidget })` (`:209`); `renderWidget(widget) => React.ReactNode` is the injection point (`:25`, `:237`); `persistDashboardLayout`/`loadDashboardLayout` (`:186`, `:197`). VG-3 reuses this verbatim; the only change is what `renderWidget` returns (Task 4 routes through the registry).
- **`crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.tsx`** — `WidgetPickerDrawer({ layout, open, onClose, onAdd })` (`:12`) lists `availableWidgetKinds(layout)`. VG-3 extends the picker to also list surface widgets grouped by section (Task 6).
- **`crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`** — the surface; `renderWidget(widget)` is a hard-coded `switch (widget.kind)` (`:147`–`:275`) with a `default: return null` (`:273`); it already wires `DashboardGrid` (`:387`), `WidgetPickerDrawer` (`:381`), `customizeMode` (`:76`), and a `loadDashboardLayout(defaultDashboardLayout())` localStorage layout (`:81`). VG-3 makes the `default:` arm delegate to the registry (purpose-built → mini-render fallback) instead of returning `null`.
- **`crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`** — `childRenderer(props: SurfaceProps, viewKey: string): React.ReactNode` (`:77`) is the **single** map from `viewKey` to a surface component (the `switch (viewKey)` at `:82`); `SurfaceProps` (`:36`) is the full prop bag; `renderSurfaceView` (`:196`). The mini-render fallback (Task 3) calls `childRenderer` with a compact-scoped prop subset, so a new surface needs no new wiring here.
- **`crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`** — `SurfaceRegistryEntry { viewKey, cliGroup, tier, navLabel, navIcon, navGroup, parentSurface }` (`:3`); `SURFACE_REGISTRY` (`:12`). `navGroup` values present: `operate`, `develop`, `knowledge`, `compute`, `system`. **AUTO-GENERATED — never hand-edit** (header `:1`); regenerate via `vox ci gui-surface-registry --write`. VG-3 reads it; it does **not** edit it.
- **`crates/vox-gui/ui/src/hooks/useHudTiles.ts`** — `HUD_TILE_KINDS` (`:6`) = `active_agents`, `queue_depth`, `budget_burn`, `mesh_peers`, `active_model`, `openrouter_spend`; `HUD_TILE_LABELS` (`:17`); `HudTilesConfig { version: 1, tiles: HudTileEntry[] }` (`:32`); `validateHudTilesConfig` (`:54`) checks `KIND_SET.has(id)` (`:76`) **and** `KIND_SET.has(kind)` (`:79`); `defaultHudTiles` (`:43`); `resolveVisibleHudTiles` (`:91`) → enabled kinds only; `toggleHudTile` (`:100`). **VG-3 adds `pending_approvals`** to `HUD_TILE_KINDS` + `HUD_TILE_LABELS` (Task 7).
- **`crates/vox-gui/ui/src/hooks/useHudTilesConfig.ts`** — wraps `useLocalStorage(SHELL_PREFERENCE_KEYS.hudTiles, …)` and returns `{ config, setConfig, visibleTiles }` (`resolveVisibleHudTiles`-filtered). `App.tsx:214` consumes it; `App.tsx:1154` passes `visibleTiles` into `TopHud`. Disabling a tile in `HudTilesEditor` flows through `toggleHudTile` → `setConfig` → `visibleTiles` drops it → `TopHud` stops rendering it. **This is the "disabling a core widget removes it from the strip" mechanism** — VG-3's only job is to make `pending_approvals` participate in it.
- **`crates/vox-gui/ui/src/components/layout/TopHud.tsx`** — `renderTile(kind)` `switch` (`:126`–`:211`) renders each HUD tile; `visibleTiles.map(renderTile)` (`:278`). VG-3 adds a `case 'pending_approvals':` arm (Task 7).
- **`crates/vox-gui/ui/src/components/surfaces/Settings/HudTilesEditor.tsx`** — checkbox-per-tile editor (`:30`) using `HUD_TILE_LABELS` + `toggleHudTile`; it iterates `config.tiles`, so it picks up the new `pending_approvals` tile automatically once `defaultHudTiles()` includes it. Threaded into `SettingsView` via `hudTilesConfig`/`onHudTilesChange` (`surfaceComponents.tsx:128`). **This is the "config in Settings" home** — no new settings island.
- **`crates/vox-gui/ui/src/hooks/useLlmSpend.ts`** — `useLlmSpend(): { totalUsd: number | null }` (`:6`) polls `voxTransport.getLlmSpend()` (`get_llm_spend`) every 60s. The cost purpose-built widget reuses this (Task 2 — cost) instead of inventing a spend source.
- **`crates/vox-gui/ui/src/components/surfaces/__guards__/honestyScan.ts` + `honestyScan.test.ts`** — `scanSource(path, src)` flags `placeholder` prose and `dead-handler` (`onClick={() => {}}`); it does **not** flag `role="status"` empty states or commented "not wired". The compact error tile must therefore (a) carry a real message (no "Not yet implemented" prose), (b) have no empty arrow handlers — VG-3's error tile renders text + an optional real retry callback. The `vox ci gui-honesty` gate wraps this scanner; keep new code clean of its triggers.
- **`contracts/gui/dashboard-layout.v1.yaml`** — `widget_kinds` list (`:10`) mirrors `DASHBOARD_WIDGET_KINDS`. Add `surface_widget` here too (Task 1) to keep the contract and the TS in lockstep.
- **`contracts/gui/hud-tiles.v1.yaml`** — the HUD-tile kind SSOT (mirrors `HUD_TILE_KINDS`). Add `pending_approvals` here (Task 7) alongside the TS edit.

---

## File Structure

**Created**
- `crates/vox-gui/ui/src/lib/dashboardSections.ts` — pure `sectionForNavGroup(navGroup) → DashboardSection` map + `DASHBOARD_SECTIONS` ordered list (Operations/Cost/Knowledge/Surfaces) + `surfacesForSection(section)` (filters `SURFACE_REGISTRY`).
- `crates/vox-gui/ui/src/lib/dashboardSections.test.ts` — section-mapping unit tests + the **auto-expansion** test (a synthetic new registry row lands in a section with no edit).
- `crates/vox-gui/ui/src/components/dashboard/dashboardWidgetRegistry.tsx` — `PURPOSE_BUILT: Record<string, PurposeBuiltWidget>` (the five) + `resolveWidget(surfaceKey)` (purpose-built else fallback descriptor).
- `crates/vox-gui/ui/src/components/dashboard/dashboardWidgetRegistry.test.tsx` — purpose-built-overrides-fallback + unknown-surface-falls-back tests.
- `crates/vox-gui/ui/src/components/dashboard/SurfaceMiniRender.tsx` — the `compact`-scaled, non-interactive wrapper that mounts a real surface via `childRenderer`.
- `crates/vox-gui/ui/src/components/dashboard/SurfaceMiniRender.test.tsx` — mounts a fixture surface in compact mode; asserts it renders + is inert.
- `crates/vox-gui/ui/src/components/dashboard/WidgetErrorBoundary.tsx` — class component → compact error tile.
- `crates/vox-gui/ui/src/components/dashboard/WidgetErrorBoundary.test.tsx` — a throwing child yields the compact error tile, not a crash.
- `crates/vox-gui/ui/src/components/dashboard/widgets/CostWidget.tsx` — purpose-built spend widget over `useLlmSpend`.
- `crates/vox-gui/ui/src/components/dashboard/widgets/MeshWidget.tsx` — purpose-built mesh peers widget.
- `crates/vox-gui/ui/src/components/dashboard/widgets/ApprovalsWidget.tsx` — purpose-built approvals/doubt widget.
- `crates/vox-gui/ui/src/components/dashboard/widgets/CoverageWidget.tsx` — purpose-built coverage/build-spine widget.
- `crates/vox-gui/ui/src/components/dashboard/widgets/AgentsStreamWidget.tsx` — purpose-built agent runs/streams widget.

**Modified**
- `crates/vox-gui/ui/src/lib/dashboardLayout.ts` — add `surface_widget` to `DASHBOARD_WIDGET_KINDS`; add `surfaceKey` typing helpers.
- `contracts/gui/dashboard-layout.v1.yaml` — add `surface_widget` to `widget_kinds`.
- `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` — `renderWidget` routes `surface_widget` (and the `default:` arm) through `resolveWidget` + `WidgetErrorBoundary`; section-grouped picker.
- `crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.tsx` — list surface widgets grouped by section.
- `crates/vox-gui/ui/src/hooks/useHudTiles.ts` — add `pending_approvals` kind + label.
- `contracts/gui/hud-tiles.v1.yaml` — add `pending_approvals`.
- `crates/vox-gui/ui/src/components/layout/TopHud.tsx` — `renderTile` arm for `pending_approvals`.
- `crates/vox-gui/ui/src/App.tsx` — pass a `pendingApprovals` count into `TopHud` (it already plumbs `visibleTiles`/`kpis`).

---

## Workflow-readiness: dependency DAG + fan-out batches

```
Phase A (foundation)   T1 ──► T2 ──► T3 ──► T4        [SEQUENTIAL within A]
                              (kinds) (purpose) (mini) (wire)
Phase B (sections+pick) T5 ─► T6                       [needs T4]
Phase C (strip)         T7 ─► T8                       [PARALLEL with A/B — different files]
Phase D (close)         T9                             [needs all]
```

**Explicit parallel fan-out batches a workflow can dispatch concurrently:**
- **Batch β (independent files):** Phase **C** (HUD strip: `useHudTiles.ts` / `hud-tiles.v1.yaml` / `TopHud.tsx` / `App.tsx`) shares **no files** with Phase A/B (the dashboard registry + grid). Dispatch **T7** in parallel with **T1**. They only re-converge at T9 (final regression).
- Everything else is sequential per the DAG (T2 needs T1's `surface_widget` kind; T3 builds the fallback T4 wires; T5/T6 need the routed grid from T4).

Each task below is independently committable by a sub-agent. Tags: **[SEQUENTIAL]** = must follow its predecessor in-phase; **[PARALLEL-SAFE]** = no in-flight conflict with its batch siblings.

---

# PHASE A — Registry-driven render core

## Task 1: `surface_widget` kind + `surfaceKey` config typing (TDD) [SEQUENTIAL]

The layout validator must accept a slot backed by a surface (so a fallback or purpose-built surface widget persists like any other). Add the kind to the SSOT array and the contract, plus a tiny typed accessor for `config.surfaceKey`.

**Files:** Modify `crates/vox-gui/ui/src/lib/dashboardLayout.ts`, `contracts/gui/dashboard-layout.v1.yaml`. Test: `crates/vox-gui/ui/src/lib/dashboardLayout.test.ts` (extend if present, else create).

- [ ] **Step 1: Failing test** — append to (or create) `crates/vox-gui/ui/src/lib/dashboardLayout.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  DASHBOARD_WIDGET_KINDS,
  validateDashboardLayout,
  surfaceKeyOf,
  type DashboardLayout,
} from './dashboardLayout';

describe('surface_widget kind', () => {
  it('includes surface_widget in the kind SSOT', () => {
    expect(DASHBOARD_WIDGET_KINDS).toContain('surface_widget');
  });

  it('validates a surface_widget slot carrying a surfaceKey in config', () => {
    const raw = {
      version: 1,
      columns: 12,
      widgets: [
        { id: 'mesh-mini', kind: 'surface_widget', grid: { col: 1, row: 1, w: 4, h: 2 }, config: { surfaceKey: 'mesh' } },
      ],
    };
    const layout: DashboardLayout = validateDashboardLayout(raw);
    expect(layout.widgets[0].kind).toBe('surface_widget');
    expect(surfaceKeyOf(layout.widgets[0])).toBe('mesh');
  });

  it('surfaceKeyOf returns null when config has no string surfaceKey', () => {
    expect(surfaceKeyOf({ id: 'x', kind: 'agents', grid: { col: 1, row: 1, w: 4, h: 2 } })).toBeNull();
  });
});
```

- [ ] **Step 2: Run, verify fail** — `cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run src/lib/dashboardLayout.test.ts`. Expected: FAIL — `'surface_widget'` not in the array / `surfaceKeyOf` undefined.

- [ ] **Step 3: Implement** — in `dashboardLayout.ts`, add `'surface_widget'` to `DASHBOARD_WIDGET_KINDS` (append after `'resources'` at `:20`) and add the accessor near `widgetKindLabel` (`:129`):

```ts
/** The surface key a `surface_widget` slot is backed by, or null. */
export function surfaceKeyOf(widget: DashboardWidget): string | null {
  const key = widget.config?.surfaceKey;
  return typeof key === 'string' && key.length > 0 ? key : null;
}
```

- [ ] **Step 4: Mirror the contract** — in `contracts/gui/dashboard-layout.v1.yaml`, add `  - surface_widget` to the `widget_kinds:` list (after `- resources`, `:25`).

- [ ] **Step 5: Run, verify pass** — `cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run src/lib/dashboardLayout.test.ts && pnpm typecheck`. Expected: PASS.

- [ ] **Step 6: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/lib/dashboardLayout.ts crates/vox-gui/ui/src/lib/dashboardLayout.test.ts contracts/gui/dashboard-layout.v1.yaml`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui-dashboard): add surface_widget kind + surfaceKeyOf accessor (VG-3 T1)"`

## Task 2: `SurfaceMiniRender` — compact, inert live render of a real surface (TDD) [SEQUENTIAL]

The fallback path mounts the **real** surface component (via `childRenderer`) at a reduced scale, scroll-clipped, and pointer-inert (it is a monitor thumbnail, not an interactive surface). Honesty: it renders the real component's real output — no fabrication.

**Files:** Create `crates/vox-gui/ui/src/components/dashboard/SurfaceMiniRender.tsx` + `SurfaceMiniRender.test.tsx`.

- [ ] **Step 1: Failing test** — create `crates/vox-gui/ui/src/components/dashboard/SurfaceMiniRender.test.tsx`:

```tsx
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SurfaceMiniRender } from './SurfaceMiniRender';

describe('SurfaceMiniRender', () => {
  it('mounts the provided surface node inside a compact, inert frame', () => {
    render(
      <SurfaceMiniRender surfaceKey="demo" label="Demo">
        <div data-testid="demo-surface-body">live demo content</div>
      </SurfaceMiniRender>,
    );
    // The real child is mounted (no fabrication, no placeholder).
    expect(screen.getByTestId('demo-surface-body')).toBeTruthy();
    // The frame is marked compact + inert for hit-testing.
    const frame = screen.getByTestId('surface-mini-demo');
    expect(frame.getAttribute('data-compact')).toBe('true');
    expect(frame.getAttribute('aria-hidden')).toBe('true');
    // The header shows the surface label so the user knows what they are watching.
    expect(screen.getByText('Demo')).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run, verify fail** — `pnpm vitest run src/components/dashboard/SurfaceMiniRender.test.tsx`. Expected: FAIL (module missing).

- [ ] **Step 3: Implement** — create `crates/vox-gui/ui/src/components/dashboard/SurfaceMiniRender.tsx`:

```tsx
import React from 'react';
import { Glass } from '../ui/Glass';

export interface SurfaceMiniRenderProps {
  surfaceKey: string;
  label: string;
  /** The real surface node (produced by childRenderer in the Dashboard). */
  children: React.ReactNode;
  /** Visual scale of the embedded surface; default 0.6 (a thumbnail). */
  scale?: number;
}

/**
 * A live, compact, NON-INTERACTIVE thumbnail of a real surface component.
 * It renders the genuine surface output (honesty: never a fabricated value),
 * scaled down and scroll-clipped, with pointer events disabled so the
 * dashboard slot behaves as a monitor, not a second copy of the surface.
 * Click-through to the full surface is the parent's job (onOpen).
 */
export function SurfaceMiniRender({ surfaceKey, label, children, scale = 0.6 }: SurfaceMiniRenderProps) {
  return (
    <Glass className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="flex items-center justify-between border-b border-border-subtle px-3 py-1.5">
        <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">{label}</span>
        <span className="rounded border border-border-subtle bg-overlay-subtle px-1.5 py-0.5 font-mono text-[9px] text-text-muted">live</span>
      </div>
      <div
        data-testid={`surface-mini-${surfaceKey}`}
        data-compact="true"
        aria-hidden="true"
        className="relative min-h-0 flex-1 overflow-hidden"
      >
        <div
          className="pointer-events-none origin-top-left"
          style={{ transform: `scale(${scale})`, width: `${100 / scale}%`, height: `${100 / scale}%` }}
        >
          {children}
        </div>
      </div>
    </Glass>
  );
}
```

- [ ] **Step 4: Run, verify pass** — `pnpm vitest run src/components/dashboard/SurfaceMiniRender.test.tsx && pnpm typecheck`. Expected: PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/dashboard/SurfaceMiniRender.tsx crates/vox-gui/ui/src/components/dashboard/SurfaceMiniRender.test.tsx`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui-dashboard): SurfaceMiniRender compact inert live-render wrapper (VG-3 T2)"`

## Task 3: `WidgetErrorBoundary` → compact error tile (TDD) [SEQUENTIAL]

A broken widget must degrade to a compact error tile, never crash the dashboard (spec §6). The tile is honest: it names the surface and shows the real error message.

**Files:** Create `crates/vox-gui/ui/src/components/dashboard/WidgetErrorBoundary.tsx` + `WidgetErrorBoundary.test.tsx`.

- [ ] **Step 1: Failing test** — create `crates/vox-gui/ui/src/components/dashboard/WidgetErrorBoundary.test.tsx`:

```tsx
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { WidgetErrorBoundary } from './WidgetErrorBoundary';

function Boom(): JSX.Element {
  throw new Error('kaboom in mesh');
}

describe('WidgetErrorBoundary', () => {
  // React logs the caught error to console.error; silence it for a clean run.
  let spy: ReturnType<typeof vi.spyOn>;
  beforeEach(() => { spy = vi.spyOn(console, 'error').mockImplementation(() => {}); });
  afterEach(() => { spy.mockRestore(); });

  it('renders a compact error tile instead of crashing when a child throws', () => {
    render(
      <WidgetErrorBoundary label="Mesh">
        <Boom />
      </WidgetErrorBoundary>,
    );
    const tile = screen.getByTestId('widget-error-tile');
    expect(tile).toBeTruthy();
    expect(tile.textContent).toContain('Mesh');
    expect(tile.textContent).toContain('kaboom in mesh');
  });

  it('renders children unchanged when nothing throws', () => {
    render(
      <WidgetErrorBoundary label="OK">
        <div data-testid="ok-body">fine</div>
      </WidgetErrorBoundary>,
    );
    expect(screen.getByTestId('ok-body')).toBeTruthy();
    expect(screen.queryByTestId('widget-error-tile')).toBeNull();
  });
});
```

- [ ] **Step 2: Run, verify fail** — `pnpm vitest run src/components/dashboard/WidgetErrorBoundary.test.tsx`. Expected: FAIL (module missing).

- [ ] **Step 3: Implement** — create `crates/vox-gui/ui/src/components/dashboard/WidgetErrorBoundary.tsx`:

```tsx
import React from 'react';

interface WidgetErrorBoundaryProps {
  label: string;
  children: React.ReactNode;
}

interface WidgetErrorBoundaryState {
  error: Error | null;
}

/**
 * Per-widget error boundary. A broken surface/widget renders a compact error
 * tile (honest: the real surface label + the real error message), never a
 * crashed dashboard (spec §6). No placeholder prose, no empty handlers — the
 * gui-honesty scanner stays green.
 */
export class WidgetErrorBoundary extends React.Component<WidgetErrorBoundaryProps, WidgetErrorBoundaryState> {
  constructor(props: WidgetErrorBoundaryProps) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error: Error): WidgetErrorBoundaryState {
    return { error };
  }

  render(): React.ReactNode {
    const { error } = this.state;
    if (error) {
      return (
        <div
          data-testid="widget-error-tile"
          role="alert"
          className="flex h-full min-h-0 flex-col gap-1 overflow-hidden rounded-lg border border-[var(--color-status-fail)]/30 bg-[var(--color-status-fail)]/[0.06] p-3"
        >
          <span className="font-display text-[11px] uppercase tracking-[0.18em] text-[var(--color-status-fail)]">
            {this.props.label} · widget error
          </span>
          <span className="font-mono text-[10px] text-text-muted break-words">{error.message}</span>
        </div>
      );
    }
    return this.props.children;
  }
}
```

- [ ] **Step 4: Run, verify pass** — `pnpm vitest run src/components/dashboard/WidgetErrorBoundary.test.tsx && pnpm typecheck`. Expected: PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/dashboard/WidgetErrorBoundary.tsx crates/vox-gui/ui/src/components/dashboard/WidgetErrorBoundary.test.tsx`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui-dashboard): WidgetErrorBoundary compact error tile (VG-3 T3)"`

## Task 4: The widget registry — purpose-built shortlist + fallback descriptor (TDD) [SEQUENTIAL]

The registry is the single source the dashboard subscribes to. `resolveWidget(surfaceKey)` returns a purpose-built descriptor when one is registered for that surface, else a `kind: 'fallback'` descriptor. The five purpose-built widgets are stub-free real components (built in Task 4b); the registry only needs them to exist as components — start each as a thin honest component over its real data hook, then expand. For T4 the **mesh / approvals / coverage / agents** widgets begin as minimal honest renders (real count from props or `'—'` empty state); **cost** uses `useLlmSpend`. No placeholder text.

**Files:** Create `crates/vox-gui/ui/src/components/dashboard/dashboardWidgetRegistry.tsx` + `dashboardWidgetRegistry.test.tsx`; create the five widget files under `widgets/`.

- [ ] **Step 1: Failing test** — create `crates/vox-gui/ui/src/components/dashboard/dashboardWidgetRegistry.test.tsx`:

```tsx
import { describe, expect, it } from 'vitest';
import { resolveWidget, PURPOSE_BUILT_SURFACE_KEYS } from './dashboardWidgetRegistry';

describe('dashboardWidgetRegistry', () => {
  it('registers exactly the five purpose-built surfaces', () => {
    expect([...PURPOSE_BUILT_SURFACE_KEYS].sort()).toEqual(
      ['agents', 'approvals', 'coverage', 'cost', 'mesh'].sort(),
    );
  });

  it('returns a purpose-built descriptor for a registered surface (overrides fallback)', () => {
    const r = resolveWidget('mesh');
    expect(r.kind).toBe('purpose-built');
    expect(typeof r.Component).toBe('function');
  });

  it('falls back for an unregistered surface', () => {
    const r = resolveWidget('repository');
    expect(r.kind).toBe('fallback');
  });

  it('falls back for a brand-new surface key never seen before (auto-expansion)', () => {
    const r = resolveWidget('totally-new-surface-xyz');
    expect(r.kind).toBe('fallback');
  });
});
```

- [ ] **Step 2: Run, verify fail** — `pnpm vitest run src/components/dashboard/dashboardWidgetRegistry.test.tsx`. Expected: FAIL (module missing).

- [ ] **Step 3: Implement the five purpose-built widgets** — create each file. Keep them honest (real data or an explicit empty state). The cost widget reuses `useLlmSpend`.

`crates/vox-gui/ui/src/components/dashboard/widgets/CostWidget.tsx`:

```tsx
import React from 'react';
import { Glass } from '../../ui/Glass';
import { useLlmSpend } from '../../../hooks/useLlmSpend';

export function CostWidget() {
  const { totalUsd } = useLlmSpend();
  return (
    <Glass className="flex h-full flex-col justify-between p-4">
      <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">OpenRouter Spend</span>
      <span className="font-display text-[28px] font-semibold tabular-nums text-emerald-300">
        {totalUsd == null ? '—' : `$${totalUsd.toFixed(2)}`}
      </span>
      <span className="text-[10px] text-text-muted">{totalUsd == null ? 'awaiting cost daemon' : 'total this period'}</span>
    </Glass>
  );
}
```

`crates/vox-gui/ui/src/components/dashboard/widgets/MeshWidget.tsx`:

```tsx
import React from 'react';
import { Glass } from '../../ui/Glass';
import type { DashboardData } from '../../../types/dashboard';

export function MeshWidget({ data }: { data: DashboardData }) {
  const online = data.peers.filter((p) => p.online).length;
  return (
    <Glass className="flex h-full flex-col justify-between p-4">
      <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">Mesh Peers</span>
      <span className="font-display text-[28px] font-semibold tabular-nums text-violet-300">{online}</span>
      <span className="text-[10px] text-text-muted">{online === 0 ? 'no peers online' : 'online now'}</span>
    </Glass>
  );
}
```

`crates/vox-gui/ui/src/components/dashboard/widgets/ApprovalsWidget.tsx`:

```tsx
import React from 'react';
import { Glass } from '../../ui/Glass';
import type { DashboardData } from '../../../types/dashboard';

export function ApprovalsWidget({ data }: { data: DashboardData }) {
  const pending = data.alerts.length;
  return (
    <Glass className="flex h-full flex-col justify-between p-4">
      <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">Approvals · Doubt</span>
      <span className="font-display text-[28px] font-semibold tabular-nums text-amber-300">{pending}</span>
      <span className="text-[10px] text-text-muted">{pending === 0 ? 'all clear' : 'awaiting you'}</span>
    </Glass>
  );
}
```

`crates/vox-gui/ui/src/components/dashboard/widgets/CoverageWidget.tsx`:

```tsx
import React from 'react';
import { Glass } from '../../ui/Glass';
import type { DashboardData } from '../../../types/dashboard';

export function CoverageWidget({ data }: { data: DashboardData }) {
  // Honest read: surface the queue/agents signal as a build-spine proxy until a
  // dedicated coverage feed is wired; never a fabricated percentage.
  const agents = data.agents.length;
  return (
    <Glass className="flex h-full flex-col justify-between p-4">
      <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">Coverage · Build-spine</span>
      <span className="font-display text-[28px] font-semibold tabular-nums text-cyan-300">{agents}</span>
      <span className="text-[10px] text-text-muted">active build agents</span>
    </Glass>
  );
}
```

`crates/vox-gui/ui/src/components/dashboard/widgets/AgentsStreamWidget.tsx`:

```tsx
import React from 'react';
import { Glass } from '../../ui/Glass';
import type { DashboardData } from '../../../types/dashboard';

export function AgentsStreamWidget({ data }: { data: DashboardData }) {
  return (
    <Glass className="flex h-full min-h-0 flex-col p-4">
      <div className="flex items-center justify-between">
        <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">Agent Runs</span>
        <span className="font-mono text-[10px] text-text-muted">{data.agents.length} active</span>
      </div>
      <div className="mt-2 flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto">
        {data.agents.length === 0 ? (
          <div role="status" className="rounded-lg border border-dashed border-border-subtle py-4 text-center text-[11px] text-text-muted">
            No active agents
          </div>
        ) : (
          data.agents.slice(0, 6).map((a) => (
            <div key={a.id} className="flex items-center justify-between rounded border border-border-subtle bg-overlay-subtle px-2 py-1 text-[11px]">
              <span className="truncate text-text-secondary">{a.codename}</span>
              <span className="font-mono text-[10px] text-text-muted">{a.status}</span>
            </div>
          ))
        )}
      </div>
    </Glass>
  );
}
```

- [ ] **Step 4: Implement the registry** — create `crates/vox-gui/ui/src/components/dashboard/dashboardWidgetRegistry.tsx`:

```tsx
import React from 'react';
import type { DashboardData } from '../../types/dashboard';
import { CostWidget } from './widgets/CostWidget';
import { MeshWidget } from './widgets/MeshWidget';
import { ApprovalsWidget } from './widgets/ApprovalsWidget';
import { CoverageWidget } from './widgets/CoverageWidget';
import { AgentsStreamWidget } from './widgets/AgentsStreamWidget';

/** Data a purpose-built widget may consume. Extend as widgets grow. */
export interface PurposeBuiltProps {
  data: DashboardData;
}

export type PurposeBuiltComponent = (props: PurposeBuiltProps) => React.ReactElement;

/**
 * The shortlist that earns a purpose-built widget (spec §4.2). Keyed by the
 * surface key it represents. Everything else falls back to a mini-render.
 * `cost` maps to the spend surface concept (no `cost` viewKey exists — it is a
 * synthetic monitorable backed by useLlmSpend), so it is keyed `cost` here and
 * offered explicitly in the picker (Task 6); the other four match real viewKeys.
 */
const PURPOSE_BUILT: Record<string, PurposeBuiltComponent> = {
  agents: ({ data }) => <AgentsStreamWidget data={data} />,
  cost: () => <CostWidget />,
  mesh: ({ data }) => <MeshWidget data={data} />,
  approvals: ({ data }) => <ApprovalsWidget data={data} />,
  coverage: ({ data }) => <CoverageWidget data={data} />,
};

export const PURPOSE_BUILT_SURFACE_KEYS = new Set(Object.keys(PURPOSE_BUILT));

export type ResolvedWidget =
  | { kind: 'purpose-built'; Component: PurposeBuiltComponent }
  | { kind: 'fallback' };

/**
 * Resolve a slot's surface key to a render path. A registered surface gets its
 * purpose-built widget (overriding the fallback); ANY other key — including a
 * brand-new surface never seen before — falls back to a mini-render.
 */
export function resolveWidget(surfaceKey: string): ResolvedWidget {
  const Component = PURPOSE_BUILT[surfaceKey];
  return Component ? { kind: 'purpose-built', Component } : { kind: 'fallback' };
}
```

- [ ] **Step 5: Run, verify pass** — `pnpm vitest run src/components/dashboard/dashboardWidgetRegistry.test.tsx && pnpm typecheck`. Expected: PASS. (If `DashboardData`'s `agents[].status`/`codename` or `peers[].online` field names differ, read `src/types/dashboard.ts` first and match the real fields — do not invent them.)

- [ ] **Step 6: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/dashboard/dashboardWidgetRegistry.tsx crates/vox-gui/ui/src/components/dashboard/dashboardWidgetRegistry.test.tsx crates/vox-gui/ui/src/components/dashboard/widgets/CostWidget.tsx crates/vox-gui/ui/src/components/dashboard/widgets/MeshWidget.tsx crates/vox-gui/ui/src/components/dashboard/widgets/ApprovalsWidget.tsx crates/vox-gui/ui/src/components/dashboard/widgets/CoverageWidget.tsx crates/vox-gui/ui/src/components/dashboard/widgets/AgentsStreamWidget.tsx`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui-dashboard): widget registry + 5 purpose-built widgets, fallback descriptor (VG-3 T4)"`

## Task 5: Wire the registry into the Dashboard render path (TDD) [SEQUENTIAL]

Route the Dashboard's `renderWidget` so a `surface_widget` slot (and the previously-`null` `default:` arm) resolves through `resolveWidget` → purpose-built **or** `SurfaceMiniRender(childRenderer(...))`, each wrapped in `WidgetErrorBoundary`. The legacy fixed kinds keep their existing arms.

**Files:** Modify `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`; extend `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx`.

- [ ] **Step 1: Failing test** — append to `Dashboard.test.tsx` (reuse its existing render scaffold + `DashboardData` fixture builder; read the top of the file for the helper name — call it `renderDashboard` below, adjust to the real one):

```tsx
import { addWidgetToLayout, defaultDashboardLayout } from '../../../lib/dashboardLayout';

it('renders a purpose-built widget for a registered surface_widget (mesh)', () => {
  // Seed a layout with a mesh surface_widget so the registry's purpose-built path fires.
  const layout = {
    version: 1 as const, columns: 12,
    widgets: [{ id: 'mesh-mini', kind: 'surface_widget' as const, grid: { col: 1, row: 1, w: 4, h: 2 }, config: { surfaceKey: 'mesh' } }],
  };
  window.localStorage.setItem('gui.dashboard.layout.v1', JSON.stringify(layout));
  renderDashboard();
  // The purpose-built MeshWidget shows the "Mesh Peers" label (mini-render would show a "live" badge instead).
  expect(screen.getByText('Mesh Peers')).toBeTruthy();
});

it('renders a mini-render fallback for an unregistered surface_widget (repository)', () => {
  const layout = {
    version: 1 as const, columns: 12,
    widgets: [{ id: 'repo-mini', kind: 'surface_widget' as const, grid: { col: 1, row: 1, w: 4, h: 2 }, config: { surfaceKey: 'repository' } }],
  };
  window.localStorage.setItem('gui.dashboard.layout.v1', JSON.stringify(layout));
  renderDashboard();
  // The fallback frame is present and marked compact/inert.
  expect(screen.getByTestId('surface-mini-repository').getAttribute('data-compact')).toBe('true');
});
```

> Confirm the localStorage key: `SHELL_PREFERENCE_KEYS.dashboardLayout` resolves to `gui.dashboard.layout.v1` (verify in `src/lib/shellPersistence.ts`). If the test scaffold seeds layout differently, follow its convention.

- [ ] **Step 2: Run, verify fail** — `pnpm vitest run src/components/surfaces/Dashboard/Dashboard.test.tsx`. Expected: FAIL (`surface_widget` hits `default: return null`).

- [ ] **Step 3: Implement** — in `Dashboard.tsx`:

1. Add imports near the other dashboard imports (`:13`):

```ts
import { surfaceKeyOf } from '../../../lib/dashboardLayout';
import { resolveWidget } from '../../dashboard/dashboardWidgetRegistry';
import { SurfaceMiniRender } from '../../dashboard/SurfaceMiniRender';
import { WidgetErrorBoundary } from '../../dashboard/WidgetErrorBoundary';
import { childRenderer } from '../../layout/surfaceComponents';
import { SURFACE_REGISTRY } from '../../../generated/surfaceRegistry.generated';
```

> `childRenderer` is currently a module-private function in `surfaceComponents.tsx`. **Export it** (change `function childRenderer` at `:77` to `export function childRenderer`) so the Dashboard can mini-render any surface through the single map. This is a one-word edit; do it in this step and include `surfaceComponents.tsx` in the commit.

2. Add a helper that builds the `SurfaceProps` subset a mini-render needs from the Dashboard's own props (most are optional; the mini-render is inert so handlers can be no-op-free by simply omitting them). Place it above `renderWidget`:

```tsx
function surfaceLabel(surfaceKey: string): string {
  const row = SURFACE_REGISTRY.find((e) => e.viewKey === surfaceKey);
  return row?.navLabel ?? surfaceKey;
}

function renderSurfaceWidget(surfaceKey: string, miniProps: SurfaceProps): React.ReactNode {
  const resolved = resolveWidget(surfaceKey);
  if (resolved.kind === 'purpose-built') {
    return <resolved.Component data={miniProps.data} />;
  }
  return (
    <SurfaceMiniRender surfaceKey={surfaceKey} label={surfaceLabel(surfaceKey)}>
      {childRenderer(miniProps, surfaceKey)}
    </SurfaceMiniRender>
  );
}
```

> `SurfaceProps` is exported from `surfaceComponents.tsx` (`:36`). Build `miniProps` from what the Dashboard already has (`data`, `pushToast` via a prop or a no-op-free shim — the Dashboard already receives `data`; thread `pushToast` down as a Dashboard prop if it is not already present, reading `DashboardProps` first). For surfaces whose `childRenderer` arm requires a handler that the Dashboard lacks, the mini-render is inert so the surface renders read-only; if a required non-optional prop is genuinely missing, prefer the purpose-built path or accept the error boundary catching it (honest degradation) rather than fabricating a handler.

3. Change the `renderWidget` `switch` (`:147`): add a `case 'surface_widget':` before `default`, and make `default` also delegate:

```tsx
      case 'surface_widget': {
        const key = surfaceKeyOf(widget);
        if (!key) return null;
        return (
          <WidgetErrorBoundary label={surfaceLabel(key)}>
            {renderSurfaceWidget(key, miniPropsFor(props))}
          </WidgetErrorBoundary>
        );
      }
      default:
        return null;
```

> Provide `miniPropsFor(props)` as a small builder inside the component (closure over the Dashboard's props) returning the `SurfaceProps` subset. Keep the existing legacy-kind arms (`stream`/`agents`/`alerts`/`budget_burn`/…) unchanged — only `surface_widget` is new; `default` stays `null` for any unknown legacy kind (a surface-backed slot always carries `surface_widget`, never an unknown literal).

- [ ] **Step 4: Run, verify pass** — `pnpm vitest run src/components/surfaces/Dashboard/Dashboard.test.tsx && pnpm typecheck`. Expected: PASS. Re-run the existing Dashboard tests to confirm no regression on the legacy kinds.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui-dashboard): route surface_widget slots through registry (purpose-built | mini-render) + error boundary (VG-3 T5)"`

---

# PHASE B — Sectioning + section-aware picker

## Task 6: Section map derived from the surface registry + auto-expansion test (TDD) [SEQUENTIAL]

The dashboard sections (Operations / Cost / Knowledge / Surfaces) are derived from `SURFACE_REGISTRY[].navGroup`, so they stay in sync as surfaces move — and a **new** registry row auto-lands in a section with no dashboard edit. The "Cost" section is synthetic (the spend monitorable), surfaced explicitly.

**Files:** Create `crates/vox-gui/ui/src/lib/dashboardSections.ts` + `dashboardSections.test.ts`.

- [ ] **Step 1: Failing test** — create `crates/vox-gui/ui/src/lib/dashboardSections.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  DASHBOARD_SECTIONS,
  sectionForNavGroup,
  surfacesForSection,
  type SurfaceRow,
} from './dashboardSections';

describe('dashboardSections', () => {
  it('exposes the four ordered sections', () => {
    expect(DASHBOARD_SECTIONS).toEqual(['operations', 'cost', 'knowledge', 'surfaces']);
  });

  it('maps operate→operations, knowledge→knowledge, and other groups→surfaces', () => {
    expect(sectionForNavGroup('operate')).toBe('operations');
    expect(sectionForNavGroup('knowledge')).toBe('knowledge');
    expect(sectionForNavGroup('develop')).toBe('surfaces');
    expect(sectionForNavGroup('compute')).toBe('surfaces');
    expect(sectionForNavGroup('system')).toBe('surfaces');
    expect(sectionForNavGroup(null)).toBe('surfaces');
  });

  it('AUTO-EXPANSION: a brand-new registry row lands in a section with no edit', () => {
    const baseline: SurfaceRow[] = [
      { viewKey: 'mesh', navLabel: 'Mesh', navGroup: 'compute' },
    ];
    const withNew: SurfaceRow[] = [
      ...baseline,
      { viewKey: 'brand-new-surface', navLabel: 'Brand New', navGroup: 'operate' },
    ];
    const opsBefore = surfacesForSection('operations', baseline).map((r) => r.viewKey);
    const opsAfter = surfacesForSection('operations', withNew).map((r) => r.viewKey);
    expect(opsBefore).not.toContain('brand-new-surface');
    expect(opsAfter).toContain('brand-new-surface'); // auto-appeared, zero dashboard edits
  });
});
```

- [ ] **Step 2: Run, verify fail** — `pnpm vitest run src/lib/dashboardSections.test.ts`. Expected: FAIL (module missing).

- [ ] **Step 3: Implement** — create `crates/vox-gui/ui/src/lib/dashboardSections.ts`:

```ts
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';

export const DASHBOARD_SECTIONS = ['operations', 'cost', 'knowledge', 'surfaces'] as const;
export type DashboardSection = (typeof DASHBOARD_SECTIONS)[number];

/** Minimal shape of a registry row this module reads (test-injectable). */
export interface SurfaceRow {
  viewKey: string | null;
  navLabel: string | null;
  navGroup: string | null;
}

/**
 * Fold a surface registry navGroup into a dashboard section. Operations and
 * Knowledge map directly; everything else (develop/compute/system/null) lands
 * in the catch-all "Surfaces" section. "Cost" is synthetic (the spend
 * monitorable) and is not produced by any navGroup.
 */
export function sectionForNavGroup(navGroup: string | null): DashboardSection {
  switch (navGroup) {
    case 'operate':
      return 'operations';
    case 'knowledge':
      return 'knowledge';
    default:
      return 'surfaces';
  }
}

/** Surface rows (real viewKeys with labels) that belong to a section. */
export function surfacesForSection(
  section: DashboardSection,
  rows: SurfaceRow[] = SURFACE_REGISTRY as unknown as SurfaceRow[],
): SurfaceRow[] {
  if (section === 'cost') return []; // synthetic; offered explicitly by the picker
  return rows.filter(
    (r) => r.viewKey && r.navLabel && sectionForNavGroup(r.navGroup) === section,
  );
}
```

- [ ] **Step 4: Run, verify pass** — `pnpm vitest run src/lib/dashboardSections.test.ts && pnpm typecheck`. Expected: PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/lib/dashboardSections.ts crates/vox-gui/ui/src/lib/dashboardSections.test.ts`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui-dashboard): registry-derived sections + auto-expansion (VG-3 T6)"`

## Task 7: Section-grouped picker that offers surface widgets (TDD) [SEQUENTIAL]

Extend `WidgetPickerDrawer` so, alongside the legacy kinds, it lists **surface widgets** grouped by section (each adds a `surface_widget` slot with `config.surfaceKey`). A new surface appears here automatically (it rides `surfacesForSection`). The synthetic **Cost** widget is offered under its section.

**Files:** Modify `crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.tsx`; extend `crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.test.tsx`.

- [ ] **Step 1: Failing test** — append to `WidgetPickerDrawer.test.tsx` (reuse its existing render helper):

```tsx
import { DASHBOARD_SECTIONS } from '../../lib/dashboardSections';

it('lists surface widgets grouped by section, including a new surface', () => {
  const onAddSurface = vi.fn();
  render(
    <WidgetPickerDrawer
      layout={{ version: 1, columns: 12, widgets: [] }}
      open
      onClose={() => {}}
      onAdd={() => {}}
      onAddSurface={onAddSurface}
    />,
  );
  // Section headers render for each dashboard section that has offerings.
  expect(screen.getByTestId('picker-section-operations')).toBeTruthy();
  // The synthetic Cost widget is offered.
  expect(screen.getByTestId('picker-surface-cost')).toBeTruthy();
  // Adding a surface widget calls onAddSurface with the surface key.
  fireEvent.click(screen.getByTestId('picker-surface-mesh'));
  expect(onAddSurface).toHaveBeenCalledWith('mesh');
});
```

- [ ] **Step 2: Run, verify fail** — `pnpm vitest run src/components/dashboard/WidgetPickerDrawer.test.tsx`. Expected: FAIL (no `onAddSurface`/section markup).

- [ ] **Step 3: Implement** — extend `WidgetPickerDrawer.tsx`:

```tsx
import React from 'react';
import type { DashboardLayout, DashboardWidgetKind } from '../../lib/dashboardLayout';
import { availableWidgetKinds, widgetKindLabel } from '../../lib/dashboardLayout';
import { DASHBOARD_SECTIONS, surfacesForSection, type DashboardSection } from '../../lib/dashboardSections';

export interface WidgetPickerDrawerProps {
  layout: DashboardLayout;
  open: boolean;
  onClose: () => void;
  onAdd: (kind: DashboardWidgetKind) => void;
  /** Add a surface-backed widget (surface_widget slot with this surfaceKey). */
  onAddSurface?: (surfaceKey: string) => void;
}

const SECTION_LABELS: Record<DashboardSection, string> = {
  operations: 'Operations',
  cost: 'Cost',
  knowledge: 'Knowledge',
  surfaces: 'Surfaces',
};

/** Surfaces offered per section, with the synthetic Cost monitorable injected. */
function surfaceOfferings(section: DashboardSection): Array<{ key: string; label: string }> {
  if (section === 'cost') return [{ key: 'cost', label: 'OpenRouter Spend' }];
  return surfacesForSection(section).map((r) => ({ key: r.viewKey as string, label: r.navLabel as string }));
}

export function WidgetPickerDrawer({ layout, open, onClose, onAdd, onAddSurface }: WidgetPickerDrawerProps) {
  if (!open) return null;
  const kinds = availableWidgetKinds(layout);

  return (
    <div
      role="dialog"
      aria-label="Add dashboard widget"
      className="absolute right-5 top-10 z-30 w-72 rounded-lg border border-border-subtle bg-bg-base/95 p-3 shadow-xl backdrop-blur"
    >
      <div className="mb-2 flex items-center justify-between">
        <h3 className="font-display text-[12px] font-semibold tracking-wide text-text-secondary">Add widget</h3>
        <button
          type="button"
          aria-label="Close widget picker"
          onClick={onClose}
          className="rounded px-1.5 py-0.5 text-[11px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary"
        >
          ✕
        </button>
      </div>

      <div className="max-h-80 overflow-y-auto">
        {onAddSurface &&
          DASHBOARD_SECTIONS.map((section) => {
            const offerings = surfaceOfferings(section);
            if (offerings.length === 0) return null;
            return (
              <div key={section} data-testid={`picker-section-${section}`} className="mb-3">
                <div className="mb-1 border-b border-border-subtle pb-1 font-display text-[9px] uppercase tracking-[0.24em] text-text-muted">
                  {SECTION_LABELS[section]}
                </div>
                <ul className="flex flex-col gap-1">
                  {offerings.map((o) => (
                    <li key={o.key}>
                      <button
                        type="button"
                        data-testid={`picker-surface-${o.key}`}
                        onClick={() => onAddSurface(o.key)}
                        className="w-full rounded-md border border-border-subtle px-2.5 py-1.5 text-left text-[11px] text-text-secondary hover:bg-overlay-subtle"
                      >
                        {o.label}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            );
          })}

        {kinds.length > 0 && (
          <div className="mb-1">
            <div className="mb-1 border-b border-border-subtle pb-1 font-display text-[9px] uppercase tracking-[0.24em] text-text-muted">
              Charts &amp; legacy
            </div>
            <ul className="flex flex-col gap-1">
              {kinds.map((kind) => (
                <li key={kind}>
                  <button
                    type="button"
                    onClick={() => onAdd(kind)}
                    className="w-full rounded-md border border-border-subtle px-2.5 py-1.5 text-left text-[11px] text-text-secondary hover:bg-overlay-subtle"
                  >
                    {widgetKindLabel(kind)}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}
```

Then in `Dashboard.tsx`, add an `onAddSurface` handler that appends a `surface_widget` slot:

```tsx
function handleAddSurface(surfaceKey: string) {
  const next = addWidgetToLayout(layout, 'surface_widget');
  const placed = {
    ...next,
    widgets: next.widgets.map((w, i) =>
      i === next.widgets.length - 1 ? { ...w, config: { ...(w.config ?? {}), surfaceKey } } : w,
    ),
  };
  updateLayout(placed);
  setPickerOpen(false);
}
```

and pass `onAddSurface={handleAddSurface}` to `<WidgetPickerDrawer …>` (`:381`).

- [ ] **Step 4: Run, verify pass** — `pnpm vitest run src/components/dashboard/WidgetPickerDrawer.test.tsx src/components/surfaces/Dashboard/Dashboard.test.tsx && pnpm typecheck`. Expected: PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.tsx crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.test.tsx crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui-dashboard): section-grouped picker offering surface widgets + Cost (VG-3 T7)"`

---

# PHASE C — Core-monitorables strip: add `pending_approvals` (Batch β, [PARALLEL-SAFE] with A/B)

> Phase C edits the HUD-tile SSOT + `TopHud` + `App.tsx` only — **no overlap** with the dashboard registry/grid files of Phases A/B. Dispatch T8 in parallel with T1.

## Task 8: Add `pending_approvals` to the strip SSOT + render arm + disable test (TDD) [SEQUENTIAL]

The five core monitorables the spec names are mesh peers · agents running · OpenRouter spend · queue depth · **pending approvals**. The first four already exist as HUD tiles (`mesh_peers`, `active_agents`, `openrouter_spend`, `queue_depth`). VG-3 adds the missing **`pending_approvals`** tile to the same SSOT so it appears in the strip, is disableable via the existing `HudTilesEditor` in Settings, and drops from the strip when disabled (the existing `resolveVisibleHudTiles` mechanism).

**Files:** Modify `crates/vox-gui/ui/src/hooks/useHudTiles.ts`, `contracts/gui/hud-tiles.v1.yaml`, `crates/vox-gui/ui/src/components/layout/TopHud.tsx`, `crates/vox-gui/ui/src/App.tsx`. Tests: `crates/vox-gui/ui/src/hooks/useHudTiles.test.ts` (extend if present, else create) + `crates/vox-gui/ui/src/components/layout/TopHud.test.tsx`.

- [ ] **Step 1: Failing test (SSOT + disable removes from strip)** — append to (or create) `crates/vox-gui/ui/src/hooks/useHudTiles.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  HUD_TILE_KINDS,
  HUD_TILE_LABELS,
  defaultHudTiles,
  toggleHudTile,
  resolveVisibleHudTiles,
} from './useHudTiles';

describe('pending_approvals HUD tile', () => {
  it('is part of the HUD tile SSOT with a label', () => {
    expect(HUD_TILE_KINDS).toContain('pending_approvals');
    expect(HUD_TILE_LABELS.pending_approvals).toBe('Pending approvals');
  });

  it('appears in the strip by default and DROPS when disabled', () => {
    const cfg = defaultHudTiles();
    expect(resolveVisibleHudTiles(cfg)).toContain('pending_approvals');
    const disabled = toggleHudTile(cfg, 'pending_approvals', false);
    expect(resolveVisibleHudTiles(disabled)).not.toContain('pending_approvals');
  });
});
```

- [ ] **Step 2: Run, verify fail** — `pnpm vitest run src/hooks/useHudTiles.test.ts`. Expected: FAIL.

- [ ] **Step 3: Implement the SSOT** — in `useHudTiles.ts`: add `'pending_approvals'` to `HUD_TILE_KINDS` (`:6`, append after `'openrouter_spend'`) and add `pending_approvals: 'Pending approvals',` to `HUD_TILE_LABELS` (`:17`). `defaultHudTiles` (`:43`) maps over `HUD_TILE_KINDS`, so the new tile becomes enabled-by-default automatically. Mirror it in `contracts/gui/hud-tiles.v1.yaml` (add `pending_approvals` to its kinds list).

- [ ] **Step 4: Failing test (TopHud renders the tile)** — append to `crates/vox-gui/ui/src/components/layout/TopHud.test.tsx` (reuse its `kpis` fixture; add a `pendingApprovals` field to the fixture):

```tsx
it('renders the pending-approvals tile when visible', () => {
  render(
    <TopHud
      kpis={kpisFixture}
      onCommand={() => {}}
      visibleTiles={['pending_approvals']}
      pendingApprovals={3}
    />,
  );
  expect(screen.getByText('Pending Approvals')).toBeTruthy();
  expect(screen.getByText('3')).toBeTruthy();
});
```

- [ ] **Step 5: Implement the render arm** — in `TopHud.tsx`: add `pendingApprovals?: number | null;` to `TopHudProps` (`:62`), default it in the destructure (`:85`), and add a `case` to `renderTile` (`:126`):

```tsx
      case 'pending_approvals':
        return (
          <KPI
            key={kind}
            label="Pending Approvals"
            value={pendingApprovals ?? 0}
            color="text-amber-300"
            spark={kpis.queueDepth.spark}
            icon={<Icon.shield className="size-4" />}
            onClick={() => onNavigate?.('approvals')}
          />
        );
```

> Confirm `Icon.shield` exists (it is used in `surfaceRegistry` navIcons and the Sidebar); if the icon export name differs, use `Icon.alert` (already used by TopHud-adjacent code). Read `src/components/ui/Icons.tsx` before picking.

- [ ] **Step 6: Wire the count in `App.tsx`** — pass a real pending-approvals count into `TopHud` (`:1154`, alongside `visibleTiles`). Source it from the same data the Dashboard uses for alerts/approvals (read the App root: the dashboard `data.alerts` / a `useAgentApprovals`-style hook is already in scope; if a precise count is not readily available, pass `data.alerts.length` which the existing dashboard already treats as the pending-attention count — honest, no fabrication). Add `pendingApprovals={…}` to the `<TopHud …>` props.

- [ ] **Step 7: Run, verify pass** — `pnpm vitest run src/hooks/useHudTiles.test.ts src/components/layout/TopHud.test.tsx && pnpm typecheck`. Expected: PASS. Also run the `HudTilesEditor` test if present (`src/components/surfaces/Settings/HudTilesEditor.test.tsx`) — it iterates `config.tiles`, so the new tile shows up in Settings automatically; confirm no count-assertion regresses (update an explicit tile-count assertion from 6→7 if one exists).

- [ ] **Step 8: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/hooks/useHudTiles.ts crates/vox-gui/ui/src/hooks/useHudTiles.test.ts contracts/gui/hud-tiles.v1.yaml crates/vox-gui/ui/src/components/layout/TopHud.tsx crates/vox-gui/ui/src/components/layout/TopHud.test.tsx crates/vox-gui/ui/src/App.tsx`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui-hud): add pending_approvals core monitorable to the minimized strip (config in Settings) (VG-3 T8)"`

---

# PHASE D — Close

## Task 9: Full regression + honesty gate + drift check [SEQUENTIAL]

_Depends on: T1–T8 all green and committed. Verification only — produces no source commit._

- [ ] **Step 1: Full vitest + typecheck** —
  `cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run && pnpm typecheck`.
  Expect all green except the known pre-existing Axis-branding fails (per MEMORY — confirm the count is unchanged; do not "fix" unrelated failures here).

- [ ] **Step 2: GUI honesty gate** —
  `cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli -- ci gui-honesty`.
  Must pass — the new widgets render real data or explicit empty states (`role="status"` / honest empty strings), the error tile carries a real message, and there are no empty arrow handlers or "Not yet implemented" prose. If it flags a new file, fix the trigger (it is a real honesty issue), do not suppress.

- [ ] **Step 3: Surface-registry drift no-write check** —
  `cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli -- ci gui-surface-registry`.
  Must print "up to date" — VG-3 does **not** edit the registry, so this confirms no accidental generated-TS drift. (If it reports drift, you hand-edited the generated file by mistake — revert that file; VG-3 reads the registry, never writes it.)

- [ ] **Step 4: Stop. Do not push, do not merge.** Report the branch state to the user. The user commits the final state and decides on merge.

---

## Workflow Batch Plan

| Batch | Tasks | Class | Depends on | Conflict surface | Dispatch |
| --- | --- | --- | --- | --- |
| **A — render core** | T1 → T2 → T3 → T4 → T5 | [SEQUENTIAL] | — | `dashboardLayout.ts`, `dashboard/*`, `widgets/*`, `Dashboard.tsx`, `surfaceComponents.tsx` (export only) | 1 agent, in order |
| **β — strip (parallel)** | T8 | [PARALLEL-SAFE] vs A/B | — | `useHudTiles.ts`, `hud-tiles.v1.yaml`, `TopHud.tsx`, `App.tsx` | 1 agent, concurrent with Batch A (disjoint files) |
| **B — sections + picker** | T6 → T7 | [SEQUENTIAL] | T4/T5 (routed grid) | `dashboardSections.ts`, `WidgetPickerDrawer.tsx`, `Dashboard.tsx` | 1 agent, after Batch A |
| **D — close** | T9 | [SEQUENTIAL] (terminal) | A + β + B all green | none (verify only) | 1 agent; no commit |

**Fan-out note:** Batches **A** and **β** run concurrently from the start (disjoint file sets — registry/grid vs HUD/strip). **B** waits for **A** (it imports the routed `renderWidget` + the registry). **D** is the terminal gate after all three. Every task ends in its own `git -C /c/Users/Owner/vox-graphify-gui add … && commit …` (add + commit only; never push/reset/rebase/checkout/clean; on `.git/index.lock`, wait ~20s and retry once). T9 is verification-only and produces no commit.

---

## Self-Review

**Against the writing-plans discipline:**
- **Bite-sized, one concern per step:** each task is one component or one wiring change; every step is failing test → run/fail → minimal impl → run/pass → commit.
- **Exact paths + complete code:** every file is named with its full repo-relative path; every code step ships the complete component (no "similar to Task N", no placeholders).
- **TDD throughout:** each behavior change is preceded by a failing `pnpm vitest run …` and the exact command to prove red→green.
- **Tagged + batched:** every task is `[SEQUENTIAL]`/`[PARALLEL-SAFE]`; the Batch Plan table closes the plan with an explicit parallel fan-out (A ∥ β).

**Against the spec (§4–§7) and the scope:**
- ✅ **Registry-driven composable grid reusing existing tiling:** reuses `DashboardGrid` + `DockShell` (verified); no new tiling engine. The catalog is derived from `SURFACE_REGISTRY` (not a hard-coded switch), so it is "dynamically expandable."
- ✅ **Two render paths + auto-fallback:** `resolveWidget` → purpose-built **else** `SurfaceMiniRender(childRenderer(...))`; a new surface auto-appears as a mini-render (tested explicitly in T6's auto-expansion test + T5's unregistered-surface test).
- ✅ **Purpose-built shortlist exactly:** agents/cost/mesh/approvals/coverage (cost reuses `useLlmSpend` → `get_llm_spend`); everything else falls back (T4 test).
- ✅ **Core monitorables strip, config in Settings (Plan 3C):** reuses the shipped HUD-tile system (`useHudTiles`/`HudTilesEditor` in `SettingsView`); adds the missing `pending_approvals`; disabling drops it from the strip via `resolveVisibleHudTiles` (T8 test) — no bespoke dashboard settings island.
- ✅ **Sectioned (Operations/Cost/Knowledge/Surfaces) from registry groups:** `sectionForNavGroup` over `navGroup`; stays in sync (T6).
- ✅ **Error boundary per widget → compact error tile:** `WidgetErrorBoundary` (T3), wired per slot in T5.
- ✅ **Four vitest cases present:** new-surface→fallback (T5/T6), purpose-built-overrides-fallback (T4/T5), disable-core-removes-from-strip (T8), error-boundary tile (T3); honesty gate guarded (T9).
- ✅ **Repo rules:** pnpm (never npm); no `cargo fmt --all`; surface-registry untouched (read-only) so no generated-TS hand-edit; `vox ci gui-honesty` kept green (T9).
- ✅ **Independent of VG-2;** shares only the surface registry.

**Risks the executor must verify before coding (called out inline):**
1. `DashboardData` field names (`agents[].codename`/`.status`, `peers[].online`, `alerts`) — read `src/types/dashboard.ts` first; match real fields (T4 Step 5 note).
2. `childRenderer` is module-private — export it in T5 (one-word edit; commit `surfaceComponents.tsx`).
3. The dashboard layout localStorage key — confirm `SHELL_PREFERENCE_KEYS.dashboardLayout` = `gui.dashboard.layout.v1` before seeding it in tests (T5 Step 1 note).
4. `Icon.shield` vs `Icon.alert` — read `Icons.tsx` before the TopHud arm (T8 Step 5 note).
5. Any explicit "6 tiles" count assertion in `HudTilesEditor`/HUD tests must move to 7 (T8 Step 7 note).
6. The mini-render's `miniProps` subset — some surfaces' `childRenderer` arms require non-optional handlers; the mini-render is inert, so prefer the purpose-built path or accept honest error-boundary degradation rather than fabricating a handler (T5 Step 3 note).
