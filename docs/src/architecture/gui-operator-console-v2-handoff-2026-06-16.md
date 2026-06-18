---
title: "GUI Operator Console v2 — agent handoff"
description: "Fresh-agent handoff for the v2 operator console mega-plan: what shipped, invariants, verification, and remaining work."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Onboarding context for agents continuing GUI v2 without prior session history."
---

# GUI Operator Console v2 — agent handoff (2026-06-16)

**Purpose:** Give a new coding agent everything needed to continue the v2 operator console without reading prior chat transcripts.

**Authoritative plan (task steps, checkboxes, IMPR catalog):** [`docs/superpowers/plans/2026-06-16-gui-operator-console-v2-configurable-dashboard-omnisearch.md`](../../superpowers/plans/2026-06-16-gui-operator-console-v2-configurable-dashboard-omnisearch.md)

**Roadmap index (sequencing vs other GUI waves):** [`docs/superpowers/plans/2026-06-16-gui-roadmap-remaining.md`](../../superpowers/plans/2026-06-16-gui-roadmap-remaining.md)

**Shipped navigation reference:** [`docs/src/reference/gui-navigation.md`](../reference/gui-navigation.md)

---

## Mission (do not drift)

Transform `vox-gui` from a fixed triptych dashboard into a **user-composable operator console**:

| Pillar | Intent |
|--------|--------|
| **Configurable dashboard** | 12-col widget grid, drag reorder, resize, widget picker, layout persisted |
| **Persistent StatusBar** | Always visible on operator surfaces; real daemon/MCP signals (not decorative HUD) |
| **OmniSearch** | Federated client index + backend corpora via `vox-search`; ⌘K as primary jump surface |
| **Chat-first execution** | Full-page Chat with embedded composer, session rail, execution rail, agent-event transcript |
| **Ambient gamification** | Trophy drawer + toasts; **no** Gamify as primary nav tab; XP math **only in Rust** |

**Design north stars:** `gui-frontend-design-principles-2026-06-14.md`, `vox-dashboard-design-brief-2026.md`, `vox-gui-visual-audit-2026-06-03.md`.

---

## Architecture invariants (binding)

### Three SSOT layers

1. **Contracts** — `contracts/gui/*.v1.yaml` (dashboard-layout, shell-persistence, hud-tiles, omnisearch-index, surface-registry)
2. **Runtime prefs** — Prefer `voxTransport.getGuiPreference` / `setGuiPreference` for Tier-A keys documented in `shell-persistence.v1.yaml`; some keys still use `useLocalStorage` during migration
3. **Search** — Client builder in `federatedSearchIndex.ts` + `useFederatedSearchIndex.ts`; backend lanes in `searchController.ts` (settings scope is **client-federated only** in v1 — maps to `[]` backend corpora)

### Gamification boundary

- TypeScript: `recordGamifyGuiEvent()` in `crates/vox-gui/ui/src/lib/gamifyGuiEvents.ts` → Tauri `record_gui_event`
- Rust mapping: `map_gui_hook_event_type` in `crates/vox-gui/src/commands/gamify.rs`
- **Never** compute XP in TS; **never** call `useGamifySettings()` inside individual surfaces for gating — pass `gamifyEnabled` from `App.tsx` via `surfaceProps` / `AppShell`

### Composer placement (Appendix D — **shipped**)

| Surface | Composer |
|---------|----------|
| Chat | Yes — embedded in `ChatSurface` |
| Dashboard | No — **Open Chat** CTA (`data-testid="open-chat-cta"`) |
| Console / others | No global Loquela dock |

`App.tsx` sets `chatDocked = false` always. Plan mentions `VOX_GUI_CHAT_DOCKED=1` escape hatch for one release cycle — **not wired yet** if product still wants it.

### Shell stack

```text
Sidebar → AppShell → TopHud → BreadcrumbBar → StatusBar → DockShell (dockview) → surface
```

Dockview: ⌘/Ctrl+\\ split, ⌘/Ctrl+W close panel; layout key `gui.layout.v1`.

