# Navigation Shell Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the app's two overlapping navigation surfaces (left `Sidebar` + top `WorkbenchTabBar`) with a single expand-in-place sidebar tree, and move the separate "open a doc" use case out of the tab bar into its own small slide-over drawer.

**Architecture:** `useWorkbenchTabs` (which currently conflates "which surface view is active" with "which surfaces/docs are open as tabs") is split into two independent, single-purpose hooks: `useActiveView` (one active view key, URL-hash-synced, no open-tab list) and `useDocViewer` (one open doc at a time, drawer-style). `Sidebar.tsx` renders each top-level parent's children inline, indented, only for the parent containing the current active view (accordion, derived from state — no separate expand/collapse state needed). `WorkbenchTabBar.tsx` and the `tabBar` prop threaded through `AppShell.tsx` are deleted entirely; `BreadcrumbBar` (already implemented, already wired into `AppShell.tsx` between `TopHud` and `StatusBar`) continues to provide `Parent › Child` orientation with no changes needed.

**Tech Stack:** React + TypeScript, Tailwind, Vitest + Testing Library, existing `useLocalStorage` hook for persistence.

---

### Task 0: Read current state of touched files before starting (dependency note)

**Do this first, every time you start this plan (and again before Task 4 specifically):** a separate, independently-dispatched agent may be fixing a live "sidebar cut off at bottom" bug in `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` in this same worktree, concurrently with this plan's execution. Before editing `Sidebar.tsx` in Task 4, run `git log --oneline -5 -- crates/vox-gui/ui/src/components/layout/Sidebar.tsx` and `git status` to check for a recent commit or uncommitted change to that file that isn't reflected in this plan's code snippets. If you find one, read the current file fresh and adapt Task 4's changes to apply on top of it rather than reverting it — do not overwrite that fix.

No commit for this task — it's a standing instruction, not a code change.

---

### Task 1: `useActiveView` hook — single active view, no open-tab list

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
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/hooks/useActiveView.test.ts`
Expected: FAIL with `Cannot find module './useActiveView'` (or similar — the file doesn't exist yet).

- [ ] **Step 3: Write minimal implementation**

```ts
// crates/vox-gui/ui/src/hooks/useActiveView.ts
import { useCallback } from 'react';
import { useLocalStorage } from './useLocalStorage';
import { DEFAULT_CHILD_BY_PARENT, LEGACY_VIEW_ALIASES, resolveNavigation } from '../lib/navigation';

const STORAGE_KEY = 'vox_active_view.v2';
const FALLBACK_VIEW = 'dashboard';

function normalizeViewKey(key: string): string {
  return LEGACY_VIEW_ALIASES[key] ?? key;
}

/**
 * Single-view navigation state: exactly one active view key, persisted and
 * URL-hash-syncable. Replaces the "open tabs" half of the old useWorkbenchTabs
 * hook — there is no list of open views, just the current one. Navigating to a
 * parent key resolves to that parent's default child (same behavior the old
 * hook's openParent provided); navigating to a child/leaf key goes there directly.
 */
