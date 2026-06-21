# Dashboard ↔ Operator Console Unification (SP-1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **Target executor:** Gemini 3.5 Flash via the antigravity pipeline (Sonnet 4.6 fallback). See the **Flash Execution Addendum** at the end — read it FIRST.

**Goal:** Fold the Claude-Design "Operator Console" layout into the existing `dashboard` surface as its default widget-grid composition — wired to live data with zero placeholders — and retire the standalone `operator-console` surface, eliminating the duplicate-console split-brain.

**Architecture:** The operator console is NOT a new component — it is the default configuration of the Dashboard's existing widget-grid (`dashboardLayout.ts` + `DashboardGrid`), composed from the app's own primitives (`Glass`, `Kpi`, `AgentRow`, `Pill`). We add one new widget kind (`resources`), upgrade the agents widget with inline approvals, widen the KPI strip to 4 tiles, and set the default layout. Every value binds to real data: KPIs/agents/peers from `DashboardData`, the vector-store count from the existing `get_memory_status` command, and approvals from the existing `vox_pending_approvals` / `vox_resolve_approval` MCP tools.

**Tech Stack:** React 19 + TypeScript + Vite + vitest (jsdom) at `crates/vox-gui/ui`; Tauri 2 Rust host at `crates/vox-gui/src`; YAML contracts under `contracts/gui/`. Run all `pnpm` commands from `crates/vox-gui/ui`.

---

## Why this exists (design decisions, locked in brainstorming)

1. **Component SSOT = the app.** The app's own primitives are authoritative; the `ds/` Claude-Design bundle becomes generated *from* them in a later sub-project (SP-2). This plan therefore builds ONLY with existing app primitives — it introduces no parallel component library.
2. **One console.** The operator console folds into the existing `dashboard` surface (which already has live `DashboardData`, handlers, and tests). The standalone `operator-console` surface added in commit `a72d6a84ec` is retired.
3. **No placeholders.** Every displayed value binds to a real source. Audit finding: the only value without an obvious source (Vector Store doc count) is in fact available from the existing `get_memory_status` command's `corpus_counts` — so **no new Rust backend is required**; a frontend hook suffices.
4. **No feature loss.** The Dashboard's widget grid, charts, Stream feed, alerts, and gamify mini-map remain. The operator console is the *default layout*, not a replacement engine.

## No-split-brain / no-placeholder methodology (the invariants every task upholds)

- **One surface:** after Phase 0, `operator-console` does not exist as a view, registry entry, or component. `grep -r "operator-console" crates/vox-gui/ui/src` returns nothing except inside `Dashboard/` comments.
- **One component per concept:** reuse `Glass`, `Kpi`, `AgentRow`, `Pill`. Do NOT import anything from `crates/vox-gui/ds/` into the app. The `ds/` directory is a publish target, not an app dependency.
- **One layout SSOT:** widget kinds live in `contracts/gui/dashboard-layout.v1.yaml` AND `dashboardLayout.ts`; both are edited together (Task 4) and kept identical.
- **Real data only:** no hardcoded agent/resource arrays in shipped components. Sample data lives ONLY in tests/fixtures. A tile whose fetch fails renders an honest "unavailable" state (the existing `EmptyHint` pattern), never a fabricated number.

## File structure

**Delete (Phase 0):**
- `crates/vox-gui/ui/src/components/surfaces/OperatorConsole/OperatorConsole.tsx`
- `crates/vox-gui/ui/src/components/surfaces/OperatorConsole/OperatorConsole.test.tsx`

**Create:**
- `crates/vox-gui/ui/src/hooks/useMemoryStatus.ts` — fetch `get_memory_status`, expose `{ vectorCount, loading, error }`.
- `crates/vox-gui/ui/src/hooks/useMemoryStatus.test.ts`
- `crates/vox-gui/ui/src/hooks/useAgentApprovals.ts` — fetch/resolve pending approvals, map to agents.
- `crates/vox-gui/ui/src/hooks/useAgentApprovals.test.ts`
- `crates/vox-gui/ui/src/components/surfaces/Dashboard/ResourcesWidget.tsx` — Compute Mesh / Vector Store / Token Budget cards.
- `crates/vox-gui/ui/src/components/surfaces/Dashboard/ResourcesWidget.test.tsx`

