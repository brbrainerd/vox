---
title: "GUI navigation"
description: "Operator console v2 surfaces, shell chrome, OmniSearch entry points, chat layout, and keybinds."
category: "Language Reference"
---

# GUI navigation

The operator console compresses legacy sidebar entries into **nine surfaces** plus a **Settings** system row. v2 (June 2026) adds a **configurable dashboard**, a persistent **StatusBar**, federated **OmniSearch**, and **Chat-first** composer placement (see [operator console v2 plan](../../superpowers/plans/2026-06-16-gui-operator-console-v2-configurable-dashboard-omnisearch.md)).

## Top-level surfaces

| Surface | Inner tabs (default first) |
|---------|---------------------------|
| Chat | Full-page sessions + **embedded composer** (no global dock) |
| Agents | Dashboard · Agents (flow) · Routing (matrix) · Tasks |
| Runs & Approvals | Runs · Approvals · Policies |
| Workspace | Repository · Browser · Console · Quick Harness (composer redirect) |
| Commands | Browse & Run · MCP Skills & Plugins |
| Search | Unified search (Memory as a scope chip) |
| Knowledge | Research · Scientia · Review · Claims · Publications |
| Compute | Models · Mens · Populi · Oratio · Mesh |
| Settings | Orchestrator · Model routing · Mesh & peers · Signing · Secrets · Telemetry · Keybinds · Theme · Gamification (prefs) · Coverage (CI surface gaps) |

**Gamification:** no longer an Agents inner tab. When gamification is enabled, open **Achievements** from the trophy control on the StatusBar (`AchievementsDrawer`). Preferences remain under Settings › Gamification. Legacy `gamify` view keys still deep-link for compatibility.

Legacy view keys (`approvals`, `flow`, `catalog`, …) still deep-link: they open the correct parent and inner tab.

## Workbench tabs (Axis, July 2026)

Each **leaf surface** opens as its own workbench tab in the header stack. The sidebar opens the **default child** for a parent (for example Workspace → Console). Open tabs persist under `vox_workbench_tabs.v1`; the location hash `#view=<key>` stays in sync with the active tab.

```text
┌ Sidebar ─┬─ TopHud ────────────────────────────────────────────────┐
│          ├─ WorkbenchTabBar (open leaf tabs; close non-pinned)       │
│          ├─ StatusBar                                                │
│          ├─ Active surface (`SurfaceScrollHost` — single scrollport)   │
└──────────┴──────────────────────────────────────────────────────────┘
```

- **Pinned by default:** Chat. Dashboard is opened on first run when no saved state exists.
- **Documentation:** Help docs open as `doc:<path>` tabs via Omnibar (`/help …` queries) or `read_doc_markdown` IPC; in-app `DocReader` renders markdown.
- **Attention budget:** shown only on the Chat tab (composer meter), not as a global strip.
- **Legacy inner tabs (`ParentSurface` / `SubTabs`)** are retired for primary navigation — use workbench tabs and sidebar parents instead.

E2E visual audit: `pnpm exec playwright test e2e/workbench-tabs.spec.ts e2e/screenshots.spec.ts --project=chromium`.

## Shell chrome

Operator surfaces share a fixed header stack inside `AppShell` (sidebar left, chrome + surface right):

```text
┌ Sidebar ─┬─ TopHud (KPI tiles, OpenRouter spend, ⌘K trigger) ─────────┐
│          ├─ WorkbenchTabBar (open surfaces)                            │
│          ├─ StatusBar (agents · queue · budget · mesh · model · live)  │
│          ├─ Active surface (`SurfaceScrollHost`)                       │
└──────────┴────────────────────────────────────────────────────────────┘
```

