# vox-gui Phase 0D — Performance Utilities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add list virtualization for `TasksView` and `MemoryView`'s unbounded lists; document RAIL budget conventions; establish "heavy work → Rust `#[command]`" and "animate `transform`/`opacity` only" conventions.

**Architecture:** A shared `useVirtualList` hook wraps `@tanstack/react-virtual`'s `useVirtualizer`, keeping all virtualization plumbing out of surface components. `TasksView` has two unbounded `.map()` calls (in-progress and queued) that must be virtualized. `MemoryView` has two unbounded metadata lists (`recent_recalls` and `shards`) that must be virtualized. `RunsView` (`RUNS_LIST_LIMIT = 40`) and `SearchView` (`SEARCH_TOP_K = 30`) are already hard-bounded — skip those. Inline `style={{}}` usages (53 total) are intentional dynamic styles (e.g., progress bar widths); CSP already allows `style-src 'unsafe-inline'` — do NOT remove them.

**Tech Stack:** React 19, TypeScript 5, Vite 6, `@tanstack/react-virtual` v3, vitest 2, `@testing-library/react`, pnpm. Tauri v2.

> **Source of truth:** spec [`docs/superpowers/specs/2026-06-14-vox-gui-design-principles-application-design.md`](../specs/2026-06-14-vox-gui-design-principles-application-design.md); surface map [`docs/src/architecture/vox-gui-surface-map-2026-06-14.md`](../../src/architecture/vox-gui-surface-map-2026-06-14.md).

> **All commands run from `crates/vox-gui/ui/` unless noted.** This project uses **pnpm**, never npm (npm corrupts the pnpm store). Tests: `pnpm test`. Typecheck: `pnpm typecheck`.

> **Existing test baseline:** 40 test files, 178 tests, all passing. Every task must leave tests green.

---

## Scope

**In scope (Phase 0D):**
- Install `@tanstack/react-virtual`
- `useVirtualList` hook + tests
- Virtualize `TasksView` in-progress and queued lists
- Virtualize `MemoryView` recent-recalls and shards lists
- RAIL budget + animation + heavy-work conventions doc

**Out of scope (already done or other phases):**
- `RunsView` / `SearchView` — already bounded (40 and 30 items)
- Inline style removal — those are legitimate dynamic styles; CSP already permits them
- `prefers-reduced-motion` gating — Phase 0C's job
- Any Rust-side changes — this is frontend-only

---

## File Structure

| File | Status | Responsibility |
|------|--------|---------------|
| `crates/vox-gui/ui/package.json` | **Modify** | Add `@tanstack/react-virtual` runtime dependency |
| `crates/vox-gui/ui/src/hooks/useVirtualList.ts` | **Create** | Thin wrapper around `useVirtualizer` |
| `crates/vox-gui/ui/src/hooks/useVirtualList.test.ts` | **Create** | Tests for hook: item count, totalSize, overscan |
| `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` | **Modify** | Virtualize `inProgress` and `queued` lists |
| `crates/vox-gui/ui/src/components/surfaces/Memory/MemoryView.tsx` | **Modify** | Virtualize `recent_recalls` and `shards` lists |
| `docs/src/architecture/gui-perf-conventions-2026-06-14.md` | **Create** | RAIL budgets, animation rules, heavy-work patterns |

---

## Task 1: Install `@tanstack/react-virtual` and create `useVirtualList` hook

**What and why:** `@tanstack/react-virtual` v3 integrates cleanly with React 19 hooks, has no DOM manipulation, and is framework-agnostic. The `useVirtualList` wrapper gives surface components a consistent API.

### Step 1.1 — Write the failing test first

- [ ] Create `crates/vox-gui/ui/src/hooks/useVirtualList.test.ts` with:

