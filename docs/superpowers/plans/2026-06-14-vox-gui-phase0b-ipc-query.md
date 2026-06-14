# vox-gui Phase 0B — IPC→Query Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add TanStack Query as the async data layer; extend `VoxTransport` with the four highest-value missing methods (`logFrontend`, `getGuiPreference`, `setGuiPreference`, `openLocator`); build `useVoxQuery`/`useVoxMutation` typed hooks; create a shared `<Async>` display component; migrate `consoleBridge.ts`, `usePersistedDbState.ts`, `DockShell.tsx`, and `CommandPalette.tsx` off raw `invoke()`.

**Architecture:** `VoxTransport` remains the single IPC chokepoint. TanStack Query sits above it: surfaces call `useVoxQuery(key, () => voxTransport.method())` instead of raw `invoke`. The `<Async>` component wraps `UseQueryResult` and renders idle/loading/empty/error/success states uniformly. `consoleBridge` and `usePersistedDbState` are the infrastructure hooks — migrating them unlocks the rest without touching the full ~53-call surface (that spans later per-surface waves).

**Tech Stack:** React 19, TypeScript 5, @tanstack/react-query 5, Vite 6, vitest 2, pnpm. Tauri v2.

> **Source of truth:** spec [`docs/superpowers/specs/2026-06-14-vox-gui-design-principles-application-design.md`](../specs/2026-06-14-vox-gui-design-principles-application-design.md); Phase 0A plan [`docs/superpowers/plans/2026-06-14-vox-gui-phase0a-visual-security-foundation.md`](2026-06-14-vox-gui-phase0a-visual-security-foundation.md).

> **All commands run from `crates/vox-gui/ui/` unless noted.** Uses **pnpm**, never npm. Tests: `pnpm test`. Typecheck: `pnpm typecheck`.

---

## Scope

**In Phase 0B:**
- Install `@tanstack/react-query` + wire `QueryClientProvider` in `main.tsx`
- Add `logFrontend`, `getGuiPreference`, `setGuiPreference`, `openLocator` to `VoxTransport`
- Create `useVoxQuery` + `useVoxMutation` typed wrapper hooks
- Create shared `<Async>` display wrapper component
- Migrate: `lib/consoleBridge.ts`, `hooks/usePersistedDbState.ts`, `layout/DockShell.tsx`, `layout/CommandPalette.tsx`

**Out of scope (later per-surface waves):**
- Migrating `App.tsx` (11 invoke calls — complex bootstrap flow, own wave)
- `BrowserView.tsx` (10 browser control calls — own wave)
- `SettingsView.tsx`, `TasksView.tsx`, `MemoryView.tsx` (complex mutation surfaces — own waves)
- Full browser-command suite on VoxTransport (own wave)

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/vox-gui/ui/package.json` | Modify | Add `@tanstack/react-query` |
| `crates/vox-gui/ui/src/main.tsx` | Modify | Wrap app in `QueryClientProvider` |
| `crates/vox-gui/ui/src/transport.ts` | Modify | Add `logFrontend`, `getGuiPreference`, `setGuiPreference`, `openLocator` methods |
| `crates/vox-gui/ui/src/hooks/useVoxQuery.ts` | Create | Typed `useVoxQuery<T>` + `useVoxMutation<T,V>` hooks |
| `crates/vox-gui/ui/src/hooks/useVoxQuery.test.ts` | Create | Tests via vitest + @testing-library/react |
| `crates/vox-gui/ui/src/components/ui/Async.tsx` | Create | `<Async>` display component |
| `crates/vox-gui/ui/src/components/ui/Async.test.tsx` | Create | Tests for all 5 states |
| `crates/vox-gui/ui/src/lib/consoleBridge.ts` | Modify | Replace raw `invoke('log_frontend',...)` with `voxTransport.logFrontend(...)` |
| `crates/vox-gui/ui/src/hooks/usePersistedDbState.ts` | Modify | Replace raw `invoke('get_gui_preference',...)` / `invoke('set_gui_preference',...)` with VoxTransport methods |
| `crates/vox-gui/ui/src/layout/DockShell.tsx` | Modify | Replace 2 direct invoke calls with VoxTransport |
| `crates/vox-gui/ui/src/layout/CommandPalette.tsx` | Modify | Replace `invoke('open_locator',...)` with `voxTransport.openLocator(...)` |

---

## Task 1: Install @tanstack/react-query + wire QueryClientProvider

**Files:**
- Modify: `crates/vox-gui/ui/package.json`
- Modify: `crates/vox-gui/ui/src/main.tsx`

- [ ] **Step 1: Install the package**

Run from `crates/vox-gui/ui/`: `pnpm add "@tanstack/react-query@^5.0.0"`
Expected: `@tanstack/react-query` in `dependencies` in `package.json`.

- [ ] **Step 2: Read current main.tsx**

Read `src/main.tsx` to understand its current structure before editing.

- [ ] **Step 3: Wire QueryClientProvider in main.tsx**

In `src/main.tsx`, add the QueryClient import and wrap the root:

```tsx
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
```

Create a QueryClient instance and wrap `<App />`:

```tsx
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 30_000,
    },
  },
});
```

And in the render call, wrap `<App />` with:
```tsx
<QueryClientProvider client={queryClient}>
  <App />