**Modify:**
- `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` — remove OperatorConsole import + case.
- `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` — remove `operator-console` entry.
- `contracts/gui/surface-registry.v1.yaml` — remove `operator-console` entry.
- `crates/vox-gui/ui/src/App.tsx` — remove `operator-console` from `View` union + `LEGACY_VIEWS`.
- `contracts/gui/dashboard-layout.v1.yaml` — add `resources` kind + new `default_profile`.
- `crates/vox-gui/ui/src/lib/dashboardLayout.ts` — add `resources` kind + new `defaultDashboardLayout()`.
- `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` — 4-tile KPI strip, `resources` render case, upgraded `agents` render case.
- `crates/vox-gui/ui/src/components/surfaces/Dashboard/AgentRow.tsx` — optional inline Approve/Reject.
- `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx` — update for 4 KPIs + new widgets.

---

## Phase 0 — Retire the standalone Operator Console surface

### Task 0: Delete the surface and its wiring

**Files:**
- Delete: `crates/vox-gui/ui/src/components/surfaces/OperatorConsole/OperatorConsole.tsx`, `.../OperatorConsole.test.tsx`
- Modify: `surfaceComponents.tsx`, `surfaceRegistry.generated.ts`, `contracts/gui/surface-registry.v1.yaml`, `App.tsx`

- [ ] **Step 1: Delete the component + test**

```bash
git rm crates/vox-gui/ui/src/components/surfaces/OperatorConsole/OperatorConsole.tsx \
       crates/vox-gui/ui/src/components/surfaces/OperatorConsole/OperatorConsole.test.tsx
```

- [ ] **Step 2: Remove the renderer import + case** in `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`

Delete the import line:
```ts
import { OperatorConsole } from '../surfaces/OperatorConsole/OperatorConsole';
```
Delete the case (lines around the `childRenderer` switch):
```ts
    case 'operator-console':
      return <OperatorConsole />;
```

- [ ] **Step 3: Remove the registry entry** in `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`

Delete this exact line:
```ts
  { viewKey: 'operator-console', cliGroup: null, tier: 'live_backend', navLabel: 'Operator Console', navIcon: 'dashboard', navGroup: 'operate', parentSurface: 'agents' },
```

- [ ] **Step 4: Remove the YAML source entry** in `contracts/gui/surface-registry.v1.yaml`

Delete the 8-line block beginning `- view_key: operator-console` through its `notes:` line.

- [ ] **Step 5: Remove the View additions** in `crates/vox-gui/ui/src/App.tsx`

Remove `  | 'operator-console'` from the `View` union, and remove `'operator-console', ` from the `LEGACY_VIEWS` array.

- [ ] **Step 6: Verify nothing references it**

Run: `grep -rn "operator-console" crates/vox-gui/ui/src crates/vox-gui/src contracts`
Expected: no matches (the string is fully gone).

- [ ] **Step 7: Typecheck + tests**

Run: `cd crates/vox-gui/ui && pnpm typecheck && pnpm vitest run`
Expected: typecheck clean; full suite green (the 3 OperatorConsole tests are gone; everything else passes).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(vox-gui): retire standalone operator-console surface (folds into dashboard)"
```

---

## Phase 1 — Widen the KPI strip to 4 tiles

### Task 1: Add the Mesh Peers KPI tile

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` (the fixed KPI row, currently 3 `<Kpi>` in a `md:grid-cols-3` block)
- Test: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx`

- [ ] **Step 1: Write the failing test** (append to `Dashboard.test.tsx`)

```tsx
it('renders a 4-tile KPI strip including Mesh Peers', () => {
  renderDashboard({ peers: [
    { id: 'p1', name: 'node-a', backend: 'cuda', online: true },
    { id: 'p2', name: 'node-b', backend: 'cuda', online: false },
  ] });
  expect(screen.getByText('Active Agents')).toBeInTheDocument();
  expect(screen.getByText('Queue Depth')).toBeInTheDocument();
  expect(screen.getByText('Budget Spent')).toBeInTheDocument();
  expect(screen.getByText('Mesh Peers')).toBeInTheDocument();
  // online peers only → 1
  expect(screen.getByText('1')).toBeInTheDocument();
});
```
(Use the file's existing `renderDashboard`/props helper; if none exists, render `<Dashboard {...baseProps} data={{...baseData, peers}} />` with the file's existing `baseProps`/`baseData`.)

- [ ] **Step 2: Run → FAIL**

Run: `pnpm vitest run src/components/surfaces/Dashboard/Dashboard.test.tsx -t "Mesh Peers"`
Expected: FAIL — no "Mesh Peers" text.

- [ ] **Step 3: Implement** — in `Dashboard.tsx`, change the KPI row container from `md:grid-cols-3` to `md:grid-cols-4` and add a 4th tile after "Budget Spent":

```tsx
<div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4 px-5">
  <Kpi label="Active Agents" value={data.agents.length} accent="cyan" />
  <Kpi label="Queue Depth" value={data.kpis.queueDepth.value} accent="amber" />
  <Kpi label="Budget Spent" value={typeof data.kpis.budgetBurn.value === 'number' ? `$${data.kpis.budgetBurn.value.toFixed(2)}` : data.kpis.budgetBurn.value} accent="brass" />
  <Kpi label="Mesh Peers" value={data.peers.filter((p) => p.online).length} accent="emerald" />