### Build / toolchain

- **Vite target `es2022`** in `vite.config.ts` (required for `@dnd-kit` / `immer` with esbuild 0.28+)
- Format Rust: `vox run scripts/fmt.vox` — never `cargo fmt --all` on Windows
- Bootstrap when `vox` not on PATH: `pwsh -File scripts/windows/vox-dev.ps1 <cmd>`

### Agent workflow expectations

- **TDD:** failing test before new `pub fn` / surface behavior (see `AGENTS.md`)
- **Parallel dispatch:** independent domains (resize vs gamify vs e2e) can use subagents; avoid shared-file conflicts
- **Commits / PRs:** only when the human explicitly asks
- **Local CI first:** `vox ci pre-push` before pushing; do not use GitHub as primary feedback loop
- Plan markdown checkboxes (`- [ ]`) were **not** bulk-updated — treat code + tests as source of truth

---

## What is done (by plan phase)

### Phase 0 — Contracts & AppShell ✅ (substantially)

- `contracts/gui/dashboard-layout.v1.yaml`, `shell-persistence.v1.yaml`, `hud-tiles.v1.yaml`, `omnisearch-index.v1.yaml`
- `dashboardLayout.ts` + validation tests
- `AppShell.tsx` extracted from `App.tsx`
- `shellPersistence.ts` key SSOT

**Partial:** Full migration off raw `localStorage` for all legacy keys; `vox ci gui-surface-registry` drift for every shell key

### Phase 1 — StatusBar & HUD ✅ (substantially)

- `StatusBar.tsx` — agents, queue, budget, mesh, model, live freshness
- `TopHud.tsx` — configurable tiles via `useHudTiles.ts`, `HudTilesEditor.tsx`
- Real mesh KPI / identity / LLM spend / OpenRouter tile (placeholders reduced)
- Playwright: `e2e/status-bar-surfaces.spec.ts` (use **sidebar-scoped** `exact: true` for Chat nav — dashboard **Open Chat** CTA substring-matches "Chat")

### Phase 2 — Configurable dashboard ✅ (mostly)

- `DashboardGrid.tsx` — @dnd-kit reorder + **corner resize** (min 2×2) in customize mode
- `resizeDashboardWidget()` in `dashboardGrid.ts` + tests
- Recharts widgets, `WidgetPickerDrawer.tsx`, layout persistence (`gui.dashboard.layout.v1`)
- Bundle budget: `dashboardBundleBudget.test.ts` + `contracts/budgets/gui-dashboard-chunk.v1.yaml` (~120 KiB gzipped cap; verify headroom after widget changes)
- **Partial 2.3:** Queue/budget charts use KPI spark + `useMetricSeries.append` when spark empty; **no** full orchestrator event feed (`cost_incurred`, `task_completed`) → `vox.metric.series.v1` yet

### Phase 3 — OmniSearch ✅ (v1 subset)

- `federatedSearchIndex.ts`, `useFederatedSearchIndex.ts`
- CommandPalette federation, sidebar filter, TopHud ⌘K trigger
- Settings scope: client-federated only (contract + `searchController.ts` + SearchView merge)
- Playwright: `e2e/palette-search-navigate.spec.ts`

**Not done:** `deferred_kinds` in `omnisearch-index.v1.yaml` — task, chat_session, achievement, corpus backend lanes

### Phase 4 — Chat / Imperium ✅ (core)

- Appendix D: no global dock; Open Chat CTA on dashboard
- `ChatExecutionRail.tsx`, `ChatSessionRail.tsx` (collapsible, persisted)
- Three-column Chat layout; `ChatTranscript.tsx` + `mapAgentEvent.ts` / agent events interleaved
- Palette "Submit new task…" → `handleSubmitTaskAction()` → navigate chat + focus composer
- Playwright: `chat-composer-dock`, `chat-session-rail`, `submit-task-palette`

**Partial:** Execution rail live task list from `list_orchestrator_tasks` by session; token stream collapsed by default; Flow deep-link from agent row