</QueryClientProvider>
```

The exact integration depends on what main.tsx currently wraps App with — read it first and preserve all existing wrappers (StrictMode, etc.). Add QueryClientProvider as the outermost Vox-specific wrapper (inside StrictMode if present).

- [ ] **Step 4: Verify typecheck passes**

Run: `pnpm typecheck`
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/package.json crates/vox-gui/ui/pnpm-lock.yaml crates/vox-gui/ui/src/main.tsx
git commit -m "feat(vox-gui): install @tanstack/react-query + wire QueryClientProvider"
```

---

## Task 2: Add logFrontend, getGuiPreference, setGuiPreference, openLocator to VoxTransport

These four methods currently exist as scattered raw `invoke()` calls. Moving them to VoxTransport makes them testable, typed, and centralized.

**Files:**
- Modify: `crates/vox-gui/ui/src/transport.ts`

- [ ] **Step 1: Read the current end of VoxTransport class**

Read `src/transport.ts` to find where to append the new methods (end of the `VoxTransport` class body, before the closing `}`).

- [ ] **Step 2: Write a failing test**

Create `crates/vox-gui/ui/src/transport.test.ts`:

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock @tauri-apps/api/core so tests don't need a real Tauri window
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { VoxTransport } from './transport';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('VoxTransport new methods', () => {
  let transport: VoxTransport;

  beforeEach(() => {
    transport = new VoxTransport();
    mockInvoke.mockReset();
  });

  it('logFrontend calls log_frontend with level + message', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await transport.logFrontend('warn', 'test warning');
    expect(mockInvoke).toHaveBeenCalledWith('log_frontend', { level: 'warn', message: 'test warning' });
  });

  it('getGuiPreference returns null when backend returns null', async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await transport.getGuiPreference('dock-layout');
    expect(mockInvoke).toHaveBeenCalledWith('get_gui_preference', { key: 'dock-layout' });
    expect(result).toBeNull();
  });

  it('getGuiPreference returns value when backend returns string', async () => {
    mockInvoke.mockResolvedValue('{"collapsed":false}');
    const result = await transport.getGuiPreference('dock-layout');
    expect(result).toBe('{"collapsed":false}');
  });

  it('setGuiPreference calls set_gui_preference', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await transport.setGuiPreference('dock-layout', '{"collapsed":true}');
    expect(mockInvoke).toHaveBeenCalledWith('set_gui_preference', { key: 'dock-layout', value: '{"collapsed":true}' });
  });

  it('openLocator calls open_locator', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await transport.openLocator('file://path/to/file');
    expect(mockInvoke).toHaveBeenCalledWith('open_locator', { locator: 'file://path/to/file' });
  });
});
```

Run: `pnpm test -- src/transport.test.ts`
Expected: FAIL — methods don't exist yet.

- [ ] **Step 3: Add the four methods to VoxTransport class**

Append to the VoxTransport class body (before the closing `}`):

```ts
  /** Forward a frontend log entry to the Rust backend unified log stream. */
  logFrontend(level: 'error' | 'warn' | 'info', message: string): Promise<void> {
    return invoke('log_frontend', { level, message });
  }

  /** Load a persisted GUI preference value (raw JSON string or null). */
  getGuiPreference(key: string): Promise<string | null> {
    return invoke<string | null>('get_gui_preference', { key });
  }

  /** Persist a GUI preference value (raw JSON string). */
  setGuiPreference(key: string, value: string): Promise<void> {
    return invoke('set_gui_preference', { key, value });
  }

  /** Open a locator (file path, URL, or vox:// URI) in the appropriate handler. */
  openLocator(locator: string): Promise<void> {
    return invoke('open_locator', { locator });
  }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm test -- src/transport.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Run full test suite**