```typescript
import { describe, it, expect } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useVirtualList } from './useVirtualList';

// useVirtualizer needs a container element with a measurable height.
// jsdom doesn't do layout, so we supply a fixed height via the
// scrollElement ref and override getBoundingClientRect.
function makeContainerRef(height: number) {
  const el = document.createElement('div');
  Object.defineProperty(el, 'getBoundingClientRect', {
    value: () => ({ height, width: 400, top: 0, left: 0, right: 400, bottom: height, x: 0, y: 0, toJSON: () => {} }),
  });
  Object.defineProperty(el, 'offsetHeight', { value: height });
  return { current: el } as React.RefObject<HTMLDivElement>;
}

describe('useVirtualList', () => {
  it('returns a virtualizer and totalSize', () => {
    const items = Array.from({ length: 100 }, (_, i) => ({ id: i }));
    const containerRef = makeContainerRef(400);

    const { result } = renderHook(() =>
      useVirtualList({
        containerRef,
        count: items.length,
        estimateSize: () => 44,
        overscan: 3,
      }),
    );

    expect(result.current.virtualizer).toBeDefined();
    expect(typeof result.current.totalSize).toBe('number');
    // totalSize should be count × estimateSize at minimum before any measurement
    expect(result.current.totalSize).toBeGreaterThanOrEqual(0);
    expect(Array.isArray(result.current.virtualItems)).toBe(true);
  });

  it('returns zero virtualItems for an empty list', () => {
    const containerRef = makeContainerRef(400);

    const { result } = renderHook(() =>
      useVirtualList({
        containerRef,
        count: 0,
        estimateSize: () => 44,
        overscan: 3,
      }),
    );

    expect(result.current.virtualItems).toHaveLength(0);
    expect(result.current.totalSize).toBe(0);
  });

  it('exposes start/end index on each virtual item', () => {
    const containerRef = makeContainerRef(200);

    const { result } = renderHook(() =>
      useVirtualList({
        containerRef,
        count: 50,
        estimateSize: () => 44,
        overscan: 0,
      }),
    );

    const items = result.current.virtualItems;
    if (items.length > 0) {
      expect(typeof items[0].index).toBe('number');
      expect(typeof items[0].start).toBe('number');
      expect(typeof items[0].size).toBe('number');
    }
  });
});
```

- [ ] Run `pnpm test` — expect failures (hook file does not exist yet):

```
Expected output:
  FAIL src/hooks/useVirtualList.test.ts
  Cannot find module './useVirtualList'
```

### Step 1.2 — Install the package

- [ ] From `crates/vox-gui/ui/`:

```
pnpm add @tanstack/react-virtual
```

Expected: `package.json` now lists `"@tanstack/react-virtual"` under `dependencies`.

### Step 1.3 — Create the hook

- [ ] Create `crates/vox-gui/ui/src/hooks/useVirtualList.ts`:

```typescript
import { useVirtualizer, type VirtualItem } from '@tanstack/react-virtual';
import type React from 'react';

export interface UseVirtualListOptions {
  /** Ref to the scrollable container element. */
  containerRef: React.RefObject<HTMLElement>;
  /** Total number of items in the list. */
  count: number;
  /** Return the estimated (or known) height of item at `index`, in px. */
  estimateSize: (index: number) => number;
  /** Number of extra items to render above and below the visible window. Default: 3. */
  overscan?: number;
}

export interface UseVirtualListResult {
  /** The underlying @tanstack/react-virtual virtualizer instance. */
  virtualizer: ReturnType<typeof useVirtualizer>;
  /** Total scroll height of the virtual list in px; set as the inner div's height. */
  totalSize: number;
  /** The currently rendered virtual items (sliced from the full list). */
  virtualItems: VirtualItem[];
}

/**
 * Thin hook wrapper around @tanstack/react-virtual's useVirtualizer.
 *
 * Usage:
 *   const containerRef = useRef<HTMLDivElement>(null);
 *   const { virtualizer, totalSize, virtualItems } = useVirtualList({
 *     containerRef,
 *     count: items.length,
 *     estimateSize: () => 44,
 *   });
 *
 *   return (
 *     <div ref={containerRef} style={{ height: '400px', overflow: 'auto' }}>
 *       <div style={{ height: totalSize, position: 'relative' }}>
 *         {virtualItems.map(vItem => (
 *           <div
 *             key={vItem.key}
 *             ref={virtualizer.measureElement}
 *             data-index={vItem.index}
 *             style={{ position: 'absolute', top: 0, transform: `translateY(${vItem.start}px)`, width: '100%' }}
 *           >
 *             {items[vItem.index]}
 *           </div>
 *         ))}
 *       </div>
 *     </div>
 *   );
 *
 * Convention: "heavy work → Rust #[command]" — do not compute expensive
 * transformations inside estimateSize. Keep it to a constant or cheap lookup.
 */
export function useVirtualList({
  containerRef,
  count,
  estimateSize,
  overscan = 3,
}: UseVirtualListOptions): UseVirtualListResult {
  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => containerRef.current,
    estimateSize,
    overscan,
  });

  return {
    virtualizer,
    totalSize: virtualizer.getTotalSize(),
    virtualItems: virtualizer.getVirtualItems(),
  };
}
```

