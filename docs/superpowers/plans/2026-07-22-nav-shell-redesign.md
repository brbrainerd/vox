# Navigation Shell Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Tasks 1, 2, 3, and 4 are mutually independent (see "Execution structure" below) — REQUIRED SUB-SKILL for that wave specifically: superpowers:dispatching-parallel-agents. Each task below that has a "Write the failing test" step is following superpowers:test-driven-development; Task 5 explicitly does not (see its own note) — this is a deliberate, disclosed deviation, not an oversight.

**Goal:** Replace the app's two overlapping navigation surfaces (left `Sidebar` + top `WorkbenchTabBar`) with a single expand-in-place sidebar tree, and move the separate "open a doc" use case out of the tab bar into its own small slide-over drawer.

**Architecture:** `useWorkbenchTabs` (which currently conflates "which surface view is active" with "which surfaces/docs are open as tabs") is split into two independent, single-purpose hooks: `useActiveView` (one active view key, URL-hash-synced, migrated forward from the old tab-list storage key, no open-tab list) and `useDocViewer` (one open doc at a time, drawer-style, decoupled from `DocReader`'s current `useWorkbenchTabs` dependency). `Sidebar.tsx` renders each top-level parent's children inline, indented, only for the parent containing the current active view OR the last parent expanded via a dedicated peek chevron (accordion — at most one parent's children visible at a time), and only in `wide` sidebar mode. `WorkbenchTabBar.tsx` and the `tabBar` prop threaded through `AppShell.tsx` are deleted entirely; `BreadcrumbBar` (already implemented, already wired into `AppShell.tsx` between `TopHud` and `StatusBar`) continues to provide `Parent › Child` orientation with no changes needed.

**Tech Stack:** React + TypeScript, Tailwind, Vitest + Testing Library, existing `useLocalStorage` hook for persistence.

**Execution structure:** Tasks 1 (`useActiveView`), 2 (`useDocViewer`), 3 (`DocViewerDrawer` + `DocReader` decoupling), and 4 (`Sidebar` accordion) touch entirely disjoint files and share no state — dispatch these four as parallel subagents (superpowers:dispatching-parallel-agents), not sequentially. Task 5 (`App.tsx` wiring) requires Tasks 1, 2, and 4 to exist first (it imports from all three) and must start only after all of wave 1 lands. Task 6 requires Task 5. Task 7 requires Tasks 5 and 6. Task 8 requires everything. This cuts the plan's critical path from 8 sequential tasks to effectively 5 (wave 1, in parallel → 5 → 6 → 7 → 8).

---

### Task 0: Read current state of touched files before starting (dependency note)

**Do this first, every time you start or resume this plan:** a separate, independently-dispatched agent fixed a live "sidebar cut off at bottom" bug in `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` and `crates/vox-gui/ui/src/components/layout/AppShell.tsx` in this same worktree, landed as commit `447286392d` (`Sidebar.tsx`'s `<aside>`: `h-screen` → `h-full`; `AppShell.tsx`'s root div: `h-full` → `flex-1 min-h-0`). This plan's Task 4 and Task 6 snippets are written against the *post*-`447286392d` state of those two files. Before editing either file, run `git log --oneline -5 -- <file>` and `git status` to confirm you're building on top of `447286392d` (or later) and not an older or newer state this plan hasn't seen — if the file has moved further since this plan was written, read it fresh and adapt rather than blindly pasting this plan's snippets.

No commit for this task — it's a standing instruction, not a code change.

---

### Task 1: `useActiveView` hook — single active view, no open-tab list, migrated from the old tab storage

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useActiveView.ts`
- Test: `crates/vox-gui/ui/src/hooks/useActiveView.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// crates/vox-gui/ui/src/hooks/useActiveView.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useActiveView } from './useActiveView';

describe('useActiveView', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('defaults to dashboard when nothing is stored', () => {
    const { result } = renderHook(() => useActiveView());
    expect(result.current.activeView).toBe('dashboard');
  });

  it('navigateTo resolves a parent key to its default child and stores it', () => {
    const { result } = renderHook(() => useActiveView());
    act(() => {
      result.current.navigateTo('agents');
    });
    // 'agents' parent's default child per DEFAULT_CHILD_BY_PARENT in navigation.ts
    expect(result.current.activeView).not.toBe('agents');
    expect(result.current.activeView).toBe('dashboard');
  });

  it('navigateTo a leaf child view navigates directly to it', () => {
    const { result } = renderHook(() => useActiveView());
    act(() => {
      result.current.navigateTo('flow');
    });
    expect(result.current.activeView).toBe('flow');
  });

  it('persists the active view across remounts via localStorage', () => {
    const { result, unmount } = renderHook(() => useActiveView());
    act(() => {
      result.current.navigateTo('console');
    });
    unmount();
    const { result: result2 } = renderHook(() => useActiveView());
    expect(result2.current.activeView).toBe('console');
  });

  it('migrates forward from the old vox_workbench_tabs.v1 activeTab on first read', () => {
    localStorage.setItem(
      'vox_workbench_tabs.v1',
      JSON.stringify({ openTabs: ['chat', 'repository'], activeTab: 'repository' }),
    );
    const { result } = renderHook(() => useActiveView());
    expect(result.current.activeView).toBe('repository');
  });

  it('ignores the old key once its own key has ever been written', () => {
    localStorage.setItem(
      'vox_workbench_tabs.v1',
      JSON.stringify({ openTabs: ['chat', 'repository'], activeTab: 'repository' }),
    );
    const { result, unmount } = renderHook(() => useActiveView());
    act(() => {
      result.current.navigateTo('models'); // writes its own key
    });
    unmount();
    // Change the old key after migration already happened once — should have no further effect.
    localStorage.setItem(
      'vox_workbench_tabs.v1',
      JSON.stringify({ openTabs: ['chat', 'settings'], activeTab: 'settings' }),
    );
    const { result: result2 } = renderHook(() => useActiveView());
    expect(result2.current.activeView).toBe('models');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/hooks/useActiveView.test.ts`
Expected: FAIL with `Cannot find module './useActiveView'`.

- [ ] **Step 3: Write minimal implementation**

```ts
// crates/vox-gui/ui/src/hooks/useActiveView.ts
import { useCallback } from 'react';
import { useLocalStorage } from './useLocalStorage';
import { DEFAULT_CHILD_BY_PARENT, LEGACY_VIEW_ALIASES, resolveNavigation } from '../lib/navigation';

const STORAGE_KEY = 'vox_active_view.v2';
const LEGACY_TABS_KEY = 'vox_workbench_tabs.v1';
const FALLBACK_VIEW = 'dashboard';

function normalizeViewKey(key: string): string {
  return LEGACY_VIEW_ALIASES[key] ?? key;
}

/**
 * One-time, one-directional migration from the old open-tabs storage shape.
 * Read only as the initial value for useLocalStorage — once the new key has
 * been written even once (including this migration's own first write), the
 * old key is never consulted again.
 */
function migrateFromLegacyTabs(): string {
  try {
    const raw = localStorage.getItem(LEGACY_TABS_KEY);
    if (!raw) return FALLBACK_VIEW;
    const parsed = JSON.parse(raw) as { activeTab?: string | null };
    if (parsed.activeTab && !parsed.activeTab.startsWith('doc:')) {
      return normalizeViewKey(parsed.activeTab);
    }
  } catch {
    // fall through to default
  }
  return FALLBACK_VIEW;
}

/**
 * Single-view navigation state: exactly one active view key, persisted and
 * URL-hash-syncable. Replaces the "open tabs" half of the old useWorkbenchTabs
 * hook — there is no list of open views, just the current one. Navigating to a
 * parent key resolves to that parent's default child (same behavior the old
 * hook's openParent provided); navigating to a child/leaf key goes there directly.
 */
export function useActiveView() {
  const [activeView, setActiveView] = useLocalStorage<string>(STORAGE_KEY, migrateFromLegacyTabs());

  const navigateTo = useCallback(
    (viewKey: string) => {
      const key = normalizeViewKey(viewKey);
      const { child } = resolveNavigation(key);
      setActiveView(child);
    },
    [setActiveView],
  );

  const navigateToParent = useCallback(
    (parentKey: string) => {
      const child = DEFAULT_CHILD_BY_PARENT[parentKey] ?? parentKey;
      navigateTo(child);
    },
    [navigateTo],
  );

  return { activeView, navigateTo, navigateToParent };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/hooks/useActiveView.test.ts`
Expected: PASS (6/6).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/hooks/useActiveView.ts crates/vox-gui/ui/src/hooks/useActiveView.test.ts
git commit -m "feat(gui): add useActiveView hook — single active view, migrated from old tab storage"
```

---

### Task 2: `useDocViewer` hook — one open doc at a time, drawer-style

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useDocViewer.ts`
- Test: `crates/vox-gui/ui/src/hooks/useDocViewer.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// crates/vox-gui/ui/src/hooks/useDocViewer.test.ts
import { describe, it, expect } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useDocViewer } from './useDocViewer';

describe('useDocViewer', () => {
  it('starts closed with no active doc', () => {
    const { result } = renderHook(() => useDocViewer());
    expect(result.current.activeDoc).toBeNull();
  });

  it('openDoc opens the drawer with the given path and optional title', () => {
    const { result } = renderHook(() => useDocViewer());
    act(() => {
      result.current.openDoc('docs/foo.md', 'Foo');
    });
    expect(result.current.activeDoc).toEqual({ path: 'docs/foo.md', title: 'Foo' });
  });

  it('opening a second doc while one is open replaces it, not stacks', () => {
    const { result } = renderHook(() => useDocViewer());
    act(() => {
      result.current.openDoc('docs/foo.md', 'Foo');
      result.current.openDoc('docs/bar.md', 'Bar');
    });
    expect(result.current.activeDoc).toEqual({ path: 'docs/bar.md', title: 'Bar' });
  });

  it('closeDoc clears the active doc', () => {
    const { result } = renderHook(() => useDocViewer());
    act(() => {
      result.current.openDoc('docs/foo.md');
      result.current.closeDoc();
    });
    expect(result.current.activeDoc).toBeNull();
  });

  it('openDoc without a title falls back to the filename', () => {
    const { result } = renderHook(() => useDocViewer());
    act(() => {
      result.current.openDoc('docs/some-guide.md');
    });
    expect(result.current.activeDoc?.title).toBe('some-guide');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/hooks/useDocViewer.test.ts`
Expected: FAIL with `Cannot find module './useDocViewer'`.

- [ ] **Step 3: Write minimal implementation**

```ts
// crates/vox-gui/ui/src/hooks/useDocViewer.ts
import { useCallback, useState } from 'react';

export interface ActiveDoc {
  path: string;
  title: string;
}

function titleFromPath(path: string): string {
  return path.split('/').pop()?.replace(/\.md$/i, '') ?? path;
}

/**
 * Doc-viewer state: at most one open doc, presented as a drawer (see
 * DocViewerDrawer.tsx). Deliberately NOT persisted across reloads (unlike
 * useActiveView) — a doc reference is a transient "I clicked a link" action,
 * not a navigation destination worth restoring on next launch. No-stacking is
 * safe today because DocReader renders raw text with no clickable in-doc
 * links (confirmed by reading DocReader.tsx while writing this plan) —
 * revisit if DocReader ever gains markdown link rendering.
 */
export function useDocViewer() {
  const [activeDoc, setActiveDoc] = useState<ActiveDoc | null>(null);

  const openDoc = useCallback((path: string, title?: string) => {
    setActiveDoc({ path, title: title ?? titleFromPath(path) });
  }, []);

  const closeDoc = useCallback(() => {
    setActiveDoc(null);
  }, []);

  return { activeDoc, openDoc, closeDoc };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/hooks/useDocViewer.test.ts`
Expected: PASS (5/5).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/hooks/useDocViewer.ts crates/vox-gui/ui/src/hooks/useDocViewer.test.ts
git commit -m "feat(gui): add useDocViewer hook — single-doc drawer state"
```

---

### Task 3: `DocViewerDrawer` component, and decouple `DocReader` from the doomed `useWorkbenchTabs` hook

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/DocViewerDrawer.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/DocViewerDrawer.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/DocReader/DocReader.tsx`
- Modify (if it exists): `crates/vox-gui/ui/src/components/surfaces/DocReader/DocReader.test.tsx`

This component is modeled directly on the existing `crates/vox-gui/ui/src/components/gamify/AchievementsDrawer.tsx` (right-edge slide-over: `fixed inset-0 z-50 flex justify-end` scrim + `Glass` panel, Escape-to-close) — read that file first if anything below is unclear, and keep the same structural pattern rather than inventing a new one.

**Why this task also touches `DocReader.tsx`:** `DocReader.tsx` currently imports `docPathFromTab` from `crates/vox-gui/ui/src/hooks/useWorkbenchTabs.ts` (the hook Task 7 deletes) and takes a `tabId: string` prop shaped like `doc:${path}`, reconstructing the real path via `docPathFromTab(tabId)` internally. Since `useDocViewer` (Task 2) already stores the path unprefixed, threading it back through a `doc:`-prefix reconstruction just to immediately undo it is unnecessary indirection that also leaves a live import of a file scheduled for deletion. Fix it at the source: change `DocReader`'s prop from `tabId: string` to `path: string`, remove its `docPathFromTab`/`useWorkbenchTabs` import, and use `path` directly wherever it previously called `docPathFromTab(tabId)`. Read the current `DocReader.tsx` in full before editing — its query/rendering logic (`<pre>{q.data}</pre>` per this plan's own earlier investigation) is unaffected by this change; only the prop-to-path resolution changes.

- [ ] **Step 1: Write the failing test for `DocViewerDrawer`**

```tsx
// crates/vox-gui/ui/src/components/layout/DocViewerDrawer.test.tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DocViewerDrawer } from './DocViewerDrawer';

vi.mock('../surfaces/DocReader/DocReader', () => ({
  DocReader: ({ path }: { path: string }) => <div data-testid="doc-reader-stub">{path}</div>,
}));

describe('DocViewerDrawer', () => {
  it('renders nothing when doc is null', () => {
    const { container } = render(
      <DocViewerDrawer doc={null} onClose={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the doc title, a close button, and passes path to DocReader', () => {
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={vi.fn()} />,
    );
    expect(screen.getByText('Foo Guide')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /close doc/i })).toBeInTheDocument();
    expect(screen.getByTestId('doc-reader-stub')).toHaveTextContent('docs/foo.md');
  });

  it('calls onClose when the close button is clicked', () => {
    const onClose = vi.fn();
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={onClose} />,
    );
    fireEvent.click(screen.getByRole('button', { name: /close doc/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when Escape is pressed', () => {
    const onClose = vi.fn();
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={onClose} />,
    );
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when the scrim is clicked', () => {
    const onClose = vi.fn();
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={onClose} />,
    );
    fireEvent.click(screen.getByRole('button', { name: /close doc overlay/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/DocViewerDrawer.test.tsx`
Expected: FAIL with `Cannot find module './DocViewerDrawer'`.

- [ ] **Step 3: Update `DocReader.tsx` to take `path` instead of `tabId`**

Read `crates/vox-gui/ui/src/components/surfaces/DocReader/DocReader.tsx` in full. Change its props interface from `{ tabId: string }` to `{ path: string }`; remove the `import { docPathFromTab } from '../../../hooks/useWorkbenchTabs'` line (or wherever that import currently lives — confirm the real relative path by reading the file, don't guess); replace every internal use of `docPathFromTab(tabId)` with the `path` prop directly. If `DocReader.test.tsx` exists, update its render calls from `tabId="doc:docs/foo.md"` to `path="docs/foo.md"`.

- [ ] **Step 4: Write `DocViewerDrawer`**

```tsx
// crates/vox-gui/ui/src/components/layout/DocViewerDrawer.tsx
import React, { useEffect } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { DocReader } from '../surfaces/DocReader/DocReader';
import type { ActiveDoc } from '../../hooks/useDocViewer';

export interface DocViewerDrawerProps {
  doc: ActiveDoc | null;
  onClose: () => void;
}

export function DocViewerDrawer({ doc, onClose }: DocViewerDrawerProps) {
  useEffect(() => {
    if (!doc) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [doc, onClose]);

  if (!doc) return null;

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <button
        type="button"
        aria-label="Close doc overlay"
        className="flex-1 bg-black/50"
        onClick={onClose}
      />
      <Glass
        role="dialog"
        aria-label={doc.title}
        aria-modal="true"
        className="flex h-full w-full max-w-2xl flex-col rounded-none border-l border-border-subtle shadow-2xl"
        inset={false}
      >
        <div className="flex items-center justify-between border-b border-border-subtle px-4 py-3">
          <span className="font-display text-[13px] uppercase tracking-[0.14em] text-text-primary">
            {doc.title}
          </span>
          <button
            type="button"
            aria-label="Close doc"
            onClick={onClose}
            className="flex size-7 items-center justify-center rounded-md text-text-muted hover:bg-overlay-hover hover:text-text-primary"
          >
            <Icon.x className="size-4" aria-hidden="true" />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          <DocReader path={doc.path} />
        </div>
      </Glass>
    </div>
  );
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/DocViewerDrawer.test.tsx src/components/surfaces/DocReader`
Expected: PASS on both — `DocViewerDrawer.test.tsx` (5/5) and any existing `DocReader` tests (updated in Step 3, still passing).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/DocViewerDrawer.tsx crates/vox-gui/ui/src/components/layout/DocViewerDrawer.test.tsx crates/vox-gui/ui/src/components/surfaces/DocReader/DocReader.tsx
git commit -m "feat(gui): add DocViewerDrawer; decouple DocReader from useWorkbenchTabs (path prop, not tabId)"
```

---

### Task 4: Sidebar accordion — render children under the active/peeked parent, `wide` mode only

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx`

**Before starting**, re-read Task 0's note and confirm you're building on top of commit `447286392d` (the sidebar-cut-off fix: `<aside>` now uses `h-full`, not `h-screen`).

Current relevant structure (`Sidebar.tsx`, post-`447286392d`, as read while writing this plan — re-confirm against the live file, line numbers may shift): the `<nav aria-label="Primary navigation">` block (around lines 162-191) maps `visibleTopLevel` (the 9 top-level keys minus `settings`) to `NavItem` components, each calling `onOpenParent(key)` on click, with no child rendering at all today. `SidebarMode` is `'rail' | 'default' | 'wide'` (line 14); `collapsed` (line 99) is `mode === 'rail'` specifically — `default` mode is not collapsed but also has no room for a child tree (212px wide, per `SIDEBAR_WIDTHS`).

**Read `Sidebar.test.tsx` in full before writing the new test below** — it uses a `renderSidebar()` helper that wraps `<Sidebar>` in a `<LanguageProvider>` (required, since `Sidebar` calls `useLang()` internally) with a real inline `data` object (there is no `mockDashboardData` fixture anywhere in the codebase — do not reference one). Match that file's existing helper/mock conventions exactly rather than the illustrative sketch below if they differ.

- [ ] **Step 1: Write the failing test**

```tsx
// Add to crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx
// Adapt to the file's real renderSidebar()/baseProps helper — this sketch
// assumes one exists per the file's own established pattern; if the helper
// is named differently, use the real name, do not invent `mockDashboardData`.

it('expands the parent containing the active view in wide mode', () => {
  renderSidebar({ view: 'flow', mode: 'wide' });
  // 'flow' resolves to parent 'agents' (per PARENT_CHILD_MAP) — its children
  // (dashboard, flow, tasks, mesh, sub-agents) should be visible.
  expect(screen.getByRole('button', { name: /^flow$/i })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: /^tasks$/i })).toBeInTheDocument();
  // A different parent's children ('runs' → approvals/needs-you/policies) should NOT be rendered.
  expect(screen.queryByRole('button', { name: /^policies$/i })).not.toBeInTheDocument();
});

it('does not render a child tree in rail mode', () => {
  renderSidebar({ view: 'flow', mode: 'rail' });
  expect(screen.queryByRole('button', { name: /^tasks$/i })).not.toBeInTheDocument();
});

it('does not render a child tree in default mode either', () => {
  renderSidebar({ view: 'flow', mode: 'default' });
  expect(screen.queryByRole('button', { name: /^tasks$/i })).not.toBeInTheDocument();
});

it('clicking a child calls onOpenTab with that child key', () => {
  const onOpenTab = vi.fn();
  renderSidebar({ view: 'flow', mode: 'wide', onOpenTab });
  fireEvent.click(screen.getByRole('button', { name: /^tasks$/i }));
  expect(onOpenTab).toHaveBeenCalledWith('tasks');
});

it('the peek chevron expands a parent without navigating (does not call onOpenParent/onOpenTab)', () => {
  const onOpenParent = vi.fn();
  const onOpenTab = vi.fn();
  renderSidebar({ view: 'flow', mode: 'wide', onOpenParent, onOpenTab });
  // 'knowledge' is a different parent from the active one ('agents', via 'flow').
  fireEvent.click(screen.getByRole('button', { name: /expand knowledge/i }));
  expect(onOpenParent).not.toHaveBeenCalled();
  expect(onOpenTab).not.toHaveBeenCalled();
  // Its children are now visible even though activeView is still 'flow'.
  expect(screen.getByRole('button', { name: /^memory$/i })).toBeInTheDocument();
  // Expanding a new parent via chevron collapses the previously-active one.
  expect(screen.queryByRole('button', { name: /^tasks$/i })).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/Sidebar.test.tsx`
Expected: FAIL — no child buttons or chevrons rendered today.

- [ ] **Step 3: Implement the accordion with a peek chevron**

Add this import to `Sidebar.tsx`:

```tsx
import { TOP_LEVEL_VIEWS, resolveNavigation, CHILD_ORDER_BY_PARENT, labelForNavKey } from '../../lib/navigation';
```

Add local state for the peeked (expanded-without-navigating) parent, and a `useEffect` to reset it whenever the real active parent changes (so navigating away always collapses any manually-peeked group back to following the active view):

```tsx
const [peekedParent, setPeekedParent] = useState<string | null>(null);

useEffect(() => {
  setPeekedParent(null);
}, [activeParent]);

const expandedParent = peekedParent ?? activeParent;
```

Replace the `NavItem` mapping inside the `<nav>` block (the `visibleTopLevel.map(key => { ... })` block) so that, immediately after each parent `NavItem`, it conditionally renders a chevron (for parents with children) and that parent's children when `key === expandedParent`, `mode === 'wide'`:

```tsx
{visibleTopLevel.map(key => {
  const label = navLabelFor(key, lang);
  const IconCmp = (Icon as Record<string, any>)[TOP_NAV_ICON[key] ?? 'file'] ?? Icon.file;
  const isActive = activeParent === key;
  const isExpanded = expandedParent === key && mode === 'wide';
  const children = CHILD_ORDER_BY_PARENT[key];
  const badge =
    key === 'agents' ? agentsCount
    : key === 'runs' && needsYouCount != null && needsYouCount > 0 ? needsYouCount
    : undefined;
  const navAriaLabel =
    key === 'runs'
      ? needsYouCount != null && needsYouCount > 0
        ? `Review, ${needsYouCount} items need you`
        : 'Review'
      : undefined;
  return (
    <React.Fragment key={key}>
      <div className="flex items-center gap-0.5">
        <div className="flex-1 min-w-0">
          <NavItem
            innerRef={isActive ? activeRef : undefined}
            collapsed={collapsed}
            active={isActive}
            onClick={() => onOpenParent(key)}
            icon={<IconCmp className="size-4" />}
            label={label}
            badge={badge}
            ariaLabel={navAriaLabel}
          />
        </div>
        {children && mode === 'wide' && (
          <button
            type="button"
            aria-label={`${isExpanded ? 'Collapse' : 'Expand'} ${label}`}
            aria-expanded={isExpanded}
            onClick={() => setPeekedParent(isExpanded ? null : key)}
            className="flex size-6 shrink-0 items-center justify-center rounded-md text-text-muted hover:bg-overlay-hover hover:text-text-primary"
          >
            <Icon.chevR className={`size-3 transition-transform ${isExpanded ? 'rotate-90' : ''}`} aria-hidden="true" />
          </button>
        )}
      </div>
      {isExpanded && children && (
        <div className="ml-4 flex flex-col gap-0.5 border-l border-border-subtle pl-2">
          {children.map(childKey => (
            <button
              key={childKey}
              type="button"
              onClick={() => onOpenTab(childKey)}
              aria-current={view === childKey ? 'page' : undefined}
              className={`w-full rounded-lg px-2 py-1.5 text-left font-display text-[11px] tracking-[0.1em] uppercase transition ${
                view === childKey
                  ? 'bg-brass/10 text-brass'
                  : 'text-text-muted hover:bg-overlay-hover hover:text-text-secondary'
              }`}
            >
              {labelForNavKey(childKey)}
            </button>
          ))}
        </div>
      )}
    </React.Fragment>
  );
})}
```

This is additive to the existing `<nav>` block — the parent `NavItem`'s own rendering/props are unchanged, only wrapped with a chevron sibling and a conditional child list.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/Sidebar.test.tsx`
Expected: PASS, including all pre-existing tests in this file (run the whole file, not just the 5 new tests, to confirm no regression).

- [ ] **Step 5: Live-verify the accordion stays height-bounded**

This directly matters for the recently-fixed "sidebar cut off at bottom" bug (`447286392d`) — an accordion that only ever shows one parent's children (never all 9 groups' children at once) should not reproduce that bug class, but confirm live: launch `vox-gui.exe` with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` (rebuild `pnpm build` + `cargo build -p vox-gui` first if stale), navigate to a view whose parent has the most children (`knowledge`, 6 children per `CHILD_ORDER_BY_PARENT`), and confirm via CDP `getBoundingClientRect`/`scrollHeight` that the sidebar's `<nav>` doesn't overflow even at a short window height (500-600px) — screenshot the result. Also confirm the peek chevron itself doesn't push content off-screen when expanding a parent other than the active one (briefly two groups' worth of vertical space could be implied if not careful — confirm only one is ever expanded at a time, matching Step 3's `expandedParent` single-value state).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx
git commit -m "feat(gui): sidebar expands active/peeked parent's children in place (accordion + peek chevron), wide mode only"
```

---

### Task 5: Wire `App.tsx` to the new hooks, remove workbench-tab wiring

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx`
- Modify: `crates/vox-gui/ui/src/guards/surfaceRegistryEscape.test.ts`
- Test: whichever existing top-level integration test file already covers `App.tsx` (read it first to find it and match conventions)

**This task is verify-after, not test-first**, unlike Tasks 1-4: it's integration wiring across an existing 1358-line file with no new business logic of its own (every piece of logic being wired already has its own unit test from Tasks 1, 2, 4). Writing a synthetic "failing test" for a rename/wiring pass would be performative. Instead: make the changes, then run the full suite and fix every failure for real, including the one pre-existing test this task is known to break (Step 4 below) — do not treat "the suite went green" as sufficient without first identifying that specific known failure by name.

Requires Tasks 1, 2, and 4 to be complete and merged first (imports `useActiveView`, `useDocViewer`, and relies on `Sidebar`'s new props being stable).

- [ ] **Step 1: Replace the hook import and destructure**

Find (around line 11 and lines 227-229 as read while writing this plan — re-confirm against the live file):

```tsx
import { useWorkbenchTabs, isDocTab, docPathFromTab, isPinnedTab } from './hooks/useWorkbenchTabs';
// ...
const workbench = useWorkbenchTabs();
const { openTab, openDocTab, closeTab, openTabs, activeTab, activeViewKey, docLabels } = workbench;
// ...
const activeView = (activeViewKey ?? 'dashboard') as View;
```

Replace with:

```tsx
import { useActiveView } from './hooks/useActiveView';
import { useDocViewer } from './hooks/useDocViewer';
// ...
const { activeView, navigateTo: openTab, navigateToParent: openParentFromHook } = useActiveView();
const { activeDoc, openDoc: openDocTab, closeDoc: closeDocViewer } = useDocViewer();
```

**The `const activeView = (activeViewKey ?? 'dashboard') as View;` line must be deleted, not left in place** — `useActiveView` already returns `activeView` directly as a plain `string`; there is no `activeViewKey` anymore to derive it from. If `App.tsx` casts to a `View` type elsewhere, check whether that cast is still needed against the new `string`-typed `activeView` and adjust the type only if the compiler actually complains (`npx tsc --noEmit` in Step 5 will catch this either way — don't pre-guess).

(Naming note: kept `openTab`/`openDocTab` as local aliases for `navigateTo`/`openDoc` deliberately, since dozens of call sites below already call `openTab(...)`/`openDocTab(...)` with the same single-argument call shape — only the underlying hook moved, the call shape didn't change. Renaming every call site is unnecessary churn.)

- [ ] **Step 2: Update `openParentNav`**

Find (around lines 589-592):

```tsx
const openParentNav = useCallback((parentKey: string) => {
  const child = DEFAULT_CHILD_BY_PARENT[parentKey] ?? parentKey;
  navigateTo(child);
}, [navigateTo]);
```

Replace with a direct alias to the hook's own resolver (no behavior change — `useActiveView.navigateToParent` already does this exact thing):

```tsx
const openParentNav = openParentFromHook;
```

- [ ] **Step 3: Remove `openTabs`/`closeTab`/doc-tab references, replace every `activeView` read**

Search the whole file for `openTabs`, `closeTab`, `isDocTab`, `docPathFromTab`, `isPinnedTab`, `docLabels`, `activeTab`, and `activeView` (the last one because Step 1 removed its declaration — every remaining reference now needs to resolve to the hook's `activeView` directly, no rename needed since the new destructure already uses that exact name):

- The `workbenchTabBar` construction (around lines 1062-1082, building the `<WorkbenchTabBar>` props object) — delete this whole block; it's no longer used once `tabBar` is removed from `AppShell` (Task 6).
- `mainSurface` (around lines 1235-1237):
  ```tsx
  const mainSurface = isDocTab(activeTab ?? '')
    ? <DocReader tabId={activeTab!} />
    : renderSurfaceContent(activeView, surfaceProps);
  ```
  Replace with:
  ```tsx
  const mainSurface = renderSurfaceContent(activeView, surfaceProps);
  ```
  (Docs no longer render as the main surface at all — they're now exclusively in the `DocViewerDrawer` overlay, always layered on top of whatever surface is active underneath.)
- `surfaceKey`/`surfaceLabel` props passed to `<AppShell>` (around lines 1280-1281):
  ```tsx
  surfaceKey={activeTab ?? activeView}
  surfaceLabel={isDocTab(activeTab ?? '') ? 'Doc' : labelForNavKey(activeView)}
  ```
  Replace with:
  ```tsx
  surfaceKey={activeView}
  surfaceLabel={labelForNavKey(activeView)}
  ```
- `tabBar={workbenchTabBar}` prop on `<AppShell>` — delete this line entirely (Task 6 removes the prop from `AppShell` itself).
- `onOpenDoc={(path) => openDocTab(path)}` (on `<Omnibar>`, around line 1328) — unchanged in shape (still calls `openDocTab`, now backed by `useDocViewer`'s `openDoc`), no edit needed here beyond the Step 1 rename.
- The hash-sync `useEffect` (around lines 620-653, referencing `openTab` for `hashchange` handling) — keep the `openTab(...)` calls (unchanged call shape per Step 1), remove only the now-dead `[openTab]` dependency-array entries if they reference the old `workbench` object identity rather than the new stable-callback identity (check `useCallback`/`useEffect` dependency arrays for correctness after the hook swap — this is standard React-hooks hygiene, not new logic).
- Remove the now-dead `PINNED_TABS`/`isPinnedTab` import if nothing else in the file uses it after this pass (grep to confirm — Task-writing-time investigation found its only consumers were inside `useWorkbenchTabs.ts` itself plus one read for the tab-bar's `pinned` badge, both being deleted).

- [ ] **Step 4: Fix the `surfaceRegistryEscape.test.ts` guard, which this task intentionally breaks**

`crates/vox-gui/ui/src/guards/surfaceRegistryEscape.test.ts` contains a test (confirmed while writing this plan, around lines 54-63) that regex-scans `App.tsx` for the exact pattern `isDocTab\([^)]*\)\s*\?\s*<(\w+)` and asserts the result is `['DocReader']` — i.e. it exists specifically to track the `isDocTab(...) ? <DocReader .../> : ...` ternary this task's Step 3 deletes. Once that ternary is gone, the regex matches nothing and the assertion fails. Read the full test file, understand what broader invariant it's protecting (a "no surface bypasses the registry" guard, of which the `DocReader` ternary was one tracked exception), and update it to reflect that `DocReader` is no longer rendered as a special-cased main-surface exception at all (it's now exclusively inside `DocViewerDrawer`, outside `renderSurfaceContent`'s registry-driven path entirely) — this may mean deleting the specific assertion for this pattern, or updating its expected match list to `[]`, depending on how the rest of that guard file is structured. Do not leave this test broken or skip it.

- [ ] **Step 5: Add `<DocViewerDrawer>` to the render tree**

Find the closing of the top-level `<>` fragment (around lines 1327-1335, near `<Omnibar>` and `<Toasts>`), and add the drawer as a sibling, always rendered (it internally returns `null` when `activeDoc` is `null`):

```tsx
<DocViewerDrawer doc={activeDoc} onClose={closeDocViewer} />
```

Add the import near the other layout-component imports:

```tsx
import { DocViewerDrawer } from './components/layout/DocViewerDrawer';
```

- [ ] **Step 6: Run the full frontend test suite and fix any remaining reference**

Run: `cd crates/vox-gui/ui && npx vitest run`

This will surface any remaining `openTabs`/`closeTab`/`activeTab`/`isDocTab` reference this plan's grep missed (App.tsx is large and was read only in excerpts while writing this plan) — fix each compile/test failure by applying the same rename pattern from Steps 1-4. Do not skip or `.only()` failing tests to make this pass faster.

Run: `npx tsc --noEmit` and fix any type errors from the hook shape change the same way.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/guards/surfaceRegistryEscape.test.ts
git commit -m "feat(gui): wire App.tsx to useActiveView/useDocViewer, remove workbench-tab state"
```

---

### Task 6: Remove `tabBar` prop from `AppShell.tsx`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/AppShell.tsx`

Requires Task 5 complete (this task's Step 2 depends on `App.tsx` no longer passing `tabBar`).

- [ ] **Step 1: Remove the prop and its render slot**

Remove `tabBar?: React.ReactNode;` from `AppShellProps` (around line 44), remove `tabBar,` from the destructured props (around line 81), and remove only the `{tabBar}` line from the wrapper div — **keep `flex flex-col` on that wrapper's className as-is**. That class was added deliberately in an earlier fix this session (commit `c8b09935e4`) so `SurfaceScrollHost`'s `flex-1` child could correctly claim remaining space after its `tabBar` sibling; even with `tabBar` gone and `SurfaceScrollHost` now the wrapper's only child, a flex container with one `flex-1` child is harmless and correct — do not revert that class or touch `SurfaceScrollHost.tsx` in this task, only remove the `{tabBar}` line itself:

```tsx
<div className={`flex-1 min-h-0 flex flex-col overflow-hidden p-5 ${mainPaddingBottom}`}>
  <SurfaceErrorBoundary key={surfaceKey} surface={surfaceLabel}>
    <SurfaceScrollHost>{children}</SurfaceScrollHost>
  </SurfaceErrorBoundary>
</div>
```

- [ ] **Step 2: Confirm the `App.tsx` call site is already clean**

Task 5 Step 3 already removed the `tabBar={workbenchTabBar}` line from the `<AppShell>` call — verify with `grep -n "tabBar" crates/vox-gui/ui/src/App.tsx` and confirm no remaining reference. If one remains (Task 5 was executed by a different agent/session and missed it), remove it now.

- [ ] **Step 3: Run the full suite**

Run: `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`
Expected: PASS, clean typecheck.

- [ ] **Step 4: Live-verify no regression in the AppShell scroll fix**

Since this task touches the exact wrapper div two earlier fixes this session modified (`c8b09935e4` for the scroll-clipping fix, `447286392d` for the sidebar-cut-off fix's `AppShell.tsx` root-div change — a *different*, outer div than this task's wrapper, confirm they're genuinely distinct elements while you're in the file), re-run the scroll-clipping fix's own live verification: launch the real app, navigate to Settings (or any tab), force a short window height (500px), confirm `SurfaceScrollHost`'s viewport still shows `scrollHeight` reaching real overflowing content with `overflowY: auto` engaged — do not just trust that removing one line is safe, confirm it live.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/AppShell.tsx
git commit -m "fix(gui): remove tabBar prop/slot from AppShell now that WorkbenchTabBar is gone"
```

---

### Task 7: Delete `WorkbenchTabBar` and `useWorkbenchTabs`

**Files:**
- Delete: `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx`
- Delete: `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.test.tsx`
- Delete: `crates/vox-gui/ui/src/hooks/useWorkbenchTabs.ts`
- Delete: `crates/vox-gui/ui/src/hooks/useWorkbenchTabs.test.ts`

Requires Tasks 5 and 6 complete.

- [ ] **Step 1: Confirm nothing else imports these files**

Run:
```bash
cd crates/vox-gui/ui && grep -rn "WorkbenchTabBar\|useWorkbenchTabs" src --include=*.ts --include=*.tsx | grep -v "\.test\."
```
Expected: no output. Task 3 already removed `DocReader.tsx`'s import of `docPathFromTab` from this hook (the one real consumer this plan's earlier drafting missed on first pass) — if this grep still finds something, it means either Task 3 or Task 5 was skipped/incomplete; resolve it before deleting, do not delete a file something still imports.

Also check the `#workbench-tabbar-trailing-slot` DOM-id reference (in `StatusBar.tsx`, from this session's earlier "move Panels trigger into StatusBar" work) — that id/slot is **independent** of `WorkbenchTabBar.tsx` itself (it now lives entirely in `StatusBar.tsx`) and must NOT be deleted; only the `WorkbenchTabBar.tsx` component file and the old `useWorkbenchTabs.ts` hook are being removed here.

- [ ] **Step 2: Delete the files**

```bash
git rm crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx
git rm crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.test.tsx
git rm crates/vox-gui/ui/src/hooks/useWorkbenchTabs.ts
git rm crates/vox-gui/ui/src/hooks/useWorkbenchTabs.test.ts
```

- [ ] **Step 3: Run the full suite**

Run: `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`
Expected: PASS, clean typecheck, with 4 fewer test files than before this task but no drop in passing-test count beyond exactly those files' own tests.

- [ ] **Step 4: Commit**

```bash
git commit -m "chore(gui): delete WorkbenchTabBar and useWorkbenchTabs, superseded by useActiveView/useDocViewer"
```

---

### Task 8: Whole-effort live verification

**Files:** none (verification only). Requires all prior tasks complete.

- [ ] **Step 1: Full suite + typecheck one more time**

Run: `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`
Expected: PASS, clean, and record the exact test count for the final report.

- [ ] **Step 2: Live CDP verification of the whole redesign**

Rebuild (`pnpm build` then `cargo build -p vox-gui`), launch with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, and confirm via CDP + screenshots:

1. No top tab bar renders anywhere — only `TopHud`, `BreadcrumbBar`, `StatusBar` remain in the header.
2. Clicking a sidebar parent's label (with children, e.g. Knowledge) navigates to its default child AND expands its children; the breadcrumb updates to match.
3. Clicking a different parent's peek chevron (not its label) expands that parent's children WITHOUT navigating — the breadcrumb and active surface stay unchanged, only the sidebar's expanded group changes.
4. Clicking a different parent (label or chevron) collapses the previous parent's children and expands the new one (accordion, not multi-expand) — confirm at most one group's children are ever visible at once.
5. At a short window height (500-600px), the sidebar with the largest child group (Knowledge, 6 children) expanded does not get cut off / does not lose scroll access to its own bottom content (System/Settings section, identity footer) — this directly re-verifies both Task 4's Step 5 check and the underlying `447286392d` fix still hold once the whole app is wired together, not just the isolated component.
6. Opening a doc (via the Omnibar's existing `onOpenDoc` entry point) shows the `DocViewerDrawer` sliding in from the right without navigating away from the current surface underneath; closing it (✕, Escape, or scrim click) returns to that same surface unchanged.
7. Opening a second doc while one is open replaces the drawer's content rather than stacking.
8. Sidebar `rail` and `default` modes both still work: clicking a parent icon navigates to its default child without attempting to render an accordion tree or chevron in either mode (there's no room for one).
9. A browser profile with an old `vox_workbench_tabs.v1` localStorage entry (simulate by setting it manually via CDP before first load) lands on that entry's `activeTab` on first launch, not the hardcoded `dashboard` fallback — confirms Task 1's migration path works end-to-end, not just in the hook's own unit test.

Screenshot each of the above. If any check fails, fix the root cause (read the relevant task's file fresh, don't guess) before considering this plan complete.

- [ ] **Step 3: Final report**

No commit for this task (verification only) — summarize: final test count, typecheck status, and confirmation (with screenshot references) that all 9 live checks in Step 2 passed.

---

## Self-Review

**Spec coverage** — every section of the (adversarially-revised) `docs/superpowers/specs/2026-07-22-nav-shell-redesign-design.md` maps to a task: sidebar-as-sole-nav-surface + accordion + peek chevron → Task 4; tab bar removed → Tasks 6-7; breadcrumb → already implemented (`BreadcrumbBar.tsx`, confirmed via file read — no task needed, noted explicitly); doc-viewer separate affordance + DocReader decoupling → Tasks 2-3; localStorage migration → Task 1; mobile-forward note → informational only, no task (matches the spec's own scope boundary).

**Placeholder scan** — no remaining "TBD"/vague items. The one previously-open uncertainty (Task 3's `DocReader` import path) has been resolved to the real path (`../surfaces/DocReader/DocReader`) and the task now also fixes that component's prop shape rather than leaving a `tabId`/`docPathFromTab` bridge to a file scheduled for deletion.

**Type consistency** — `useActiveView`'s `navigateTo`/`navigateToParent` and `useDocViewer`'s `openDoc`/`closeDoc` naming is used consistently across Tasks 1, 2, 3, and 5. Task 5 explicitly deletes the old `activeView` derived-variable declaration rather than leaving a dangling reference to the removed `activeViewKey`, and explicitly renames every other `activeTab`/`activeView` read site rather than leaving a mix of old and new names.

**Adversarial-review corrections applied in this revision** (for traceability — both critique passes' findings are folded in above, not left as a separate addendum):
- Blocking: Task 5 now explicitly deletes the `const activeView = (activeViewKey ?? 'dashboard') as View;` line instead of only flagging it as a question.
- Blocking: Task 4's test snippet no longer references a nonexistent `mockDashboardData` fixture; instructs reading the real `Sidebar.test.tsx` helper conventions first.
- Blocking: Task 4's accordion now gates on `mode === 'wide'` explicitly (not `!collapsed`, which would incorrectly also fire in `default` mode), with a dedicated `default`-mode test.
- Blocking: Task 5 now has an explicit step (Step 4) naming and fixing the `surfaceRegistryEscape.test.ts` guard test it breaks, rather than relying on a generic "fix any remaining failure" catch-all.
- Needs-correction: Task 3's `DocReader` import path corrected to the real one, and upgraded to a full decoupling (path prop, not tabId) rather than a corrected-but-still-fragile guess.
- Design gap: added the peek-chevron affordance so clicking a parent to look at its children no longer forces an unwanted navigation away from the current surface.
- Design gap: added a real migration step (Task 1) reusing the existing `migrateLegacyView()` precedent, instead of silently starting fresh from a new storage key.
- Design gap: Task 5 now states explicitly that it's verify-after rather than test-first, instead of silently deviating from the plan's own TDD framing.
- Process improvement: added an explicit "Execution structure" section identifying Tasks 1/2/3/4 as parallelizable via superpowers:dispatching-parallel-agents, cutting the critical path from 8 sequential tasks to 5.
- Simplification: Task 6's former double-snippet ("here's a wrong one, here's why, here's the right one") replaced with a single correct snippet plus a direct constraint statement.