</div>
```
(`accent` keys are defined in `ui/Kpi.tsx`: `cyan|amber|emerald|violet|brass|zinc|sky`. `emerald` = verdigris-adjacent, fits the live/secondary accent.)

- [ ] **Step 4: Run → PASS.** `pnpm vitest run src/components/surfaces/Dashboard/Dashboard.test.tsx`

- [ ] **Step 5: Commit** — `git commit -am "feat(vox-gui): dashboard KPI strip → 4 tiles (add Mesh Peers)"`

---

## Phase 2 — Resources widget (real data, no new backend)

### Task 2: `useMemoryStatus` hook (Vector Store count)

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useMemoryStatus.ts`, `.../useMemoryStatus.test.ts`

The Rust command already exists (`crates/vox-gui/src/commands/memory.rs::get_memory_status`) and returns `corpus_counts: { proj, docs, chats, rules, web }` where `proj` = `COUNT(*) FROM search_documents` (the vector/embedding corpus). No backend change.

- [ ] **Step 1: Write the failing test** — `useMemoryStatus.test.ts`

```ts
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { useMemoryStatus } from './useMemoryStatus';

beforeEach(() => invoke.mockReset());

describe('useMemoryStatus', () => {
  it('exposes the vector-store (proj) corpus count', async () => {
    invoke.mockResolvedValue({ corpus_counts: { proj: 12400, docs: 30 }, shards: [], recent_recalls: [], embedding_dim: 1024 });
    const { result } = renderHook(() => useMemoryStatus());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.vectorCount).toBe(12400);
    expect(result.current.error).toBeNull();
  });
  it('reports error and null count when the command rejects', async () => {
    invoke.mockRejectedValue(new Error('No workspace db found'));
    const { result } = renderHook(() => useMemoryStatus());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.vectorCount).toBeNull();
    expect(result.current.error).toBe('No workspace db found');
  });
});
```

- [ ] **Step 2: Run → FAIL** (`pnpm vitest run src/hooks/useMemoryStatus.test.ts`) — module not found.

- [ ] **Step 3: Implement** — `useMemoryStatus.ts`

```ts
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface MemoryStatus {
  corpus_counts: Record<string, number>;
}

export interface UseMemoryStatus {
  /** search_documents (embedding/vector corpus) count, or null when unavailable. */
  vectorCount: number | null;
  loading: boolean;
  error: string | null;
}

export function useMemoryStatus(): UseMemoryStatus {
  const [state, setState] = useState<UseMemoryStatus>({ vectorCount: null, loading: true, error: null });
  useEffect(() => {
    let live = true;
    invoke<MemoryStatus>('get_memory_status')
      .then((s) => { if (live) setState({ vectorCount: s.corpus_counts?.proj ?? 0, loading: false, error: null }); })
      .catch((e) => { if (live) setState({ vectorCount: null, loading: false, error: String(e instanceof Error ? e.message : e) }); });
    return () => { live = false; };
  }, []);
  return state;
}
```

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** — `git commit -am "feat(vox-gui): useMemoryStatus hook (vector-store corpus count)"`