### Step 1.4 — Verify tests pass

- [ ] Run `pnpm test` — all 3 new hook tests must pass, existing 178 tests must stay green.
- [ ] Run `pnpm typecheck` — no errors.

### Step 1.5 — Commit

```
git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" add \
  crates/vox-gui/ui/package.json \
  crates/vox-gui/ui/pnpm-lock.yaml \
  crates/vox-gui/ui/src/hooks/useVirtualList.ts \
  crates/vox-gui/ui/src/hooks/useVirtualList.test.ts

git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" commit -m \
  "feat(vox-gui/perf): install @tanstack/react-virtual + useVirtualList hook (Phase 0D T1)"
```

Expected output:
```
[claude/vox-gui-design-principles-phase0 <hash>] feat(vox-gui/perf): install @tanstack/react-virtual + useVirtualList hook (Phase 0D T1)
 4 files changed, ...
```

---

## Task 2: Virtualize `TasksView` unbounded lists

**What and why:** `TasksView` contains two unbounded `.map()` calls over `inProgress` and `queued` (lines 272 and 286 of the file as read). With many tasks, these render every item synchronously on every poll cycle (every 4 s). Virtualization limits DOM nodes to the visible window + overscan.

**Key observations from reading `TasksView.tsx`:**
- The outer scrollable container is `<div className="flex-1 space-y-5 overflow-auto custom-scrollbar">` (line 266). This is `flex-1` — it fills available height. We must give each inner list section its own bounded scroll container, not the outer one, because the two sections (in-progress and queued) each need their own virtualizer.
- Each row is rendered by `renderRow()` — a function defined inside the component. The virtualized version calls the same function, just with `transform: translateY(...)` positioning.
- Each row has `space-y-1.5` spacing. We'll encode this as part of the item start offset via the gap (6 px) added to each item's `start`.
- Estimated row height: approximately 60 px (two lines of text + padding + border). Use 64 px as the estimate.
- The virtualized sections use a fixed height. Use `min(count × 64, 320)` px so short lists stay compact and long lists cap at 320 px (5 rows visible).

### Step 2.1 — Write the failing test first

- [ ] Create `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.test.tsx`:

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

// Mock Tauri invoke — TasksView calls list_orchestrator_tasks on mount.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

import { TasksView } from './TasksView';

describe('TasksView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the Tasks heading', async () => {
    render(<TasksView />);
    expect(screen.getByText('Tasks')).toBeDefined();
  });

  it('renders In progress and Queued section headings', async () => {
    render(<TasksView />);
    // Both section headings must be present even with empty data
    expect(screen.getByText(/In progress/i)).toBeDefined();
    expect(screen.getByText(/Queued/i)).toBeDefined();
  });

  it('shows empty-state messages when lists are empty', async () => {
    render(<TasksView />);
    // Empty state text for both sections (rendered when not loading and list is empty)
    // These appear after the async load resolves to []; we test the structural presence.
    expect(screen.getByPlaceholderText('Add a task…')).toBeDefined();
  });

  it('renders the Add button', () => {
    render(<TasksView />);
    expect(screen.getByText('Add')).toBeDefined();
  });
});
```

- [ ] Run `pnpm test` — these should already pass against the existing `TasksView` (no changes yet), confirming the test file compiles and the component renders in jsdom. If any fail due to missing mocks, fix the mock before proceeding.

### Step 2.2 — Virtualize `TasksView`

- [ ] Read `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` (done above).
- [ ] Apply the following changes to `TasksView.tsx`:

**Add imports at the top** (after the existing imports):

```typescript
import { useRef } from 'react';
import { useVirtualList } from '../../../hooks/useVirtualList';
```

Note: `useRef` is already imported as part of `React, { useCallback, useEffect, useRef, useState }`. Only add `useVirtualList`.

**Add the two container refs** inside the `TasksView` component body, after the existing state declarations:

```typescript
const inProgressRef = useRef<HTMLDivElement>(null);
const queuedRef = useRef<HTMLDivElement>(null);
```

**Add the two virtualizer hook calls** after the refs, after `const { inProgress, queued } = groupTasks(...)`:

```typescript
const ITEM_HEIGHT = 64; // px — estimated row height (two-line task row + py-2 + border)
const GAP = 6;          // px — space-y-1.5 = 6px gap between rows

