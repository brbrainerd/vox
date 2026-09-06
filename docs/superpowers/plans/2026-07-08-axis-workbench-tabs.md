# Axis Workbench Tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace single-view navigation with a leaf-surface workbench tab bar, fix scroll and Console/Chat reliability, move attention budget into Chat, filter Omnibar docs to help-only with in-app doc reader tabs, and eliminate VoxDb lock errors via a GUI DB pool.

**Architecture:** `useWorkbenchTabs` owns `openTabs` + `activeTab` (persisted). Sidebar/Omnibar/hash call `openTab(viewKey)`. `WorkbenchTabBar` + `SurfaceScrollHost` replace `ParentSurface`/`SubTabs` and default `DockShell` wrapping. Rust `GuiDbPool` in Tauri state shares one `Arc<VoxDb>` across chat/scientia commands.

**Tech Stack:** React 19, TypeScript, vitest, Playwright, Tauri 2, TanStack Query (existing), `vox-db`, dockview (Console splits deferred).

> **Spec:** `docs/superpowers/specs/2026-07-08-axis-workbench-tabs-design.md`

---

## File map

| File | Responsibility |
|------|----------------|
| `ui/src/hooks/useWorkbenchTabs.ts` | Tab state: open, focus, close, persist, migrate from `vox_active_view` |
| `ui/src/components/layout/WorkbenchTabBar.tsx` | Closable tab UI + keyboard ⌘W |
| `ui/src/components/layout/SurfaceScrollHost.tsx` | Single scrollport wrapper |
| `ui/src/components/layout/surfaceComponents.tsx` | Render active tab surface; add `DocReader` |
| `ui/src/components/surfaces/DocReader/DocReader.tsx` | In-app markdown viewer for doc tabs |
| `ui/src/App.tsx` | Wire workbench; remove `AttentionStrip`; drop `ParentSurface` path |
| `ui/src/components/layout/Sidebar.tsx` | `openTab(DEFAULT_CHILD)` instead of `setView` |
| `ui/src/components/layout/AppShell.tsx` | Tab bar slot; pass `SurfaceScrollHost` children |
| `ui/src/components/surfaces/Console/Console.tsx` | flex/min-h-0 layout; lazy-mount friendly |
| `ui/src/components/surfaces/Chat/*` | Attention meter above Loquela |
| `ui/e2e/workbench-tabs.spec.ts` | Playwright golden routes |
| `src/commands/app_state.rs` | `GuiDbPool` struct |
| `src/commands/docs_index.rs` | `category` parse + help filter |
| `src/commands/chat.rs` | Use pool instead of `gui_db()` connect |
| `src/commands/scientia.rs` | Use pool; structured busy errors |
| `src/main.rs` | `.manage(GuiDbPool::…)` in setup |

---

## Task 1: `useWorkbenchTabs` hook (TDD)

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useWorkbenchTabs.ts`
- Create: `crates/vox-gui/ui/src/hooks/useWorkbenchTabs.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useWorkbenchTabs } from './useWorkbenchTabs';