export function useActiveView() {
  const [activeView, setActiveView] = useLocalStorage<string>(STORAGE_KEY, FALLBACK_VIEW);

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
Expected: PASS (4/4).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/hooks/useActiveView.ts crates/vox-gui/ui/src/hooks/useActiveView.test.ts
git commit -m "feat(gui): add useActiveView hook — single active view, no open-tab list"
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
 * not a navigation destination worth restoring on next launch.
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

### Task 3: `DocViewerDrawer` component

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/DocViewerDrawer.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/DocViewerDrawer.test.tsx`

This component is modeled directly on the existing `crates/vox-gui/ui/src/components/gamify/AchievementsDrawer.tsx` (right-edge slide-over: `fixed inset-0 z-50 flex justify-end` scrim + `Glass` panel, Escape-to-close) — read that file first if anything below is unclear, and keep the same structural pattern rather than inventing a new one.

- [ ] **Step 1: Write the failing test**

```tsx
// crates/vox-gui/ui/src/components/layout/DocViewerDrawer.test.tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DocViewerDrawer } from './DocViewerDrawer';

describe('DocViewerDrawer', () => {
  it('renders nothing when doc is null', () => {
    const { container } = render(
      <DocViewerDrawer doc={null} onClose={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the doc title and a close button when doc is set', () => {
    render(
      <DocViewerDrawer doc={{ path: 'docs/foo.md', title: 'Foo Guide' }} onClose={vi.fn()} />,
    );
    expect(screen.getByText('Foo Guide')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /close doc/i })).toBeInTheDocument();
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

- [ ] **Step 3: Write minimal implementation**

```tsx
// crates/vox-gui/ui/src/components/layout/DocViewerDrawer.tsx
import React, { useEffect } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { DocReader } from '../surfaces/Docs/DocReader';
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
          <DocReader tabId={`doc:${doc.path}`} />
        </div>
      </Glass>
    </div>
  );
}
```

Note: verify `DocReader`'s actual import path and its prop shape (`tabId`) by reading `crates/vox-gui/ui/src/App.tsx`'s existing `<DocReader tabId={activeTab!} />` usage and the component's own file before writing this — adjust the import path/prop name in the snippet above if it differs from what's assumed here (the exact path wasn't confirmed while writing this plan; grep `App.tsx` for `import.*DocReader` to get the real path).

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/DocViewerDrawer.test.tsx`
Expected: PASS (5/5). If `DocReader` requires additional context/providers not present in this test's render, mock it (`vi.mock('../surfaces/Docs/DocReader', ...)` with the real resolved path) rather than skipping the test.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/DocViewerDrawer.tsx crates/vox-gui/ui/src/components/layout/DocViewerDrawer.test.tsx
git commit -m "feat(gui): add DocViewerDrawer, modeled on AchievementsDrawer's slide-over pattern"
```

---

### Task 4: Sidebar accordion — render children under the active parent

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx`

**Before starting**, re-read Task 0's note: check `git log`/`git status` on this file for a concurrent bug fix and adapt accordingly.

Current relevant structure (`Sidebar.tsx`, as read while writing this plan — re-confirm line numbers against the live file, they may shift): the `<nav aria-label="Primary navigation">` block (around lines 162-191) maps `visibleTopLevel` (the 9 top-level keys minus `settings`) to `NavItem` components, each calling `onOpenParent(key)` on click, with no child rendering at all today.

- [ ] **Step 1: Write the failing test**

```tsx
// Add to crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx
// (read the existing file first to match its render-helper/prop-mocking conventions)

it('expands the active parent to show its children, collapses others', () => {
  render(
    <Sidebar
      view="flow"
      onOpenParent={vi.fn()}
      onOpenTab={vi.fn()}
      agentsCount={0}
      data={mockDashboardData}
      mode="wide"
      setMode={vi.fn()}
      pushToast={vi.fn()}
    />,
  );
  // 'flow' resolves to parent 'agents' (per PARENT_CHILD_MAP) — its children
  // (dashboard, flow, tasks, mesh, sub-agents) should be visible.
  expect(screen.getByRole('button', { name: /^flow$/i })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: /^tasks$/i })).toBeInTheDocument();
  // A different parent's children ('runs' → approvals/needs-you/policies) should NOT be rendered.
  expect(screen.queryByRole('button', { name: /^policies$/i })).not.toBeInTheDocument();
});

it('does not render a child tree in rail (collapsed) mode', () => {
  render(
    <Sidebar
      view="flow"
      onOpenParent={vi.fn()}
      onOpenTab={vi.fn()}
      agentsCount={0}
      data={mockDashboardData}
      mode="rail"
      setMode={vi.fn()}
      pushToast={vi.fn()}
    />,
  );
  expect(screen.queryByRole('button', { name: /^tasks$/i })).not.toBeInTheDocument();
});

it('clicking a child calls onOpenTab with that child key', () => {
  const onOpenTab = vi.fn();
  render(
    <Sidebar
      view="flow"
      onOpenParent={vi.fn()}
      onOpenTab={onOpenTab}
      agentsCount={0}
      data={mockDashboardData}
      mode="wide"
      setMode={vi.fn()}
      pushToast={vi.fn()}
    />,
  );
  fireEvent.click(screen.getByRole('button', { name: /^tasks$/i }));
  expect(onOpenTab).toHaveBeenCalledWith('tasks');
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/Sidebar.test.tsx`
Expected: FAIL — no child buttons rendered today.

- [ ] **Step 3: Implement the accordion**

Add this import to `Sidebar.tsx`:

```tsx
import { TOP_LEVEL_VIEWS, resolveNavigation, CHILD_ORDER_BY_PARENT, labelForNavKey } from '../../lib/navigation';
```

(`CHILD_ORDER_BY_PARENT` and `labelForNavKey` are new imports alongside the existing `TOP_LEVEL_VIEWS`/`resolveNavigation`.)

Replace the `NavItem` mapping inside the `<nav>` block (the `visibleTopLevel.map(key => { ... })` block) so that, immediately after each parent `NavItem`, it conditionally renders that parent's children when `isActive` and `!collapsed`:

```tsx
{visibleTopLevel.map(key => {
  const label = navLabelFor(key, lang);
  const IconCmp = (Icon as Record<string, any>)[TOP_NAV_ICON[key] ?? 'file'] ?? Icon.file;
  const isActive = activeParent === key;
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
  const children = CHILD_ORDER_BY_PARENT[key];
  return (
    <React.Fragment key={key}>
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
      {isActive && !collapsed && children && (
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

This is purely additive to the existing `<nav>` block — the parent `NavItem` rendering itself is unchanged, only wrapped in a `Fragment` with a conditional child list appended.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/Sidebar.test.tsx`
Expected: PASS, including all pre-existing tests in this file (run the whole file, not just the 3 new tests, to confirm no regression).

- [ ] **Step 5: Live-verify the accordion stays height-bounded**

This directly matters for the concurrently-fixed "sidebar cut off at bottom" bug — an accordion that only ever shows one parent's children (never all 9 groups' children at once) should not reproduce that bug class, but confirm live: launch `vox-gui.exe` with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` (rebuild `pnpm build` + `cargo build -p vox-gui` first if stale), navigate to a view whose parent has the most children (`knowledge`, 6 children per `CHILD_ORDER_BY_PARENT`), and confirm via CDP `getBoundingClientRect`/`scrollHeight` that the sidebar's `<nav>` doesn't overflow even at a short window height (500-600px) — screenshot the result.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx
git commit -m "feat(gui): sidebar expands the active parent's children in place (accordion)"
```

---

### Task 5: Wire `App.tsx` to the new hooks, remove workbench-tab wiring

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx`
- Test: `crates/vox-gui/ui/src/App.test.tsx` (or the closest existing top-level integration test file — read `App.tsx`'s existing test coverage first to find it)

This is the largest, most integration-heavy task — `App.tsx` is 1358 lines and threads `openTab`/`closeTab`/`openTabs`/`activeTab`/`activeViewKey`/`openDocTab`/`docLabels` through many call sites. Do this task in one sitting rather than splitting further, since partial completion would leave the file in a broken intermediate state.

- [ ] **Step 1: Replace the hook import and destructure**

Find (around line 11 and line 227-228 as read while writing this plan — re-confirm against the live file):

```tsx
import { useWorkbenchTabs, isDocTab, docPathFromTab, isPinnedTab } from './hooks/useWorkbenchTabs';
// ...
const workbench = useWorkbenchTabs();
const { openTab, openDocTab, closeTab, openTabs, activeTab, activeViewKey, docLabels } = workbench;
```

Replace with:

```tsx
import { useActiveView } from './hooks/useActiveView';
import { useDocViewer } from './hooks/useDocViewer';
// ...
const { activeView: activeViewFromHook, navigateTo: openTab, navigateToParent: openParentFromHook } = useActiveView();
const { activeDoc, openDoc: openDocTab, closeDoc: closeDocViewer } = useDocViewer();
```

(Naming note: kept `openTab` as the local variable name for `navigateTo` deliberately, since dozens of call sites below already call `openTab(...)` with a single view-key argument and that call shape is unchanged — only its implementation moved hooks. Renaming every call site is unnecessary churn; YAGNI. Same reasoning for keeping `activeViewFromHook` distinct from the existing `activeView` variable defined elsewhere in this file — check whether `App.tsx` already has its own `activeView` derived variable before introducing this one, and reconcile rather than shadowing.)

- [ ] **Step 2: Update `openParentNav`**

Find (around lines 589-591):

```tsx
const openParentNav = useCallback((parentKey: string) => {
  const child = DEFAULT_CHILD_BY_PARENT[parentKey] ?? parentKey;
  navigateTo(child);
}, [navigateTo]);
```

Replace with a direct call to the hook's own resolver (no behavior change, just removing the now-redundant local re-implementation since `useActiveView.navigateToParent` already does this):

```tsx
const openParentNav = openParentFromHook;
```

- [ ] **Step 3: Remove `openTabs`/`closeTab`/doc-tab references**

Search the whole file for `openTabs`, `closeTab`, `isDocTab`, `docPathFromTab`, `isPinnedTab`, `docLabels`, `activeTab` and address each:

- The `workbenchTabBar` construction (around lines 1062-1081, building the `<WorkbenchTabBar>` props object) — delete this whole block; it's no longer used once `tabBar` is removed from `AppShell` (Task 6).
- `mainSurface` (around lines 1235-1237):
  ```tsx
  const mainSurface = isDocTab(activeTab ?? '')
    ? <DocReader tabId={activeTab!} />
    : renderSurfaceContent(activeView, surfaceProps);
  ```
  Replace with:
  ```tsx
  const mainSurface = renderSurfaceContent(activeViewFromHook, surfaceProps);
  ```
  (Docs no longer render as the main surface at all — they're now exclusively in the `DocViewerDrawer` overlay, always layered on top of whatever surface is active underneath.)
- `surfaceKey`/`surfaceLabel` props passed to `<AppShell>` (around lines 1280-1281):
  ```tsx
  surfaceKey={activeTab ?? activeView}
  surfaceLabel={isDocTab(activeTab ?? '') ? 'Doc' : labelForNavKey(activeView)}
  ```
  Replace with:
  ```tsx
  surfaceKey={activeViewFromHook}
  surfaceLabel={labelForNavKey(activeViewFromHook)}
  ```
- `tabBar={workbenchTabBar}` prop on `<AppShell>` — delete this line entirely (Task 6 removes the prop from `AppShell` itself).
- `onOpenDoc={(path) => openDocTab(path)}` (on `<Omnibar>`, around line 1328) — unchanged in shape (still calls `openDocTab`, now backed by `useDocViewer`'s `openDoc`), no edit needed here beyond the Step 1 rename.
- Every other `activeView`/`activeTab` reference in the file (hash-sync `useEffect`, keybind handlers, etc.) — replace `activeTab` reads with `activeViewFromHook`, and any `openTab(...)`/`navigateTo(...)` calls keep their existing call shape unchanged (Step 1's rename preserved the call signature).
- Remove the now-dead `PINNED_TABS`/`isPinnedTab` import if nothing else in the file uses it after this pass (grep to confirm).

- [ ] **Step 4: Add `<DocViewerDrawer>` to the render tree**

Find the closing of the top-level `<>` fragment (around line 1327-1335, near `<Omnibar>` and `<Toasts>`), and add the drawer as a sibling, always rendered (it internally returns `null` when `activeDoc` is `null`):

```tsx
<DocViewerDrawer doc={activeDoc} onClose={closeDocViewer} />
```

Add the import near the other layout-component imports:

```tsx
import { DocViewerDrawer } from './components/layout/DocViewerDrawer';
```

- [ ] **Step 5: Run the full frontend test suite and fix any remaining reference**

Run: `cd crates/vox-gui/ui && npx vitest run`

This will surface any remaining `openTabs`/`closeTab`/`activeTab`/`isDocTab` reference this plan's grep missed (App.tsx is large and was read only in excerpts while writing this plan) — fix each compile/test failure by applying the same rename pattern from Steps 1-3. Do not skip or `.only()` failing tests to make this pass faster.

Run: `npx tsc --noEmit` and fix any type errors from the hook shape change the same way.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui): wire App.tsx to useActiveView/useDocViewer, remove workbench-tab state"
```

---

### Task 6: Remove `tabBar` prop from `AppShell.tsx`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/AppShell.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/AppShell.test.tsx` (if it exists — check; if not, this task's verification is the full suite run in Step 3)

- [ ] **Step 1: Remove the prop and its render slot**

Remove `tabBar?: React.ReactNode;` from `AppShellProps` (around line 44), remove `tabBar,` from the destructured props (around line 81), and change:

```tsx
<div className={`flex-1 min-h-0 flex flex-col overflow-hidden p-5 ${mainPaddingBottom}`}>
  {tabBar}
  <SurfaceErrorBoundary key={surfaceKey} surface={surfaceLabel}>
    <SurfaceScrollHost>{children}</SurfaceScrollHost>
  </SurfaceErrorBoundary>
</div>
```

to:

```tsx
<div className={`flex-1 min-h-0 overflow-hidden p-5 ${mainPaddingBottom}`}>
  <SurfaceErrorBoundary key={surfaceKey} surface={surfaceLabel}>
    <SurfaceScrollHost>{children}</SurfaceScrollHost>
  </SurfaceErrorBoundary>
</div>
```

(Also dropping `flex flex-col` from this wrapper since it now has exactly one child again — re-check this against the current file state first: this wrapper was made a flex column deliberately in the earlier "AppShell scroll-clipping" fix this session (commit c8b09935e4) specifically so `SurfaceScrollHost`'s `flex-1` could claim remaining space after the `tabBar` sibling. With `tabBar` gone, `SurfaceScrollHost` is the only child, so `h-full`-style sizing would work again, BUT changing `SurfaceScrollHost.tsx`'s own `flex-1` class back to something else is NOT part of this task's scope and risks re-breaking that earlier fix. **Do not touch `SurfaceScrollHost.tsx` in this task** — leave the wrapper as `flex flex-col` (even though it now has only one child, a flex container with one `flex-1` child is harmless and correct) rather than reverting the earlier fix's class choice. This note supersedes the code snippet above: keep `flex flex-col` in the wrapper's className, only remove the `{tabBar}` line itself.)

Corrected snippet (keep `flex flex-col`, only remove the `{tabBar}` line):

```tsx
<div className={`flex-1 min-h-0 flex flex-col overflow-hidden p-5 ${mainPaddingBottom}`}>
  <SurfaceErrorBoundary key={surfaceKey} surface={surfaceLabel}>
    <SurfaceScrollHost>{children}</SurfaceScrollHost>
  </SurfaceErrorBoundary>
</div>
```

- [ ] **Step 2: Update the call site in `App.tsx`**

If Task 5's Step 3 already removed the `tabBar={workbenchTabBar}` line from the `<AppShell>` call, this is already done — verify with `grep -n "tabBar" crates/vox-gui/ui/src/App.tsx` and confirm no remaining reference.

- [ ] **Step 3: Run the full suite**

Run: `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`
Expected: PASS, clean typecheck. This also re-confirms Task 5's changes compile correctly together with this task's.

- [ ] **Step 4: Live-verify no regression in the AppShell scroll fix**

Since this task touches the exact wrapper div the earlier scroll-clipping fix (commit c8b09935e4) modified, re-run that fix's own live verification: launch the real app, navigate to Settings (or any tab), force a short window height (500px), confirm `SurfaceScrollHost`'s viewport still shows `scrollHeight` reaching real overflowing content with `overflowY: auto` engaged (same check performed and documented earlier this session) — do not just trust that removing one line is safe, confirm it live.

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

- [ ] **Step 1: Confirm nothing else imports these files**

Run:
```bash
cd crates/vox-gui/ui && grep -rn "WorkbenchTabBar\|useWorkbenchTabs" src --include=*.ts --include=*.tsx | grep -v "\.test\."
```
Expected: no output (Tasks 5/6 already removed every real usage). If anything remains, resolve it before deleting — do not delete a file something still imports.

Also check the earlier session's F-07 accessibility test and `#workbench-tabbar-trailing-slot` DOM-id references (in `StatusBar.tsx`, per this session's earlier "move Panels trigger into StatusBar" work) — that id/slot is **independent** of `WorkbenchTabBar.tsx` itself (it now lives entirely in `StatusBar.tsx`, confirmed by this plan's own file reads) and must NOT be deleted; only the `WorkbenchTabBar.tsx` component file and the old `useWorkbenchTabs.ts` hook are being removed here.

- [ ] **Step 2: Delete the files**

```bash
git rm crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx
git rm crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.test.tsx
git rm crates/vox-gui/ui/src/hooks/useWorkbenchTabs.ts
git rm crates/vox-gui/ui/src/hooks/useWorkbenchTabs.test.ts
```

- [ ] **Step 3: Run the full suite**

Run: `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`
Expected: PASS, clean typecheck, with 4 fewer test files than before this task (the two deleted `.test.` files) but no drop in passing-test count beyond exactly those files' own tests.

- [ ] **Step 4: Commit**

```bash
git commit -m "chore(gui): delete WorkbenchTabBar and useWorkbenchTabs, superseded by useActiveView/useDocViewer"
```

---

### Task 8: Whole-effort live verification

**Files:** none (verification only)

- [ ] **Step 1: Full suite + typecheck one more time**

Run: `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`
Expected: PASS, clean, and record the exact test count for the final report.

- [ ] **Step 2: Live CDP verification of the whole redesign**

Rebuild (`pnpm build` then `cargo build -p vox-gui`), launch with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, and confirm via CDP + screenshots:

1. No top tab bar renders anywhere — only `TopHud`, `BreadcrumbBar`, `StatusBar` remain in the header.
2. Clicking a sidebar parent with children (e.g. Knowledge) expands its children in place; clicking a child navigates and the breadcrumb updates to match.
3. Clicking a different parent collapses the previous parent's children and expands the new one (accordion, not multi-expand).
4. At a short window height (500-600px), the sidebar with the largest child group (Knowledge, 6 children) expanded does not get cut off / does not lose scroll access to its own bottom content (System/Settings section, identity footer) — this directly re-verifies Task 4's Step 5 check still holds once the whole app is wired together, not just the isolated component.
5. Opening a doc (via any existing `onOpenDoc`/Omnibar entry point) shows the `DocViewerDrawer` sliding in from the right without navigating away from the current surface underneath; closing it (✕, Escape, or scrim click) returns to that same surface unchanged.
6. Opening a second doc while one is open replaces the drawer's content rather than stacking.
7. Sidebar `rail` (collapsed) mode still works: clicking a parent icon navigates to its default child without attempting to render an accordion tree (there's no room for one in rail mode).

Screenshot each of the above. If any check fails, fix the root cause (read the relevant task's file fresh, don't guess) before considering this plan complete.

- [ ] **Step 3: Final report**

No commit for this task (verification only) — summarize: final test count, typecheck status, and confirmation (with screenshot references) that all 7 live checks in Step 2 passed.

---

## Self-Review

**Spec coverage** — every section of `docs/superpowers/specs/2026-07-22-nav-shell-redesign-design.md` maps to a task: sidebar-as-sole-nav-surface + accordion → Task 4; tab bar removed → Tasks 6-7; breadcrumb → already implemented (`BreadcrumbBar.tsx`, confirmed via file read while writing this plan — no task needed, noted explicitly rather than silently assumed); doc-viewer separate affordance → Tasks 2-3; mobile-forward note → informational only, no task (matches the spec's own "no mobile-specific code is written as part of this effort" scope boundary).

**Placeholder scan** — Task 3's `DocReader` import path is flagged explicitly as needing live re-confirmation against the current file rather than asserted as fact, because it wasn't independently verified while writing this plan (only its usage site in `App.tsx` was read, not the component's own file/export) — this is a disclosed uncertainty with a concrete instruction for how to resolve it (grep `App.tsx`'s import line), not a vague "TBD".

**Type consistency** — `useActiveView`'s `navigateTo`/`navigateToParent` and `useDocViewer`'s `openDoc`/`closeDoc` naming is used consistently across Tasks 1, 2, 3, and 5 (Task 5 explicitly renames them back to `openTab`/`openDocTab` at the call-site boundary via destructuring aliases, with the reasoning stated inline, rather than silently drifting between two names for the same thing across tasks).