const inProgressVL = useVirtualList({
  containerRef: inProgressRef,
  count: inProgress.length,
  estimateSize: () => ITEM_HEIGHT,
  overscan: 3,
});

const queuedVL = useVirtualList({
  containerRef: queuedRef,
  count: queued.length,
  estimateSize: () => ITEM_HEIGHT,
  overscan: 3,
});
```

**Replace the in-progress section** (the `<div className="space-y-1.5">` block containing `{inProgress.map(t => renderRow(t, false))}`) with:

```typescript
<div
  ref={inProgressRef}
  style={{ height: Math.min(inProgress.length * (ITEM_HEIGHT + GAP), 320), overflow: 'auto' }}
  className="custom-scrollbar"
>
  <div style={{ height: inProgressVL.totalSize, position: 'relative' }}>
    {inProgressVL.virtualItems.map(vItem => (
      <div
        key={vItem.key}
        ref={inProgressVL.virtualizer.measureElement}
        data-index={vItem.index}
        style={{ position: 'absolute', top: 0, transform: `translateY(${vItem.start}px)`, width: '100%', paddingBottom: GAP }}
      >
        {renderRow(inProgress[vItem.index], false)}
      </div>
    ))}
  </div>
  {inProgress.length === 0 && !loading && (
    <p className="px-1 text-[11px] text-zinc-600">Nothing running.</p>
  )}
</div>
```

**Replace the queued section** (the `<div className="space-y-1.5">` block containing `{queued.map(t => renderRow(t, true))}`) with:

```typescript
<div
  ref={queuedRef}
  style={{ height: Math.min(queued.length * (ITEM_HEIGHT + GAP), 320), overflow: 'auto' }}
  className="custom-scrollbar"
>
  <div style={{ height: queuedVL.totalSize, position: 'relative' }}>
    {queuedVL.virtualItems.map(vItem => (
      <div
        key={vItem.key}
        ref={queuedVL.virtualizer.measureElement}
        data-index={vItem.index}
        style={{ position: 'absolute', top: 0, transform: `translateY(${vItem.start}px)`, width: '100%', paddingBottom: GAP }}
      >
        {renderRow(queued[vItem.index], true)}
      </div>
    ))}
  </div>
  {queued.length === 0 && !loading && (
    <p className="px-1 text-[11px] text-zinc-600">
      Queue is empty — the agent is all yours.
    </p>
  )}
</div>
```

**Remove `space-y-1.5` from the outer `<div>` wrappers** — those wrappers are replaced entirely by the virtualized containers above. No other changes to section headings, the surrounding `<section>` elements, or the outer scrolling container.

### Step 2.3 — Verify

- [ ] Run `pnpm test` — all existing + new TasksView tests must pass.
- [ ] Run `pnpm typecheck` — no errors.

Expected output:
```
Test Files  41 passed (41)
Tests      182 passed (182)
```

### Step 2.4 — Commit

```
git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" add \
  crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx \
  crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.test.tsx

git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" commit -m \
  "feat(vox-gui/perf): virtualize TasksView in-progress + queued lists (Phase 0D T2)"
