# vox-gui Navigation and Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Cross-cutting IA improvements — breadcrumbs, deep links, sidebar badges, Policies two-rail layout — landing before Wave 5 Config/Ops.

**Architecture:** `PARENT_CHILD_MAP` in `ui/src/lib/navigation.ts` drives breadcrumb labels. View keys sync to URL hash or `?view=` query. Badges fed from orchestrator/approval IPC snapshots.

**Tech Stack:** React 19, Radix, existing sidebar in `App.tsx`.

> **Spec:** `docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md`

---

## Task 1: Breadcrumb bar

**Files:**
- Create: `ui/src/components/layout/BreadcrumbBar.tsx`
- Create: `ui/src/components/layout/BreadcrumbBar.test.tsx`
- Modify: `ui/src/App.tsx`

- [ ] **Step 1:** Resolve `{ parent, child }` from `PARENT_CHILD_MAP[viewKey]`
- [ ] **Step 2:** Render `Agents › Dashboard` with keyboard-focusable segments
- [ ] **Step 3:** Mount below top bar on all non-chat views

---

## Task 2: Deep-linkable view keys

**Files:**
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/lib/navigation.ts`

- [ ] **Step 1:** On mount, read `window.location.hash` (`#view=dashboard`) or `?view=`
- [ ] **Step 2:** On view change, `history.replaceState` with hash
- [ ] **Step 3:** Vitest: `parseViewFromLocation` / `viewToHash` helpers

---

## Task 3: Sidebar badges

**Files:**
- Modify: `ui/src/App.tsx` (nav item render)

- [ ] **Step 1:** Pending approvals count on **Runs & Approvals**
- [ ] **Step 2:** Policy failure count on **Policies** child (when summary IPC available)
- [ ] **Step 3:** `aria-label` includes count ("Runs and Approvals, 3 pending")

---

## Task 4: Policies two-rail layout

**Files:**
- Modify: `ui/src/components/surfaces/Policies/PoliciesView.tsx`

- [ ] **Step 1:** Left rail: policy tree (`policyTree.ts`); right pane: detail + violations
- [ ] **Step 2:** Match spec breakpoints (collapse rail < xl)
- [ ] **Step 3:** Playwright golden route for Policies

---

## Task 5: IA fixes

- [ ] **Step 1:** Move Gamify nav under Agents (not Settings) per audit D*
- [ ] **Step 2:** Surface Coverage under Settings with link from CI failures
- [ ] **Step 3:** Update `gui-navigation.md` IA diagram

---

## Exit criteria

- Breadcrumbs visible on Dashboard, Console, Policies pilots
- Shareable URL opens correct view
- Policies matches two-rail spec at desktop width