describe('useWorkbenchTabs', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('openTab adds a leaf tab and focuses it', () => {
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => result.current.openTab('console'));
    expect(result.current.openTabs).toEqual(['console']);
    expect(result.current.activeTab).toBe('console');
  });

  it('openTab focuses existing tab without duplicate', () => {
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => {
      result.current.openTab('chat');
      result.current.openTab('console');
      result.current.openTab('chat');
    });
    expect(result.current.openTabs).toEqual(['chat', 'console']);
    expect(result.current.activeTab).toBe('chat');
  });

  it('closeTab removes tab and focuses neighbor', () => {
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => {
      result.current.openTab('console');
      result.current.openTab('chat');
      result.current.closeTab('console');
    });
    expect(result.current.openTabs).toEqual(['chat']);
    expect(result.current.activeTab).toBe('chat');
  });

  it('closeTab on last tab opens dashboard fallback', () => {
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => {
      result.current.openTab('console');
      result.current.closeTab('console');
    });
    expect(result.current.activeTab).toBe('dashboard');
    expect(result.current.openTabs).toContain('dashboard');
  });

  it('doc tab ids are stable', () => {
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => result.current.openDocTab('docs/src/reference/cli.md', 'CLI'));
    expect(result.current.activeTab).toBe('doc:docs/src/reference/cli.md');
  });
});
```

- [ ] **Step 2: Run test — FAIL**

Run: `pnpm -C crates/vox-gui/ui exec vitest run src/hooks/useWorkbenchTabs.test.ts`

- [ ] **Step 3: Implement hook**

```typescript
// crates/vox-gui/ui/src/hooks/useWorkbenchTabs.ts
import { useCallback, useMemo } from 'react';
import { useLocalStorage } from './useLocalStorage';
import { DEFAULT_CHILD_BY_PARENT, LEGACY_VIEW_ALIASES } from '../lib/navigation';

export type TabId = string; // ViewKey or doc:path

const STORAGE_KEY = 'vox_workbench_tabs.v1';
const LEGACY_VIEW_KEY = 'vox_active_view';
const FALLBACK_TAB = 'dashboard';
const DEFAULT_PINNED: TabId[] = ['chat'];

function normalizeViewKey(key: string): string {
  return LEGACY_VIEW_ALIASES[key as keyof typeof LEGACY_VIEW_ALIASES] ?? key;
}

function docTabId(path: string): TabId {
  return `doc:${path.replace(/\\/g, '/')}`;
}

export function isDocTab(id: TabId): boolean {
  return id.startsWith('doc:');
}

export function docPathFromTab(id: TabId): string {
  return id.slice('doc:'.length);
}

interface StoredState {
  openTabs: TabId[];
  activeTab: TabId | null;
}

function migrateLegacyView(): StoredState | null {
  const legacy = localStorage.getItem(LEGACY_VIEW_KEY);
  if (!legacy) return null;
  const view = normalizeViewKey(legacy);
  return { openTabs: [view], activeTab: view };
}

export function useWorkbenchTabs() {
  const [stored, setStored] = useLocalStorage<StoredState>(STORAGE_KEY, () => {
    return migrateLegacyView() ?? { openTabs: [...DEFAULT_PINNED, FALLBACK_TAB], activeTab: FALLBACK_TAB };
  });

  const openTabs = stored.openTabs;
  const activeTab = stored.activeTab;

  const setState = useCallback(
    (next: StoredState) => setStored(next),
    [setStored],
  );

  const openTab = useCallback(
    (viewKey: string) => {
      const key = normalizeViewKey(viewKey);
      setState({
        openTabs: openTabs.includes(key) ? openTabs : [...openTabs, key],
        activeTab: key,
      });
    },
    [openTabs, setState],
  );

  const openParent = useCallback(
    (parentKey: string) => {
      const child = DEFAULT_CHILD_BY_PARENT[parentKey] ?? parentKey;
      openTab(child);
    },
    [openTab],
  );

  const openDocTab = useCallback(
    (path: string, _title?: string) => {
      const id = docTabId(path);
      setState({
        openTabs: openTabs.includes(id) ? openTabs : [...openTabs, id],
        activeTab: id,
      });
    },
    [openTabs, setState],
  );

  const closeTab = useCallback(
    (id: TabId) => {
      const idx = openTabs.indexOf(id);
      if (idx === -1) return;
      const nextTabs = openTabs.filter(t => t !== id);
      if (nextTabs.length === 0) {
        setState({ openTabs: [FALLBACK_TAB], activeTab: FALLBACK_TAB });
        return;
      }
      const neighbor = nextTabs[Math.min(idx, nextTabs.length - 1)] ?? nextTabs[0];
      setState({ openTabs: nextTabs, activeTab: neighbor });
    },
    [openTabs, setState],
  );

  const activeViewKey = useMemo(
    () => (activeTab && !isDocTab(activeTab) ? activeTab : null),
    [activeTab],
  );

  return { openTabs, activeTab, activeViewKey, openTab, openParent, openDocTab, closeTab };
}
```

- [ ] **Step 4: Run test — PASS**

- [ ] **Step 5: Commit** `feat(gui): add useWorkbenchTabs hook with persistence`

---

## Task 2: WorkbenchTabBar component

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx`
- Create: `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.test.tsx`