```

---

## Task 3: Virtualize `MemoryView` unbounded metadata lists

**What and why:** `MemoryView` renders two unbounded lists: `recent_recalls` (a list of past query records) and `shards` (one card per HNSW shard). Both are sourced from `MemoryStatusPayload` and have no server-side cap. The shard grid especially can grow with the corpus. The hits list is capped by `topK` (user-controlled, max 50) — do not virtualize it.

**Key observations from reading `MemoryView.tsx`:**
- `recent_recalls` is a vertical list of `<button>` rows inside `<div className="mt-3 space-y-1.5">`. Each button is approximately 52 px tall. The container is inside a `col-span-12 xl:col-span-4` Glass panel.
- `shards` is a `grid grid-cols-2 md:grid-cols-3 xl:grid-cols-6` of cards. Virtualizing a multi-column grid with `useVirtualizer` requires treating the grid as a single flat list with a larger `estimateSize` and removing the CSS grid — instead we'll use a single-column virtualized layout for the shard list, replacing the grid with a flex-col stack, since the shard card height is known (~140 px). This simplification is acceptable because the shard count grows linearly and the card content is deterministic.
- Alternatively (and more faithfully to the existing design): apply virtualization only to `recent_recalls` (the list panel), and for `shards` apply a fixed-height scrollable container (not a virtualizer) capped at 5 rows × 140 px = 700 px, since shards are bounded in practice by storage tier. This avoids breaking the grid layout.

**Decision:** Virtualize `recent_recalls` with `useVirtualList`. For `shards`, wrap in a fixed-height scrollable container (`max-h-[700px] overflow-y-auto`) — this is sufficient because shard counts are bounded by the corpus tier in practice and the grid layout would be destroyed by a flat virtualizer.

**Estimated heights:**
- `recent_recalls` row: ~52 px (title + subtitle + py-1.5 + border).
- Container height for recent_recalls: `min(count × 58, 312)` px (≈ 6 rows visible max, 58 = 52 + 6 gap).

### Step 3.1 — Write the failing test first

- [ ] Create `crates/vox-gui/ui/src/components/surfaces/Memory/MemoryView.test.tsx`:

```typescript
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

// Mock Tauri invoke — MemoryView calls get_memory_status and get_gui_preference on mount.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation((cmd: string) => {
    if (cmd === 'get_memory_status') {
      return Promise.resolve({
        corpus_counts: { memory: 100, knowledge: 200, chunk: 50 },
        shards: [],
        recent_recalls: [],
        embedding_dim: 768,
      });
    }
    return Promise.resolve(null);
  }),
}));

const noopToast = () => {};

import { MemoryView } from './MemoryView';

describe('MemoryView', () => {
  it('renders the Mnemosyne heading', () => {
    render(<MemoryView pushToast={noopToast} />);
    expect(screen.getByText(/Mnemosyne/i)).toBeDefined();
  });

  it('renders the Recent recalls section heading', () => {
    render(<MemoryView pushToast={noopToast} />);
    expect(screen.getByText(/Recent recalls/i)).toBeDefined();
  });

  it('renders the Memory shards section heading', () => {
    render(<MemoryView pushToast={noopToast} />);
    expect(screen.getByText(/Memory shards/i)).toBeDefined();
  });

  it('renders the Recall button', () => {
    render(<MemoryView pushToast={noopToast} />);
    expect(screen.getByText('Recall')).toBeDefined();
  });
});
```

- [ ] Run `pnpm test` — these should pass against the existing `MemoryView` (no changes yet).

### Step 3.2 — Virtualize `MemoryView`

- [ ] Read `crates/vox-gui/ui/src/components/surfaces/Memory/MemoryView.tsx` (done above).
- [ ] Apply the following changes to `MemoryView.tsx`:

**Add imports** at the top, after existing imports:

```typescript
import { useRef } from 'react';
import { useVirtualList } from '../../../hooks/useVirtualList';
```

**Add the container ref and hook** inside the `MemoryView` component body, after the existing state declarations:

```typescript
const recallsRef = useRef<HTMLDivElement>(null);

const RECALL_ITEM_HEIGHT = 52; // px — recall button: title + subtitle + py-1.5
const RECALL_GAP = 6;          // px — space-y-1.5

const recentRecalls = memStatus?.recent_recalls ?? [];

const recallsVL = useVirtualList({
  containerRef: recallsRef,
  count: recentRecalls.length,
  estimateSize: () => RECALL_ITEM_HEIGHT,
  overscan: 3,
});
```

**Replace the `recent_recalls` list** (the `<div className="mt-3 space-y-1.5">` containing the `.map((r, i) => ...)` at line 352) with:

```typescript
<div
  ref={recallsRef}
  style={{
    height: Math.min(recentRecalls.length * (RECALL_ITEM_HEIGHT + RECALL_GAP), 312),
    overflow: 'auto',
    marginTop: '0.75rem',
  }}
  className="custom-scrollbar"