Run: `pnpm test`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/transport.test.ts
git commit -m "feat(vox-gui): add logFrontend/getGuiPreference/setGuiPreference/openLocator to VoxTransport"
```

---

## Task 3: Create useVoxQuery + useVoxMutation hooks

Thin typed wrappers that enforce the convention: all data fetching goes through VoxTransport, all async state is in TanStack Query.

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useVoxQuery.ts`
- Create: `crates/vox-gui/ui/src/hooks/useVoxQuery.test.ts`

- [ ] **Step 1: Write failing tests**

Create `crates/vox-gui/ui/src/hooks/useVoxQuery.test.ts`:

```ts
import { describe, it, expect, vi } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useVoxQuery, useVoxMutation } from './useVoxQuery';

function makeWrapper() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return ({ children }: { children: React.ReactNode }) =>
    React.createElement(QueryClientProvider, { client: qc }, children);
}

describe('useVoxQuery', () => {
  it('returns data when fetcher resolves', async () => {
    const { result } = renderHook(
      () => useVoxQuery(['test-key'], () => Promise.resolve('hello')),
      { wrapper: makeWrapper() }
    );
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toBe('hello');
  });

  it('returns error when fetcher rejects', async () => {
    const { result } = renderHook(
      () => useVoxQuery(['test-err'], () => Promise.reject(new Error('boom'))),
      { wrapper: makeWrapper() }
    );
    await waitFor(() => expect(result.current.isError).toBe(true));
    expect((result.current.error as Error).message).toBe('boom');
  });
});

describe('useVoxMutation', () => {
  it('calls mutator and returns data', async () => {
    const mutator = vi.fn().mockResolvedValue('done');
    const { result } = renderHook(
      () => useVoxMutation(mutator),
      { wrapper: makeWrapper() }
    );
    result.current.mutate('input');
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(mutator).toHaveBeenCalledWith('input');
    expect(result.current.data).toBe('done');
  });
});
```

Run: `pnpm test -- src/hooks/useVoxQuery.test.ts`
Expected: FAIL — `Cannot find module './useVoxQuery'`.

- [ ] **Step 2: Install @testing-library/react if not present**

Check `package.json` devDependencies for `@testing-library/react`. If absent, run:
`pnpm add -D "@testing-library/react@^16.0.0"`

- [ ] **Step 3: Implement useVoxQuery.ts**

Create `crates/vox-gui/ui/src/hooks/useVoxQuery.ts`:

```ts
import { useQuery, useMutation, type UseQueryResult, type UseMutationResult, type QueryKey } from '@tanstack/react-query';

/**
 * Typed wrapper around TanStack useQuery enforcing the convention that all
 * data comes through VoxTransport (caller provides the fetcher fn).
 */
export function useVoxQuery<T>(
  queryKey: QueryKey,
  fetcher: () => Promise<T>,
  options?: { enabled?: boolean; staleTime?: number }
): UseQueryResult<T, Error> {
  return useQuery<T, Error>({
    queryKey,
    queryFn: fetcher,
    ...options,
  });
}

/**
 * Typed wrapper around TanStack useMutation for IPC write operations.
 */
export function useVoxMutation<TData = void, TVariables = void>(
  mutator: (variables: TVariables) => Promise<TData>,
  options?: { onSuccess?: (data: TData) => void; onError?: (err: Error) => void }
): UseMutationResult<TData, Error, TVariables> {
  return useMutation<TData, Error, TVariables>({
    mutationFn: mutator,
    ...options,
  });
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm test -- src/hooks/useVoxQuery.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/hooks/useVoxQuery.ts crates/vox-gui/ui/src/hooks/useVoxQuery.test.ts crates/vox-gui/ui/package.json crates/vox-gui/ui/pnpm-lock.yaml
git commit -m "feat(vox-gui): add useVoxQuery + useVoxMutation typed hooks (TanStack Query)"
```

---

