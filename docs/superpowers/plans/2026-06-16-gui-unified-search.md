# vox-gui Unified Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** One `SearchController` SSOT consumed by CommandPalette, SearchView, and Chat `/` router — with prefix routing, MRU, facet state, and `open_locator` navigation.

**Architecture:** Backend: `commands/search.rs` → `vox_search_query`. Frontend SSOT: `src/lib/searchController.ts` (reducer + scope mapping). Transport: `voxTransport.voxSearchQuery`, `voxTransport.voxDocsIndex`, `voxTransport.openLocator`.

**Tech Stack:** React 19, `vox-search` hybrid scopes, vitest, Playwright.

> **Source of truth:** Master roadmap Track 3; audit items C71–C93.

---

## Current assets

| Asset | Path | Status |
|-------|------|--------|
| Reducer + scopes | `ui/src/lib/searchController.ts` | Partial (~30%) |
| SearchView | `ui/src/components/surfaces/Search/SearchView.tsx` | Own fetch path |
| CommandPalette | `ui/src/components/layout/CommandPalette.tsx` | Mixed palette sources |
| Memory recall | `MemoryView.tsx` | Direct `vox_search_query` invoke |

---

## Task 1: `useSearchController` hook

**Files:**
- Create: `ui/src/hooks/useSearchController.ts`
- Create: `ui/src/hooks/useSearchController.test.ts`

- [ ] **Step 1:** Wrap `searchReducer` + debounced `voxTransport.voxSearchQuery` in a hook returning `{ state, setQuery, setScopes, runSearch }`
- [ ] **Step 2:** Stale-request guard via `requestToken` (already in reducer)
- [ ] **Step 3:** Tests: scope mapping, token discard, debounce

---

## Task 2: Palette prefix routing

**Files:**
- Modify: `ui/src/components/layout/CommandPalette.tsx`
- Modify: `ui/src/components/layout/paletteSources.ts`

- [ ] **Step 1:** `>` → CLI catalog hits; `@` → agent targets; `/` → skills/docs
- [ ] **Step 2:** Show safety/tier badges on CLI hits from action manifest
- [ ] **Step 3:** Vitest: prefix selects correct source without network

---

## Task 3: SearchView consumes hook

**Files:**
- Modify: `ui/src/components/surfaces/Search/SearchView.tsx`

- [ ] **Step 1:** Replace local query state with `useSearchController`
- [ ] **Step 2:** Wrap results in `<Async>` + `EmptyState`
- [ ] **Step 3:** Extend `SearchView.test.tsx` for loading/error paths

---

## Task 4: `open_locator` routing

**Files:**
- Modify: `ui/src/components/layout/surfaceComponents.tsx`
- Modify: `ui/src/lib/navigation.ts`

- [ ] **Step 1:** Map locator kinds → view keys (file → repository/browser, web → browser, surface → registry id)
- [ ] **Step 2:** Export `navigateFromLocator(locator)` used by palette + Search + Memory hits
- [ ] **Step 3:** Playwright: palette search → select hit → correct surface visible

---

## Task 5: Typed argument preview (depends on Track 4 manifest)

- [ ] **Step 1:** When hit is CLI command with args, show `GenericCommandForm` preview before `execute_command`
- [ ] **Step 2:** Gate destructive commands behind confirmation dialog

---

## Exit criteria

- Single vitest suite covers `useSearchController` + palette prefix routing
- Playwright: palette search → navigate → surface opens
- No duplicate scope-mapping logic outside `searchController.ts`
