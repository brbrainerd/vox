# Dashboard & Top-Bar Unification — Design Spec

**Date:** 2026-06-18
**Status:** Design (approved for planning)
**Author:** Audit + brainstorming session (Claude, Opus 4.8)
**Sibling specs:** [task-list-cascade-spine](2026-06-18-task-list-cascade-spine-design.md) · [activity-log-surface](2026-06-18-activity-log-surface-design.md) · [gamification-surfacing-and-minimap](2026-06-18-gamification-surfacing-and-minimap-design.md)
**Execution target:** Gemini 3.5 Flash inside Antigravity — see [limitations doc](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md).

---

## Amendment 2026-06-18 (unification) — see [unified-task-message-envelope spec](2026-06-18-unified-task-message-envelope-registers-budget-ssot-design.md)

Two additions land on top of the widget-registry SSOT below: (1) **Registers** — the registry's
`render` becomes register-aware (`render({widget, navigate, register})`), with **Office as the
default** (the dense/legible floor) and **gamified opt-in** (falls back to Office when a kind has
no gamified variant); a top-bar toggle switches register without changing truth. (2) The **budget
widget reads the budget SSOT** — the *existing* `BudgetManager` extended with `snapshot()`,
surfaced via `budget_get` + `vox://cost-changed` — not its own `useMetricSeries` sum — so the TopHud tile, the widget, the activity cost-fold, and the gamified
treasury all show one number. Implemented in the unified plan's Tasks 6–7.

---

## 1. Problem

The dashboard shell is genuinely good bones, but half-built and partly hardcoded:

- **6 of 14 widget kinds are dead.** `dashboardLayout.ts` declares `kpi_spark`,
  `mesh_peers`, `model_active`, `openrouter_spend`, `task_summary`, `custom_text`, but
  `Dashboard.tsx`'s `renderWidget()` has no `case` for them → they render `null`. The
  **widget picker still offers them**, so a user can add a widget that shows nothing.
- **Hardcoded, non-widget regions:** the top KPI row (Active Agents / Queue Depth / Budget
  Spent), the `LudusSandbox` mini-map block, and a "Submit tasks in Chat" callout are all
  fixed JSX, not part of the modular grid — you can't remove or rearrange them.
- **The TopHud is already a mini-dashboard** (configurable KPI tiles with sparklines, each
  navigating to a view) — exactly the user's "top bar as a minification of the dashboard"
  idea — but its tile config and the dashboard widget set are **two separate registries**
  that drift.
- **Backend data exists but isn't surfaced as widgets:** mesh peers, active model,
  OpenRouter spend, task summary all flow through App state / TopHud but have no working
  dashboard widget.

The user's ask: *"the dashboard view — what widgets are available, what can be added and
subtracted … the top bar might feed into it as a minification of the dashboard … unify
our design to make it easy to discover, easier to control, with more real-time feedback."*

## 2. Goal

Make the widget system a real, single-source-of-truth catalog and finish the modular
shell, so every advertised widget renders real data and the TopHud is a coherent
minification of the same catalog:

- a **widget registry SSOT** (`{ kind → {label, description, render, defaultGrid, dataKey} }`)
  that drives *both* the dashboard `renderWidget` and the widget picker, so the picker can
  never offer a non-rendering widget;
- **implement the 6 dead widget kinds** (they all have data already: mesh peers, active
  model, OpenRouter spend, task summary, kpi sparkline, custom text);
- add the three sibling-spec widgets — **mini-map**, **activity feed**, **task list** — as
  first-class widget kinds;
- make the hardcoded KPI row / mini-map / callout into **registry widgets** (default
  layout, but removable/movable);
- align the TopHud tile catalog to read from the same registry so the top bar and the
  dashboard never drift.

Non-goals (YAGNI): a new grid engine (keep dnd-kit + localStorage), per-user cloud layout
sync, and theming changes (the token SSOT already exists).

## 3. Architecture

```
 widgetRegistry.ts  (SSOT)
   kind → { label, description, defaultGrid, render(props), dataSelector }
        │                         │                         │
        ▼                         ▼                         ▼
 Dashboard.renderWidget    WidgetPickerDrawer        TopHud tile catalog
   (no local switch)        (lists registry          (subset flagged
                             minus active)             `topHudEligible`)
```