## Task 4: Create shared `<Async>` display component

Renders five states (idle, loading, empty, error, success) consistently across every surface.

**Files:**
- Create: `crates/vox-gui/ui/src/components/ui/Async.tsx`
- Create: `crates/vox-gui/ui/src/components/ui/Async.test.tsx`

- [ ] **Step 1: Write failing tests**

Create `crates/vox-gui/ui/src/components/ui/Async.test.tsx`:

```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Async } from './Async';

describe('<Async>', () => {
  it('renders loading skeleton when isLoading=true', () => {
    render(<Async isLoading={true}>{null}</Async>);
    expect(screen.getByRole('status')).toBeTruthy();
  });

  it('renders error message when error is set', () => {
    render(<Async isLoading={false} error={new Error('oops')}>{null}</Async>);
    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText(/oops/)).toBeTruthy();
  });

  it('renders empty state when data is null/undefined and no error', () => {
    render(<Async isLoading={false} isEmpty={true}>{null}</Async>);
    expect(screen.getByText(/no data/i)).toBeTruthy();
  });

  it('renders children when data is available', () => {
    render(
      <Async isLoading={false}>
        <span>hello</span>
      </Async>
    );
    expect(screen.getByText('hello')).toBeTruthy();
  });
});
```

Run: `pnpm test -- src/components/ui/Async.test.tsx`
Expected: FAIL — `Cannot find module './Async'`.

- [ ] **Step 2: Implement Async.tsx**

Create `crates/vox-gui/ui/src/components/ui/Async.tsx`:

```tsx
import React from 'react';

interface AsyncProps {
  isLoading?: boolean;
  error?: Error | null;
  isEmpty?: boolean;
  emptyMessage?: string;
  children: React.ReactNode;
}

/** Renders async state uniformly: loading skeleton → error alert → empty state → children. */
export function Async({ isLoading, error, isEmpty, emptyMessage = 'No data', children }: AsyncProps) {
  if (isLoading) {
    return (
      <div role="status" className="animate-pulse space-y-2 p-4">
        <div className="h-4 bg-bg-elevated rounded w-3/4" />
        <div className="h-4 bg-bg-elevated rounded w-1/2" />
      </div>
    );
  }
  if (error) {
    return (
      <div role="alert" className="p-4 rounded border border-red-500/30 text-red-400 text-sm">
        {error.message}
      </div>
    );
  }
  if (isEmpty) {
    return (
      <div className="p-4 text-text-muted text-sm text-center">{emptyMessage}</div>
    );
  }
  return <>{children}</>;
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `pnpm test -- src/components/ui/Async.test.tsx`
Expected: PASS (4 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/ui/Async.tsx crates/vox-gui/ui/src/components/ui/Async.test.tsx
git commit -m "feat(vox-gui): add shared <Async> display component (loading/error/empty/success)"
```

---

## Task 5: Migrate consoleBridge + usePersistedDbState + DockShell + CommandPalette