### Phase 5 — Gamification ✅ (v1 hooks)

- `AchievementsDrawer.tsx`, StatusBar trophy, achievement toasts, gamify gating
- Wired hooks (TS → Rust): see [Appendix C status](#appendix-c-gamify-hooks-status) below
- `orchestrator_first_connect` via `useOrchestratorFirstConnectGamify`

**Partial:** `config_gate::is_enabled()` hide trophy when disabled; policy **rule toggled** (Policies UI still read-only for enable/disable)

### Phase 6 — Dockview ✅ (baseline)

- `DockShell.tsx`: split, close, persist `gui.layout.v1`
- Collapsible rails audit: ChatSessionRail, ChatExecutionRail, DiscoveryRail, Policies sidebar
- Playwright: `e2e/dock-layout.spec.ts` (uses shared `e2e/lib/operatorShellMock.ts` with `seedGuiPrefs`)

**Not done:** IMPR catalog 221–250+ (float panels, sidebar drag reorder, keyboard collapse all rails, dockview floating panels, etc.)

---

## Verification state (last known green)

Run these before claiming done:

```powershell
cd c:\Users\Owner\vox\crates\vox-gui\ui
pnpm test
pnpm typecheck
pnpm exec playwright test `
  e2e/status-bar-surfaces.spec.ts `
  e2e/chat-composer-dock.spec.ts `
  e2e/chat-session-rail.spec.ts `
  e2e/submit-task-palette.spec.ts `
  e2e/palette-search-navigate.spec.ts `
  e2e/dashboard-pilot.spec.ts `
  e2e/policies.spec.ts `
  e2e/dock-layout.spec.ts `
  --project=chromium

cd c:\Users\Owner\vox
pwsh -File scripts/windows/vox-dev.ps1 ci gui-surface-registry
cargo test -p vox-gui map_gui_hook_event_type
```

| Gate | Expected |
|------|----------|
| vitest | **569+** tests, 120 files |
| Playwright (8 specs above) | **11** tests |
| `gui-surface-registry` | PASS |
| `vox ci gui-smoke` | Default lane: `web_ir_lower_emit_test` ignored TanStack guard; **pnpm build** when `VOX_GUI_PNPM_BUILD=1` or `CI=1` |
| Production build | `pnpm run build` in `crates/vox-gui/ui` |

**Not routinely run in session:** `e2e/dashboard.spec.ts`, `e2e/settings.spec.ts`, `e2e/visual-review.spec.ts`, opt-in gui-smoke lanes (`VOX_WEB_VITE_SMOKE=1`, `VOX_GUI_PLAYWRIGHT=1`, `VOX_GUI_RELAUNCH_SMOKE=1`), full `vox ci gui-smoke` via `cargo run -p vox-cli` (may hit sccache / cargo directory lock on Windows — run nextest lane directly if blocked).

---

## Key file map

| Area | Paths |
|------|-------|
| App orchestration | `crates/vox-gui/ui/src/App.tsx` |
| Shell | `components/layout/AppShell.tsx`, `StatusBar.tsx`, `TopHud.tsx`, `DockShell.tsx`, `BreadcrumbBar.tsx`, `Sidebar.tsx` |
| Dashboard | `components/dashboard/*`, `lib/dashboardLayout.ts`, `lib/dashboardGrid.ts` |
| Search | `lib/federatedSearchIndex.ts`, `hooks/useFederatedSearchIndex.ts`, `lib/searchController.ts`, `components/CommandPalette.tsx` |
| Chat | `components/surfaces/Chat/*`, `lib/commandPaletteActions.ts`, `lib/mapAgentEvent.ts` |
| Gamify | `lib/gamifyGuiEvents.ts`, `components/gamify/*`, `crates/vox-gui/src/commands/gamify.rs` |
| Persistence | `lib/shellPersistence.ts`, `transport` GUI preference IPC |
| E2E mock SSOT | `e2e/lib/operatorShellMock.ts` |
| Contracts | `contracts/gui/*.v1.yaml`, `contracts/budgets/gui-dashboard-chunk.v1.yaml` |
| Docs | `docs/src/reference/gui-navigation.md` |

---

## Remaining work (prioritized)

### P0 — Close v2 plan verification gaps

1. **Run and fix** legacy e2e not in the operator bundle: `dashboard.spec.ts`, `settings.spec.ts`
2. **Full `vox ci gui-smoke`** on a clean runner (or document env vars for opt-in lanes)
3. **Update plan checkboxes** in the mega-plan markdown OR add a generated status table (optional hygiene)

### P1 — Phase 2.3 metric series (real data)

- Extend `useMetricSeries` / new pref `vox.metric.series.v1` fed by orchestrator events (`cost_incurred`, `task_completed`)
- Widget config: metric key, range (1h/6h/24h/7d), chart type
- Vitest with mocked series; optional Playwright snapshot

### P1 — OmniSearch deferred kinds

Implement per `contracts/gui/omnisearch-index.v1.yaml` `deferred_kinds`:

| Kind | Source IPC |
|------|------------|
| task | `list_orchestrator_tasks` |
| chat_session | `chat_list_sessions` |
| achievement | gamify profile |
| corpus | `vox_search_query` |

### P2 — Appendix C remaining gamify hooks

See table below. Prefer TDD: test mock `recordGamifyGuiEvent` at call site, then add Rust mapping.

### P2 — Policies Phase 2+ 

- Enable/disable policy rules (currently disabled buttons in `PoliciesView.tsx`)
- Wire `policy_rule_toggled` gamify when toggles ship

### P2 — Appendix D escape hatch

- Wire `VOX_GUI_CHAT_DOCKED=1` (or `gui.shell.chat_docked` pref) if product wants one-release rollback

### P3 — IMPR catalog (350 items)

Plan Appendix A — collapsibility polish (221–230), float panels (233–234), dockview float (246), sidebar drag (231–232), dashboard grid snap/collision (248–250), gamification sprinkles (251–310). **Do not** attempt bulk IMPR in one PR; pick slices with tests.

### P3 — Master roadmap waves (parallel track)

Not replaced by v2 — see `gui-roadmap-remaining.md`: Wave 2–6 IPC allowlist shrink, deploy packaging CI leg, action manifest forms, MemoryView query migration, etc.

---

## Appendix C gamify hooks status

| Hook / surface | Status |
|----------------|--------|
| `chat_message_sent` | ✅ |
| `task_submitted` | ✅ (palette) |
| `search_query_executed` | ✅ |
| `policy_rule_viewed` | ✅ |
| `palette_navigation` | ✅ |
| `console_command_success` | ✅ (`Console.tsx` exit 0) |
| `discovery_action_used` | ✅ |
| `workflow_completed` | ✅ |
| `model_activated` | ✅ |
| `approval_decision` | ✅ |
| `browser_preview_loaded` | ✅ |
| `mesh_dispatch_success` | ✅ |
| `isolation_strategy_set` | ✅ |
| `isolation_scan_complete` | ✅ (`RepositoryView` successful `runAction`) |
| `harness_redirect_viewed` | ✅ |
| `breadcrumb_navigation` | ✅ |
| `claim_approved`, `nanopub_built` | ✅ (Scientia) |
| `secret_rotated`, `signing_key_rotated` | ✅ (Settings) |
| `orchestrator_first_connect` | ✅ |
| **policy_rule_toggled** | ❌ Policies enable/disable not implemented |
| **vox run from GUI terminal** | ❌ distinguish VoxScript vs shell in Console |
| **vox skill use** | ❌ when GUI skill invocation ships |
| **plugin catalog install** | ❌ future |
| **Matrix routing nudge** | ❌ optional |
| **TasksView CRUD** | ❌ partial / verify |
| **Meta CLI achievements** (pre-push, audit clean) | ❌ out of GUI scope |

---

## E2E coverage matrix

| Spec | Uses `operatorShellMock` | Notes |
|------|--------------------------|-------|
| `status-bar-surfaces.spec.ts` | ✅ | Sidebar nav: `exact: true` for Chat |
| `chat-composer-dock.spec.ts` | ✅ | Dashboard: no dock, Open Chat CTA |
| `chat-session-rail.spec.ts` | ✅ | |
| `submit-task-palette.spec.ts` | ✅ | |
| `palette-search-navigate.spec.ts` | ✅ | |
| `dashboard-pilot.spec.ts` | ✅ | |
| `policies.spec.ts` | ✅ | |
| `dock-layout.spec.ts` | ✅ | `seedGuiPrefs` for `gui.layout.v1` |
| `dashboard.spec.ts` | ❓ | Migrate to shared mock |
| `settings.spec.ts` | ❓ | May need mock extension |
| `browser-*.spec.ts`, `visual-review`, `screenshots` | separate | Browser / visual lanes |

When adding Tauri IPC to surfaces, **extend `operatorShellMock.ts` first** — do not duplicate 90-line inline mocks per spec.

---

## Git / worktree notes

- Large **uncommitted** working tree expected — GUI v2 work spans many files under `crates/vox-gui/ui/` plus `contracts/gui/`, `gamify.rs`, docs
- Initial session git status also showed unrelated changes (`.github/workflows/ci.yml`, `vox-cli-ci`, `vox-orchestrator-mcp`, etc.) — **scope PRs to GUI** unless user asks for combined commit
- Do **not** hand-regenerate SSOT artifacts after merge; CI `ssot-autoregen` bot handles drift on same-repo PRs

---

## Suggested next parallel batch

Independent tracks safe to dispatch concurrently:

| Agent | Scope | Touch mainly |
|-------|-------|--------------|
| A | Phase 2.3 orchestrator metric series | `useMetricSeries.ts`, widgets, tests |
| B | OmniSearch deferred kind: `task` | `federatedSearchIndex.ts`, palette tests |
| C | Migrate `settings.spec.ts` + `dashboard.spec.ts` to `operatorShellMock` | `e2e/` |
| D | Policies rule toggle + `policy_rule_toggled` gamify | `PoliciesView.tsx`, Tauri IPC if needed |
| E | `VOX_GUI_CHAT_DOCKED` escape hatch | `App.tsx`, env registry if new var |

After agents return: full vitest + 8-spec Playwright + spot-check bundle budget.

---

## Common pitfalls (learned in session)

1. **Playwright `{ name: 'Chat' }`** matches **Open Chat** CTA — scope to `navigation` + `exact: true`
2. **Dashboard nav parent** is `agents` with child `dashboard` — tests using `nav.parent === 'dashboard'` were wrong; use `nav.child === 'dashboard'` for dashboard-specific behavior (composer/dock removed anyway)
3. **`chatDocked = false`** removes `pb-[180px]` padding — surfaces should not assume bottom composer reserve on dashboard
4. **Cargo / sccache** on Windows can block `vox ci gui-smoke` — run nextest filter directly for web_ir lane
5. **Cold `cargo test -p vox-gui`** can take 10+ minutes — prefer `cargo test -p vox-gui map_gui_hook_event_type` when cache warm
6. Plan doc still shows `- [ ]` for completed work — verify in code, not checkboxes

---

## Related documents

- [Operator console v2 plan](../../superpowers/plans/2026-06-16-gui-operator-console-v2-configurable-dashboard-omnisearch.md)
- [Async chat / OmniSearch federation plan](../../superpowers/plans/2026-06-12-async-chat-tasklist-resource-scheduling-omnisearch.md)
- [GUI navigation reference](../reference/gui-navigation.md)
- [Where things live](where-things-live.md) — crate lookup before adding code
- [Runner contract](../ci/runner-contract.md) — CI self-hosted labels

---

*Last session synthesis: 2026-06-16. Update this handoff when closing a major phase or before long-running agent handoffs.*
