# vox-gui Operator Console v2 — Configurable Dashboard, Persistent Status, OmniSearch & Chat-First Execution

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Fresh session?** Read the handoff first: [`docs/src/architecture/gui-operator-console-v2-handoff-2026-06-16.md`](../../src/architecture/gui-operator-console-v2-handoff-2026-06-16.md) — shipped state, invariants, verification, remaining work (plan checkboxes may lag code).

**Goal:** Transform `vox-gui` from a fixed triptych dashboard + decorative HUD into a **user-composable operator console**: configurable dashboard widgets and status bar, fully collapsible/draggable shell chrome, **true OmniSearch** across every navigable artifact, **Chat/Imperium as a dedicated full-page view** with inline agent execution + task/resource/cost visibility, and **gamification as ambient rewards** (not a primary nav destination).

**Architecture:** Three SSOT layers — (1) **layout contracts** in `contracts/gui/` for dashboard layouts, HUD tiles, and shell persistence; (2) **runtime state** via `voxTransport` + Tier-A GUI prefs (not raw `localStorage` sprawl); (3) **federated search index** built from registry + policy catalog + settings index + action manifest + live orchestrator snapshots, with `vox-search` as the backend corpus lane. Shell uses **dockview** (already integrated) for draggable panels, **@dnd-kit** for widget grid reorder, **Recharts** (or visx primitives) for time-series widgets styled with existing `Glass` + `visualTokens`. Top status bar becomes **always-visible slim mode** (never fully hidden on operator surfaces) fed by real daemon/MCP signals — mesh placeholders removed from sidebar; mesh KPIs move to configurable HUD tiles.

**Tech Stack:** Tauri 2, React 19, TanStack Query v5, dockview 6.x, @dnd-kit/core, Recharts 3.x (dashboard charts), vitest, Playwright, `vox-search`, `vox-gamify` event router, contracts in `contracts/gui/*.v1.yaml`.

**Design north stars (binding):**
- `docs/src/architecture/gui-frontend-design-principles-2026-06-14.md` — visibility of system status, user control, recognition over recall, flexibility for experts
- `docs/src/architecture/vox-dashboard-design-brief-2026.md` — kill Latin LARP labels, persistent status bar, empty states with actions, ⌘K omni-search
- `docs/src/architecture/vox-gui-visual-audit-2026-06-03.md` — TopHud collision fixes, real connectivity semantics
- `docs/superpowers/plans/2026-06-12-async-chat-tasklist-resource-scheduling-omnisearch.md` — Tracks A–K (tasks, sessions, scaling, OmniSearch federation)

**Supersedes / extends:** `2026-06-16-gui-roadmap-remaining.md` Phases for dashboard/HUD/search/chat; does **not** replace Wave 2–6 crate IPC work — references it.

**Parallel dispatch map (independent tracks):**
| Track | Owner plan section | Can run parallel with |
|-------|-------------------|------------------------|
| A — Layout contracts + persistence | Phase 0 | B, C, D |
| B — Status bar / HUD real data | Phase 1 | C, E |
| C — Configurable dashboard widgets | Phase 2 | B, D |
| D — OmniSearch federation | Phase 3 | A, C |
| E — Chat/Imperium view + execution rail | Phase 4 | D |
| F — Gamification sprinkles | Phase 5 | all (event-only) |

---

## File structure (new / modified)

| File | Action | Responsibility |
|------|--------|----------------|
| `contracts/gui/dashboard-layout.v1.yaml` | Create | Widget catalog + layout schema version |
| `contracts/gui/hud-tiles.v1.yaml` | Create | Allowed TopHud tile types + data bindings |
| `contracts/gui/omnisearch-index.v1.yaml` | Create | Federated index kinds + refresh policy |
| `contracts/gui/shell-persistence.v1.yaml` | Create | Keys for sidebar/HUD/dock/chat-rail prefs |
| `crates/vox-gui/ui/src/lib/dashboardLayout.ts` | Create | Parse/validate layout contract |
| `crates/vox-gui/ui/src/lib/dashboardLayout.test.ts` | Create | Layout validation tests |
| `crates/vox-gui/ui/src/components/dashboard/DashboardGrid.tsx` | Create | DnD widget grid host |
| `crates/vox-gui/ui/src/components/dashboard/widgets/*.tsx` | Create | One file per widget type |
| `crates/vox-gui/ui/src/components/layout/StatusBar.tsx` | Create | Persistent bottom or top slim bar |
| `crates/vox-gui/ui/src/components/layout/TopHud.tsx` | Modify | Configurable tiles; remove IMPERIUM LARP |
| `crates/vox-gui/ui/src/components/layout/AppShell.tsx` | Create | Extract shell from App.tsx |
| `crates/vox-gui/ui/src/hooks/useHudTiles.ts` | Create | HUD tile config + live data merge |
| `crates/vox-gui/ui/src/hooks/useFederatedSearchIndex.ts` | Create | Build client index for OmniSearch |
| `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx` | Create | Task list + resource strip beside chat |
| `crates/vox-gui/ui/src/components/gamify/AchievementToast.tsx` | Create | Ambient XP/achievement surfacing |
| `crates/vox-gui/ui/src/components/gamify/AchievementsDrawer.tsx` | Create | Earned rewards menu (replaces Gamify nav) |
| `docs/src/reference/gui-navigation.md` | Modify | Chat-first IA, achievements drawer |

---

## Phase 0 — Shell persistence contracts & AppShell extraction

### Task 0.1: Dashboard layout contract (TDD)

**Files:**
- Create: `contracts/gui/dashboard-layout.v1.yaml`
- Create: `crates/vox-gui/ui/src/lib/dashboardLayout.test.ts`
- Create: `crates/vox-gui/ui/src/lib/dashboardLayout.ts`

- [ ] **Step 1: Write failing test**