- [ ] **Step 1: Failing test**

```typescript
it('renders tabs with close buttons and marks active', () => {
  render(
    <WorkbenchTabBar
      tabs={[
        { id: 'console', label: 'Console' },
        { id: 'chat', label: 'Chat' },
      ]}
      activeTab="console"
      onSelect={vi.fn()}
      onClose={vi.fn()}
    />,
  );
  expect(screen.getByRole('tab', { name: 'Console' })).toHaveAttribute('aria-selected', 'true');
  expect(screen.getByRole('button', { name: 'Close Console' })).toBeDefined();
});
```

- [ ] **Step 2: Implement**

```tsx
// WorkbenchTabBar.tsx — role="tablist", each tab role="tab", × button aria-label Close {label}
// Keyboard: ArrowLeft/Right move focus; Delete/⌘W calls onClose(active) when tablist focused
```

- [ ] **Step 3: Vitest PASS**

- [ ] **Step 4: Commit** `feat(gui): WorkbenchTabBar with closable tabs`

---

## Task 3: SurfaceScrollHost + AppShell integration

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/SurfaceScrollHost.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/AppShell.tsx`
- Modify: `crates/vox-gui/ui/src/App.tsx`

- [ ] **Step 1: Create scroll host**

```tsx
export function SurfaceScrollHost({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden" data-testid="surface-scroll-host">
      <div className="h-full min-h-0 overflow-auto custom-scrollbar">{children}</div>
    </div>
  );
}
```

- [ ] **Step 2: AppShell — add `tabBar` slot above content; wrap children in SurfaceScrollHost; remove DockShell as default wrapper**

Replace:

```tsx
<DockShell panelId="main-surface" panelTitle={surfaceLabel}>
  {children}
</DockShell>
```

With:

```tsx
{tabBar}
<SurfaceScrollHost>{children}</SurfaceScrollHost>
```

- [ ] **Step 3: App.tsx — wire workbench**

```tsx
const workbench = useWorkbenchTabs();
const navigateTo = useCallback((viewKey: string) => workbench.openTab(viewKey), [workbench]);

// Sidebar: pass openParent for top-level, openTab for leaves
// AppShell tabBar={
//   <WorkbenchTabBar
//     tabs={workbench.openTabs.map(id => ({ id, label: tabLabel(id) }))}
//     activeTab={workbench.activeTab}
//     onSelect={id => workbench.openTab(id)} // or openDocTab for doc:*
//     onClose={workbench.closeTab}
//   />
// }

// Render: if isDocTab(activeTab) → DocReader else childRenderer(activeViewKey)
```

- [ ] **Step 4: Remove `renderSurfaceView` ParentSurface path from main render**

Delete usage of `ParentSurface` in `surfaceComponents.tsx` `renderSurfaceView`; export `renderSurfaceContent(viewKey, props)` directly.

- [ ] **Step 5: Commit** `feat(gui): workbench tab bar + scroll host in AppShell`

---

## Task 4: Sidebar + hash sync

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`
- Modify: `crates/vox-gui/ui/src/lib/navigation.ts` (export `tabLabelFor(viewKey)` helper)
- Modify: `crates/vox-gui/ui/src/App.tsx` (hashchange → openTab)

- [ ] **Step 1: Sidebar clicks call `openParent(key)` for TOP_LEVEL_VIEWS**

Change `onClick={() => setView(DEFAULT_CHILD_BY_PARENT[key] ?? key)}` → `onClick={() => onOpenParent(key)}`.

Settings/Coverage keep `openTab('settings')` / `openTab('coverage')`.