### Task 3: ResourcesWidget component

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Dashboard/ResourcesWidget.tsx`, `.../ResourcesWidget.test.tsx`

- [ ] **Step 1: Write the failing test** — `ResourcesWidget.test.tsx`

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('../../../hooks/useMemoryStatus', () => ({
  useMemoryStatus: () => ({ vectorCount: 12400, loading: false, error: null }),
}));

import { ResourcesWidget } from './ResourcesWidget';
import type { DashboardData } from '../../../types/dashboard';

const data = {
  peers: [{ id: 'p1', name: 'a', backend: 'cuda', online: true }, { id: 'p2', name: 'b', backend: 'cuda', online: false }],
  kpis: { budgetBurn: { label: 'Budget', value: 4.2, cap: 20, spark: [] }, mesh: { label: 'Mesh', value: 1, cap: 0, spark: [] }, queueDepth: { value: 0, spark: [] } },
  agents: [], stream: [], alerts: [], contextChips: [], skills: [],
} as unknown as DashboardData;

describe('ResourcesWidget', () => {
  it('renders compute mesh (online peers), vector store, and token budget from real data', () => {
    render(<ResourcesWidget data={data} />);
    expect(screen.getByText('Compute Mesh')).toBeInTheDocument();
    expect(screen.getByText('1 peer')).toBeInTheDocument();        // 1 online
    expect(screen.getByText('Vector Store')).toBeInTheDocument();
    expect(screen.getByText('12.4k')).toBeInTheDocument();          // 12400 compacted
    expect(screen.getByText('Token Budget')).toBeInTheDocument();
    expect(screen.getByText('$4.20 / $20')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run → FAIL.**

- [ ] **Step 3: Implement** — `ResourcesWidget.tsx` (uses `Glass`; honest "—" when a source is unavailable; section heading underlines per the Limes divider rule)

```tsx
import React from 'react';
import { Glass } from '../../ui/Glass';
import { useMemoryStatus } from '../../../hooks/useMemoryStatus';
import type { DashboardData } from '../../../types/dashboard';