Four infrastructure files that call `invoke()` directly — highest migration value because they're shared across many surfaces.

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/consoleBridge.ts`
- Modify: `crates/vox-gui/ui/src/hooks/usePersistedDbState.ts`
- Modify: `crates/vox-gui/ui/src/layout/DockShell.tsx`
- Modify: `crates/vox-gui/ui/src/layout/CommandPalette.tsx`

- [ ] **Step 1: Read each file before editing**

Read all four files to understand their full current content.

- [ ] **Step 2: Migrate consoleBridge.ts**

`consoleBridge.ts` calls `invoke('log_frontend', { level, message })`.

Replace the raw `invoke` import + call with `voxTransport.logFrontend(level, message)`.

Current pattern to replace (example):
```ts
import { invoke } from '@tauri-apps/api/core';
// ...
invoke('log_frontend', { level, message }).catch(() => {});
```

Replacement:
```ts
import { voxTransport } from '../transport';
// ...
voxTransport.logFrontend(level, message).catch(() => {});
```

Remove the `@tauri-apps/api/core` import if `invoke` was its only use. Keep the rest of the file unchanged.

- [ ] **Step 3: Migrate usePersistedDbState.ts**

Replaces two raw invoke calls. Current pattern:
```ts
import { invoke } from '@tauri-apps/api/core';
// ...
const raw = await invoke<string | null>('get_gui_preference', { key });
// ...
await invoke('set_gui_preference', { key, value: JSON.stringify(...) });
```

Replacement:
```ts
import { voxTransport } from '../transport';
// ...
const raw = await voxTransport.getGuiPreference(key);
// ...
await voxTransport.setGuiPreference(key, JSON.stringify(...));
```

Keep the full hook logic (deferred writes, isLoaded flag, type deserialization) unchanged.

- [ ] **Step 4: Migrate DockShell.tsx**

Replaces two raw invoke calls with `voxTransport.getGuiPreference` / `voxTransport.setGuiPreference`. Keep error handling unchanged (`.catch(() => {})`).

- [ ] **Step 5: Migrate CommandPalette.tsx**

Replace the `invoke('open_locator', { locator: hit.locator })` call with `voxTransport.openLocator(hit.locator)`. Also check if `invoke('vox_docs_index')` and `invoke('vox_search_query', ...)` calls can be wrapped — if VoxTransport already has methods for these, use them; if not, leave them as-is (adding all CommandPalette commands to VoxTransport is a separate task).

- [ ] **Step 6: Typecheck + tests**

Run: `pnpm typecheck`
Expected: no new errors.
Run: `pnpm test`
Expected: all existing tests still pass.

- [ ] **Step 7: Verify no @tauri-apps/api/core imports remain in the migrated files**

Run: `grep -l "from '@tauri-apps/api/core'" src/lib/consoleBridge.ts src/hooks/usePersistedDbState.ts src/layout/DockShell.tsx src/layout/CommandPalette.tsx 2>/dev/null`
Expected: empty output (no direct invoke imports remaining in those files).

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/lib/consoleBridge.ts crates/vox-gui/ui/src/hooks/usePersistedDbState.ts crates/vox-gui/ui/src/layout/DockShell.tsx crates/vox-gui/ui/src/layout/CommandPalette.tsx
git commit -m "refactor(vox-gui): route consoleBridge/usePersistedDbState/DockShell/CommandPalette through VoxTransport"
```

---

## Task 6: Phase 0B verification gate

**Files:** none (verification only).

- [ ] **Step 1: Typecheck**

Run: `pnpm typecheck`
Expected: no errors.

- [ ] **Step 2: Full unit test suite**

Run: `pnpm test`
Expected: all suites pass, including the new transport.test.ts (×5), useVoxQuery.test.ts (×3), Async.test.tsx (×4), and all pre-existing tests.

- [ ] **Step 3: Build**

Run: `pnpm build`
Expected: succeeds end-to-end.

- [ ] **Step 4: Verify raw invoke() is gone from the four migrated files**

Run from `crates/vox-gui/ui/`:
```bash
grep -n "invoke(" src/lib/consoleBridge.ts src/hooks/usePersistedDbState.ts src/layout/DockShell.tsx src/layout/CommandPalette.tsx
```
Expected: no matches (or only comments).

- [ ] **Step 5: Final commit (if any verification fixups were needed)**

```bash
git add -A
git commit -m "chore(vox-gui): Phase 0B IPC→Query foundation green"
```

---

## Self-Review

**Spec coverage:** TanStack Query provider ✅ (T1); `logFrontend`/`getGuiPreference`/`setGuiPreference`/`openLocator` on VoxTransport ✅ (T2); `useVoxQuery`/`useVoxMutation` ✅ (T3); `<Async>` wrapper ✅ (T4); consoleBridge/usePersistedDbState/DockShell/CommandPalette migrated ✅ (T5). Remaining ~40 raw invoke() calls (App, Browser, Settings, Tasks, Memory, etc.) → deferred to per-surface waves (post-0D), consistent with the Phase 0A plan's description.

**Placeholder scan:** Every step contains complete code. The "read before editing" step in T5 is a pre-condition guard, not a placeholder. The `vox_docs_index`/`vox_search_query` caveat in T5 Step 5 is explicit — leave them if not already on VoxTransport, defer.

**Type consistency:** `useVoxQuery<T>` returns `UseQueryResult<T, Error>` matching what `useQuery` returns; `<Async>` accepts `Error | null` matching the TanStack error type; `VoxTransport.logFrontend` accepts `'error' | 'warn' | 'info'` matching what `consoleBridge` uses; `getGuiPreference` returns `string | null` matching what `usePersistedDbState` expects.
