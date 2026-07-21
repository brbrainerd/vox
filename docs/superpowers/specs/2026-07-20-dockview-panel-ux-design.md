---
title: Dockview panel UX — Plan panel migration, reopen/reset controls
status: approved
---

# Dockview panel UX fixes — design

> **Revision note (post-adversarial-audit):** the original version of this
> spec's Problem section misdescribed the root cause (it claimed
> `ChatSurface`'s root container lost `flex` entirely in Task B2 — false,
> verified against the actual file: the class is
> `"relative flex min-h-[60vh] gap-4"` today, still a flex row). Corrected
> below. This revision also adds two design decisions the original spec
> left implicit and which an independent audit found would otherwise
> produce a broken feature and an accessibility violation: closed-panel
> tracking (Approach §2) and the Panels-menu's ARIA shape (Approach §3).

## Problem

`ChatSurface.tsx`'s Plan panel (the checklist/plan-DAG panel) is a
hand-rolled flex sibling of the dockview shell, with its own
`useLocalStorage`-backed collapse toggle (`planPanelCollapsed`,
key `gui.chat.plan_panel_collapsed.v1`). Its root container is still a flex
row (`relative flex min-h-[60vh] gap-4`) — Task B2 did not remove `flex`,
only added `relative` and dropped `flex-col` (row vs. column). So the
container context is not the bug.

The actual bug is simpler: "collapsed" is a CSS state, not a real removal.
The collapsed variant still renders a `<aside className="shrink-0">`
containing a `<Glass>`-wrapped toggle button — a real, padded, bordered
element with non-zero width. That's what "hangs in empty space, still
takes up width while collapsed" describes: a persistent ~40-50px strip
that never actually goes away, because collapse was implemented as "render
less content," not "remove the panel."

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
   in `CHAT_DOCK_COMPONENTS`, and inclusion in the existing
   `dockApiRef`-driven refresh `useEffect` (the B2/B3-established pattern
   for keeping panel content live across re-renders, and — for
   `executionRail`/`flow`/now `plan` — for lazily creating the panel once
   its underlying data/props are ready). Closing the panel via dockview's
   native tab-close removes it and its layout footprint completely — not a
   CSS collapse hack. The old `planPanelCollapsed` state, its
   `useLocalStorage` key, and the `<aside>` toggle-strip JSX are deleted
   entirely.

2. **Fix the pre-existing "refresh effect fights the user's own close"
   race — required, not optional.** The refresh `useEffect` has no
   dependency array (runs after every render) and, for `executionRail` and
   `flow`, does `const p = api.getPanel(id); if (p) update(); else
   addPanel();`. There is currently no way to distinguish "this panel was
   never created because its data isn't ready yet" from "the user just
   closed it." Any re-render after a close — a streamed chat token, a
   session-list poll, a plan-node edit — will see `getPanel(id) ===
   undefined` and silently recreate the panel the user just closed. This
   bug already exists today for `executionRail`/`flow` (landed in
   B2/B3); this plan's Plan-panel migration would import a third instance
   of it verbatim if not fixed, and it would also directly defeat this
   spec's own reopen-menu feature (a "closed" panel would typically already
   be resurrected by the next unrelated render, before a user ever opens
   the Panels menu to reopen it deliberately).

   Fix: track user-initiated (or reset-triggered) removals in a
   `closedPanelIds` ref (a `Set<ChatDockPanelId>`), populated via
   dockview's `DockviewApi.onDidRemovePanel` event. The lazy-create
   branches in the refresh effect check `!closedPanelIds.current.has(id)`
   before auto-adding a missing panel. The reopen action (below) and the
   reset action both clear the relevant id(s) from the set before
   recreating those panels — this is *the* mechanism that distinguishes
   "recreate because ready" from "recreate because the user asked to."