function compact(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

function ResourceCard({ label, value, tone, status, note }: {
  label: string; value: string; tone: string; status: string; note: string;
}) {
  return (
    <Glass size="sm">
      <div className="flex flex-col gap-2.5">
        <div className="flex items-center justify-between gap-2.5">
          <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-secondary">{label}</span>
          <span className="inline-flex items-center gap-1.5 rounded-full bg-overlay-subtle px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider ring-1 ring-border-subtle" style={{ color: tone }}>
            <span className="size-1.5 rounded-full" style={{ background: tone }} />{status}
          </span>
        </div>
        <div className="font-display text-[22px] font-semibold tabular-nums text-text-primary">{value}</div>
        <div className="font-serif text-[13px] italic text-text-muted">{note}</div>
      </div>
    </Glass>
  );
}

export function ResourcesWidget({ data }: { data: DashboardData }) {
  const mem = useMemoryStatus();
  const onlinePeers = data.peers.filter((p) => p.online).length;
  const budget = data.kpis.budgetBurn;
  const budgetValue = typeof budget.value === 'number' ? `$${budget.value.toFixed(2)} / $${budget.cap}` : String(budget.value);
  const overBudget = typeof budget.value === 'number' && budget.cap > 0 && budget.value / budget.cap > 0.5;

  return (
    <Glass className="h-full p-5">
      <div className="mb-3.5 border-b border-border-subtle pb-1.5 font-display text-[10px] uppercase tracking-[0.3em] text-text-muted">Resources</div>
      <div className="grid grid-cols-1 gap-3.5 md:grid-cols-3">
        <ResourceCard label="Compute Mesh" value={`${onlinePeers} ${onlinePeers === 1 ? 'peer' : 'peers'}`}
          tone="var(--color-accent-secondary)" status={onlinePeers > 0 ? 'Live' : 'Offline'} note={onlinePeers > 0 ? 'mesh quorum holding' : 'no peers online'} />
        <ResourceCard label="Vector Store"
          value={mem.loading ? '…' : mem.vectorCount == null ? '—' : compact(mem.vectorCount)}
          tone="var(--color-status-pass)" status={mem.error ? 'Unavailable' : 'Synced'}
          note={mem.error ? 'memory db not reachable' : 'documents indexed'} />
        <ResourceCard label="Token Budget" value={budgetValue}
          tone={overBudget ? 'var(--color-status-warn)' : 'var(--color-accent-secondary)'}
          status={overBudget ? 'Watch' : 'OK'} note={overBudget ? 'burn rate elevated' : 'within budget'} />
      </div>
    </Glass>
  );
}
```

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** — `git commit -am "feat(vox-gui): ResourcesWidget (mesh/vector-store/budget, real data)"`

### Task 4: Register the `resources` widget kind (contract + TS, kept identical)

**Files:**
- Modify: `contracts/gui/dashboard-layout.v1.yaml`, `crates/vox-gui/ui/src/lib/dashboardLayout.ts`, `Dashboard.tsx`

- [ ] **Step 1: Write the failing test** (append to `Dashboard.test.tsx`)

```tsx
it('renders the resources widget when present in the layout', () => {
  // force a layout containing the resources widget
  window.localStorage.setItem('gui.dashboard.layout.v1', JSON.stringify({
    version: 1, columns: 12, widgets: [{ id: 'resources', kind: 'resources', grid: { col: 1, row: 1, w: 12, h: 2 } }],
  }));
  renderDashboard({});
  expect(screen.getByText('Resources')).toBeInTheDocument();
});
```
(Confirm the persistence key against `SHELL_PREFERENCE_KEYS.dashboardLayout` in `lib/shellPersistence.ts`; use that exact key.)

- [ ] **Step 2: Run → FAIL** — `unknown widget kind "resources"` from the layout validator.

- [ ] **Step 3a: Add the kind to the contract** `contracts/gui/dashboard-layout.v1.yaml` — add `  - resources` to the `widget_kinds:` list (after `custom_text`).

- [ ] **Step 3b: Add the kind to TS** `dashboardLayout.ts` — add `'resources',` to the `DASHBOARD_WIDGET_KINDS` array (after `'custom_text'`).

- [ ] **Step 3c: Add the render case** in `Dashboard.tsx` `renderWidget` switch:
```tsx
    case 'resources':
      return <ResourcesWidget data={data} />;
```
and add the import at the top: `import { ResourcesWidget } from './ResourcesWidget';`

- [ ] **Step 4: Run → PASS.** Also run `pnpm vitest run src/lib` to confirm the layout validator tests still pass (the new kind is accepted).

- [ ] **Step 5: Commit** — `git commit -am "feat(vox-gui): register 'resources' dashboard widget kind"`

---

## Phase 3 — Agents widget upgrade: inline Approve/Reject (real approvals)

### Task 5: `useAgentApprovals` hook

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useAgentApprovals.ts`, `.../useAgentApprovals.test.ts`

Reuses the same IPC as `InlineApprovals.tsx`: `invoke('invoke_mcp_tool', { tool: 'vox_pending_approvals', args: {} })` and `invoke('invoke_mcp_tool', { tool: 'vox_resolve_approval', args: { approval_id, outcome } })`, plus the existing `parsePendingApprovals` / `unwrapMcpEnvelope` from `lib/mcpToolResult.ts`. An approval maps to an agent when its `summary` or `tool` contains the agent's `id` or `codename` (documented heuristic — agents without a pending approval get no buttons).

- [ ] **Step 1: Write the failing test** — `useAgentApprovals.test.ts`

```ts
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../lib/mcpToolResult', () => ({
  parsePendingApprovals: (r: any) => r.__rows,
  unwrapMcpEnvelope: (r: any) => r,
}));

import { useAgentApprovals } from './useAgentApprovals';

beforeEach(() => invoke.mockReset());

describe('useAgentApprovals', () => {
  it('maps a pending approval to the agent named in its summary', async () => {
    invoke.mockResolvedValue({ __rows: [{ approval_id: 'ap1', tool: 'shell', summary: 'Atlas wants to run rm' }] });
    const { result } = renderHook(() => useAgentApprovals(['Atlas', 'Surveyor']));
    await waitFor(() => expect(result.current.approvalFor('Atlas')).not.toBeNull());
    expect(result.current.approvalFor('Atlas')!.approval_id).toBe('ap1');
    expect(result.current.approvalFor('Surveyor')).toBeNull();
  });
});
```

- [ ] **Step 2: Run → FAIL.**

- [ ] **Step 3: Implement** — `useAgentApprovals.ts`

```ts
import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { parsePendingApprovals, unwrapMcpEnvelope, type PendingApprovalRow, type McpInvokeResult } from '../lib/mcpToolResult';
import { APPROVALS_POLL_MS } from '../config/constants';

export interface UseAgentApprovals {
  approvalFor: (agentKey: string) => PendingApprovalRow | null;
  resolve: (approvalId: string, outcome: 'approved' | 'rejected') => Promise<void>;
}

export function useAgentApprovals(agentKeys: string[]): UseAgentApprovals {
  const [rows, setRows] = useState<PendingApprovalRow[]>([]);
  const refresh = useCallback(async () => {
    try {
      const res = await invoke<McpInvokeResult>('invoke_mcp_tool', { tool: 'vox_pending_approvals', args: {} });
      setRows(parsePendingApprovals(res));
    } catch { setRows([]); }
  }, []);
  useEffect(() => {
    refresh();
    const id = setInterval(refresh, APPROVALS_POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);
  const approvalFor = useCallback((agentKey: string): PendingApprovalRow | null => {
    const k = agentKey.toLowerCase();
    return rows.find((r) => `${r.summary} ${r.tool}`.toLowerCase().includes(k)) ?? null;
  }, [rows]);
  const resolve = useCallback(async (approvalId: string, outcome: 'approved' | 'rejected') => {
    const res = await invoke<McpInvokeResult>('invoke_mcp_tool', { tool: 'vox_resolve_approval', args: { approval_id: approvalId, outcome } });
    unwrapMcpEnvelope(res.result);
    setRows((prev) => prev.filter((r) => r.approval_id !== approvalId));
    await refresh();
  }, [refresh]);
  return { approvalFor, resolve };
}
```
(Verify the exact exported names `PendingApprovalRow`, `McpInvokeResult`, `parsePendingApprovals`, `unwrapMcpEnvelope` in `lib/mcpToolResult.ts`, and `APPROVALS_POLL_MS` in `config/constants.ts` — both are already imported by `InlineApprovals.tsx`.)

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** — `git commit -am "feat(vox-gui): useAgentApprovals hook (pending approvals → agent)"`

### Task 6: Inline Approve/Reject on AgentRow

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/AgentRow.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Dashboard/AgentRow.test.tsx` (create if absent)

- [ ] **Step 1: Write the failing test**

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AgentRow } from './AgentRow';
import type { Agent } from '../../../types/dashboard';

const agent: Agent = { id: 'A-1', codename: 'Atlas', phase: 'Executing', progress: 0.6, task: 'refactor', cost: 1, budget: null, eta: '2m' };

it('shows Approve/Reject only when a pending approval is supplied, and calls onApprove', async () => {
  const onApprove = vi.fn();
  render(<AgentRow a={agent} onPause={vi.fn()} onResume={vi.fn()} pendingApprovalId="ap1" onApprove={onApprove} onReject={vi.fn()} />);
  await userEvent.click(screen.getByRole('button', { name: 'Approve' }));
  expect(onApprove).toHaveBeenCalledWith('ap1');
});

it('renders no Approve/Reject without a pending approval', () => {
  render(<AgentRow a={agent} onPause={vi.fn()} onResume={vi.fn()} />);
  expect(screen.queryByRole('button', { name: 'Approve' })).toBeNull();
});
```

- [ ] **Step 2: Run → FAIL.**

- [ ] **Step 3: Implement** — extend `AgentRowProps` and render buttons when `pendingApprovalId` is set. Add to the props interface:
```ts
  pendingApprovalId?: string | null;
  onApprove?: (approvalId: string) => void;
  onReject?: (approvalId: string) => void;
```
Add to the destructure and render this block at the end of the row's right-hand controls (inside the existing `<div className="flex items-center gap-3">`):
```tsx
{pendingApprovalId && onApprove && onReject && (
  <div className="flex items-center gap-2">
    <button type="button" onClick={() => onApprove(pendingApprovalId)}
      className="rounded-md border border-brass/30 bg-brass/10 px-2.5 py-1 font-display text-[10px] uppercase tracking-[0.18em] text-brass transition hover:bg-brass/20">Approve</button>
    <button type="button" onClick={() => onReject(pendingApprovalId)}
      className="rounded-md border border-border-subtle px-2.5 py-1 font-display text-[10px] uppercase tracking-[0.18em] text-text-muted transition hover:text-text-secondary">Reject</button>
  </div>
)}
```

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** — `git commit -am "feat(vox-gui): AgentRow optional inline Approve/Reject"`

### Task 7: Wire approvals into the Dashboard agents widget

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`
- Test: `Dashboard.test.tsx`

- [ ] **Step 1: Write the failing test** (mock `useAgentApprovals` to return an approval for one agent; assert exactly one Approve button appears).

```tsx
vi.mock('../../../hooks/useAgentApprovals', () => ({
  useAgentApprovals: () => ({
    approvalFor: (k: string) => (k === 'Atlas' ? { approval_id: 'ap1', tool: 't', summary: 'Atlas' } : null),
    resolve: vi.fn(),
  }),
}));
// ...in a test:
it('surfaces inline approval only for the agent with a pending approval', () => {
  renderDashboard({ agents: [
    { id: 'A-1', codename: 'Atlas', phase: 'Executing', progress: 0.6, task: 't', cost: 1, budget: null, eta: '1m' },
    { id: 'A-2', codename: 'Surveyor', phase: 'Validated', progress: 1, task: 't', cost: 1, budget: null, eta: '0m' },
  ] });
  expect(screen.getAllByRole('button', { name: 'Approve' })).toHaveLength(1);
});
```

- [ ] **Step 2: Run → FAIL.**

- [ ] **Step 3: Implement** — in `Dashboard.tsx`, call the hook near the top of the component:
```tsx
const approvals = useAgentApprovals(data.agents.map((a) => a.codename));
```
and in the `'agents'` widget case, pass approval props to each `AgentRow`:
```tsx
data.agents.map((a) => {
  const ap = approvals.approvalFor(a.codename);
  return (
    <AgentRow key={a.id} a={a} onPause={onPause} onResume={onResume} onOpenInConsole={onOpenInConsole}
      pendingApprovalId={ap?.approval_id ?? null}
      onApprove={(id) => approvals.resolve(id, 'approved')}
      onReject={(id) => approvals.resolve(id, 'rejected')} />
  );
})
```
Add the import: `import { useAgentApprovals } from '../../../hooks/useAgentApprovals';`

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** — `git commit -am "feat(vox-gui): wire live approvals into dashboard agents widget"`

---

## Phase 4 — Operator-console default layout

### Task 8: Make the operator console the default Dashboard layout

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/dashboardLayout.ts` (`defaultDashboardLayout`), `contracts/gui/dashboard-layout.v1.yaml` (`default_profile`)
- Test: `crates/vox-gui/ui/src/lib/dashboardLayout.test.ts` (the file's existing test for the default profile)

- [ ] **Step 1: Write/adjust the failing test** — assert the default layout leads with `resources` then `agents`, and still validates:

```ts
it('default layout is the operator-console composition (resources + agents lead)', () => {
  const l = defaultDashboardLayout();
  expect(validateDashboardLayout(l)).toEqual(l); // stays schema-valid
  expect(l.widgets[0].kind).toBe('resources');
  expect(l.widgets.some((w) => w.kind === 'agents')).toBe(true);
  expect(l.widgets.some((w) => w.kind === 'stream')).toBe(true); // no feature loss
});
```

- [ ] **Step 2: Run → FAIL.**

- [ ] **Step 3: Implement** — new `defaultDashboardLayout()` body (12 cols; resources full-width on top, agents + stream + alerts below — all within `col + w - 1 <= 12`):

```ts
export function defaultDashboardLayout(): DashboardLayout {
  return {
    version: 1,
    columns: 12,
    widgets: [
      { id: 'resources', kind: 'resources', grid: { col: 1, row: 1, w: 12, h: 2 } },
      { id: 'agents', kind: 'agents', grid: { col: 1, row: 3, w: 8, h: 4 } },
      { id: 'alerts', kind: 'alerts', grid: { col: 9, row: 3, w: 4, h: 2 } },
      { id: 'stream', kind: 'stream', grid: { col: 9, row: 5, w: 4, h: 2 } },
    ],
  };
}
```

- [ ] **Step 3b:** Update `contracts/gui/dashboard-layout.v1.yaml` `default_profile.widgets` to the identical 4-widget composition (resources/agents/alerts/stream with the same grids).

- [ ] **Step 4: Run → PASS** (`pnpm vitest run src/lib/dashboardLayout.test.ts`).

- [ ] **Step 5: Commit** — `git commit -am "feat(vox-gui): operator-console default dashboard layout"`

---

## Phase 5 — Full verification

### Task 9: Whole-suite + typecheck + manual data-flow check

- [ ] Run `cd crates/vox-gui/ui && pnpm typecheck` → clean.
- [ ] Run `pnpm vitest run` → all green (no orphaned references to `operator-console`; new widget/hook/approval tests pass).
- [ ] Run `grep -rn "operator-console" crates/vox-gui/ui/src` → no matches (split-brain gone).
- [ ] Confirm no app file imports from `crates/vox-gui/ds/`: `grep -rn "vox-gui/ds\|@vox-axis/limes" crates/vox-gui/ui/src` → no matches.
- [ ] Commit any test fixups: `git commit -am "test(vox-gui): finalize dashboard unification suite"`

---

## Self-review

1. **Spec coverage:** KPI strip (Task 1), Resources widget + real data (Tasks 2–4), agents approvals (Tasks 5–7), default layout (Task 8), surface retirement (Task 0), verification (Task 9). All four locked decisions are covered. ✓
2. **Placeholder scan:** no "TBD"/"add error handling" — each code step shows code; the only non-code instructions are the verification greps and the explicit "verify exported symbol X in file Y" pre-flights (legitimate, named, and bounded — not vague). The ResourcesWidget renders honest `—`/`Unavailable` states, not fabricated numbers. ✓
3. **Type consistency:** `UseMemoryStatus.vectorCount` used identically in hook + widget; `useAgentApprovals` returns `{ approvalFor, resolve }` used identically in Dashboard; `AgentRow` new props `pendingApprovalId/onApprove/onReject` match between definition and call site; `DashboardWidgetKind` adds `'resources'` in both the contract and TS. ✓
4. **No-backend confirmation:** the Vector Store count reuses the existing `get_memory_status` (`corpus_counts.proj`); no new Rust — the "minimal backend" decision is satisfied by an existing source (audit finding). ✓

---

## Flash Execution Addendum (Gemini 3.5 Flash via antigravity pipeline)

Per the repo's antigravity-pipeline conventions and the `gemini-3-5-flash-antigravity-limitations` research, shape execution for Flash:

- **[SEQUENTIAL] Phase 0 must complete and commit before any other phase** — it deletes symbols that later phases must not reference. Do not parallelize across Phase 0.
- **[PARALLEL-SAFE] Tasks 2 and 5** (the two hooks) touch disjoint files and can be built concurrently; **Tasks 3, 4, 6, 7, 8 are [SEQUENTIAL]** because they edit shared files (`Dashboard.tsx`, `dashboardLayout.ts`).
- **Symbol-correctness pre-flight (do this BEFORE coding each task that imports existing symbols):** open and confirm the exact exports before writing the import — `lib/mcpToolResult.ts` (`PendingApprovalRow`, `McpInvokeResult`, `parsePendingApprovals`, `unwrapMcpEnvelope`), `config/constants.ts` (`APPROVALS_POLL_MS`), `lib/shellPersistence.ts` (`SHELL_PREFERENCE_KEYS.dashboardLayout`), `ui/Kpi.tsx` (`accent` keys), and the `Dashboard.test.tsx` existing `baseProps`/`renderDashboard` helper name. Flash hallucinates import names — verifying first is mandatory.
- **One task = one commit.** Do not batch. Each task's tests must be green before its commit. If a test won't pass, STOP and report — do not weaken the assertion.
- **Windows/pnpm:** run all commands from `crates/vox-gui/ui`; never pipe `pnpm`/`cargo` through `head`/`grep` (orphan-process leak); component tests need `// @vitest-environment jsdom` as the first line.
- **Do NOT touch `crates/vox-gui/ds/`** — that is SP-2's territory; importing from it would re-introduce split-brain.
- **No new dashboard-layout CI gate exists**, but keep `contracts/gui/dashboard-layout.v1.yaml` and `dashboardLayout.ts` byte-aligned on the widget-kind list and default profile (Tasks 4 + 8) so SP-2's parity work has a clean base.
