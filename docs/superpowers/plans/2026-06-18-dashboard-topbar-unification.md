# Dashboard & Top-Bar Unification — Implementation Plan (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `crates/vox-skills/skills/superpowers/test-driven-development.skill.md`. Steps use `- [ ]` checkboxes.

> **🤖 EXECUTION TARGET — READ FIRST.** Gemini 3.5 Flash inside Google Antigravity (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context). Plan engineered accordingly. Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md). Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.**
2. **Verify-before-use.** `rg`/read before any symbol; reality differs → STOP + report.
3. **Self-contained.**
4. **Two-strike circuit breaker.**
5. **Parallel dispatch** per tags; never two subagents on one file. (The registry task is `[SEQUENTIAL]` — many others import it.)
6. **Vox house rules.** No `cargo fmt --all`; `.vox` automation; `docs/src/` frontmatter.
7. **Verification ritual** before commit: `npm test` + `npm run build` (tsc) from `crates/vox-gui/ui`. Paste output.
8. **Rollback on broken tree.**
9. **TS:** keep components pure/presentational where possible; reuse existing UI primitives + tokens; no new theme system.

**Goal:** A single widget-registry SSOT that drives the dashboard, the picker, and the TopHud — implementing the 6 dead widget kinds, adding mini-map/activity/task widgets, converting hardcoded regions to registry widgets, and aligning the top bar as a minification of the dashboard.

**Architecture:** Introduce `widgetRegistry.ts` (`kind → {label, description, defaultGrid, render, topHudEligible?}`). `Dashboard.renderWidget`, `WidgetPickerDrawer`, and the TopHud tile catalog all read from it. A completeness test forbids a declared kind without a registry entry — killing the dead-widget bug class permanently.

> **⚠️ DATA-WIRING RULE (verified constraint — read before Task 1).** The existing chart widgets get their data from **React hooks** inside `Dashboard.tsx` (`useMetricSeries('budget_burn')`, `useMetricSeries('queue_depth')`). A plain registry `render(props)` function **cannot call hooks** (Rules of Hooks). Therefore **`render` must return a self-contained component that calls its own hooks internally** — e.g. `render: (props) => <BudgetBurnWidget />`, where `BudgetBurnWidget` calls `useMetricSeries(...)` itself. Do **NOT** try to pass live `budgetSeries`/`queueSeries` through a `dataSelector(appState)` snapshot — that loses real-time updates. `render` receives only `{ widget, navigate }` (config + nav); each widget sources its own live data via hooks. (This is why `dataSelector` is dropped from the registry shape above.)

**Tech Stack:** React/TS, dnd-kit (existing grid), Recharts (existing), vitest.

**Design:** [`../specs/2026-06-18-dashboard-topbar-unification-design.md`](../specs/2026-06-18-dashboard-topbar-unification-design.md).

> **Sibling dependency note:** Task 5 adds `MiniMapWidget`/`ActivityWidget`/`TaskListWidget` wrappers. The mini-map (`LudusSandbox`) is from the gamification plan; the activity surface from the activity-log plan; `TasksView` from the cascade-spine plan. If those components don't yet exist when you reach Task 5, wrap a placeholder `EmptyState` ("coming soon") instead and note it — do NOT block the registry work.

---

## Flash Execution Addendum (2026-06-18 — second hardening pass)

These override task granularity + wave order where they conflict. Source: Flash-executability critique.

**Global gates:**
1. Each Step-1 `rg`/read is a **BLOCKING gate** — paste output before any code step; reality differs → STOP.
2. **Split-on-overrun:** one atomic green commit per sub-bullet when a step touches >1 file or >1 new component.

**Mandatory splits + reorder:**
- **Task 1 → 1a / 1b.** 1a: create `widgetRegistry.ts` with all 14 entries using **placeholder `render: () => <EmptyState/>`** for every kind + the completeness test; commit green. 1b: refactor the 8 working widget components so each **sources its own data via hooks internally** (e.g. `AreaChartWidget` calls `useMetricSeries('budget_burn')` itself), since the registry `render({widget, navigate})` cannot pass live `budgetSeries`/`queueSeries` (Rules of Hooks); then point the 8 registry entries at those components; commit.
- **Task 4 → 4a / 4b.** 4a: the 3 simple widgets (`KpiSparkWidget`, `CustomTextWidget`, `ModelActiveWidget`) — fixture/props only; commit. 4b: the 3 data-wiring widgets (`MeshPeersWidget`, `OpenRouterSpendWidget`, `TaskSummaryWidget`) — each preceded by an `rg` confirming its App-state selector exists (placeholder + note if missing); commit.
- **Task 6 (`upgradeLayoutIfNeeded`):** detection = `!layout.widgets.some(w => w.kind === 'kpi_spark')` → prepend the default KPI + mini-map widgets; **idempotent** (returns unchanged if already present); test runs it twice and asserts no duplicates. Inline this logic.
- **Task 7:** extend the registry entry type with `topHudLabel?` + `topHudNavTarget?` (the nav target was unspecified); eligible tiles = `Object.entries(widgetRegistry).filter(([,e]) => e.topHudEligible)`, navigating to `e.topHudNavTarget`.

