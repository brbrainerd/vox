---
title: Universal dock workspace — pinned chat, quiet-chrome dockview, curated surface allowlist
status: approved
---

# Universal dock workspace — design

## Context and problem

Tasks B1-B3 of the earlier chat-flow-docking-redesign plan made every chat-adjacent panel (sessions, the chat transcript itself, execution rail, Flow, Plan) an equal dockview panel. A design critique (this session) found this was the wrong model for chat: it produced three stacked header bars, made the chat transcript itself accidentally closable/draggable instead of a fixed center anchor, and starved panels of width. Two crash-level bugs were also found and fixed independently of this redesign (a real-CSS-layout height-collapse bug, and a React-element-serialization crash in layout persistence) — both fixes are generic dock-shell infrastructure and are preserved here, not reverted.

Separately, the user wants this capability generalized: not just Chat's four adjacent panels, but *any* app surface should be draggable into a dockable workspace, resizable, restorable to a sensible default, and NOT require a whole extra tab-bar's worth of chrome to do it. This spec covers both: fixing Chat's own layout, and building the generalized primitive plus a curated set of other surfaces wired into it.

## Surface audit — grounded in actual code, not guessed

Every one of the app's 22 top-level surfaces (`crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`) was read in full to determine real natural/minimum dimensions and whether a condensed "docked narrow" state is meaningful. Full findings live in this conversation's transcript; the actionable conclusion:

**Strong dockable candidates** (secondary/glanceable content, already narrow-tolerant or trivially so): `vox-search`/`graphify` (`VoxGraphStatusPanel.tsx`), `needs-you` (`NeedsYouSurface.tsx`), `activity` (`DiscoverySurface.tsx` and its 4 sub-views), `repository` (`RepositoryView.tsx`), `mercatus` (`Mercatus.tsx`), `harness` (`HarnessRedirect.tsx` — a near-empty redirect stub).

**Condensed-capable candidates** (the full view wants real width/height, but a narrow "N pending / N active" badge state is genuinely useful and cheap to build): `approvals`, `mesh`, `tasks` (the global task queue, `TasksView.tsx` — distinct from the chat Plan/To-dos panel, see naming note below), `coderabbit`, `skills`, `gamify` (its `LudusSandbox` mini-map is hard-fixed at 560px tall and must never render in a condensed state), `models`, `memory`.

**Confirmed full-page-only — excluded from dockability entirely**: `settings` (dense multi-section config, destructive flows — this independently confirms what the user already suspected), `flow` (SVG graph canvas sized against a 1200×640 viewBox), `catalog` (two-pane form + exec output, code asserts `min-h-[560px]`), `browser` (embeds a live `<iframe>` plus a coordinate-mapped remote-control frame — structurally cannot shrink), `console` (primary terminal workspace, a hardcoded non-responsive 280px rail, zero responsive logic anywhere in the file), `policies` (two-pane master-detail with a wide detail inspector), `runs` (a wide two-table dashboard designed for `xl` breakpoints).

**Dashboard is a special case**: also full-page-only, but additionally it already contains its *own* separate dockable/resizable widget-grid system (`DashboardGrid`, with drag/resize/customize mode, distinct from `dockview`). Building a second docking mechanism that also tries to swallow Dashboard would mean two competing dock systems in one app. **Decision: Dashboard and its internal `DashboardGrid` are explicitly out of scope — untouched, not wired into the new dock workspace.** Unifying the two systems is a real future project, not this one.

Net effect: 14 of 22 surfaces are genuine dock candidates (6 strong + 8 condensed-capable); 7 are structurally excluded regardless of how a collapsed state is designed, and Dashboard is excluded for architectural-duplication reasons.

## Naming decision: "To-dos," not "Tasks"

The user asked for an editable task list — this already exists as the chat-session-scoped Plan panel (`PlanPanel.tsx`, per-node checklist, click-to-edit pending steps, "+ Add step"). Renaming it to "Tasks" (the obvious label) would collide with the *already-existing, unrelated* top-level `tasks` surface (`TasksView.tsx` — the global cross-session task/job queue with priority/lifecycle/dependency tracking). These are different concepts at different scopes (one chat-session's plan-DAG steps vs. the whole app's job queue) and must not share a label. **Decision: rename the Plan panel to "To-dos" in the UI** (component/prop names can stay as-is internally where renaming would be pure churn — see the plan for exactly what changes).

## Architecture

### 1. Generalize `ChatDockShell` into `DockWorkspaceShell`

`crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx` becomes a non-Chat-specific, reusable component (new location: `crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx`), parameterized by a `storageKeyPrefix` (so each host view — Chat today, potentially others later — gets its own persisted layout, not a shared one). It keeps everything already fixed this session:
- Layout persistence with `params` stripped before every write (the React-element-serialization crash fix) — generalized, not chat-specific.
- The closed-panel-tracking fix (`closedPanelIds` via `onDidRemovePanel`) so the refresh mechanism never fights a user's explicit close.
- A real, definite `h-full` container (the CSS layout-collapse fix).

New in this generalization:
- **Quiet tab chrome.** `dockview-vox.css`'s active-tab styling is restyled away from the saturated blue currently leaking through to a slim, low-contrast brass treatment — a single-line tab strip that only appears when a group holds 2+ panels (dockview's own behavior for single-panel groups should already suppress the tab row; this needs verification during implementation, not assumed).
- **A generalized "Panels" launcher menu** (evolving the reopen/reset menu already designed for Chat) that lists every panel *eligible for the current host view* — for Chat, that starts as the existing sessions/execution/flow/to-dos set and grows to include the 14-surface allowlist above, each launchable as a new docked panel. This is the "drag from any tab" request's v1 implementation: **click-to-add from a menu**, not literal HTML5 drag-from-sidebar — dockview's support for accepting an external (non-dockview-origin) drag source is a real capability but wasn't verified during this design pass; it's called out as an explicit, separate, lower-priority follow-up task at the end of the plan, not promised as part of this spec's delivered behavior.
- **Resize and reset-to-default.** Dockview's own splitter-drag resize is native and already works. "Double-click a divider to reset that split to its default size" was requested but dockview's sash/splitter API wasn't verified to support a reset-on-double-click hook during this design pass — the reliably-buildable equivalent is the existing **"Reset layout" action** in the Panels menu (clears persisted geometry, recreates the default arrangement). A true double-click-splitter reset is a candidate follow-up, not committed here.

### 2. Chat transcript becomes a pinned, non-dockable center

The transcript panel keeps living inside the dock grid (so other panels can still position themselves "around" it using dockview's normal reference-panel geometry), but gets a **custom, empty `tabComponent`** — dockview supports per-panel custom tab renderers (`AddPanelOptions.tabComponent`, confirmed against the library's type definitions); a tab component that renders nothing means no visible tab strip, no visible close button, and no visible drag handle for that specific panel, while every other panel keeps the normal tab UI. This needs to be verified end-to-end during implementation (does an empty tab component fully prevent drag-out in practice, or only hide the close affordance?) — the plan's first Chat-specific task is an investigation step for exactly this, mirroring how Task B3 had to investigate AgentFlow's real data source before writing code.

### 3. No in-window File/Edit/View menu bar

Confirmed during this session: the app already has a real command palette (`Omnibar.tsx`, 454 lines, faceted search across surfaces/commands/agents/docs) and no native OS menu (`tauri::menu` — absent). A classic in-window dropdown menu bar would reintroduce exactly the "extra stacked chrome row" problem just fixed. **Decision: no in-window menu bar.** A native OS-level menu (Quit/Preferences/standard Edit shortcuts) was discussed as a small, separate, non-blocking follow-up — **explicitly out of scope for this spec**, not included in the plan below.

### 4. Condensed views are bespoke per surface, sourced from the audit

Each of the 8 "condensed-capable" surfaces gets a specific, evidence-based collapsed-state summary (not a generic placeholder) — e.g. Approvals shows "N pending" + the current permission mode; Mesh shows node count + pending queue count; Gamify shows HP/level/leaderboard rank and never renders its 560px sandbox map when narrow. The full list-per-surface is in the implementation plan's Phase 3 table, sourced directly from the audit conversation.

## What this spec does not include

- Dashboard/`DashboardGrid` unification (explicitly deferred).
- True external drag-from-sidebar-to-dock (verified-unproven dockview capability; follow-up task only).
- Double-click-splitter-to-reset (unproven API; "Reset layout" menu action is the delivered equivalent).
- A native OS menu bar (separate, smaller, non-blocking follow-up).
- Wiring the 7 confirmed full-page-only surfaces (Settings/Flow/Catalog/Browser/Console/Policies/Runs) or Dashboard into any dock workspace, ever, per the audit.

## Testing

Same TDD discipline as every prior task this session: failing test first, confirm the failure reason, implement, confirm pass. Phase 1 (shell generalization + chat pinning + rename + chrome) gets full per-step tests. Phase 2/3 (wiring the 14 surfaces) follows one fully-worked example task with real tests, then a table-driven enumeration for the rest — each entry in that table is still a real task with a real test, just following the worked example's established pattern rather than re-deriving it from scratch every time (DRY, per the writing-plans skill's own principle).