```typescript
import { describe, it, expect } from 'vitest';
import { validateDashboardLayout, defaultDashboardLayout } from './dashboardLayout';

describe('dashboardLayout', () => {
  it('default layout has stream, agents, and alerts widgets', () => {
    const layout = defaultDashboardLayout();
    expect(layout.widgets.map(w => w.kind)).toEqual(
      expect.arrayContaining(['stream', 'agents', 'alerts']),
    );
  });

  it('rejects unknown widget kind', () => {
    expect(() =>
      validateDashboardLayout({
        version: 1,
        columns: 12,
        widgets: [{ id: 'x', kind: 'not-real', grid: { col: 1, row: 1, w: 4, h: 2 } }],
      }),
    ).toThrow(/unknown widget kind/i);
  });
});
```

- [ ] **Step 2: Run test — FAIL**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/lib/dashboardLayout.test.ts`

- [ ] **Step 3: Implement contract + validator**

`contracts/gui/dashboard-layout.v1.yaml` widget kinds (v1): `stream`, `agents`, `alerts`, `kpi_spark`, `line_chart`, `bar_chart`, `area_chart`, `queue_depth`, `budget_burn`, `mesh_peers`, `model_active`, `openrouter_spend`, `task_summary`, `custom_text`.

- [ ] **Step 4: Run test — PASS**

- [ ] **Step 5: Commit** `feat(gui): dashboard layout contract v1`

### Task 0.2: Extract AppShell from App.tsx

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/AppShell.tsx`
- Modify: `crates/vox-gui/ui/src/App.tsx`
- Modify: `crates/vox-gui/ui/src/App.test.tsx`

- [ ] **Step 1: Failing test** — `AppShell` renders sidebar + status region + `children` slot without duplicating Loquela when `chatDocked={false}`

- [ ] **Step 2–4: Move layout JSX; pass `surface`, `onNavigate`, KPI props**

- [ ] **Step 5: Commit** `refactor(gui): extract AppShell`

### Task 0.3: Shell persistence SSOT

**Files:**
- Create: `contracts/gui/shell-persistence.v1.yaml`
- Modify: `crates/vox-gui/ui/src/hooks/useLocalStorage.ts` (document keys)

- [ ] **Step 1: Enumerate keys** — migrate `vox_sidebar_mode`, `vox_hud_mode`, `vox_parent_tabs`, `gui.layout.v1`, `vox.spark.kpi.*` into contract

- [ ] **Step 2: Add `voxTransport.getGuiPreference` / `setGuiPreference` wrappers** for each key (shrink direct localStorage in new code)

- [ ] **Step 3: `vox ci gui-surface-registry` drift check** includes shell-persistence contract

- [ ] **Step 4: Commit**

---

## Phase 1 — Persistent status bar & real HUD tiles (remove placeholders)

### Task 1.1: StatusBar component (always visible)

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/StatusBar.tsx`
- Create: `crates/vox-gui/ui/src/components/layout/StatusBar.test.tsx`

- [ ] **Step 1: Failing test** — StatusBar shows orchestrator freshness pill + clickable segments for queue, budget, model, mesh peers

- [ ] **Step 2: Implement** — slim bar; **cannot** be hidden on operator surfaces (replaces `HudMode.hidden` for production; hidden reserved for demo/screenshot mode only)

- [ ] **Step 3: Wire `freshnessTone` from `useFreshness`**

- [ ] **Step 4: Commit**

### Task 1.2: Replace mesh throughput placeholder

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (`applyStatus`)
- Modify: `crates/vox-gui/ui/src/hooks/useOrchestratorStatus.ts`

- [ ] **Step 1: Test** — when `mesh_throughput` absent, HUD shows peer count from `status.peers.length` not `0 MB/s`

- [ ] **Step 2: Optional enrich** — poll `vox_mesh_queue_stats` when compute parent active (reuse MeshView transport)

- [ ] **Step 3: Remove hardcoded `delta: 0` where backend provides trend**

- [ ] **Step 4: Commit**

### Task 1.3: Sidebar identity truthfulness

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`

- [ ] **Step 1: Failing test** — online dot reflects `freshnessTone`, not always green

- [ ] **Step 2: Remove fake "always online" gradient dot semantics**

- [ ] **Step 3: Rename Settings aria-label** (done in prior PR) — verify policy badge on Policies nav under Runs, not Settings

- [ ] **Step 4: Commit**

### Task 1.4: TopHud de-LARP + configurable tiles

**Files:**
- Create: `contracts/gui/hud-tiles.v1.yaml`
- Create: `crates/vox-gui/ui/src/hooks/useHudTiles.ts`
- Modify: `crates/vox-gui/ui/src/components/layout/TopHud.tsx`

- [ ] **Step 1: Replace "IMPERIUM" / Latin branding** with `Operator` / workspace name from `get_identity_summary`

- [ ] **Step 2: Tile registry** — user can enable/disable/reorder tiles via Settings › Display › HUD (persist `gui.hud.tiles.v1`)

- [ ] **Step 3: Default tiles:** `active_agents`, `queue_depth`, `budget_burn`, `mesh_peers`, `active_model`, `openrouter_spend` (last from `get_llm_spend` poll)

- [ ] **Step 4: Playwright** — status bar visible on dashboard, console, chat, policies

- [ ] **Step 5: Commit**

---

## Phase 2 — Configurable dashboard widget grid

### Task 2.1: Add chart dependencies (evaluated)

**Decision matrix (2026-06 audit):**