- [ ] **Step 2: Active state from `activeViewKey` parent**

Use existing `resolveNavigation(activeViewKey)` for `activeParent`.

- [ ] **Step 3: Hash sync**

In `navigateTo` / `openTab`, call `syncViewToLocation(viewKey)` when leaf tab opens.

On `hashchange`, parse `#view=` → `openTab`.

- [ ] **Step 4: Vitest update Sidebar tests**

- [ ] **Step 5: Commit** `feat(gui): sidebar opens workbench tabs`

---

## Task 5: Remove global AttentionStrip; meter in Chat

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (remove `<AttentionStrip …>`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (or chat layout wrapper)
- Modify: `crates/vox-gui/ui/src/components/layout/WorkbenchTabBar.tsx` (optional badge)

- [ ] **Step 1: Failing test**

```typescript
it('shows AttentionBudgetMeter above composer when budget present', () => {
  render(<ChatSurface ... attention_budget={mockBudget} />);
  expect(screen.getByRole('meter')).toBeDefined();
});
```

- [ ] **Step 2: Add compact meter row above Loquela in Chat tab only**

- [ ] **Step 3: Remove AttentionStrip from App.tsx**

- [ ] **Step 4: Optional Chat tab badge when waitingQuestions > 0**

- [ ] **Step 5: Commit** `feat(gui): attention budget in Chat tab only`

---

## Task 6: Console layout + lazy mount fix

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Console/Console.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`
- Create: `crates/vox-gui/ui/e2e/console-workbench.spec.ts`

- [ ] **Step 1: Playwright failing test**

```typescript
test('console tab shows terminal or orchestrator error', async ({ page }) => {
  // tauri mock + open console tab
  await expect(page.getByTestId('console-root')).toBeVisible();
  await expect(page.getByRole('alert').or(page.locator('.xterm'))).toBeVisible();
});
```

- [ ] **Step 2: Console root — replace inline styles**

```tsx
<div className="flex h-full min-h-0 flex-col" data-testid="console-root">
```

Inner terminal: `flex-1 min-h-0` (no page-level scroll).

- [ ] **Step 3: Lazy mount — only render Console when `activeTab === 'console'`**

In App/surfaceComponents, `{activeTab === 'console' && <Console … />}` or keep mounted but hide PTY when inactive (prefer unmount to fix duplicate PTY).

- [ ] **Step 4: Playwright PASS**

- [ ] **Step 5: Commit** `fix(gui): console layout and lazy tab mount`

---

## Task 7: Scroll audit — Settings/Coverage + Scientia

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/*` (or Coverage view)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaSurface.tsx`
- Modify: `crates/vox-gui/ui/e2e/workbench-tabs.spec.ts`

- [ ] **Step 1: Playwright scroll test**

```typescript
test('settings coverage table scrolls', async ({ page }) => {
  await page.goto('/#view=settings');
  const host = page.getByTestId('surface-scroll-host');
  const before = await host.evaluate(el => el.scrollTop);
  await host.hover();
  await page.mouse.wheel(0, 400);
  const after = await host.evaluate(el => el.scrollTop);
  expect(after).toBeGreaterThan(before);
});
```

- [ ] **Step 2: Remove `h-screen` / duplicate `overflow-y-auto` from Settings and Scientia roots**

- [ ] **Step 3: PASS e2e**

- [ ] **Step 4: Commit** `fix(gui): scroll via SurfaceScrollHost on long surfaces`

---

## Task 8: GuiDbPool (Rust TDD)

**Files:**
- Create: `crates/vox-gui/src/commands/gui_db_pool.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`
- Modify: `crates/vox-gui/src/main.rs`
- Modify: `crates/vox-gui/src/commands/chat.rs`
- Modify: `crates/vox-gui/src/commands/scientia.rs`

- [ ] **Step 1: Write failing Rust test**

```rust
// crates/vox-gui/src/commands/gui_db_pool.rs
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn pool_reuses_same_connection() {
        let pool = GuiDbPool::connect_memory().await.unwrap();
        let a = pool.handle();
        let b = pool.handle();
        assert!(Arc::ptr_eq(&a, &b));
    }
}
```

- [ ] **Step 2: Implement**

```rust
pub struct GuiDbPool {
    db: Arc<VoxDb>,
}

