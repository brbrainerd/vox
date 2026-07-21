---
title: Dockview panel UX — Plan panel migration, reopen/reset controls
status: approved
---

# Dockview panel UX fixes — design

## Problem

`ChatSurface.tsx`'s Plan panel (the checklist/plan-DAG panel added in an earlier
phase) is a hand-rolled flex sibling with its own `useLocalStorage`-backed
collapse toggle, added when `ChatSurface`'s root container was still a
`flex` row. Task B2 (this session) changed that root container to
`relative min-h-[60vh]` (no longer `flex`) to make room for the dockview
shell holding the sessions/transcript/execution-rail/flow panels. The Plan
panel's positioning relative to the dockview shell is now undefined by the
layout system — this produces the reported bug: the panel appears to "hang
in empty space" and reserves width even while notionally collapsed.

Separately, the user wants panels freely draggable to any edge (left/right/
top) and mergeable into shared tabs. This is dockview's default behavior —
nothing in `ChatDockShell.tsx` disables drag-and-drop — so once every panel
lives inside dockview, this requirement is satisfied with no additional
code.

## Approach

1. **Migrate the Plan panel into dockview** as a fifth panel
   (`PlanDockPanel`, added to `CHAT_DOCK_COMPONENTS` alongside
   `sessions`/`transcript`/`executionRail`/`flow`), mirroring exactly how
   Task B3 added the Flow panel: a module-scope panel component, an entry
   in `CHAT_DOCK_COMPONENTS`, an `addPanel` call in `onReady`, and inclusion
   in the existing `dockApiRef`-driven refresh `useEffect` (the
   B2/B3-established pattern for keeping panel content live across
   re-renders).
2. **Remove the hand-rolled collapse state**: `planPanelCollapsed`, its
   `useLocalStorage` key (`gui.chat.plan_panel_collapsed.v1`), and the
   `<aside>` toggle-strip JSX are deleted entirely. Closing the Plan panel
   is now dockview's native tab-close, which removes the panel and its
   layout footprint completely (not a CSS collapse hack).
3. **Add a "Panels ▾" menu button** (same visual pattern as the existing
   sidebar-toggle buttons: a small bordered icon button, positioned near
   the top of `ChatSurface`'s root). It has two actions:
   - **Reopen**: lists any of the five known panel ids
     (`sessions`/`transcript`/`executionRail`/`flow`/`plan`) not currently
     present in `dockApiRef.current.api.panels`, and re-runs that panel's
     `addPanel` call on click.
   - **Reset layout**: clears whatever Task B4 persists (its
     `containerApi.toJSON()` snapshot) and re-invokes the full default
     `onReady` panel-creation sequence.
4. No dockview configuration changes are needed for drag-to-reposition or
   drag-to-tab — both are on by default. This spec does not add or modify
   any dockview option flags.

## Data flow

`PlanDockPanel` renders the same `<PlanPanel>` component and props
(`planNodes`, `planSessionId`, `planVersion`, the `listPlanNodes`/
`updatePlanNode`/`insertPlanNode` transport calls) that the current
`<aside>` block passes — this is a pure relocation of existing JSX into a
dockview panel wrapper, not a behavior change to the Plan panel's own
contents.

The Panels▾ menu needs read access to `dockApiRef.current` (already an
existing ref in `ChatSurface.tsx` from Task B2) to compute which panels are
currently absent, and calls the same per-panel `addPanel` logic already
factored out for the refresh `useEffect` — this logic should be extracted
into a small `addAllDefaultPanels(api)` / per-id `addPanelById(api, id)`
helper so the menu, the refresh effect, and the reset action share one
source of truth for "what does panel X look like when (re)created."

## Sequencing

This depends on Tasks B4 (layout persistence — the reset action needs
something to clear) and ideally follows B5/B6 too, since all of B2-B6
repeatedly touch the same `onReady`/`CHAT_DOCK_COMPONENTS`/refresh-`useEffect`
region of `ChatSurface.tsx`. Implementing this in parallel with B4-B6 risks
merge conflicts; it should be sequenced as a task *after* B6 lands.

## Testing

Same TDD pattern as B1-B3: a failing test asserting `chat-dock-plan` exists
and the old collapse-toggle testids are gone, plus tests for the Panels▾
menu (reopening a closed panel re-adds its testid; reset restores the
five-panel default layout).