See plan [Appendix D](../../superpowers/plans/2026-06-16-gui-operator-console-v2-configurable-dashboard-omnisearch.md#appendix-d--chat-vs-global-composer-decision) for the Chat-first composer rationale. Shipped v2 has **no global Loquela dock** — the composer lives only on the Chat surface; Dashboard exposes an **Open Chat** CTA (`data-testid="open-chat-cta"`).

## Composer placement (Chat vs global dock)

| Surface | Composer | Execution rail |
|---------|----------|----------------|
| Chat | Yes — Loquela embedded in `ChatSurface` | Yes — collapsible right rail |
| Dashboard (Agents › Dashboard) | No — **Open Chat** CTA navigates to Chat | No |
| Console | No — terminal is primary | Optional task strip only |
| Other surfaces | No | No |

`App.tsx` keeps `chatDocked = false` always. Chat hosts the sole shell composer; Console and other surfaces do not show Loquela at the shell level.

## Chat layout

Three columns on the Chat surface (each side rail is collapsible):

```text
┌──────────┬─────────────────────────────┬─────────────────┐
│ Sessions │ Transcript + embedded       │ Execution rail  │
│ rail     │ Loquela composer            │ (tasks, KPIs)   │
└──────────┴─────────────────────────────┴─────────────────┘
```

- **Sessions rail** (`ChatSessionRail`) — session list and create; collapse persisted under `gui.chat.sessions_collapsed.v1`.
- **Center** — transcript plus inline composer (not the global shell dock).
- **Execution rail** (`ChatExecutionRail`) — live tasks, queue/agents/mesh KPIs, active model, OpenRouter spend; collapse persisted per session UI state.

## Configurable dashboard

Agents › **Dashboard** uses `DashboardGrid` with drag-and-drop reorder (`@dnd-kit`) and a **Customize dashboard** mode that opens the widget picker drawer. Layout is validated against `contracts/gui/dashboard-layout.v1.yaml` and persisted as `gui.dashboard.layout.v1`. Widget kinds include stream, agents, alerts, KPI sparklines, charts, queue depth, budget burn, mesh peers, model active, OpenRouter spend, and task summary.

## StatusBar

A slim **StatusBar** sits below the breadcrumb in the header stack (always visible on operator surfaces; not hidden in production HUD modes). Clickable segments navigate to the related surface. **OpenRouter spend** and richer KPI sparklines live in **TopHud** tiles and the Chat **execution rail**, not in StatusBar segments.

| Segment | Navigates to | Data |
|---------|--------------|------|
| Agents | Agents | Active agent count |
| Queue | Runs & Approvals | Queue depth |
| Budget | Settings | Spend vs cap |
| Mesh | Compute | Peer count |
| Model | Compute › Models | Active routing label |
| Freshness (right) | — | Live / Poll / Offline from orchestrator events |
| Trophy (when gamify on) | Achievements drawer | XP / badges |

Full HUD mode (`TopHud`) adds configurable KPI tiles (including OpenRouter spend) above the StatusBar; cycle with ⌘/Ctrl+Shift+H.

## OmniSearch

Three entry points share one federated index (`useFederatedSearchIndex` + backend `vox_search_query`):

| Entry | Location | Behavior |
|-------|----------|----------|
| ⌘/Ctrl+K | `CommandPalette` | Primary omni palette; prefix modes `>` commands, `@` agents, `/` docs/skills |
| TopHud trigger | “Search or jump…” chip | Opens the same palette (slim and full HUD modes) |
| Sidebar filter | Collapsible filter field | Narrows top-level nav and matching child tabs client-side |

Palette sections rank surfaces, settings, docs, policies, commands, actions, skills, and backend corpora.

### Search scopes (OmniSearch vs Search surface)

The **⌘/Ctrl+K palette** and the **Search** surface share `searchController.ts` scope chips, but not every chip maps to a `vox-search` backend corpus. SSOT for index kinds: `contracts/gui/omnisearch-index.v1.yaml`.

| User scope | Where results come from (v1) | Backend (`vox_search_query` / `search.rs`) |
|------------|------------------------------|---------------------------------------------|
| Code | `repo`, `symbol` corpora | Yes |
| Docs | `chunk`, `knowledge` corpora | Yes |
| Chats | GUI message LIKE search | Yes (special-case in `search.rs`) |
| Memory | `memory` corpus | Yes |
| Web | `web` corpus | Yes |
| Commands | CLI catalog (`get_command_catalog`) | Client merge in Search surface; no `SearchCorpus` variant |
| Settings | `SETTINGS_INDEX` (`settingsIndex.ts` + `config-gui-codegen`) | **No — client-merged in Search surface** (same pattern as Commands) |

**Settings search is client-federated only in v1.** Rows are built at compile time from `settingsIndex.ts` and `crates/vox-gui/ui/src/config/generatedSettingsIndex.ts` (via `vox ci config-gui-codegen`). `useFederatedSearchIndex` and `federatedSearchIndex.ts` index them for the palette and sidebar filter. There is no `settings` scope in `search.rs` `scope_to_corpus` and no `SearchCorpus::Settings` variant — a backend settings corpus is explicitly out of scope for v1 (see plan task 3.4).

The Search surface **Settings** chip merges `SETTINGS_INDEX` client-side via `filterSettingsIndexHits` (mirroring **Commands** + `get_command_catalog`). Selecting a hit seeds `vox_settings_seed` for the Settings surface. **⌘/Ctrl+K** also indexes settings via the federated palette.

## Keybinds

| Key | Action |
|-----|--------|
| ⌘/Ctrl+K | Quick search palette (OmniSearch) |
| ⌘/Ctrl+B | Cycle sidebar width (rail → default → wide) |
| ⌘/Ctrl+Shift+H | Cycle HUD (full → slim → hidden) |
| ⌘/Ctrl+\\ | Split panel horizontally (dockview; focus must be in `DockShell`) |
| ⌘/Ctrl+W | Close focused dockview panel (when more than one panel exists) |

Dockview split/close is implemented in `DockShell` (`dockShellKeybindingForEvent`).

## SSOT

Surface hierarchy is defined in `contracts/gui/surface-registry.v1.yaml` and regenerated with `vox ci gui-surface-registry --write`. Shell persistence keys live in `contracts/gui/shell-persistence.v1.yaml`; dashboard widgets in `contracts/gui/dashboard-layout.v1.yaml`; federated OmniSearch kinds in `contracts/gui/omnisearch-index.v1.yaml`.

## Developer bootstrap

```bash
vox ci gui-surface-registry --write
vox ci config-gui-codegen --write
cd crates/vox-gui/ui && pnpm install && pnpm build
cargo run -p vox-gui
vox ci gui-smoke
```
