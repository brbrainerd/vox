# vox-gui Wave 1 Query Adoption Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close Wave 1 exit criteria by exercising `useVoxQuery`, `useVoxMutation`, and `<Async>` on the three pilot surfaces (App shell bootstrap subset, Dashboard data path, Settings preference IPC).

**Architecture:** `VoxTransport` remains the IPC chokepoint; TanStack Query wraps transport calls. App.tsx orchestrator status uses `listenOrchStatus` event stream as the primary source with a one-shot `get_orchestrator_status_bin` query for cold start. Settings keeps the two preference methods on transport (other 24 invokes deferred to Wave 6).

**Tech Stack:** React 19, @tanstack/react-query v5, vitest, `@testing-library/react`. Commands from `crates/vox-gui/ui/`.

> **Prerequisite:** Phase 0B landed (`QueryClientProvider` in `main.tsx`). **Blocks:** Waves 2–6 per master roadmap.

---

## File structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/hooks/useOrchestratorStatus.ts` | Create | `useVoxQuery` + `listenOrchStatus` subscription |
| `src/hooks/useOrchestratorStatus.test.ts` | Create | Mock transport + event listener |
| `src/App.tsx` | Modify | Replace inline orch polling with hook |
| `src/components/surfaces/Dashboard/Dashboard.tsx` | Modify | Wrap agent/stream panels with `<Async>` when loading |
| `src/components/surfaces/Dashboard/Dashboard.test.tsx` | Modify | Loading + empty state tests |
| `src/components/surfaces/Settings/SettingsView.tsx` | Modify | `useVoxMutation` for `setGuiPreference` writes |
| `src/components/surfaces/Settings/SettingsView.test.tsx` | Modify | Mutation + aria-live save feedback |

---

## Task 1: `useOrchestratorStatus` hook

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useOrchestratorStatus.ts`
- Create: `crates/vox-gui/ui/src/hooks/useOrchestratorStatus.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';

vi.mock('../transport', () => ({
  listenOrchStatus: vi.fn().mockResolvedValue(() => {}),
  voxTransport: {
    getOrchestratorStatusBin: vi.fn().mockResolvedValue(new Uint8Array([0x80])),
  },
}));

import { useOrchestratorStatus } from './useOrchestratorStatus';
import { listenOrchStatus } from '../transport';

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return React.createElement(QueryClientProvider, { client }, children);
}

describe('useOrchestratorStatus', () => {
  beforeEach(() => vi.clearAllMocks());

  it('returns isSuccess after cold-start fetch', async () => {
    const { result } = renderHook(() => useOrchestratorStatus(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(listenOrchStatus).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/hooks/useOrchestratorStatus.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Add transport method + hook**

In `transport.ts` add:

```typescript
getOrchestratorStatusBin(): Promise<Uint8Array> {
  return invoke('get_orchestrator_status_bin');
}
```

Create `useOrchestratorStatus.ts`:

```typescript
import { useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { decode } from '@msgpack/msgpack';
import { listenOrchStatus, voxTransport } from '../transport';

const ORCH_KEY = ['orchestrator', 'status'] as const;

export function useOrchestratorStatus() {
  const qc = useQueryClient();
  const query = useQuery({
    queryKey: ORCH_KEY,
    queryFn: async () => {
      const raw = await voxTransport.getOrchestratorStatusBin();
      return decode(raw) as Record<string, unknown>;
    },
    staleTime: 5_000,
  });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenOrchStatus((status) => {
      qc.setQueryData(ORCH_KEY, status);
    }).then((fn) => { unlisten = fn; });
    return () => unlisten?.();
  }, [qc]);

  return query;
}
```

- [ ] **Step 4: Run test — expect PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/hooks/useOrchestratorStatus.ts crates/vox-gui/ui/src/hooks/useOrchestratorStatus.test.ts crates/vox-gui/ui/src/transport.ts
git commit -m "feat(gui): add useOrchestratorStatus query hook"
```

---

## Task 2: Wire App.tsx to hook (incremental)

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx`

- [ ] **Step 1:** Import `useOrchestratorStatus`
- [ ] **Step 2:** Replace the manual `get_orchestrator_status_bin` `useEffect` + duplicate listener with hook output mapped through existing `mapAgent` / stream reducers
- [ ] **Step 3:** Keep `listenAgentEvents` as-is (separate wave)
- [ ] **Step 4:** Run `pnpm test src/App.test.tsx` — must pass
- [ ] **Step 5: Commit** `feat(gui): App orchestrator bootstrap via useOrchestratorStatus`

---

## Task 3: Dashboard `<Async>` wrapper

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx`

- [ ] **Step 1: Failing test**

```typescript
it('shows skeleton when loading prop is true', () => {
  render(<Dashboard data={emptyDash} loading {...defaultHandlers} />);
  expect(screen.getByRole('status', { name: /loading/i })).toBeDefined();
});
```

- [ ] **Step 2:** Add optional `loading?: boolean` prop; wrap stream + agents columns in `<Async loading={loading}>` with `<Skeleton>` fallback
- [ ] **Step 3:** App passes `loading={orchQuery.isLoading}` into Dashboard surface props via `surfaceComponents.tsx`
- [ ] **Step 4:** Run `pnpm test src/components/surfaces/Dashboard/` — PASS
- [ ] **Step 5: Commit**

---

## Task 4: Settings `useVoxMutation` for preferences

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`

- [ ] **Step 1: Failing test** — mock `voxTransport.setGuiPreference`; toggle theme; expect `aria-live` region announces save

```typescript
it('announces preference save via aria-live', async () => {
  render(<SettingsView pushToast={vi.fn()} />);
  await userEvent.click(screen.getByRole('button', { name: /theme/i }));
  expect(await screen.findByRole('status')).toHaveTextContent(/saved/i);
});
```

- [ ] **Step 2:** Create `useGuiPreferenceMutation(key)` in `useVoxQuery.ts` or inline `useMutation` calling `voxTransport.setGuiPreference`
- [ ] **Step 3:** Add `<div aria-live="polite" className="sr-only">` for save confirmation
- [ ] **Step 4:** Run Settings tests — PASS
- [ ] **Step 5: Commit**

---

## Task 5: Playwright dashboard pilot

**Files:**
- Create: `crates/vox-gui/ui/e2e/dashboard-pilot.spec.ts`

- [ ] **Step 1:** Copy Tauri mock from `e2e/dashboard.spec.ts`
- [ ] **Step 2:** Assert `#view=dashboard` in URL after load; click sidebar Console; assert hash updates to `#view=console`
- [ ] **Step 3:** Run `pnpm exec playwright test e2e/dashboard-pilot.spec.ts` (local only unless `VOX_GUI_PLAYWRIGHT=1`)

---

## Exit criteria

- [ ] `useOrchestratorStatus` tested; App uses it for cold start + live updates
- [ ] Dashboard accepts `loading` and renders `<Async>` skeleton
- [ ] Settings preference writes use `useVoxMutation` + aria-live
- [ ] `pnpm test` green; no new raw `invoke` in Settings for get/set preference paths
- [ ] Deep-link e2e: hash round-trip dashboard ↔ console

---

## Out of scope (Wave 6)

- Remaining ~24 Settings `invoke` calls (`get_orchestrator_config`, etc.)
- Full App.tsx TanStack migration for chat/policies/approvals
