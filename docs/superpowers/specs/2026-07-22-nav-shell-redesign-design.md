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

Each of the 9 top-level `Sidebar.tsx` nav items becomes expandable in place. Clicking a parent that has children (per `CHILD_ORDER_BY_PARENT`) expands an indented child list directly beneath it, using the same `NavItem` component pattern at a smaller visual scale (smaller icon slot, tighter row height) — no new component family, a variant of the existing one. Clicking a parent with no children (`chat`, `mercatus` — neither appears in `CHILD_ORDER_BY_PARENT`) or clicking a leaf child navigates immediately by setting `activeView`.

**Accordion, not multi-expand**: only the parent group containing the current `activeView` is expanded at any time; selecting a child in a different parent collapses the previous parent's children and expands the new one. This keeps the sidebar's total rendered height bounded (at most: 9 parent rows + one parent's children, never all children of all 9 groups simultaneously) — directly relevant since an unbounded-height sidebar was the root cause of the currently-being-fixed "sidebar cut off at bottom" bug, and an accordion sidebar is far less likely to reproduce that class of bug than a flat expand-all tree would be.

Sidebar's existing collapse modes (`rail` / `wide`, already present via the collapse/expand toggle button audited earlier this session) are unaffected — the expand-in-place tree only applies in `wide` mode, since `rail` mode shows icons only with no room for a nested child list; clicking a parent in `rail` mode navigates to that parent's default child directly (same behavior as today for icon-only sidebars generally), without expanding a tree that has nowhere to render.

### 2. Workbench tab bar and its state are removed

`WorkbenchTabBar.tsx`'s render call in `App.tsx` is deleted, along with the `openTabs`/`activeTab`/`closeTab` portion of the `workbench` hook's public surface. `activeViewKey` (or whatever the underlying single-view state is renamed to, since "activeTab" terminology no longer fits once there's no tab concept) becomes the one source of truth for what's rendered, set directly by sidebar clicks. `viewToHash`/`parseViewFromLocation` continue to operate unchanged since they already key off view identifiers, not tab identifiers.

The `mb-3 flex items-center gap-2 border-b border-border-subtle pb-2` row that housed the tab bar in `AppShell.tsx` is removed entirely — `StatusBar` becomes the sole header row, and the vertical space the tab-bar row occupied returns to surface content.

### 3. Breadcrumb added to StatusBar for orientation

A small `Parent › Child` breadcrumb (built from the existing `breadcrumbsForView(activeView)` helper — no new data needed) is added to `StatusBar.tsx`'s row, positioned so it doesn't compete with the existing KPI cluster or the Panels ▾ trailing slot (both already established as `shrink-0`/non-wrapping elements in that row from this session's earlier StatusBar fix). This matters most in `rail` (collapsed) sidebar mode, where there's no visible parent/child label anywhere else on screen.

### 4. Doc-viewer stays separate, detached from the removed tab bar

`openDocTab`/`docLabels` keeps its own state (unchanged data shape), but its UI is decoupled from `WorkbenchTabBar` entirely. Concretely: a doc opened via `openDocTab` renders as a slide-over panel from the right edge of the viewport, built from the existing `Glass` component (matching the app's established panel styling rather than introducing a new visual treatment), with a title bar showing the doc's label and a close (✕) affordance. Opening a second doc while one is already open replaces the current doc overlay's content (no doc-tab-stacking — this keeps the "genuinely different but still simple" character the user asked for, without reintroducing a second tab-management system). The exact width/breakpoint of the slide-over is an implementation-time detail (informed by whatever similar overlay pattern, if any, already exists in the codebase — grep for one before inventing a new one), not left open as a design ambiguity.

### 5. Mobile-forward compatibility (not built now, informs the shape only)

The accordion-tree interaction chosen for #1 is the same interaction a slide-out drawer would use on a narrow viewport — expand/collapse of nested groups, single active selection. When mobile support is eventually built, the sidebar's `wide` mode becomes a drawer triggered by a hamburger icon instead of an always-visible column, with the same `NavItem`/expand logic reused verbatim; only the trigger and positioning (overlay + scrim vs. persistent column) change. No structural rework is anticipated. This section is informational — no mobile-specific code is written as part of this effort.

## What this does not include

- No mobile-responsive implementation — only avoiding decisions that would make one harder later.
- No redesign of the sidebar's own visual styling (icons, colors, spacing) beyond what's needed to support the new expand/collapse child rows — this is a structural/behavioral change, not a reskin.
- No changes to the Chat surface's internal dockview panel system (already reworked earlier this session) — this effort is one level up, the app shell's own top-level navigation only.
- No change to `PARENT_CHILD_MAP`/`CHILD_ORDER_BY_PARENT`'s actual grouping/ordering of views — this effort changes how that existing data is rendered and navigated, not what it contains.

## Testing

Existing tests asserting on `WorkbenchTabBar`'s presence/behavior in `App.test.tsx` and any workbench-hook tests covering `openTab`/`closeTab`/`openTabs` are removed or rewritten to reflect single-view navigation. New tests cover: sidebar accordion expand/collapse (selecting a child in group B collapses group A), rail-mode click-navigates-to-default-child behavior, breadcrumb rendering matches `breadcrumbsForView()` output, and the doc-overlay open/replace/close behavior. Live CDP verification (this session's established practice for real-layout-dependent behavior) is used to confirm the accordion sidebar's height stays bounded and doesn't reproduce the "cut off at bottom" bug class, at both a tall and a short (500-600px) window height.