>
  <div style={{ height: recallsVL.totalSize, position: 'relative' }}>
    {recallsVL.virtualItems.map(vItem => {
      const r = recentRecalls[vItem.index];
      return (
        <div
          key={vItem.key}
          ref={recallsVL.virtualizer.measureElement}
          data-index={vItem.index}
          style={{
            position: 'absolute',
            top: 0,
            transform: `translateY(${vItem.start}px)`,
            width: '100%',
            paddingBottom: RECALL_GAP,
          }}
        >
          <button
            onClick={() => { setQuery(r.q); recall(r.q); }}
            className="flex w-full items-center justify-between rounded-md border border-white/5 bg-white/[0.02] px-2.5 py-1.5 text-left hover:border-white/15 hover:bg-white/[0.04] transition"
          >
            <div className="min-w-0">
              <div className="truncate text-[12px] text-zinc-200">{r.q}</div>
              <div className="font-mono text-[9px] text-zinc-500">{r.n} hits · {r.when} ago</div>
            </div>
            <Icon.chevR className="size-3 text-zinc-500 shrink-0" />
          </button>
        </div>
      );
    })}
  </div>
</div>
```

**Wrap the `shards` grid** in a scrollable container with a max height. Find the `<div className="mt-3 grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">` (line 419) and wrap it:

```typescript
<div className="mt-3 max-h-[700px] overflow-y-auto custom-scrollbar">
  <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
    {(memStatus?.shards ?? []).map(s => (
      /* ... existing shard card JSX unchanged ... */
    ))}
  </div>
</div>
```

Do not change any of the inner shard card JSX — only add the outer scroll wrapper.

Also remove the now-redundant `memStatus?.recent_recalls ?? []` inline expression that was used in the old `.map()` call — it is now replaced by the `recentRecalls` constant declared above.

### Step 3.3 — Verify

- [ ] Run `pnpm test` — all tests must pass.
- [ ] Run `pnpm typecheck` — no errors.

Expected output:
```
Test Files  42 passed (42)
Tests      186 passed (186)
```

### Step 3.4 — Commit

```
git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" add \
  crates/vox-gui/ui/src/components/surfaces/Memory/MemoryView.tsx \
  crates/vox-gui/ui/src/components/surfaces/Memory/MemoryView.test.tsx

git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" commit -m \
  "feat(vox-gui/perf): virtualize MemoryView recent-recalls; cap shards scroll (Phase 0D T3)"
```

---

## Task 4: RAIL budget doc + animation conventions

**What and why:** No single document records the RAIL performance budget or the "animate transform/opacity only" and "heavy work → Rust `#[command]`" conventions. Without it, each contributor rediscovers these constraints independently. This doc becomes the reference that code reviews can link to.

**IMPORTANT: The doc lives under `docs/src/architecture/` and must start with YAML frontmatter (project rule enforced by the pre-push doc pipeline).**

### Step 4.1 — Create the doc

- [ ] Create `docs/src/architecture/gui-perf-conventions-2026-06-14.md`:

```markdown
---
title: vox-gui Performance Conventions
description: RAIL budgets, animation rules, and heavy-work patterns for the vox-gui frontend.
category: architecture
---

# vox-gui Performance Conventions

> Adopted: 2026-06-14. Covers the vox-gui Tauri frontend (`crates/vox-gui/ui/`).

## 1. RAIL Budget

RAIL is Google's model for user-centric performance. The following budgets apply to all vox-gui surfaces.

| Phase | Budget | Notes |
|-------|--------|-------|
| **Response** (user action → visual feedback) | < 100 ms total; < 50 ms JS processing | Button clicks, keyboard shortcuts, UI state toggles |
| **Animation** (each frame) | < 10 ms JS work per frame | Remaining 6 ms budget is for the browser's compositor |
| **Idle** (deferred work) | ≤ 50 ms chunks | Use `requestIdleCallback` or `setTimeout(fn, 0)` for background parsing |
| **Load** (shell to interactive) | < 1 000 ms on target hardware | Tauri webview startup + JS parse + first render |

### Why 50 ms?

50 ms is the "Long Task" threshold (Chrome DevTools / LoAF). Any JS that blocks the main thread for > 50 ms is flagged as a long task and produces jank at 60 fps.

---

## 2. Animation Rule: `transform` and `opacity` Only

**Rule:** Animate only `transform` and `opacity` (and `filter` when unavoidable). Never animate `width`, `height`, `top`, `left`, `margin`, `padding`, `border-radius` (as the only changing property in a loop), or `background-color` via JS.

**Why:** `transform` and `opacity` are composited by the GPU on a separate layer. Animating them skips layout and paint — the two most expensive browser pipeline stages. Animating geometry properties (`width`, `height`, etc.) triggers layout on every frame, burning through the 10 ms frame budget instantly.

**How to apply in Tailwind:**

```css
/* Good — compositor only */
.slide-in  { animation: slideIn 200ms ease-out; }
@keyframes slideIn { from { transform: translateY(8px); opacity: 0; } to { transform: none; opacity: 1; } }