**Corrected wave order (supersedes the Parallel-waves section below):**
1. Task 1a → 2. Task 1b → 3. **parallel** {Task 2, Task 3, Task 4a} (disjoint: `Dashboard.tsx` / `WidgetPickerDrawer.tsx` / `widgets/*`+registry) → 4. Task 4b (registry) → 5. **sequential** Task 5 → Task 6 → Task 7 (all share registry/layout). `widgetRegistry.ts` is written by 1a/1b/4a/4b/5/7 → those are never concurrent.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-gui/ui/src/lib/widgetRegistry.ts` | SSOT widget catalog | Create (Task 1) |
| `crates/vox-gui/ui/src/lib/dashboardLayout.ts` | widget-kind union, default layout | Modify (Tasks 1, 6) |
| `.../surfaces/Dashboard/Dashboard.tsx` | dispatch via registry; move hardcoded regions | Modify (Tasks 2, 6) |
| `.../dashboard/WidgetPickerDrawer.tsx` | enumerate registry | Modify (Task 3) |
| `.../dashboard/widgets/*.tsx` | 6 new widget components | Create (Task 4) |
| `.../dashboard/widgets/{MiniMap,Activity,TaskList}Widget.tsx` | sibling-feature wrappers | Create (Task 5) |
| `.../layout/TopHud.tsx` + `useHudTiles` hook | source tiles from registry | Modify (Task 7) |

**Pre-flight (run once, paste output):**
- `rg -n "WidgetKind|kind:|widget" crates/vox-gui/ui/src/lib/dashboardLayout.ts` — confirm the widget-kind union (all 14 kinds) + the default layout export.
- `rg -n "renderWidget|case '|switch" crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` — confirm the current switch + which 8 kinds render.
- `rg -n "WidgetPickerDrawer|available|kinds" crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.tsx` — confirm how it lists addable widgets.
- `rg -n "useHudTiles|topHud|tile" crates/vox-gui/ui/src/components/layout/TopHud.tsx` — confirm the tile config hook.
- `cd crates/vox-gui/ui && npm test -- --run` — baseline tests green.

---

## Task 1 `[SEQUENTIAL]`: `widgetRegistry` SSOT + completeness test

**Files:**
- Create: `crates/vox-gui/ui/src/lib/widgetRegistry.ts` (+ `widgetRegistry.test.ts`)
- Modify: `crates/vox-gui/ui/src/lib/dashboardLayout.ts` (export the kind union if not already)

- [ ] **Step 1 (verify-before-use):** **VERIFIED:** `dashboardLayout.ts` exports a const array **`DASHBOARD_WIDGET_KINDS`** (14 kinds) and a type **`DashboardWidgetKind`** (NOT `WidgetKind`). Iterate the const directly so the test is order-independent and self-maintaining. Confirm with `rg -n "DASHBOARD_WIDGET_KINDS|DashboardWidgetKind" crates/vox-gui/ui/src/lib/dashboardLayout.ts`.

- [ ] **Step 2: Write the failing test.** Create `widgetRegistry.test.ts` — import the real constant and iterate it (do NOT hardcode/reorder the list):

```ts
import { describe, it, expect } from 'vitest';
import { widgetRegistry } from './widgetRegistry';
import { DASHBOARD_WIDGET_KINDS } from './dashboardLayout';

describe('widgetRegistry', () => {
  it('has an entry for every declared widget kind', () => {
    for (const k of DASHBOARD_WIDGET_KINDS) {
      expect(widgetRegistry[k], `missing registry entry for ${k}`).toBeDefined();
      expect(typeof widgetRegistry[k].render).toBe('function');
      expect(widgetRegistry[k].label.length).toBeGreaterThan(0);
    }
  });
});
```

Driving the loop off `DASHBOARD_WIDGET_KINDS` means adding a kind to the union later automatically extends this test — the dead-widget bug class stays dead.

- [ ] **Step 3: Run → FAIL.** `npm test -- widgetRegistry` → FAIL (file missing).

- [ ] **Step 4: Implement.** Create `widgetRegistry.ts` mapping **all 14 kinds** to `{ label, description, defaultGrid, render(props: { widget, navigate }), topHudEligible? }`. Per the DATA-WIRING RULE above, each `render` returns a **self-contained component that calls its own hooks** — for the 8 working kinds, wrap the existing widget component (e.g. `render: () => <AreaChartWidget kind="budget_burn" />`, where the chart widget calls `useMetricSeries` internally). For the 6 not-yet-implemented kinds, set `render` to a temporary `() => <EmptyState title="…"/>` so the completeness test passes now (Task 4 replaces them). This keeps the tree green.

- [ ] **Step 5: Run → PASS.** `npm test -- widgetRegistry` → PASS; `npm run build` clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/lib/widgetRegistry.ts crates/vox-gui/ui/src/lib/widgetRegistry.test.ts crates/vox-gui/ui/src/lib/dashboardLayout.ts
git commit -m "feat(gui): widget registry SSOT + completeness test"
```

---

## Task 2 `[PARALLEL-SAFE]` (Wave 2; only file: `Dashboard.tsx`): dispatch `renderWidget` through the registry

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`

- [ ] **Step 1 (verify-before-use):** Read the current `renderWidget` switch (Pre-flight). Note the props each case receives (`data`, series, `navigate`).

- [ ] **Step 2: Write the failing test.** Add `Dashboard.render.test.tsx`: rendering a widget of kind `agents` via the dashboard's render path produces the same agents content as before (assert a known agent row appears). Then assert that kind `mesh_peers` renders the registry's component (placeholder for now), not `null`.

- [ ] **Step 3: Run → FAIL.** `npm test -- Dashboard.render` → FAIL.

- [ ] **Step 4: Implement.** Replace the switch body with `const entry = widgetRegistry[widget.kind]; return entry ? entry.render({ widget, navigate }) : <UnknownWidget kind={widget.kind} />;` (note: `render` takes only `{ widget, navigate }` — widgets fetch their own live data via hooks per the DATA-WIRING RULE). Add a small `UnknownWidget` placeholder (label + remove button). Delete the dead `default → null` arms.

- [ ] **Step 5: Run → PASS.** `npm test -- Dashboard.render` → PASS; full suite + build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx
git commit -m "refactor(gui): dashboard renders widgets via registry (no dead null arms)"
```

---

## Task 3 `[PARALLEL-SAFE]` (after Task 1): picker enumerates the registry

**Files:**
- Modify: `crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.tsx` (+ test)

- [ ] **Step 1 (verify-before-use):** Read `WidgetPickerDrawer.tsx` (Pre-flight) — how it currently computes addable kinds.

- [ ] **Step 2: Write the failing test.** `WidgetPickerDrawer.test.tsx`: given active layout has `stream`, the picker lists every other registry kind (using registry `label`), and does NOT list `stream`.

- [ ] **Step 3: Run → FAIL.** `npm test -- WidgetPickerDrawer` → FAIL.

- [ ] **Step 4: Implement.** Compute `addable = Object.keys(widgetRegistry).filter(k => !activeKinds.includes(k))`; render each with `widgetRegistry[k].label` + `.description`.

- [ ] **Step 5: Run → PASS.** `npm test -- WidgetPickerDrawer` → PASS; build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.tsx crates/vox-gui/ui/src/components/dashboard/WidgetPickerDrawer.test.tsx
git commit -m "refactor(gui): widget picker enumerates the registry"
```

---

## Task 4 `[PARALLEL-SAFE]` (after Task 1): implement the 6 dead widget kinds

**Files:**
- Create: `crates/vox-gui/ui/src/components/dashboard/widgets/{KpiSparkWidget,MeshPeersWidget,ModelActiveWidget,OpenRouterSpendWidget,TaskSummaryWidget,CustomTextWidget}.tsx` (+ tests)
- Modify: `widgetRegistry.ts` (swap the 6 placeholders for real components)

- [ ] **Step 1 (verify-before-use):** `rg -n "peers|mesh_throughput|active_model|openrouter|total_cost|budget_cap" crates/vox-gui/ui/src/ -l` to confirm the App-state fields each widget reads (the TopHud already consumes peers/model/spend — reuse those selectors).

- [ ] **Step 2: Write the failing tests.** One test per widget rendering its fixture data, e.g. `MeshPeersWidget.test.tsx`: given `peers=[{id:'p1'},{id:'p2'}]`, renders "2 peers". `CustomTextWidget`: renders `widget.config.text`.

- [ ] **Step 3: Run → FAIL.** `npm test -- widgets/` → FAIL.

- [ ] **Step 4: Implement.** Build the 6 components (pure, props-driven, reuse `Kpi`/`Sparkline`/`Glass`/`EmptyState`):
  - `KpiSparkWidget` — a single metric + sparkline (config-driven dataKey).
  - `MeshPeersWidget` — peer count + per-peer VRAM/throughput list.
  - `ModelActiveWidget` — active model name + provider.
  - `OpenRouterSpendWidget` — spend vs budget cap.
  - `TaskSummaryWidget` — queued/in-progress/completed counts.
  - `CustomTextWidget` — renders `widget.config.text` (markdown-lite or plain).
  Replace the 6 placeholder `render` entries in `widgetRegistry.ts` with these.

- [ ] **Step 5: Run → PASS.** `npm test -- widgets/` → PASS; `npm test -- widgetRegistry` still PASS; build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/dashboard/widgets/ crates/vox-gui/ui/src/lib/widgetRegistry.ts
git commit -m "feat(gui): implement 6 previously-dead dashboard widgets"
```

---

## Task 5 `[SEQUENTIAL]` (Wave 3; shares `widgetRegistry.ts` with Task 4 + `dashboardLayout.ts` with Task 6): mini-map / activity / task-list widget wrappers

**Files:**
- Create: `.../dashboard/widgets/{MiniMapWidget,ActivityWidget,TaskListWidget}.tsx` (+ tests)
- Modify: `widgetRegistry.ts` (3 new kinds) + `dashboardLayout.ts` (extend the union)

- [ ] **Step 1 (verify-before-use):** Check whether sibling components exist: `rg -n "LudusSandbox|ActivityTimeline|TaskComposer|TasksView" crates/vox-gui/ui/src/ -l`. For any missing one, wrap an `EmptyState` placeholder and note it (per the dependency note above) — do not block.

- [ ] **Step 2: Write the failing tests.** Each wrapper renders its child (or placeholder) inside a widget frame; `TaskListWidget` renders a `TaskComposer` if present.

- [ ] **Step 3: Run → FAIL.** `npm test -- MiniMapWidget ActivityWidget TaskListWidget` → FAIL.

- [ ] **Step 4: Implement.** Add the 3 widget kinds to the `DASHBOARD_WIDGET_KINDS` const + `DashboardWidgetKind` union in `dashboardLayout.ts`, add registry entries, and create the 3 wrapper components (each composes the sibling-feature component or an `EmptyState`).

- [ ] **Step 5: Run → PASS.** All three tests + `widgetRegistry` completeness (now 17 kinds) PASS; build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/dashboard/widgets/ crates/vox-gui/ui/src/lib/widgetRegistry.ts crates/vox-gui/ui/src/lib/dashboardLayout.ts
git commit -m "feat(gui): mini-map/activity/task-list dashboard widgets"
```

---

## Task 6 `[SEQUENTIAL]` (same file as Task 2): hardcoded regions → registry widgets

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` + `dashboardLayout.ts` (default layout)

- [ ] **Step 1 (verify-before-use):** Read the hardcoded JSX in `Dashboard.tsx`: the top KPI row, the `LudusSandbox` mini-map block, the "Submit tasks in Chat" callout. Confirm they sit outside the grid.

- [ ] **Step 2: Write the failing tests.** (a) `Dashboard.defaultLayout.test.ts`: the default layout now includes a mini-map widget and at least one KPI widget (assert their kinds present). (b) **Migration test** `dashboardLayout.migrate.test.ts`: `upgradeLayoutIfNeeded({ widgets: [{kind:'stream'}, {kind:'agents'}] })` returns a layout that now **includes** the KPI + mini-map widgets (prepended), and is idempotent (running it twice doesn't duplicate them).

- [ ] **Step 3: Run → FAIL.** `npm test -- Dashboard.defaultLayout dashboardLayout.migrate` → FAIL.

- [ ] **Step 4: Implement.** (a) Remove the hardcoded KPI row / mini-map / callout JSX; add equivalent entries to the **default** layout array in `dashboardLayout.ts`. (b) **REGRESSION GUARD (verified gap):** the `UnknownWidget` self-heal only handles *unknown* kinds — it does NOT restore *missing expected* widgets for existing users whose saved localStorage layout predates this change (they'd silently lose the KPI row/mini-map). Add `export function upgradeLayoutIfNeeded(layout): DashboardLayout` that detects a pre-migration layout (no `kpi_spark`/`mini_map` kinds present) and prepends the default KPI + mini-map widgets; make it idempotent. Call it where the layout is loaded from `useLocalStorage(SHELL_PREFERENCE_KEYS.dashboardLayout, ...)` in `Dashboard.tsx`. Keep the visual default close to today.

- [ ] **Step 5: Run → PASS.** `npm test -- Dashboard.defaultLayout` → PASS; full suite + build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx crates/vox-gui/ui/src/lib/dashboardLayout.ts
git commit -m "refactor(gui): hardcoded dashboard regions become removable registry widgets"
```

---

## Task 7 `[SEQUENTIAL]` (after Task 1): TopHud tiles from the registry

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/TopHud.tsx` + the `useHudTiles` hook (+ test)

- [ ] **Step 1 (verify-before-use):** Read `TopHud.tsx` + `useHudTiles` (Pre-flight). Confirm the current tile kinds (`active_agents, queue_depth, budget_burn, mesh_peers, active_model, openrouter_spend`) and their navigate targets.

- [ ] **Step 2: Write the failing test.** `TopHud.tiles.test.tsx`: the eligible tile set equals `Object.keys(widgetRegistry).filter(k => widgetRegistry[k].topHudEligible)`; clicking a tile calls `navigate` with the registry entry's target view.

- [ ] **Step 3: Run → FAIL.** `npm test -- TopHud.tiles` → FAIL.

- [ ] **Step 4: Implement.** Mark KPI-shaped registry kinds `topHudEligible: true` with a `navTarget`. Source the TopHud tile catalog from `widgetRegistry` (label, dataSelector, navTarget) instead of a separate list. Keep the slim/hidden/full modes.

- [ ] **Step 5: Run → PASS.** `npm test -- TopHud.tiles` → PASS; full suite + build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/layout/TopHud.tsx
git commit -m "refactor(gui): TopHud tiles sourced from the widget registry (top bar = mini dashboard)"
```

---

## Parallel waves

**Shared-file map (the source of truth for parallelism):** `widgetRegistry.ts` is written by Tasks 1, 4, 5, 7. `dashboardLayout.ts` by Tasks 1, 5, 6. `Dashboard.tsx` by Tasks 2, 6. Two subagents must never share any of these.

- **Wave 1:** Task 1 `[SEQUENTIAL]` (creates registry + completeness test — everything imports it).
- **Wave 2 (parallel — disjoint files):** Task 2 (`Dashboard.tsx`) ∥ Task 3 (`WidgetPickerDrawer.tsx`) ∥ Task 4 (`widgets/*` + `widgetRegistry.ts`). These three write disjoint file sets.
- **Wave 3 (STRICTLY sequential — they share `widgetRegistry.ts` and/or `dashboardLayout.ts`):** Task 5 (registry + layout) → Task 6 (Dashboard + layout) → Task 7 (registry + TopHud). Run one at a time on one agent; never parallelize.

> Correction vs first draft: Task 4 also writes `widgetRegistry.ts`, and Task 7 writes it too — so Tasks 4/5/7 cannot all be parallel. The only safe parallel wave is {2,3,4}; everything touching the registry or layout after that is serialized.

## Self-review checklist

- [ ] Spec §3 covered: registry (1), dashboard dispatch (2), picker (3), 6 dead widgets (4), 3 new widgets (5), hardcoded→widgets (6), TopHud alignment (7). ✔
- [ ] Completeness test (Task 1) guarantees no future declared kind renders `null`. ✔
- [ ] Unknown-kind placeholder (Task 2) handles *unknown* kinds; `upgradeLayoutIfNeeded` (Task 6) handles *missing expected* kinds for pre-migration users. ✔
- [ ] Symbol consistency: `widgetRegistry`, `DASHBOARD_WIDGET_KINDS`/`DashboardWidgetKind` (verified real names), `UnknownWidget`, `upgradeLayoutIfNeeded`, `topHudEligible`. ✔
- [ ] `render({ widget, navigate })` only — widgets call their own hooks (Rules-of-Hooks safe; no `dataSelector` snapshot). ✔
- [ ] Parallelism: only Wave 2 {2,3,4} runs concurrently; registry/layout writers (4→5→7, 5→6) serialized. ✔
- [ ] Sibling dependency on mini-map/activity/tasks handled with placeholder fallback (no block). ✔
