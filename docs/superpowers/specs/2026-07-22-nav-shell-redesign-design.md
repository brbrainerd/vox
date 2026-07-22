---
title: Single-surface navigation shell — expand-in-place sidebar, drop the workbench tab bar
status: approved
---

# Navigation shell redesign — design

## Context

The Vox Axis GUI currently has two persistent top-level navigation surfaces: the left icon sidebar (`crates/vox-gui/ui/src/components/layout/Sidebar.tsx`) and a top "workbench" tab bar (`crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx`) that opens a closable tab every time a sidebar item is clicked (`App.tsx`'s `workbench` hook: `openTab`/`closeTab`/`openTabs`/`activeTab`). The user identified this as redundant — most sessions don't use multiple simultaneously-open tabs, so the two bars end up showing overlapping navigation choices, wasting a full header row of vertical space. The user also wants the eventual direction to be forward-compatible with a mobile-responsive version of the app (not needed now, but the chosen approach shouldn't require a rework later).

Confirmed via clarifying questions:
- Multi-tab usage is rare — the tab bar's "several open surfaces at once" value is barely exercised in practice.
- The tab bar should be dropped entirely, not replaced with a lighter breadcrumb/recents strip — sidebar click should just navigate.
- A separate `openDocTab`/`docLabels` mechanism (opening in-app documentation links) is a genuinely different use case from surface navigation and should be preserved as its own small, separate affordance, not folded into single-view navigation and not tied to the removed tab bar.

The codebase already has the infrastructure for a two-level nav model: `PARENT_CHILD_MAP` and `CHILD_ORDER_BY_PARENT` (`crates/vox-gui/ui/src/lib/navigation.ts`) define 9 top-level parents (`chat`, `runs`, `agents`, `knowledge`, `workspace`, `commands`, `compute`, `mercatus`, `settings`) each with an ordered list of children, and `breadcrumbsForView()` already produces `Parent › Child` label pairs from a view key. `activeView`/URL-hash sync (`viewToHash`/`parseViewFromLocation`) already operates on view keys, independent of the tab-bar machinery layered on top.

## Approach

### 1. Sidebar becomes the sole persistent navigation surface

Each of the 9 top-level `Sidebar.tsx` nav items becomes expandable in place. A parent row with children (per `CHILD_ORDER_BY_PARENT`) gets **two independent click targets**: clicking the label/icon navigates to that parent's default child (same as today's `onOpenParent`) AND expands its children; clicking a small trailing disclosure chevron expands/collapses the children **without navigating**. This two-target design is a direct fix for an adversarial-review finding: with only one click target, a user who clicks a parent purely to see what's underneath it (e.g. "what's in Knowledge?") is unconditionally navigated to that parent's `DEFAULT_CHILD_BY_PARENT` destination — discarding whatever surface they were just on — even though every multi-child parent resolves to a real, opinionated destination (`knowledge→memory`, `workspace→console`, `compute→models`, `runs→approvals`), none of which are neutral "no-op" landings. The chevron gives a genuine peek-without-committing path at negligible UI cost (one small icon per parent-with-children row). Clicking a parent with no children (`chat`, `mercatus` — neither appears in `CHILD_ORDER_BY_PARENT`) or clicking a leaf child navigates immediately by setting `activeView`.

**Accordion, not multi-expand**: at most one parent's children are expanded at a time (either the parent containing the current `activeView`, or — if the user used the peek chevron — whichever parent they last expanded via chevron, which collapses automatically the next time a *different* parent is expanded or navigated to). This keeps the sidebar's total rendered height bounded (at most: 9 parent rows + one parent's children, never all children of all 9 groups simultaneously) — directly relevant since an unbounded-height sidebar was the root cause of the currently-being-fixed "sidebar cut off at bottom" bug, and an accordion sidebar is far less likely to reproduce that class of bug than a flat expand-all tree would be.

Sidebar's existing collapse modes (`rail` / `default` / `wide` — three states, not two; `collapsed` today means specifically `mode === 'rail'`, `default` is NOT collapsed) are unaffected except: the expand-in-place tree (both the label-click and the chevron) only renders when `mode === 'wide'`, since `rail` and `default` modes don't have room for a nested child list at their current widths (`SIDEBAR_WIDTHS`: rail 64px, default 212px, wide 280px). In `rail` or `default` mode, clicking a parent navigates to its default child directly (same behavior as today), without attempting to expand a tree that has nowhere to render.

### 2. Workbench tab bar and its state are removed

`WorkbenchTabBar.tsx`'s render call in `App.tsx` is deleted, along with the `openTabs`/`activeTab`/`closeTab` portion of the `workbench` hook's public surface. `activeViewKey` (or whatever the underlying single-view state is renamed to, since "activeTab" terminology no longer fits once there's no tab concept) becomes the one source of truth for what's rendered, set directly by sidebar clicks. `viewToHash`/`parseViewFromLocation` continue to operate unchanged since they already key off view identifiers, not tab identifiers.

The `mb-3 flex items-center gap-2 border-b border-border-subtle pb-2` row that housed the tab bar in `AppShell.tsx` is removed entirely — `StatusBar` becomes the sole header row, and the vertical space the tab-bar row occupied returns to surface content.

### 3. Breadcrumb added to StatusBar for orientation

A small `Parent › Child` breadcrumb (built from the existing `breadcrumbsForView(activeView)` helper — no new data needed) is added to `StatusBar.tsx`'s row, positioned so it doesn't compete with the existing KPI cluster or the Panels ▾ trailing slot (both already established as `shrink-0`/non-wrapping elements in that row from this session's earlier StatusBar fix). This matters most in `rail` (collapsed) sidebar mode, where there's no visible parent/child label anywhere else on screen.

### 4. Doc-viewer stays separate, detached from the removed tab bar

`openDocTab`/`docLabels` keeps its own state (unchanged data shape), but its UI is decoupled from `WorkbenchTabBar` entirely. Concretely: a doc opened via `openDocTab` renders as a slide-over panel from the right edge of the viewport, built from the existing `Glass` component (matching the app's established panel styling rather than introducing a new visual treatment), with a title bar showing the doc's label and a close (✕) affordance. Opening a second doc while one is already open replaces the current doc overlay's content (no doc-tab-stacking — this keeps the "genuinely different but still simple" character the user asked for, without reintroducing a second tab-management system). The exact width/breakpoint of the slide-over is an implementation-time detail (informed by whatever similar overlay pattern, if any, already exists in the codebase — grep for one before inventing a new one), not left open as a design ambiguity.

**Why no-stacking is safe today, and when to revisit it**: confirmed by reading `DocReader.tsx` — it renders doc content as raw text (`<pre>{q.data}</pre>`), with no markdown link parsing and nothing clickable inside a doc. So a "doc A links to doc B" chain, which would make single-slot replacement feel lossy (no way back to A), cannot happen in the current app. This decision should be revisited if `DocReader` is ever upgraded to render real markdown with clickable internal links — at that point, single-slot replacement would need at minimum a back button, or full stacking.

### 5. Mobile-forward compatibility (not built now, informs the shape only)

The accordion-tree interaction chosen for #1 is the same interaction a slide-out drawer would use on a narrow viewport — expand/collapse of nested groups, single active selection. When mobile support is eventually built, the sidebar's `wide` mode becomes a drawer triggered by a hamburger icon instead of an always-visible column, with the same `NavItem`/expand logic reused verbatim; only the trigger and positioning (overlay + scrim vs. persistent column) change. No structural rework is anticipated. This section is informational — no mobile-specific code is written as part of this effort.

### 6. localStorage migration

`useWorkbenchTabs.ts` already has a one-hop migration precedent (`migrateLegacyView()`, reading an even-older `vox_active_view` key forward when its own `vox_workbench_tabs.v1` key is absent). Since that pattern already exists and costs little to reuse, the new `useActiveView` hook reuses it: on first read, if its own storage key is empty, it checks `vox_workbench_tabs.v1` for a `{activeTab}` shape and adopts that as its initial value (one-time, one-directional — no ongoing dual-write). This is a small, cheap addition, not "no migration needed" by default; a genuinely-empty initial state (new install, or a user who already migrated) still falls back to `dashboard` as before.

## What this does not include

- No mobile-responsive implementation — only avoiding decisions that would make one harder later.
- No redesign of the sidebar's own visual styling (icons, colors, spacing) beyond what's needed to support the new expand/collapse child rows — this is a structural/behavioral change, not a reskin.
- No changes to the Chat surface's internal dockview panel system (already reworked earlier this session) — this effort is one level up, the app shell's own top-level navigation only.
- No change to `PARENT_CHILD_MAP`/`CHILD_ORDER_BY_PARENT`'s actual grouping/ordering of views — this effort changes how that existing data is rendered and navigated, not what it contains.

## Testing

Existing tests asserting on `WorkbenchTabBar`'s presence/behavior in `App.test.tsx` and any workbench-hook tests covering `openTab`/`closeTab`/`openTabs` are removed or rewritten to reflect single-view navigation, including `crates/vox-gui/ui/src/guards/surfaceRegistryEscape.test.ts`, which currently regex-scans `App.tsx` specifically for the `isDocTab(...) ? <DocReader ...>` ternary being removed — that guard's assertion is retired along with the pattern it watches, not just mechanically updated. New tests cover: sidebar accordion expand/collapse (selecting a child in group B collapses group A), the peek-chevron expanding a parent's children without changing `activeView`, that the accordion tree only renders in `wide` mode (not `rail` or `default`), rail/default-mode click-navigates-to-default-child behavior, breadcrumb rendering matches `breadcrumbsForView()` output, and the doc-overlay open/replace/close behavior. Live CDP verification (this session's established practice for real-layout-dependent behavior) is used to confirm the accordion sidebar's height stays bounded and doesn't reproduce the "cut off at bottom" bug class, at both a tall and a short (500-600px) window height.