One registry; three consumers. Adding a widget = one registry entry → it appears in the
dashboard, the picker, and (if eligible) the TopHud, automatically. This mirrors the
project's existing "registry-drives-surfaces" pattern (`SURFACE_REGISTRY`).

### 3.1 Components

| Unit | File(s) | Responsibility |
|---|---|---|
| `widgetRegistry` | `vox-gui/ui/src/lib/widgetRegistry.ts` (**new**) | SSOT map: kind → metadata + render fn + default grid + data selector + `topHudEligible`. |
| `Dashboard.renderWidget` | `.../surfaces/Dashboard/Dashboard.tsx` (refactor) | Look up `widgetRegistry[kind].render(props)`; delete the local switch's dead arms. |
| `WidgetPickerDrawer` | `.../dashboard/WidgetPickerDrawer.tsx` (refactor) | Enumerate `widgetRegistry` (minus active), use its `label`/`description`. |
| 6 new widget components | `.../dashboard/widgets/*.tsx` (**new**) | `KpiSparkWidget`, `MeshPeersWidget`, `ModelActiveWidget`, `OpenRouterSpendWidget`, `TaskSummaryWidget`, `CustomTextWidget`. |
| 3 sibling widgets | reuse | `MiniMapWidget` (wraps `LudusSandbox`), `ActivityWidget` (wraps Activity timeline), `TaskListWidget` (wraps `TasksView`). |
| KPI-row → widgets | `Dashboard.tsx` | Move the hardcoded KPI row + mini-map + callout into the default layout as registry widgets. |
| TopHud alignment | `.../layout/TopHud.tsx` + `useHudTiles` hook | Source tile kinds from `widgetRegistry` where `topHudEligible`. |

### 3.2 Widget render contract

Every widget is a pure component `(props: { widget, data, navigate }) => JSX`. The
registry's `dataSelector(appState) → data` keeps data-wiring out of the component, so
widgets are unit-testable with fixture data and the dashboard stays a dumb dispatcher.

## 4. The "minification" model (top bar ↔ dashboard)

- A widget kind may be flagged `topHudEligible: true` (KPI-shaped: a number + delta +
  sparkline + target view).
- The TopHud renders a compact tile for each eligible, enabled kind — same label, same
  data selector, same click-to-navigate target as the full widget.
- Result: the top bar is literally a shrunk projection of the dashboard catalog; toggling
  a kind's tile and adding its widget are the same vocabulary. This is the user's "top bar
  feeds into the dashboard" made structural.

## 5. Error handling

- Unknown `kind` in a saved localStorage layout → registry lookup misses → render a small
  "Unknown widget (kind)" placeholder with a remove button (graceful, not `null`,
  not a crash). This also self-heals layouts saved before a kind was renamed/removed.
- Widget whose `dataSelector` returns empty → that widget's own `EmptyState`.

## 6. Testing strategy

- **Unit (TS):** registry completeness test — **every kind in `dashboardLayout` widget-kind
  union has a registry entry** (this is the test that kills the dead-widget class of bug
  permanently); each new widget renders its fixture data; picker lists exactly
  `registry − active`; unknown-kind placeholder renders.
- **Unit:** `topHudEligible` subset renders as tiles; click navigates to the declared view.
- Vitest, consistent with the existing 56+ vitest files.

## 7. Decomposition into plan tasks (preview)

1. Create `widgetRegistry` SSOT with the 8 already-working kinds + the registry-completeness test.
2. Refactor `Dashboard.renderWidget` to dispatch through the registry (no behavior change) — green.
3. Refactor `WidgetPickerDrawer` to enumerate the registry.
4. Implement the 6 dead widget kinds (one task per 2–3 widgets) + tests.
5. Add `MiniMapWidget` / `ActivityWidget` / `TaskListWidget` wrappers + registry entries.
6. Move hardcoded KPI row / mini-map / callout into the default layout as registry widgets.
7. Align TopHud tiles to the registry via `topHudEligible`.

All TS in `vox-gui/ui/src/`; tasks split by component-file disjointness for
`[PARALLEL-SAFE]` tagging (the registry task is `[SEQUENTIAL]` since others import it).