| Library | Fit for vox-gui | Verdict |
|---------|-----------------|--------|
| **Recharts 3** | Composable React, SVG, matches Tailwind/Glass | **Primary** — line/bar/area widgets |
| **@visx/*`** | Primitives only, smallest bundle for spark extensions | **Secondary** — custom spark + brush |
| **Apache ECharts** | Canvas, huge datasets | Optional Phase 2b for 10k+ points |
| **Tremor** | shadcn-adjacent | **Reject** — fights brass/zinc design system |
| **dashcraft** | Headless DnD grid + Recharts | **Evaluate fork** — patterns only, not npm dep (supply-chain) |
| **dockview** (existing) | Panel drag/split | **Use** for multi-surface splits; dashboard uses CSS grid + dnd-kit |

- [ ] **Step 1:** `pnpm add recharts @dnd-kit/core @dnd-kit/sortable` in `crates/vox-gui/ui`

- [ ] **Step 2:** Bundle budget test — dashboard chunk < 120kB gzipped added

- [ ] **Step 3: Commit** `chore(gui): add recharts and dnd-kit for dashboard widgets`

### Task 2.2: DashboardGrid with drag-resize

**Files:**
- Create: `crates/vox-gui/ui/src/components/dashboard/DashboardGrid.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`

- [ ] **Step 1: Failing test** — reorder widget changes persisted layout pref

- [ ] **Step 2: Implement 12-col grid** — drag handle per widget; resize via corner (min 2×2 cells)

- [ ] **Step 3: Edit mode toggle** — "Customize dashboard" in overflow menu

- [ ] **Step 4: Widget picker drawer** — add/remove from catalog

- [ ] **Step 5: Commit**

### Task 2.3: Time-series widgets (real data)

**Files:**
- Create: `crates/vox-gui/ui/src/components/dashboard/widgets/LineChartWidget.tsx`
- Create: `crates/vox-gui/ui/src/components/dashboard/widgets/BarChartWidget.tsx`
- Create: `crates/vox-gui/ui/src/hooks/useMetricSeries.ts`

- [ ] **Step 1: Series SSOT** — extend `usePersistedSparkWindow` OR new `vox.metric.series.v1` pref fed by orchestrator events (`cost_incurred`, `task_completed`)

- [ ] **Step 2: Widget config** — metric key, range (1h/6h/24h/7d), chart type

- [ ] **Step 3: Tests** — vitest with mocked series; Playwright snapshot optional

- [ ] **Step 4: Commit**

### Task 2.4: Migrate fixed triptych to default layout profile

- [ ] **Step 1:** Ship `defaultDashboardLayout()` matching current Stream + Agents + Alerts geometry

- [ ] **Step 2:** One-click "Reset to default" in customize mode

- [ ] **Step 3: Commit**

---

## Phase 3 — True OmniSearch (all GUI levels)

### Task 3.1: Federated index builder

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useFederatedSearchIndex.ts`
- Create: `crates/vox-gui/ui/src/lib/federatedSearchIndex.test.ts`
- Create: `contracts/gui/omnisearch-index.v1.yaml`

Index kinds (v1):

| Kind | Source | Refresh |
|------|--------|---------|
| `surface` | `SURFACE_REGISTRY` | build |
| `setting` | `SETTINGS_INDEX` + codegen | build |
| `policy` | `policy_list` IPC | 60s |
| `command` | `get_command_catalog` | session |
| `action` | `get_action_manifest` | session |
| `task` | `list_orchestrator_tasks` | 5s when runs parent |
| `skill` | catalog entries | session |
| `doc` | `vox_docs_index` | lazy |
| `chat_session` | `chat_list_sessions` | 30s |
| `achievement` | gamify profile | on event |
| `corpus` | `vox_search_query` | debounced |

- [ ] **Step 1: Failing test** — policy row appears when indexing `fmt.rust`

- [ ] **Step 2: Implement builder + memoization**

- [ ] **Step 3: Commit**

### Task 3.2: Unify CommandPalette keyboard selection

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/CommandPalette.tsx`

- [ ] **Step 1: Include `fedItems` in `selectableRows`** (fixes G1 from omnisearch plan)

- [ ] **Step 2: Fix `/` prefix** — skills visible in skills mode

- [ ] **Step 3: Prefix legend in empty state** (`> cmd`, `@ agent`, `/ docs+skills`)

- [ ] **Step 4: e2e** — keyboard navigate to policy surface

- [ ] **Step 5: Commit**

### Task 3.3: OmniSearch placement — dual entry

- [ ] **Sidebar filter input** (collapsible) — filters `TOP_LEVEL_VIEWS` + child tabs
- [ ] **Top bar faux-search** — always shows `Search or jump… ⌘K` (design brief §4)
- [ ] **Full Search surface** — advanced facets unchanged
- [ ] **Commit**

### Task 3.4: Backend `settings` corpus OR client-only flag

- [ ] Implement `settings` search in `search.rs` OR document as client-federated-only and remove from backend scope map

---

## Phase 4 — Chat / Imperium dedicated view + execution rail

### Task 4.1: Undock Loquela from global shell

**Files:**
- Modify: `AppShell.tsx`, `ChatSurface.tsx`, `Loquela.tsx`

- [ ] **Step 1: `chatDocked` prop** — default `false`; composer lives only in Chat surface

- [ ] **Step 2: Other surfaces** — ⌘K action "Submit task" navigates to Chat with focus

- [ ] **Step 3: Remove `pb-[180px]`** reserve when undocked

- [ ] **Step 4: Playwright** — console view has no composer; chat view has composer

- [ ] **Step 5: Commit**

### Task 4.2: ChatExecutionRail (tasks + resources + cost)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx`
- Modify: `ChatSurface.tsx`

Layout:

```
┌─────────────────────────────────────────────────────────────┐
│ StatusBar (global)                                          │
├──────────────┬──────────────────────────┬───────────────────┤
│ Sessions     │ Transcript + inline       │ Execution rail    │
│ (tabs)       │ agent steps / tool calls  │ ├─ Active tasks   │
│              │                           │ ├─ Intent map     │
│              │ Loquela composer          │ ├─ Mesh allocation │
│              │                           │ └─ Cost (OR)      │
└──────────────┴──────────────────────────┴───────────────────┘
```

- [ ] **Step 1: Wire `list_orchestrator_tasks` filtered by `session_id`**

- [ ] **Step 2: Intent map** — `get_routing_summary_live` + task descriptions

- [ ] **Step 3: Mesh strip** — peers + queue depth from MCP (compact)

- [ ] **Step 4: OpenRouter spend** — session slice from `get_llm_spend` when available

- [ ] **Step 5: Collapsible rails** — all three columns independently collapsible

- [ ] **Step 6: Commit**

### Task 4.3: Inline agent execution in transcript

- [ ] Render `vox://agent-events` for active session interleaved with chat messages (reuse `mapAgentEvent` styling)

- [ ] Token stream collapsed by default (expand per task)

- [ ] Link agent row → Flow surface with `selectedAgentId`

- [ ] **Commit**

---

## Phase 5 — Gamification sprinkles (achievements drawer, not nav tab)

### Task 5.1: Reposition Gamify surface

**Files:**
- Modify: `contracts/gui/surface-registry.v1.yaml`
- Create: `AchievementsDrawer.tsx`

- [ ] **Step 1:** Change `gamify` to `representation_tier: hidden` or `achievements` drawer entry — remove from Agents sub-tabs

- [ ] **Step 2:** Sidebar / HUD trophy icon opens drawer (earned badges + enable toggle mirrored from Settings)

- [ ] **Step 3:** Settings `gamify` section remains SSOT for enable/mode (`get_gamify_settings`)

- [ ] **Step 4: Commit**

### Task 5.2: Event router hooks (GUI)

Wire `vox_gamify::event_router` from GUI actions (see Appendix C for full hook list). Minimum v1 hooks:

- [ ] Chat message sent
- [ ] Task submitted
- [ ] Search query executed
- [ ] Policy rule viewed
- [ ] CLI command run from Console
- [ ] Palette navigation
- [ ] Achievement toast via `AchievementToast.tsx` (non-blocking, respects Serious mode)

### Task 5.3: Gate UI when disabled

- [ ] `config_gate::is_enabled()` — hide trophy icon, LudusBanner, achievement toasts

- [ ] **Commit**

---

## Phase 6 — Dockview multi-panel & draggability (shell-wide)

### Task 6.1: Enable split panels

- [ ] Wire ⌘\\ to `dockview` split horizontal
- [ ] ⌘W closes focused panel (not app)
- [ ] Persist multi-panel layout in `gui.layout.v1`
- [ ] Document in `gui-navigation.md`

### Task 6.2: Draggable collapsible rails audit

Every collapsible region must expose: `aria-expanded`, keyboard toggle, persisted state — see **Improvement catalog §Collapsibility** (items 201–230).

---

## Verification gates (every phase PR)

```bash
cd crates/vox-gui/ui && pnpm test && pnpm typecheck
pnpm exec playwright test e2e/dashboard-pilot.spec.ts e2e/policies.spec.ts e2e/palette-search-navigate.spec.ts
cargo run -q -p vox-cli -- ci gui-smoke
cargo run -q -p vox-cli -- ci gui-surface-registry
```

---

# Appendix A — Improvement catalog (350 items)

> Numbered for issue tracking. Format: `IMPR-NNN` | domain | effort S/M/L | file hint

### A. Shell & layout (001–050)

| ID | Improvement | Effort |
|----|-------------|--------|
| IMPR-001 | Extract `AppShell` from `App.tsx` | M |
| IMPR-002 | Persistent `StatusBar` always visible on operator surfaces | M |
| IMPR-003 | Deprecate `HudMode.hidden` for production builds | S |
| IMPR-004 | Top bar faux-search triggering ⌘K palette | S |
| IMPR-005 | Workspace name in top bar from identity summary | S |
| IMPR-006 | Remove "IMPERIUM" branding from TopHud | S |
| IMPR-007 | Remove Latin LARP from any remaining labels | S |
| IMPR-008 | Breadcrumb visible on all non-chat surfaces | S |
| IMPR-009 | Breadcrumb click restores parent default child | S |
| IMPR-010 | Sidebar rail mode tooltips on all nav items | S |
| IMPR-011 | Sidebar wide mode shows child tab hints | M |
| IMPR-012 | Sidebar filter input (OmniSearch lite) | M |
| IMPR-013 | Coverage shortcut stays under System section | S |
| IMPR-014 | Policy badge on Runs nav not Settings | S |
| IMPR-015 | Real connectivity dot on sidebar footer | S |
| IMPR-016 | Remove hardcoded "tauri 2" string — use build info | S |
| IMPR-017 | `gui.layout.v1` validates against JSON schema | M |
| IMPR-018 | Dockview split horizontal ⌘\\ | M |
| IMPR-019 | Dockview close panel ⌘W | M |
| IMPR-020 | Dockview drag tabs between splits | L |
| IMPR-021 | Persist dock layout per workspace profile | M |
| IMPR-022 | Reset layout to factory default | S |
| IMPR-023 | Export/import layout JSON | M |
| IMPR-024 | `AppShell` storybook/visual test fixture | M |
| IMPR-025 | Reduce main `pb-[180px]` when chat undocked | S |
| IMPR-026 | Focus trap in CommandPalette | S |
| IMPR-027 | Palette max-height respects StatusBar | S |
| IMPR-028 | TopHud collision fix narrow widths (P1-2 audit) | S |
| IMPR-029 | Z-index SSOT for overlays (palette, toasts, drawer) | S |
| IMPR-030 | Responsive breakpoint at `xl` for Policies two-rail | S |
| IMPR-031 | Mobile/Tauri min-width guard messages | S |
| IMPR-032 | Keyboard focus ring on all shell controls | M |
| IMPR-033 | Skip link to main content | S |
| IMPR-034 | `aria-current` on active sidebar item | S |
| IMPR-035 | Announce route changes via `aria-live` polite | S |
| IMPR-036 | Reduce motion respects `prefers-reduced-motion` | S |
| IMPR-037 | High-contrast theme hook (from Settings F2) | M |
| IMPR-038 | Theme applies to dockview chrome | S |
| IMPR-039 | Theme applies to Recharts axes | S |
| IMPR-040 | Density compact mode (tighter padding tokens) | M |
| IMPR-041 | Font size scaling pref | M |
| IMPR-042 | Shell loading skeleton on bootstrap | S |
| IMPR-043 | Error boundary per surface not whole app | M |
| IMPR-044 | Offline banner when daemon unreachable | M |
| IMPR-045 | Reconnect action in offline banner | S |
| IMPR-046 | Session restore last view from hash | S |
| IMPR-047 | Invalid hash falls back to dashboard | S |
| IMPR-048 | Deep link copy button in breadcrumb menu | S |
| IMPR-049 | Print stylesheet for Policies/Coverage | L |
| IMPR-050 | Shell telemetry events (`vox.gui.nav`, `vox.gui.layout`) | M |

### B. Configurable dashboard (051–100)

| ID | Improvement | Effort |
|----|-------------|--------|
| IMPR-051 | `dashboard-layout.v1.yaml` contract | M |
| IMPR-052 | `DashboardGrid` 12-column host | M |
| IMPR-053 | Widget drag reorder via dnd-kit | M |
| IMPR-054 | Widget resize handles | M |
| IMPR-055 | Widget add/remove picker | M |
| IMPR-056 | Layout persist `gui.dashboard.layout.v1` | S |
| IMPR-057 | Per-operator layout profiles | M |
| IMPR-058 | Reset dashboard to default | S |
| IMPR-059 | Duplicate layout profile | S |
| IMPR-060 | Stream widget (migrated) | S |
| IMPR-061 | Agents widget (migrated) | S |
| IMPR-062 | Alerts/Ludus widget (migrated) | S |
| IMPR-063 | KPI spark widget configurable metric | M |
| IMPR-064 | Line chart widget | M |
| IMPR-065 | Bar chart widget | M |
| IMPR-066 | Area chart widget | M |
| IMPR-067 | Stacked bar for queue by priority | M |
| IMPR-068 | Donut chart for agent phase distribution | M |
| IMPR-069 | Queue depth time series | M |
| IMPR-070 | Budget burn time series | M |
| IMPR-071 | Token usage time series | M |
| IMPR-072 | Mesh peer count time series | M |
| IMPR-073 | OpenRouter spend time series | M |
| IMPR-074 | Task throughput widget | M |
| IMPR-075 | Error rate widget from stream filters | M |
| IMPR-076 | Custom markdown/text widget | S |
| IMPR-077 | Widget title editable | S |
| IMPR-078 | Widget time range selector | S |
| IMPR-079 | Widget refresh interval | S |
| IMPR-080 | Widget loading skeleton per-tile | S |
| IMPR-081 | Widget error state with retry | S |
| IMPR-082 | Widget empty state with CTA | S |
| IMPR-083 | Chart color tokens from `visualTokens.ts` | S |
| IMPR-084 | Chart tooltips accessible | S |
| IMPR-085 | Chart keyboard focus | M |
| IMPR-086 | Export chart PNG | L |
| IMPR-087 | Export widget data CSV | M |
| IMPR-088 | Dashboard fullscreen mode | S |
| IMPR-089 | Dashboard TV/kiosk mode (auto-rotate widgets) | L |
| IMPR-090 | Dashboard share layout via gist export | L |
| IMPR-091 | Import layout from file | M |
| IMPR-092 | Validate layout on import | S |
| IMPR-093 | Widget catalog docs in UI | S |
| IMPR-094 | Default layouts: Operator, SRE, Researcher | M |
| IMPR-095 | Role-based layout defaults from settings | L |
| IMPR-096 | Dashboard `@test` golden layout fixture | S |
| IMPR-097 | Playwright customize dashboard flow | M |
| IMPR-098 | Property test random layouts validate | M |
| IMPR-099 | Widget lazy import code-splitting | M |
| IMPR-100 | Dashboard perf budget in CI | M |

### C. HUD & status bar real data (101–130)

| ID | Improvement | Effort |
|----|-------------|--------|
| IMPR-101 | `hud-tiles.v1.yaml` contract | M |
| IMPR-102 | Configurable HUD tiles in Settings | M |
| IMPR-103 | Replace mesh `0 MB/s` with peer count | S |
| IMPR-104 | Mesh queue depth tile optional | M |
| IMPR-105 | Active model tile from routing summary | S |
| IMPR-106 | OpenRouter spend tile from `get_llm_spend` | M |
| IMPR-107 | Exploration spend tile | S |
| IMPR-108 | Daemon budget cap display fix | S |
| IMPR-109 | Sparklines from real event ingestion | M |
| IMPR-110 | Trend delta from series not hardcoded 0 | M |
| IMPR-111 | StatusBar segment: agents | S |
| IMPR-112 | StatusBar segment: queue | S |
| IMPR-113 | StatusBar segment: budget | S |
| IMPR-114 | StatusBar segment: model | S |
| IMPR-115 | StatusBar segment: mesh | S |
| IMPR-116 | StatusBar segment: build/CI (optional) | L |
| IMPR-117 | Click segment navigates to surface | S |
| IMPR-118 | StatusBar shows pending approvals count | S |
| IMPR-119 | StatusBar shows policy worst status | S |
| IMPR-120 | StatusBar unread gamify notifications | S |
| IMPR-121 | Slim HUD syncs with StatusBar | S |
| IMPR-122 | HUD never hides StatusBar | S |
| IMPR-123 | Live/Poll/Offline semantics documented | S |
| IMPR-124 | Enrich live stream path with gamify alerts | M |
| IMPR-125 | Enrich live stream path with mesh peers | M |
| IMPR-126 | Agent progress bar honest when no wire field | S |
| IMPR-127 | Remove fake ETA on AgentRow | S |
| IMPR-128 | Topology graph widget from daemon snapshot | L |
| IMPR-129 | HUD tile plugin extension point | L |
| IMPR-130 | `vox ci gui-hud-tile-parity` gate | M |

### D. OmniSearch (131–180)

| ID | Improvement | Effort |
|----|-------------|--------|
| IMPR-131 | `omnisearch-index.v1.yaml` | M |
| IMPR-132 | Federated index builder hook | M |
| IMPR-133 | Policy rows in index | M |
| IMPR-134 | Action manifest rows in index | M |
| IMPR-135 | Task rows in index | M |
| IMPR-136 | Chat sessions in index | M |
| IMPR-137 | Achievements in index | M |
| IMPR-138 | Skills in `/` mode fix | S |
| IMPR-139 | Fed items keyboard selectable | S |
| IMPR-140 | Wrap-around selection in palette | S |
| IMPR-141 | Unified ranking score (RRF client) | M |
| IMPR-142 | Recent searches persisted | S |
| IMPR-143 | Pinned commands | M |
| IMPR-144 | Search settings scope backend or drop | M |
| IMPR-145 | Search web scope default opt-in doc | S |
| IMPR-146 | SearchView pagination `next_cursor` | M |
| IMPR-147 | Pass `path_glob` to backend | S |
| IMPR-148 | Pass `offset` to backend | S |
| IMPR-149 | Sidebar filter uses same index | M |
| IMPR-150 | Top bar search opens palette with query | S |
| IMPR-151 | Double-click doc opens in repo | S |
| IMPR-152 | Policy hit opens Policies with rule selected | M |
| IMPR-153 | Task hit opens Tasks/Runs | M |
| IMPR-154 | Setting hit deep-links section | S |
| IMPR-155 | Surface hit navigates with `type: navigate` | S |
| IMPR-156 | Locator hit uses `viewKeyForLocator` | S |
| IMPR-157 | MCP tool hits in palette (advanced) | L |
| IMPR-158 | Discovery suggestions in palette | M |
| IMPR-159 | Raise MAX_PER_KIND or dynamic cap | S |
| IMPR-160 | Search telemetry `vox.gui.search` | S |
| IMPR-161 | OmniSearch e2e policy navigate | M |
| IMPR-162 | OmniSearch e2e settings navigate | M |
| IMPR-163 | OmniSearch e2e task navigate | M |
| IMPR-164 | MemoryView uses shared search hook | S |
| IMPR-165 | Single `searchSettings()` SSOT | S |
| IMPR-166 | Scientia publications in index | M |
| IMPR-167 | Claims in index | M |
| IMPR-168 | Model cards in index | M |
| IMPR-169 | Mesh nodes in index | M |
| IMPR-170 | Registry tier none entries opt-in | M |
| IMPR-171 | Fuzzy match option (fuse.js) | M |
| IMPR-172 | Synonym map for nav labels | S |
| IMPR-173 | Search result previews (hover) | M |
| IMPR-174 | Search filters persist per user | S |
| IMPR-175 | Voice search trigger (Loquela) | L |
| IMPR-176 | Search from mobile menu | L |
| IMPR-177 | `callTool` memory search path | M |
| IMPR-178 | Index rebuild on registry change event | M |
| IMPR-179 | Index staleness indicator in palette | S |
| IMPR-180 | OmniSearch perf budget < 50ms rebuild | M |

### E. Chat / Imperium view (181–220)

| ID | Improvement | Effort |
|----|-------------|--------|
| IMPR-181 | Undock Loquela from global shell | M |
| IMPR-182 | Full-page Chat is default chat entry | S |
| IMPR-183 | Multi-tab sessions (Track I) | L |
| IMPR-184 | `session_id` not hardcoded gui-loquela | S |
| IMPR-185 | ChatExecutionRail component | L |
| IMPR-186 | Tasks filtered by session in rail | M |
| IMPR-187 | Intent map from routing summary | M |
| IMPR-188 | Mesh allocation compact view | M |
| IMPR-189 | OpenRouter session cost strip | M |
| IMPR-190 | Inline agent events in transcript | L |
| IMPR-191 | Collapse token streams | M |
| IMPR-192 | Tool call cards in transcript | M |
| IMPR-193 | Approval inline in chat rail | M |
| IMPR-194 | Diff review opens beside chat | M |
| IMPR-195 | Composer mode/tier passthrough fix | S |
| IMPR-196 | Model list refresh on open | S |
| IMPR-197 | Queue depth chip on composer | S |
| IMPR-198 | Session budget from KPIs | S |
| IMPR-199 | Chat rail collapsible | S |
| IMPR-200 | Chat sessions rail collapsible | S |
| IMPR-201 | Drag resize chat vs rail split | M |
| IMPR-202 | Pin rail for dual-monitor | S |
| IMPR-203 | Chat notifications in StatusBar | M |
| IMPR-204 | Unread message badge on Chat nav | M |
| IMPR-205 | Jump to latest message shortcut | S |
| IMPR-206 | Export transcript markdown | M |
| IMPR-207 | Search within transcript | M |
| IMPR-208 | Link message to task id | M |
| IMPR-209 | Regenerate/stop controls | M |
| IMPR-210 | Attach context chips from rail | M |
| IMPR-211 | Slash commands in composer | S |
| IMPR-212 | Skill deploy from chat | S |
| IMPR-213 | Voice input in chat surface | M |
| IMPR-214 | Chat-first onboarding empty state | M |
| IMPR-215 | Rename Chat nav label (not Imperium) | S |
| IMPR-216 | Remove duplicate transcript when undocked | S |
| IMPR-217 | Playwright chat rail visibility | M |
| IMPR-218 | Playwright session tab switch | M |
| IMPR-219 | Chat surface error boundary | S |
| IMPR-220 | Chat a11y live region for new messages | S |

### F. Collapsibility & drag (221–250)

| ID | Improvement | Effort |
|----|-------------|--------|
| IMPR-221 | Audit all panels for collapse control | M |
| IMPR-222 | Policies master rail collapse persisted | S |
| IMPR-223 | Policies group rail collapse | S |
| IMPR-224 | Settings secrets groups collapse | S |
| IMPR-225 | Console discovery rail collapse | S |
| IMPR-226 | Scientifica panels collapse | M |
| IMPR-227 | Memory shard rows expand | S |
| IMPR-228 | Repository tree collapse | S |
| IMPR-229 | Uniform `aria-expanded` on collapse toggles | M |
| IMPR-230 | Keyboard shortcut collapse all rails | M |
| IMPR-231 | Drag reorder SubTabs (optional) | L |
| IMPR-232 | Drag reorder sidebar favorites | L |
| IMPR-233 | Float palette as detachable window | L |
| IMPR-234 | Float achievements drawer | M |
| IMPR-235 | Snap rails to edges | L |
| IMPR-236 | Remember rail widths | M |
| IMPR-237 | Min width constraints per rail | S |
| IMPR-238 | Touch drag handles 44px min | S |
| IMPR-239 | Haptic feedback on snap (mobile) | L |
| IMPR-240 | Collision detection for drag | M |
| IMPR-241 | Undo layout change | M |
| IMPR-242 | Layout diff view in settings | L |
| IMPR-243 | `prefers-reduced-motion` on drag animations | S |
| IMPR-244 | Screen reader announce panel moves | S |
| IMPR-245 | Dockview tab reorder | M |
| IMPR-246 | Dockview floating panels | L |
| IMPR-247 | Prevent drag when text selecting | S |
| IMPR-248 | Grid snap on dashboard | M |
| IMPR-249 | Align widgets to grid on drop | S |
| IMPR-250 | Collision push on dashboard grid | M |

### G. Gamification sprinkles (251–310)

| ID | Improvement | Effort |
|----|-------------|--------|
| IMPR-251 | Hide Gamify from Agents sub-tabs | S |
| IMPR-252 | Achievements drawer from trophy icon | M |
| IMPR-253 | XP toast on chat send | M |
| IMPR-254 | XP toast on task complete | M |
| IMPR-255 | XP toast on search | S |
| IMPR-256 | XP toast on policy fix | M |
| IMPR-257 | XP toast on CLI command run | M |
| IMPR-258 | XP toast on palette navigate | S |
| IMPR-259 | XP toast on doc open | S |
| IMPR-260 | XP toast on approval granted | M |
| IMPR-261 | XP toast on skill deploy | M |
| IMPR-262 | XP toast on memory recall | S |
| IMPR-263 | XP toast on mesh dispatch | M |
| IMPR-264 | XP toast on model switch | S |
| IMPR-265 | XP toast on settings save | S |
| IMPR-266 | XP toast on dashboard customize | S |
| IMPR-267 | XP toast on layout export | S |
| IMPR-268 | XP toast on e2e green CI (meta) | L |
| IMPR-269 | Quest progress from Tasks surface | M |
| IMPR-270 | Quest progress from Chat sessions | M |
| IMPR-271 | Achievement unlock modal (rare) | M |
| IMPR-272 | Streak counter in StatusBar optional | M |
| IMPR-273 | Leaderboard link in drawer not nav | S |
| IMPR-274 | Companion mood in drawer | M |
| IMPR-275 | Shop link when enabled | M |
| IMPR-276 | Collegium hints in onboarding | L |
| IMPR-277 | Serious mode suppresses all toasts | S |
| IMPR-278 | Balanced mode shows micro-toasts | S |
| IMPR-279 | Full mode shows LudusBanner | S |
| IMPR-280 | `config_gate` gates HUD trophy | S |
| IMPR-281 | Settings toggle syncs effective config | M |
| IMPR-282 | Doctor warns config mismatch | S |
| IMPR-283 | Achievement search in OmniSearch | M |
| IMPR-284 | Quest search in OmniSearch | M |
| IMPR-285 | CLI `vox gamify` surfaces link in drawer | S |
| IMPR-286 | MCP gamify tools linked from GUI | M |
| IMPR-287 | Training corpus tags gamified actions | L |
| IMPR-288 | GRPO reward linkage documented | S |
| IMPR-289 | Anti-farm cooldown on repeated XP | M |
| IMPR-290 | Idempotent event ids for rewards | M |
| IMPR-291 | Telemetry `vox.gamify.gui_event` | S |
| IMPR-292 | A/B test gamify visibility | L |
| IMPR-293 | Parental/enterprise disable policy | M |
| IMPR-294 | Export achievement profile | M |
| IMPR-295 | Import achievement profile | L |
| IMPR-296 | Share achievement card image | L |
| IMPR-297 | Seasonal quest UI in drawer | M |
| IMPR-298 | Battle result toast | M |
| IMPR-299 | Teaching hints after 3 failures | M |
| IMPR-300 | Narrator voice lines optional | L |
| IMPR-301 | Gamify asset SVG safety retained | S |
| IMPR-302 | No gamify in CI critical path | S |
| IMPR-303 | Ludus rename cleanup in UI copy | S |
| IMPR-304 | Achievement defaults synced with crate | M |
| IMPR-305 | GUI tests mock gamify IPC | S |
| IMPR-306 | Playwright gamify disabled mode | M |
| IMPR-307 | Playwright achievement toast | M |
| IMPR-308 | Rate limit XP events per minute | M |
| IMPR-309 | Combine XP into session summary | M |
| IMPR-310 | End-of-day recap optional notification | L |

### H. Notifications & toasts (311–330)

| ID | Improvement | Effort |
|----|-------------|--------|
| IMPR-311 | Toast queue max visible | S |
| IMPR-312 | Toast priority levels | S |
| IMPR-313 | Toast actions (undo/retry) | M |
| IMPR-314 | Toast grouping by surface | M |
| IMPR-315 | In-app notification center drawer | M |
| IMPR-316 | Mark all read | S |
| IMPR-317 | Notification links to surface | M |
| IMPR-318 | Policy failure notification | M |
| IMPR-319 | Approval pending notification | M |
| IMPR-320 | Task failed notification | M |
| IMPR-321 | Mesh node lost notification | M |
| IMPR-322 | Budget threshold notification | M |
| IMPR-323 | OpenRouter rate limit notification | M |
| IMPR-324 | Desktop native notifications opt-in | M |
| IMPR-325 | Do not disturb mode | M |
| IMPR-326 | Notification prefs in Settings | M |
| IMPR-327 | `aria-live` for critical toasts | S |
| IMPR-328 | Sound optional on notify | L |
| IMPR-329 | Notification history export | L |
| IMPR-330 | Telemetry `vox.gui.notify` | S |

### I. Transport, IPC, contracts (331–350)

| ID | Improvement | Effort |
|----|-------------|--------|
| IMPR-331 | Shrink `invoke()` allowlist per wave | M |
| IMPR-332 | `mesh_throughput` field on daemon status | M |
| IMPR-333 | `recent_events` populated in status | M |
| IMPR-334 | Agent wire fields: progress, eta, task | M |
| IMPR-335 | `gui.dashboard.layout.v1` in DB prefs tier | S |
| IMPR-336 | `gui.hud.tiles.v1` in DB prefs tier | S |
| IMPR-337 | Schema generate for new contracts | S |
| IMPR-338 | `vox ci data-storage-guard` on new prefs | S |
| IMPR-339 | Surface registry achievements entry | S |
| IMPR-340 | Action manifest search metadata | M |
| IMPR-341 | Policy catalog JSON export for index | M |
| IMPR-342 | OpenRouter spend in orchestrator status | L |
| IMPR-343 | Session cost attribution API | L |
| IMPR-344 | A2A visibility in Chat rail (Track K) | L |
| IMPR-345 | Scaling service GUI controls | M |
| IMPR-346 | LLM concurrency throttle display | M |
| IMPR-347 | Resource summary populi endpoint consumer | M |
| IMPR-348 | Graphify panel optional widget | L |
| IMPR-349 | Browser preview widget | L |
| IMPR-350 | Coverage table widget on dashboard | M |

---

# Appendix B — Component / library compatibility matrix

| Need | Recommendation | Notes |
|------|--------------|-------|
| Panel split/drag | **dockview** (in repo) | Already in `DockShell.tsx`; enable multi-panel |
| Dashboard grid DnD | **@dnd-kit** | Headless; matches React 19 |
| Charts | **Recharts 3** | Line/bar/area; theme via CSS vars |
| Spark micro charts | **@visx/xychart** or custom SVG | Smaller than full Recharts for HUD |
| Grid layout math | **CSS grid** + contract | Avoid react-grid-layout legacy |
| Resizable rails | **existing `resizable-panel` pattern** or dockview | Torii-style min/max width |
| Search fuzzy | **vox-search** + optional fuse.js client | Hybrid RRF per search SSOT |
| State | **TanStack Query** + Zustand layout slice | Query for server; Zustand for drag ephemeral |
| Persistence Tier D | **`voxTransport` GUI prefs** | Per data-storage SSOT |
| Forms in widget config | **existing Dialog + Input** | No new form framework |
| Tables | **CoverageView pattern** | Reuse for widget data tables |
| Icons | **Icons.tsx** | Add chart/grid icons as needed |
| Animation | **CSS transitions** | No framer-motion unless budget allows |
| Date axis | **d3-time-format** via visx or Recharts | Timezone from user locale |
| Export | **vox-spool** for telemetry series | Tier B if historical metrics stored |

**Rejected for vox-gui:** Tremor (visual clash), MUI X (Material mismatch), AG Grid enterprise, Grafana embed (heavy), full Superset embed.

---

# Appendix C — Gamification hook points (extended)

Beyond IMPR-251–310, wire events at:

- `DiscoveryRail.tsx` — action exposure / use
- `TasksView.tsx` — CRUD task operations
- `RunsView.tsx` — workflow completion
- `RepositoryView.tsx` — isolation scan complete
- `BrowserView.tsx` — preview load
- `Scientia/*.tsx` — claim approved, nanopub built
- `PoliciesView.tsx` — rule toggled (Phase 2+)
- `SettingsView.tsx` — secret rotated
- `Console/InputEditor.tsx` — command success exit 0
- `Matrix.tsx` — routing intention nudge
- `ModelsView.tsx` — model activation
- `MeshView.tsx` — dispatch success
- `ApprovalsView.tsx` — decision
- `HarnessRedirect` — harness redirect acknowledgment
- `BreadcrumbBar` — deep navigation efficiency (meta)
- `useOrchestratorStatus` — first successful connect
- `install-hooks` / `vox ci pre-push` — meta achievements (CLI integration)
- `vox skill use` when wired — skill invocation
- `vox run` from GUI terminal — VoxScript execution
- Plugin catalog install — future
- Doc doctest pass — contributor meta
- `vox audit` clean run — governance meta

All hooks call `vox_gamify::event_router::record_gui_event(...)` (new thin Tauri command) — **do not** duplicate XP math in TypeScript.

---

# Appendix D — Chat vs global composer decision

**Decision:** Chat/Imperium becomes a **first-class full-page surface**; global docked composer **removed** from default operator layout.

| Surface | Composer | Execution rail |
|---------|----------|----------------|
| Chat | Yes (primary) | Yes |
| Dashboard | No — CTA "Open Chat" | No |
| Console | No — terminal primary | Optional mini task strip |
| Other | No | No |

**Rationale:** Design brief §3 Problem 5 (no status bar) + user request to see agent execution inline with chat; global composer steals vertical space and duplicates Chat surface.

**Migration:** One release with `chatDocked` default false + release note; keep `VOX_GUI_CHAT_DOCKED=1` escape hatch one cycle.

---

# Appendix E — OmniSearch information architecture

```
┌─────────────────────────────────────────────────────────────┐
│ [Workspace ▾]  [ Search or jump… ⌘K ]     [Status segments] │
├──────┬──────────────────────────────────────────────────────┤
│ Side │ Breadcrumb › SubTabs                                  │
│ bar  │ ┌──────────────────────────────────────────────────┐ │
│filter│ │ Active surface                                    │ │
│      │ └──────────────────────────────────────────────────┘ │
├──────┴──────────────────────────────────────────────────────┤
│ StatusBar: agents · queue · budget · model · mesh · alerts  │
└─────────────────────────────────────────────────────────────┘

⌘K layers (unified ranking):
  1. Commands/skills (>)
  2. Agents (@)
  3. Docs/skills (/)
  4. Surfaces/settings/docs (client index)
  5. Policies/tasks/actions (client index)
  6. Backend corpora (vox_search_query)
```

---

# Appendix F — Phased delivery timeline (suggested)

| Phase | Weeks | Outcome |
|-------|-------|---------|
| 0 | 1 | Contracts + AppShell |
| 1 | 2 | StatusBar + real HUD + de-LARP |
| 2 | 3 | Configurable dashboard + charts |
| 3 | 2 | OmniSearch federation |
| 4 | 3 | Chat execution rail + undock |
| 5 | 2 | Gamification sprinkles + drawer |
| 6 | 2 | Dockview splits + collapsibility audit |

**Total:** ~13 weeks serialized; **~7 weeks** with parallel tracks A–F per dispatch map.

---

# Execution handoff

**Plan complete and saved to** `docs/superpowers/plans/2026-06-16-gui-operator-console-v2-configurable-dashboard-omnisearch.md`.

**Recommended dispatch (parallel agents):**

1. **Agent A** — Phase 0–1 (shell + StatusBar + HUD real data)
2. **Agent B** — Phase 2 (dashboard grid + Recharts widgets)
3. **Agent C** — Phase 3 (OmniSearch federation)
4. **Agent D** — Phase 4 (Chat undock + execution rail)
5. **Agent E** — Phase 5 (gamification drawer + hooks)

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per phase, review between phases
2. **Inline Execution** — `executing-plans` with checkpoints after each phase

**Which approach?**

Also update `docs/superpowers/plans/2026-06-16-gui-roadmap-remaining.md` to point Phase P0+ to this plan as the v2 operator console master plan.
