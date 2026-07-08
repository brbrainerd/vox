---
title: "Axis Workbench Tabs — Shell Reliability Design"
description: "Unified leaf-surface tab bar, scroll fix, Chat-attention budget, help-only search with in-app doc tabs, Console/Scientia reliability, and VoxDb singleton for Windows lock contention."
category: "architecture"
status: "design"
date: 2026-07-08
---

# Axis Workbench Tabs — Shell Reliability Design

**Date:** 2026-07-08  
**Status:** Design approved (operator session)  
**Scope:** Vox Axis (`crates/vox-gui`) shell navigation, scroll, search, Scientia/Console reliability  

> **Supersedes (partially):** single-`activeView` navigation model and global `AttentionStrip`.  
> **Related:** `docs/superpowers/specs/2026-06-14-vox-gui-design-principles-application-design.md`, `docs/superpowers/plans/2026-06-16-gui-navigation-layout.md`, `docs/src/architecture/data-storage-ssot-2026.md`, `docs/src/reference/gui-navigation.md`.

---

## 1. Problem statement

Operator feedback from live Axis sessions (2026-07-08):

| Symptom | Impact |
|---------|--------|
| Sidebar clicks often show blank or wrong content | Core navigation untrustworthy |
| Each sidebar group feels like a separate “page” with its own sub-tab row | Cognitive overhead; conflicts with desired closable-tab UX |
| Global attention budget strip consumes vertical space | Was intended inside Chat; Chat hard to reach |
| Main content does not scroll | Long surfaces (Settings/Coverage, Scientia) unusable |
| Omnibar surfaces architecture SSOT docs | Operators want **help** docs only |
| Doc hits spawn external editor | No in-GUI reading; search feels broken |
| Console surface broken | Primary workspace tool unavailable |
| Scientia shows raw VoxDb lock errors (os error 33) | Knowledge surfaces fail when GUI + daemon both open `.vox/store.db` |
| “Not deposited” archive rollup misread as errors | Confusing Scientia/VoxGiantia messaging |

Visual AI review (2026-07-08) corroborates: contrast/hierarchy/clutter in the **shell chrome** amplify these failures.

---

## 2. Goals and non-goals

### Goals

1. **Leaf workbench tabs** — one closable tab per surface (`console`, `chat`, `dashboard`, …); sidebar opens or focuses tabs; no full-page swap.
2. **Reliable scroll** — exactly one scrollport per tab content area.
3. **Chat as first-class tab** — Loquela composer and attention budget live in Chat; remove global attention strip.
4. **Help-only doc search** — index/filter to operator-facing docs; open hits in **in-app doc reader tabs**.
5. **Console + Scientia P0 reliability** — PTY/orchestrator visible errors; shared DB connection in GUI; friendly lock retry UX.

### Non-goals (this spec)

- Full Style Dictionary / token pipeline (Phase 0 of design-principles plan — separate wave).
- Replacing dockview for **optional** in-tab splits (Console terminal + agent pane) — deferred to Console wave.
- Changing orchestrator-d’s long-lived DB ownership model (only GUI-side pooling).
- Pixel/visual redesign beyond shell layout fixes.

---

## 3. Architecture decision: Approach B (workbench tab bar)

**Chosen:** dedicated **`WorkbenchTabBar`** + tab state store; retire **`ParentSurface` / `SubTabs`** as navigation chrome.

**Rejected:**

- **A — Dockview owns all tabs:** persisted split layouts fight navigation; hard to enforce one tab per surface.
- **C — Hybrid group + leaf tabs:** two tab systems remain.

### 3.1 Tab model

```text
TabId = ViewKey | `doc:${absolutePath}`

WorkbenchState {
  openTabs: TabId[]          // left→right, persisted localStorage key vox_workbench_tabs.v1
  activeTab: TabId | null
  pinnedTabs: ViewKey[]      // default: ['chat']
}
```

| Action | Behavior |
|--------|----------|
| Sidebar click (top-level group) | `openTab(DEFAULT_CHILD_BY_PARENT[group])` — e.g. Workspace → `console` |
| Sidebar click (leaf, e.g. Settings) | `openTab('settings')` |
| Tab already open | Focus only; no duplicate |
| Close tab (× or ⌘W when tab bar focused) | Remove tab; activate right neighbor, else left; if last tab closed → open `dashboard` |
| Omnibar surface hit | `openTab(viewKey)` |
| Omnibar doc hit | `openTab(doc:path, { title })` |
| Hash `#view=console` | `openTab('console')` |

**Sidebar active highlight:** derived from `parentSurface(activeTab)`, not last clicked group.

**Registry:** `SURFACE_REGISTRY` / `PARENT_CHILD_MAP` remain SSOT for labels, breadcrumbs, and default-child mapping. `parentSurface` used for sidebar grouping only.

### 3.2 Layout

```text
┌──────────┬──────────────────────────────────────────────────────────┐
│ Sidebar  │ WorkbenchTabBar [Console ×] [Chat ×] [Dashboard ×] …     │
│          ├──────────────────────────────────────────────────────────┤
│          │ BreadcrumbBar (optional, slim — parent › child)          │
│          ├──────────────────────────────────────────────────────────┤
│          │ SurfaceScrollHost (flex-1 min-h-0 overflow-auto)         │
│          │   └ {surface or DocReader}                             │
└──────────┴──────────────────────────────────────────────────────────┘
```

