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
- A real, definite `h-full` container (the CSS layout-collapse fix).

**Scope note (post-adversarial-audit):** the closed-panel-tracking fix (`closedPanelIds` via `onDidRemovePanel`) is *not* folded into `DockWorkspaceShell` itself — it stays host-local (in `ChatSurface.tsx`), since only Chat's 5 core panels have an auto-recreate-on-missing code path that needs guarding at all. `DockWorkspaceShell` carries a doc comment flagging this as a required companion pattern for any future host that adds its own auto-recreate logic. The 14 Phase 2/3 surfaces sidestep the issue entirely by design (see below), not by reusing the shell's (nonexistent) tracking.

New in this generalization:
- **Quiet tab chrome.** `dockview-vox.css`'s active-tab styling is restyled away from the saturated blue currently leaking through to a slim, low-contrast brass treatment. (No claim is made about the tab row disappearing for single-panel groups — no dockview API for that was found; "quiet" means visually smaller/lower-contrast, always present.)
- **A "Panels" launcher menu with two distinct sections**, not one flat list: **core** panels (Sessions/Chat/Execution/Flow/To-dos — the original 5, auto-created on first mount, restored by "Reset layout") and **opt-in** panels (the 14-surface allowlist below — never auto-created, only ever added via an explicit click in the menu's "Add" section, and never touched by Reset). This two-tier split is load-bearing, not cosmetic: a flat list caused a real defect during implementation-planning review (Reset layout would have force-opened all 19 panels every time, and new panels risked reproducing the original "resurrects after close" bug via a shared auto-create code path) — see the implementation plan's `CORE_PANEL_IDS`/`OPT_IN_PANEL_IDS` split for the fix. This is also the "drag from any tab" request's v1 implementation: **click-to-add from a menu**, not literal HTML5 drag-from-sidebar — that remains an explicit, lower-priority follow-up (dockview's external-drag-source support is real but unverified during this design pass).
- **Resize and reset-to-default.** Dockview's own splitter-drag resize is native. "Double-click a divider to reset" was requested but no reset-on-double-click hook was found in dockview's sash/splitter API — the reliably-buildable equivalent is the existing **"Reset layout" action**, scoped to only the 5 core panels (see above). A true double-click-splitter reset remains a candidate follow-up.

### 2. Chat transcript becomes a pinned, non-dockable center

The transcript panel keeps living inside the dock grid (so other panels can still position themselves "around" it using dockview's normal reference-panel geometry), but gets a **custom, empty `tabComponent`** — dockview supports per-panel custom tab renderers (`AddPanelOptions.tabComponent`, confirmed against the library's type definitions); a tab component that renders nothing means no visible tab strip, no visible close button, and no visible drag handle for that specific panel, while every other panel keeps the normal tab UI. This needs to be verified end-to-end during implementation (does an empty tab component fully prevent drag-out in practice, or only hide the close affordance?) — the plan's first Chat-specific task is an investigation step for exactly this, mirroring how Task B3 had to investigate AgentFlow's real data source before writing code. If a fallback is needed, `DockviewGroupPanel.locked` is a real, group-level property (not the whole-component `DockviewOptions.locked` option — a different, easily-confused API) whose documented effect is preventing panels from being *dropped into* a group; whether it also blocks a panel already inside that group from being *dragged out* is unconfirmed and must be tested, not assumed — and its interaction with the opposite requirement (other panels must still be able to dock *next to* the locked group) needs an explicit positive-case check too.

### 3. No in-window File/Edit/View menu bar

Confirmed during this session: the app already has a real command palette (`Omnibar.tsx`, 454 lines, faceted search across surfaces/commands/agents/docs) and no native OS menu (`tauri::menu` — absent). A classic in-window dropdown menu bar would reintroduce exactly the "extra stacked chrome row" problem just fixed. **Decision: no in-window menu bar.** A native OS-level menu (Quit/Preferences/standard Edit shortcuts) was discussed as a small, separate, non-blocking follow-up — **explicitly out of scope for this spec**, not included in the plan below.

### 4. Condensed views are bespoke per surface, sourced from the audit

Each of the 8 "condensed-capable" surfaces gets a specific, evidence-based collapsed-state summary (not a generic placeholder) — e.g. Approvals shows "N pending" + the current permission mode; Mesh shows node count + pending queue count; Gamify shows HP/level/leaderboard rank and never renders its 560px sandbox map when narrow. The full list-per-surface, including a specific numeric toggle threshold for each, is in the implementation plan's Phase 3 table, sourced directly from the audit conversation.

**Toggle mechanism (confirmed, not left open):** every dockview panel component receives `props.api.width` and `props.api.height` (both live, reactive `readonly number` properties on `PanelApi`) plus `props.api.onDidDimensionsChange` to subscribe to resize events — this is a real, always-available API, not something requiring investigation to discover. Seven of the eight surfaces toggle on `width`; Gamify is the one exception, toggling on `height`, since its constraint (`LudusSandbox`'s fixed 560px map) is fundamentally a height limit, not a width one — a width-only mechanism would be the wrong tool for that surface specifically.

Two surfaces (Approvals, Mesh) run their own independent polling loops in their existing top-level components. Their condensed panels must render only already-threaded summary data (a count/status prop passed down from `App`), never mount the real `<ApprovalsView>`/`<MeshView>` component inline — otherwise a user with both the top-level tab and the dock panel open ends up with two competing poll loops hitting the same backend endpoint.

## What this spec does not include

- Dashboard/`DashboardGrid` unification (explicitly deferred).
- True external drag-from-sidebar-to-dock (verified-unproven dockview capability; follow-up task only).
- Double-click-splitter-to-reset (unproven API; "Reset layout" menu action is the delivered equivalent).
- A native OS menu bar (separate, smaller, non-blocking follow-up).
- Wiring the 7 confirmed full-page-only surfaces (Settings/Flow/Catalog/Browser/Console/Policies/Runs) or Dashboard into any dock workspace, ever, per the audit.

## Testing

Same TDD discipline as every prior task this session: failing test first, confirm the failure reason, implement, confirm pass. Phase 1 (shell generalization + chat pinning + rename + chrome) gets full per-step tests. Phase 2/3 (wiring the 14 surfaces) follows one fully-worked example task with real tests, then a table-driven enumeration for the rest — each entry in that table is still a real task with a real test, just following the worked example's established pattern rather than re-deriving it from scratch every time (DRY, per the writing-plans skill's own principle).