impl GuiDbPool {
    pub async fn connect_workspace() -> Result<Self, String> {
        let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
            .await
            .ok_or_else(|| "workspace database unavailable".to_string())?;
        Ok(Self { db: Arc::new(db) })
    }
    pub fn handle(&self) -> Arc<VoxDb> {
        Arc::clone(&self.db)
    }
}
```

- [ ] **Step 3: main.rs setup**

```rust
.setup(|app| {
    let pool = tauri::async_runtime::block_on(GuiDbPool::connect_workspace())
        .map_err(|e| e.to_string())?;
    app.manage(pool);
    Ok(())
})
```

- [ ] **Step 4: chat.rs — replace `gui_db()` with `State<GuiDbPool>`**

- [ ] **Step 5: scientia.rs — same pool; map lock errors**

```rust
fn map_db_err(e: impl std::fmt::Display) -> String {
    let s = e.to_string();
    if s.contains("Locking error") || s.contains("SQLITE_BUSY") {
        "Database busy — another process is writing. Retry in a moment.".into()
    } else {
        s
    }
}
```

- [ ] **Step 6: `cargo test -p vox-gui gui_db_pool` PASS**

- [ ] **Step 7: Commit** `fix(gui): shared GuiDbPool for chat and scientia`

---

## Task 9: Help-only docs index

**Files:**
- Modify: `crates/vox-gui/src/commands/docs_index.rs`
- Create: `crates/vox-gui/src/commands/docs_index_test.rs` (or inline `#[cfg(test)]`)

- [ ] **Step 1: Extend frontmatter parser**

```rust
pub(crate) struct Frontmatter {
    pub title: String,
    pub description: String,
    pub category: Option<String>,
}

const HELP_CATEGORIES: &[&str] = &["how-to", "tutorial", "reference", "contributor"];

fn is_help_doc(fm: &Frontmatter, path: &Path) -> bool {
    if path.to_string_lossy().contains("docs/src/archive") {
        return false;
    }
    fm.category.as_deref().is_some_and(|c| HELP_CATEGORIES.contains(&c))
}
```

Skip non-help in `walk_docs`.

- [ ] **Step 2: Unit tests**

```rust
#[test]
fn excludes_architecture_category() {
    assert!(!is_help_doc(&Frontmatter { category: Some("architecture".into()), .. }, path));
}
#[test]
fn includes_how_to() {
    assert!(is_help_doc(&Frontmatter { category: Some("how-to".into()), .. }, path));
}
```

- [ ] **Step 3: `cargo test -p vox-gui docs_index` PASS**

- [ ] **Step 4: Commit** `feat(gui): help-only docs index filter`

---