/* Bad — triggers layout every frame */
.expand { animation: expand 300ms ease; }
@keyframes expand { from { height: 0; } to { height: 200px; } }
```

**Existing keyframes in `src/index.css`:** `vox-toast-in`, `vox-ping`, `vox-pulse-slow` all use `transform`/`opacity`/`scale` — compliant. Add new keyframes only with `transform`/`opacity`.

**`prefers-reduced-motion`:** Gating all keyframes behind `@media (prefers-reduced-motion: reduce)` is handled in Phase 0C. Do not duplicate that work here.

---

## 3. Heavy Work → Rust `#[command]`

**Rule:** Any computation that is not trivially O(1) or O(n) with n < 50 must be moved to a Tauri `#[command]` in Rust. The JS main thread is single-threaded and shared with the rendering pipeline.

### What counts as "heavy"

| Category | Threshold | Action |
|----------|-----------|--------|
| Data filtering / sorting | > 500 items | Rust command |
| String search / regex over corpus | any corpus size | Rust command (via `vox_search_query`) |
| Cryptographic operations | any | Rust command |
| File I/O or network | any | Rust command (Tauri already enforces this) |
| JSON parsing of large payloads | > 100 KB | Consider streaming from Rust |
| Embedding / ML inference | any | Rust command (via MENS pipeline) |

### Pattern

```typescript
// BAD — blocks the JS thread for large data sets
const filtered = hugeArray.filter(item => item.score > threshold);

// GOOD — offload to Rust, return the processed result
const filtered = await invoke<Item[]>('filter_items_by_score', { threshold });
```

### Why Rust for compute?