3. **Add a "Panels" menu button** (same visual pattern as the existing
   sidebar-toggle buttons: a small bordered icon button, positioned near
   the top of `ChatSurface`'s root). It has two actions:
   - **Reopen**: lists any panel id currently in `closedPanelIds` (not
     merely "absent from `api.panels`" — a panel can be legitimately absent
     because its data isn't ready, which is not something a user should be
     offered to "reopen"), and on click clears that id from the set and
     recreates the panel via the shared per-id creator (Data flow, below).
   - **Reset layout**: clears whatever Task B4 persists, clears the entire
     `closedPanelIds` set, removes every currently-mounted panel, and
     recreates the five-panel default arrangement.

   **ARIA shape — explicit decision, not left to task-time default:** do
   *not* give this menu `role="menu"`/`role="menuitem"`. That role pair
   obligates full keyboard menu semantics (arrow-key navigation, Home/End,
   roving tabindex) per the ARIA Authoring Practices Guide, and this
   codebase's one existing instance of the same pattern
   (`ChatSessionRail.tsx`'s session-actions menu) already claims the role
   without implementing that contract — this spec will not add a second
   broken instance. Use a plain popover: an unstyled `<div>` of ordinary
   `<button type="button">`s, no ARIA role claim beyond what's native to
   `<button>`. It still needs, and must implement, Escape-to-close and
   click-outside-to-close (cheap, and already precedented in this same
   file by the `routingOpen` state's Escape handler) and must return focus
   to the "Panels" trigger button when it closes by any path. This is a
   deliberately smaller contract than a real ARIA menu, chosen because it's
   achievable correctly in this scope — a full keyboard-navigable menu is
   out of scope for this plan and should be its own follow-up if wanted
   (tracked nowhere yet; flag if desired).

   **Known v1 limitation, stated explicitly:** reopening a panel always
   places it at its default position/size. If a user had dragged a panel
   to a custom location before closing it, reopening does not restore that
   customization — only Task B4's whole-layout persistence (separate
   mechanism, captures the *current* arrangement periodically) preserves
   custom positions, and only for panels that stay open. This is
   acceptable for v1; per-panel position memory across a close/reopen
   cycle is a real added-complexity feature, not a bug fix, and is out of
   scope here.

4. No dockview configuration changes are needed for drag-to-reposition or
   drag-to-tab — both are on by default. This spec does not add or modify
   any dockview option flags.

## Data flow

`PlanDockPanel` renders the same `<PlanPanel>` component and props that the
current `<aside>` block passes — `planSessionId`, `planVersion`, and
`nodes` (the prop is named `nodes`; `planNodes` is only the local
`ChatSurface` state-variable name that gets passed as that prop). This is a
pure relocation of existing JSX into a dockview panel wrapper, not a
behavior change to the Plan panel's own contents.

The Panels menu needs read access to `dockApiRef.current` and
`closedPanelIds.current` to compute which panels are reopenable, and calls
the same per-panel creation logic already needed by the refresh effect's
lazy-create branches — this logic must be extracted into one shared
per-id creator function so the refresh effect, the reopen action, and the
reset action share a single source of truth for "what does panel X look
like when (re)created," including its fallback reference-panel chain (the
existing `flow`/`executionRail` panels already fall back through multiple
candidate reference panels if their preferred neighbor doesn't exist yet —
the shared creator must preserve that, not regress to a single
hardcoded reference with no fallback).

## Sequencing

This depends on Task B4 (layout persistence — the reset action needs
something to clear) and follows B5/B6 too, since all of B2-B6 repeatedly
touch the same `onReady`/`CHAT_DOCK_COMPONENTS`/refresh-`useEffect` region
of `ChatSurface.tsx`. **Do not implement or execute this plan concurrently
with an in-progress B4-B6 effort — same file, same region, guaranteed
merge conflicts.** Sequence this as a task *after* B6 lands, and before
starting, check whether B4 already introduced any panel-close/registry
tracking mechanism (plausible, since serializing a layout needs to know
current panel membership) that the closed-panel-tracking mechanism above
(Approach §2) should reuse rather than duplicate.

## Testing

Same TDD pattern as B1-B3, plus explicit coverage for the two things this
revision added:
- A failing-first test asserting `chat-dock-plan` exists and the old
  collapse-toggle testids are gone.
- Reopen-menu and reset-layout tests, using dockview-react's *real*
  rendered DOM for the tab close button (`data-testid` =
  `"dockview-dv-default-tab"` on the tab, `.dv-default-tab-action` for the
  close-click target inside it — confirmed against dockview-react
  6.6.1's actual source, not guessed).
- **The regression test that matters most**: close a panel via the real
  dockview close action, force one additional unrelated re-render (e.g.
  change an unrelated prop), and assert the panel is *still* closed — this
  is the exact bug Approach §2 exists to fix, and it must be provable, not
  just asserted away in prose.
- A keyboard/focus test for the Panels popover: Escape closes it and
  returns focus to the trigger button.
- A reset-layout test when no layout was ever persisted (localStorage key
  absent) — must not throw.