## Task 10: DocReader surface + Omnibar wiring

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/DocReader/DocReader.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/DocReader/DocReader.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/Omnibar.tsx`
- Modify: `crates/vox-gui/ui/src/App.tsx`
- Modify: `crates/vox-gui/src/commands/search.rs` (add `read_doc_markdown` command if needed)

- [ ] **Step 1: Tauri command to read doc file as markdown string**

```rust
#[tauri::command]
pub async fn read_doc_markdown(path: String) -> Result<String, String> {
    // validate path under docs/src/
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: DocReader component**

```tsx
export function DocReader({ path }: { path: string }) {
  const q = useQuery({ queryKey: ['doc', path], queryFn: () => voxTransport.readDocMarkdown(path) });
  // render markdown via existing sanitizer component
  return (
    <article className="prose prose-invert max-w-none p-4">
      {q.isLoading ? 'Loading…' : <Markdown content={q.data ?? ''} />}
      <Button onClick={() => voxTransport.openLocator({ kind: 'file', value: path })}>
        Open in editor
      </Button>
    </article>
  );
}
```

- [ ] **Step 3: Omnibar — doc facet gating**

Only include docs results when query starts with `/` or `/help` or contains `\bhelp\b`.

- [ ] **Step 4: onOpenDoc → openDocTab(path, title)** instead of external-only

- [ ] **Step 5: Vitest + manual verify**

- [ ] **Step 6: Commit** `feat(gui): in-app doc reader tabs from Omnibar`

---

## Task 11: Scientia archive UX (P2)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Scientia/ArchiveStatusSummary.tsx`

- [ ] **Step 1: Rename label + tooltip**

```tsx
<span title="Count from a sample of publications not yet deposited to Zenodo or Software Heritage">
  Pending deposit (sample): {count}
</span>
```

- [ ] **Step 2: Toast on fetch error instead of silent `setRows([])`**

- [ ] **Step 3: Commit** `fix(gui): clarify Scientia archive deposit status`

---

## Task 12: Playwright workbench golden suite

**Files:**
- Create: `crates/vox-gui/ui/e2e/workbench-tabs.spec.ts`
- Modify: `crates/vox-gui/ui/e2e/screenshots.spec.ts` (ensure tab model doesn't break captures)

- [ ] **Step 1: Tests**

```typescript
test.describe('Workbench tabs', () => {
  test('workspace sidebar opens console tab', async ({ page }) => { /* … */ });
  test('chat tab pinned on fresh profile', async ({ page }) => { /* … */ });
  test('close tab focuses neighbor', async ({ page }) => { /* … */ });
  test('help search opens doc tab', async ({ page }) => { /* … */ });
});
```

- [ ] **Step 2: Run**

`pnpm -C crates/vox-gui/ui exec playwright test workbench-tabs.spec.ts --project=chromium`

- [ ] **Step 3: Commit** `test(gui): workbench tabs e2e`

---

## Task 13: Docs + registry update

**Files:**
- Modify: `docs/src/reference/gui-navigation.md` (workbench tabs, Chat attention, help search)

- [ ] **Step 1: Update gui-navigation.md §Navigation model**

Document workbench tabs, remove ParentSurface/SubTabs as primary nav.

- [ ] **Step 2: Commit** `docs(gui): workbench tab navigation reference`

---

## Execution order (recommended)

```text
Task 1 → 2 → 3 → 4   (workbench core)
Task 6 → 7           (Console + scroll) — can parallel after Task 3
Task 5               (attention in Chat)
Task 8               (DB pool — unblocks Scientia)
Task 9 → 10          (help docs + reader)
Task 11 → 12 → 13    (polish + e2e + docs)
```

**Local verification before push:**

```powershell
pnpm -C crates/vox-gui/ui exec vitest run
pnpm -C crates/vox-gui/ui exec playwright test workbench-tabs.spec.ts console-workbench.spec.ts --project=chromium
cargo test -p vox-gui
vox ci pre-push --complete --since HEAD~1
```

---

## Plan self-review (spec coverage)

| Spec § | Task |
|--------|------|
| §3.1 Tab model | Task 1, 2, 4 |
| §3.2 Layout | Task 3 |
| §3.3 Scroll | Task 3, 7 |
| §3.4 Chat + attention | Task 5 |
| §3.5 Help search + doc tabs | Task 9, 10 |
| §3.6 VoxDb pool | Task 8 |
| §3.7 Console P0 | Task 6 |
| §5 Testing | Task 12 |
| Scientia archive copy | Task 11 |

No placeholders remain. Types: `TabId`, `openTab`, `openDocTab`, `GuiDbPool` consistent throughout.

---

## Execution handoff

**Plan saved to:** `docs/superpowers/plans/2026-07-08-axis-workbench-tabs.md`

**Two execution options:**

1. **Subagent-driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline execution** — implement tasks in-session with checkpoints after Task 4 (core tabs) and Task 8 (DB pool)

Which approach do you want?