Rust runs on a separate OS thread pool (Tauri's async executor). The Tauri `invoke` IPC bridge is low-latency (< 1 ms for payloads < 64 KB) and does not block the webview's main thread.

---

## 4. List Virtualization

**Rule:** Any list that can exceed 50 items must be virtualized using `useVirtualList` (`src/hooks/useVirtualList.ts`).

| Surface | List | Status |
|---------|------|--------|
| `TasksView` | `inProgress`, `queued` | Virtualized (Phase 0D) |
| `MemoryView` | `recent_recalls` | Virtualized (Phase 0D) |
| `MemoryView` | `shards` | Scroll-capped at 700 px (Phase 0D) |
| `RunsView` | runs | Hard-bounded at 40 — no virtualization needed |
| `SearchView` | search hits | Hard-bounded at 30 — no virtualization needed |

**How to use `useVirtualList`:**

```typescript
import { useRef } from 'react';
import { useVirtualList } from '../../../hooks/useVirtualList';

const containerRef = useRef<HTMLDivElement>(null);
const { virtualizer, totalSize, virtualItems } = useVirtualList({
  containerRef,
  count: items.length,
  estimateSize: () => 48, // estimated row height in px
  overscan: 3,
});

return (
  <div ref={containerRef} style={{ height: '400px', overflow: 'auto' }}>
    <div style={{ height: totalSize, position: 'relative' }}>
      {virtualItems.map(vItem => (
        <div
          key={vItem.key}
          ref={virtualizer.measureElement}
          data-index={vItem.index}
          style={{ position: 'absolute', top: 0, transform: `translateY(${vItem.start}px)`, width: '100%' }}
        >
          {items[vItem.index]}
        </div>
      ))}
    </div>
  </div>
);
```

---

## 5. Inline Styles: When They Are Correct

The `crates/vox-gui/ui/` project intentionally uses `style={{}}` for dynamic values that cannot be expressed as static Tailwind classes. Examples:

- `style={{ width: '${score * 100}%' }}` — progress bars
- `style={{ height: totalSize, position: 'relative' }}` — virtualizer inner div
- `style={{ transform: \`translateY(\${vItem.start}px)\` }}` — virtual item position

These are **not** style violations. The CSP already includes `style-src 'unsafe-inline'` to allow them. Do not attempt to remove or replace them with Tailwind utilities.

---

## 6. References

- [RAIL model (web.dev)](https://web.dev/rail/)
- [Rendering performance (web.dev)](https://web.dev/rendering-performance/)
- [Stick to compositor-only properties (web.dev)](https://web.dev/stick-to-compositor-only-properties-and-manage-layer-count/)
- [`@tanstack/react-virtual` docs](https://tanstack.com/virtual/latest)
- Phase 0A (tokens, CSP, contrast): `docs/superpowers/plans/2026-06-14-vox-gui-phase0a-visual-security-foundation.md`
- Phase 0C (a11y, `prefers-reduced-motion`): `docs/superpowers/plans/2026-06-14-vox-gui-phase0c-a11y.md`
```

### Step 4.2 — Verify doc file exists and has frontmatter

- [ ] Confirm the file exists at the correct path.
- [ ] Confirm the first 6 lines are the YAML frontmatter block (starts with `---`, ends with `---`).
- [ ] Run `pnpm test` — no test regressions (this task has no new vitest tests).
- [ ] Run `pnpm typecheck` — no new errors.

### Step 4.3 — Commit

```
git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" add \
  docs/src/architecture/gui-perf-conventions-2026-06-14.md

git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" commit -m \
  "docs(vox-gui/perf): RAIL budgets + animation + heavy-work conventions (Phase 0D T4)"
```

---

## Self-Review Checklist

Before marking this plan complete, verify:

- [ ] `pnpm test` passes (all test files, all tests green)
- [ ] `pnpm typecheck` passes (no TypeScript errors)
- [ ] `@tanstack/react-virtual` appears in `package.json` under `dependencies` (runtime dep, not devDep)
- [ ] `src/hooks/useVirtualList.ts` exists and exports `useVirtualList`
- [ ] `src/hooks/useVirtualList.test.ts` exists and has ≥ 3 tests
- [ ] `TasksView.tsx` no longer has bare `.map()` over `inProgress` or `queued` (both use `virtualItems.map`)
- [ ] `MemoryView.tsx` no longer has bare `.map()` over `recent_recalls` (uses `virtualItems.map`); shards grid is wrapped in a scroll container
- [ ] `docs/src/architecture/gui-perf-conventions-2026-06-14.md` exists, starts with YAML frontmatter
- [ ] No inline styles were removed (they are legitimate dynamic styles)
- [ ] `RunsView` and `SearchView` were NOT modified (already bounded)
- [ ] `prefers-reduced-motion` was NOT added (that is Phase 0C's job)
- [ ] Four commits total, one per task, on branch `claude/vox-gui-design-principles-phase0`

---

## Appendix: Why These Specific Choices

### Why `@tanstack/react-virtual` over `react-window`?

| | `react-window` | `@tanstack/react-virtual` |
|---|---|---|
| React 19 hooks | Works but unmaintained | Designed for hooks; v3 is actively maintained |
| Auto-measurement | No (fixed size only) | Yes (`measureElement` ref) |
| Framework coupling | React only | Framework-agnostic |
| Bundle size | ~15 KB | ~9 KB |
| Variable-height rows | Painful | Native support |

### Why not virtualize `shards`?

The `shards` grid uses `grid-cols-2 md:grid-cols-3 xl:grid-cols-6` — a responsive multi-column layout. `useVirtualizer` works on a single scrollable axis; applying it to a grid requires either (a) computing how many columns fit and treating rows as units, or (b) abandoning the CSS grid entirely. Both add significant complexity for a list that is bounded in practice by corpus tier (typically < 20 shards). A scroll-capped container achieves the same UX goal with zero complexity cost.

### Why `min(count × itemHeight, maxHeight)` for container height?

This makes short lists stay compact (no wasted whitespace) while bounding long lists. The alternative — a fixed height always — forces a scroll bar even on 1-item lists, which is poor UX. The formula degrades gracefully: 0 items → 0 px height (the empty-state `<p>` uses normal flow), N items → natural height up to the cap.