- **Remove** global `AttentionStrip` from `App.tsx`.
- **Retire** `ParentSurface` + `SubTabs` for navigation (delete or reduce to breadcrumb-only helper).
- **DockShell:** default path renders `SurfaceScrollHost` only; dockview splits opt-in inside Console (later).

### 3.3 Scroll contract

**Single scroll owner per tab:**

```text
WorkbenchTabContent (flex-1 min-h-0 overflow-hidden)
  └ SurfaceScrollHost (h-full min-h-0 overflow-auto custom-scrollbar)
       └ surface component
```

Surfaces audited in P0: **Console, Chat, Settings/Coverage, Scientia**. Rules:

- No `h-screen` on surface roots.
- No outer `overflow-y-auto` on surfaces that sit inside `SurfaceScrollHost` (terminal/chat may use internal scroll only with `flex-1 min-h-0`).
- `html, body, #root` remain `overflow: hidden` (unchanged).

### 3.4 Chat and attention budget

- `chat` is a normal leaf tab; **pinned by default** on fresh profile (`pinnedTabs` includes `chat`).
- Loquela composer renders **only** inside Chat tab (keep `chatDocked = false` globally).
- **`AttentionBudgetMeter`** compact row above Loquela in Chat (waiting questions, blocked tasks, budget bar from orchestrator stream).
- Optional: badge on Chat tab when `waitingQuestions > 0`.
- Dashboard retains its inline meter (Agents context only).

### 3.5 Help-only search and doc reader tabs

**Index filter** (`docs_index.rs`): parse `category` from frontmatter.

| Include in help index | `category` values |
|-----------------------|-------------------|
| Yes | `how-to`, `tutorial`, `reference`, `contributor` |
| No | `architecture`, `research`, and anything under `docs/src/archive/` |

**Omnibar default facets:** Surfaces, Commands, On Screen. **Docs facet** only when:

- User types `/` or `/help` prefix, or
- Query contains token `help` (case-insensitive).

**Doc reader tab:**

- Tab id: `doc:<repo-relative-path>` (stable, dedupe on open).
- Content: markdown render (reuse Research/Publications markdown path or add thin `DocReader` surface).
- Primary action: read in tab. Secondary: “Open in editor” → existing `open_locator`.
- Close: normal tab close.

### 3.6 VoxDb locking (GUI)

**Problem:** GUI Tauri commands call `connect_workspace_journey_optional` / `connect_canonical` per invoke while `vox-orchestrator-d` holds `.vox/store.db` → Windows error 33.

**P0 fix:**

1. Add `GuiDbPool` (or extend `GuiState`) in Tauri: one `Arc<VoxDb>` for workspace journey DB, initialized in `setup` hook, shared by `chat`, `scientia`, and related commands.
2. Route `gui_db()` and `connect_canonical_db()` through the pool (clone handle, no new connect per command).
3. On `SQLITE_BUSY` / lock errors: return structured error; UI shows “Database busy — retry in a moment” with retry button (not raw Turso string).

**P1 UX:** Scientia `ArchiveStatusSummary` — rename “Not deposited” → **“Pending deposit (sample)”**; tooltip explains Zenodo/SWH; toast on fetch failure.

### 3.7 Console P0

Within tab host:

1. Mount Console only when `activeTab === 'console'` (lazy mount avoids duplicate PTY).
2. Show orchestrator + PTY spawn errors in-tab (`role="alert"`).
3. Replace Console root `height: 100%` inline styles with `flex flex-col flex-1 min-h-0` inside scroll host; terminal panel scrolls internally.

---

## 4. Phasing

| Phase | Deliverables |
|-------|----------------|
| **P0** | Workbench tabs, scroll host, Chat attention meter, remove global strip, DB singleton, Console mount/layout fix |
| **P1** | Help-filtered docs index, doc reader tabs, Omnibar wiring |
| **P2** | Scientia archive copy, tab badges, optional Console dockview splits |

---

## 5. Testing strategy

| Layer | Coverage |
|-------|----------|
| **Vitest** | `useWorkbenchTabs` open/focus/close/persist; `WorkbenchTabBar` a11y; doc tab id parsing |
| **Playwright** | `workbench-tabs.spec.ts`: sidebar opens Console tab; second click focuses; close tab; scroll on settings; Chat tab shows attention meter |
| **Rust** | `docs_index` category filter unit tests; `GuiDbPool` single-connect integration test |
| **Manual** | GUI + orchestrator-d running → Scientia dashboard loads without error 33 |

---

## 6. Risks

| Risk | Mitigation |
|------|------------|
| Tab state migration from `vox_active_view` | On first load, convert single view → one open tab |
| Doc reader XSS | Render markdown through sanitized pipeline (same as Research) |
| DB pool stale after daemon crash | Pool health check + reconnect on `SQLITE_BUSY` |
| Large plan touches `App.tsx` | Extract `useWorkbenchTabs` hook first; thin `App.tsx` wiring |

---

## 7. Success criteria

- [ ] Clicking Workspace opens/focuses Console tab; closing and reopening works.
- [ ] Chat tab reachable from sidebar; attention budget visible above composer; no global strip.
- [ ] Settings/Coverage table scrolls with mouse wheel.
- [ ] Omnibar `/help query` returns no `architecture` category docs.
- [ ] Doc hit opens readable tab; external editor is secondary action.
- [ ] Scientia dashboard loads with GUI + daemon running (no error 33 in normal use).
- [ ] Console shows terminal or actionable error (not blank).
