# Sub-Agent Activity Visualization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development`
> (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a first-class, nested, editable, skill-aware, controllable **Sub-Agent
Activity** GUI surface in `vox-gui` — replacing the "one line in a chat" view of sub-agent
work with an expandable tree + live activity + per-node model-relative context editor +
control actions.

**Architecture:** A new `SubAgents` surface (registered as a decorator) composed of focused
React components over a single transport-client interface (`subAgentClient`) and a `zustand`
store. The client is the only seam to the backend; in tests it is mocked exactly like every
other surface (`vi.mock('@tauri-apps/api/core')`). Real Tauri wiring of the assumed commands
(`subagent_tree`, `context_get`, `context_set`, `subagent_control`) is a named backend
follow-on (design §6.2/§8.1, chunks 6–7) — this plan delivers the fully-tested frontend in
isolation.

**Tech Stack:** React 19, TypeScript, `zustand`, `@xyflow/react` v12, `@tanstack/react-virtual`,
`@dnd-kit`, `lucide-react`, `dockview`; tests with `vitest` + `@testing-library/react` + `jsdom`.

**Design SSOT:** `docs/superpowers/specs/2026-06-20-context-window-management-design.md`
(esp. §6.2 model-relative projections + sub-agent handoff, §8.1 sub-agent visualization).

---

## Executor notes (Sonnet 4.6 / Claude Code harness)

- **Branch:** work on a dedicated branch off the current branch; this is a local Claude Code
  run. Do not push or open a PR unless asked.
- **Gates (run from `crates/vox-gui/ui`):** `pnpm test` (vitest), `pnpm typecheck`
  (`tsc --noEmit`). NEVER run `cargo fmt --all`. No Rust changes in this plan.
- **Every component test file starts with `// @vitest-environment jsdom`** and mocks the
  Tauri core: `vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invokeMock(...a) }))`.
  For event listeners, mock `@tauri-apps/api/event` `listen` to capture and replay handlers.
- **No stubs / no placeholders / prove effect** — tests render real components and assert on
  rendered output or client calls, not string shapes.
- After all tasks, use `superpowers:requesting-code-review` before declaring done.

## Flash Execution Addendum (2026-06-20) — and an executor-fit warning

> ⚠️ **Recommended executor for this plan is Sonnet/Claude Code, not Flash.** This is a
> React/TS surface with framework-coupled rendering, a generated-registry SSOT + regen
> command, and a cross-crate backend prerequisite — exactly the cluster of work the
> Antigravity ledger flags as Flash's highest failure rate (GUI/TS gates + open-ended
> integration). If running on **Gemini Flash 3.5**, apply this addendum strictly and split
> the riskiest task; expect to babysit Tasks 8–10.

**Operating Rules (apply to EVERY task, Flash):**
1. **Atomic + green + committed.** No mid-task checkpoint; a kill between tasks must leave a
   compiling, test-passing tree.
2. **Verify before use (BLOCKING).** Each task's pre-flight commands run first; paste output;
   if reality differs from the plan, STOP and report — never invent component/prop/command names.
3. **Two-strike circuit breaker.** A step's gate failing twice → STOP, write a ledger note.
4. **House rules (GUI/TS):** run from `crates/vox-gui/ui`; `pnpm test -- <file>` for one file,
   `pnpm test` + `pnpm typecheck` for the full gate. Every component test starts with
   `// @vitest-environment jsdom` and mocks `@tauri-apps/api/core` (and `@xyflow/react` when a
   real `ReactFlow` is rendered — see `App.test.tsx:37`). No `cargo fmt --all`.
5. **Prove EFFECT not SHAPE:** tests render real components / call real client functions; never
   assert on source-string contents. Route every TSX change through `pnpm typecheck`.
6. **No unplanned shared edits.** Only Task 9 (`decoratorRegistry.ts`) and Task 10
   (`surface-registry.v1.yaml` + the regen command) touch shared files. Report any other.
7. **Branch:** local on-machine run; do NOT push/PR.

**Mandatory global pre-flight (run, paste, confirm before Task 1):**
```
git rev-parse --abbrev-ref HEAD
rg -n "@vitest-environment jsdom" crates/vox-gui/ui/src/App.test.tsx
rg -n "SurfaceDecoratorProps|surfaceDecorators" crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts
rg -n "view_key: mesh" -A6 contracts/gui/surface-registry.v1.yaml
rg -n "AgentEventFrame" crates/vox-gui/ui/src/transport.ts        # confirm NO window_id field (audit #1)
```

**Task-split table (Flash):**
| Task | Touches | Tag |
|---|---|---|
| 1 types · 2 client · 3 store | disjoint new files | **[PARALLEL-SAFE]** |
| 4 tree · 5 editor · 6 controls · 7 stream · 8 graph | disjoint new files (each imports 1–3) | **[PARALLEL-SAFE]** after 1–3 |
| 9 view + decoratorRegistry edit | shared `decoratorRegistry.ts` | **[SEQUENTIAL]** (after 4–8) |
| 10 registry SSOT + regen | shared YAML + generated file | **[SEQUENTIAL]** (after 9) |

If Flash stalls on Task 8 (real `@xyflow/react`) or Task 10 (regen/wiring gate), hand those
two back to Sonnet/Claude Code rather than burning the two-strike budget.

## Codebase-audit corrections (READ FIRST — verified 2026-06-20)

1. **Agent events are NOT window-scoped.** `AgentEventFrame` (`transport.ts:59-63`) has no
   `window_id`/`session_id`; `vox://agent-events` is globally broadcast. **True per-window
   routing needs a Rust daemon change** (add a session/window id to the `AgentEvent` enum in
   `vox-orchestrator` + emit it) — that is a named **backend prerequisite**, not part of this
   frontend plan. Until it lands, the activity stream degrades: events with a `window_id`
   route to that window; events without one route to the **currently-selected** window
   (Task 9). The component tests drive the store directly, so they are unaffected.
2. **A decorator entry does NOT make a surface reachable.** The surface must ALSO exist in
   `SURFACE_REGISTRY`, generated from `contracts/gui/surface-registry.v1.yaml` via
   `vox ci gui-surface-registry --write` (Task 10). `surfaceComponents.tsx` checks
   `surfaceDecorators[viewKey]` first, then a built-in switch, but nav/deep-link needs the
   registry row.
3. **Never hand-edit `surfaceRegistry.generated.ts`** (`// AUTO-GENERATED … DO NOT EDIT`,
   drift-gated by `vox ci gui-surface-registry`). Edit the YAML SSOT and regenerate.

## File Structure (all new, under `crates/vox-gui/ui/src/components/surfaces/SubAgents/`)

- `types.ts` — DTOs + pure helpers (`SubAgentNode`, `ProjectionItem`, `ControlAction`, `flattenTree`, `tokenFate`).
- `subAgentClient.ts` — the transport seam (`fetchTree`, `getContext`, `setContext`, `control`, `listenActivity`).
- `subAgentStore.ts` — `zustand` store: tree + per-node context + merged live events + selection.
- `SubAgentTree.tsx` — nested, virtualized, expandable tree; per-node skill badge + model/token meter + status.
- `SubAgentContextEditor.tsx` — editable committed set for the selected node (pin/remove/reorder + budget).
- `SubAgentControls.tsx` — per-node control actions (pause/resume/overrule/adjust/kill).
- `SubAgentActivityStream.tsx` — live per-node event log incl. retrieval pulls.
- `SubAgentGraph.tsx` — `@xyflow/react` nesting graph view.
- `SubAgentsView.tsx` — surface decorator composing the panes.
- Registration: one line in `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`.

---

### Task 1: Types + pure helpers

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SubAgents/types.ts`
- Test: `crates/vox-gui/ui/src/components/surfaces/SubAgents/types.test.ts`

- [ ] **Step 1: Write the failing test**
```ts
import { describe, it, expect } from 'vitest';
import { flattenTree, tokenFate, type SubAgentNode } from './types';

const tree: SubAgentNode[] = [
  { windowId: 'w1', parentWindowId: null, title: 'root', skill: 'plan', model: { id: 'sonnet', maxTokens: 200000, toolCapable: true }, status: 'running', usedTokens: 1000, depth: 0,
    children: [
      { windowId: 'w2', parentWindowId: 'w1', title: 'child', skill: 'search', model: { id: 'haiku', maxTokens: 8000, toolCapable: true }, status: 'idle', usedTokens: 7000, depth: 1, children: [] },
    ] },
];

describe('SubAgents types', () => {
  it('flattenTree yields depth-ordered rows for virtualization', () => {
    const rows = flattenTree(tree, new Set(['w1']));
    expect(rows.map((r) => r.windowId)).toEqual(['w1', 'w2']);
    expect(rows[1].depth).toBe(1);
  });
  it('flattenTree hides children of collapsed nodes', () => {
    expect(flattenTree(tree, new Set()).map((r) => r.windowId)).toEqual(['w1']);
  });
  it('tokenFate flags a node over its model budget', () => {
    expect(tokenFate(7000, 8000)).toBe('warn');
    expect(tokenFate(8200, 8000)).toBe('over');
    expect(tokenFate(100, 8000)).toBe('ok');
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — Run: `pnpm test -- types.test` Expected: FAIL (module not found).

- [ ] **Step 3: Write minimal implementation**
```ts
export interface ModelProfileLite { id: string; maxTokens: number; toolCapable: boolean; }
export type SubAgentStatus = 'running' | 'idle' | 'paused' | 'blocked' | 'done' | 'failed';
export type ItemFate = 'included' | 'summarized' | 'dropped' | 'on_demand';

export interface SubAgentNode {
  windowId: string;
  parentWindowId: string | null;
  title: string;
  skill: string | null;
  model: ModelProfileLite;
  status: SubAgentStatus;
  usedTokens: number;
  depth: number;
  children: SubAgentNode[];
}

export interface ProjectionItem {
  itemId: string;
  role: string;
  itemKind: string;
  preview: string;
  byteLen: number;
  tokenEstimate: number;
  pinned: boolean;
  fate: ItemFate;
}

export type ControlAction =
  | { kind: 'pause' } | { kind: 'resume' } | { kind: 'kill' }
  | { kind: 'overrule'; note: string }
  | { kind: 'set_budget'; maxTokens: number }
  | { kind: 'set_model'; modelId: string };

export interface FlatRow { windowId: string; depth: number; node: SubAgentNode; hasChildren: boolean; }

export function flattenTree(nodes: SubAgentNode[], expanded: Set<string>): FlatRow[] {
  const out: FlatRow[] = [];
  const walk = (list: SubAgentNode[]) => {
    for (const n of list) {
      out.push({ windowId: n.windowId, depth: n.depth, node: n, hasChildren: n.children.length > 0 });
      if (n.children.length && expanded.has(n.windowId)) walk(n.children);
    }
  };
  walk(nodes);
  return out;
}

export function tokenFate(used: number, max: number): 'ok' | 'warn' | 'over' {
  if (used > max) return 'over';
  if (used >= max * 0.85) return 'warn';
  return 'ok';
}
```

- [ ] **Step 4: Run test to verify it passes** — Run: `pnpm test -- types.test` Expected: PASS.
- [ ] **Step 5: Commit** — `git add crates/vox-gui/ui/src/components/surfaces/SubAgents && git commit -m "feat(gui): sub-agent activity types + tree helpers"`

---

### Task 2: Transport client seam

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SubAgents/subAgentClient.ts`
- Test: `crates/vox-gui/ui/src/components/surfaces/SubAgents/subAgentClient.test.ts`

- [ ] **Step 1: Write the failing test**
```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
import { fetchTree, getContext, setContext, control } from './subAgentClient';

describe('subAgentClient', () => {
  beforeEach(() => invokeMock.mockReset());
  it('fetchTree calls subagent_tree and returns nodes', async () => {
    invokeMock.mockResolvedValue({ is_error: false, result: { nodes: [{ windowId: 'w1', parentWindowId: null, title: 't', skill: null, model: { id: 'm', maxTokens: 1, toolCapable: false }, status: 'idle', usedTokens: 0, depth: 0, children: [] }] } });
    const nodes = await fetchTree();
    expect(invokeMock).toHaveBeenCalledWith('subagent_tree', {});
    expect(nodes[0].windowId).toBe('w1');
  });
  it('setContext sends ordered item ids to context_set', async () => {
    invokeMock.mockResolvedValue({ is_error: false, result: {} });
    await setContext('w2', ['i1', 'i2']);
    expect(invokeMock).toHaveBeenCalledWith('context_set', { windowId: 'w2', orderedItemIds: ['i1', 'i2'] });
  });
  it('control forwards a typed action to subagent_control', async () => {
    invokeMock.mockResolvedValue({ is_error: false, result: {} });
    await control('w2', { kind: 'pause' });
    expect(invokeMock).toHaveBeenCalledWith('subagent_control', { windowId: 'w2', action: { kind: 'pause' } });
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — Run: `pnpm test -- subAgentClient.test` Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**
```ts
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { SubAgentNode, ProjectionItem, ControlAction } from './types';

interface Envelope<T> { is_error: boolean; result: T; }
async function call<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  const env = (await invoke(cmd, args)) as Envelope<T>;
  if (env.is_error) throw new Error(`${cmd} failed`);
  return env.result;
}

export async function fetchTree(): Promise<SubAgentNode[]> {
  return (await call<{ nodes: SubAgentNode[] }>('subagent_tree', {})).nodes;
}
export async function getContext(windowId: string): Promise<ProjectionItem[]> {
  return (await call<{ items: ProjectionItem[] }>('context_get', { windowId })).items;
}
export async function setContext(windowId: string, orderedItemIds: string[]): Promise<void> {
  await call('context_set', { windowId, orderedItemIds });
}
export async function control(windowId: string, action: ControlAction): Promise<void> {
  await call('subagent_control', { windowId, action });
}

/** Subscribe to live agent-events; rejects outside Tauri (caller degrades). */
export const SUBAGENT_ACTIVITY_EVENT = 'vox://agent-events';
export function listenActivity(onEvent: (e: { id: number; timestamp_ms: number; kind: { type: string; [k: string]: unknown } }) => void): Promise<UnlistenFn> {
  return listen(SUBAGENT_ACTIVITY_EVENT, (e) => onEvent(e.payload as never));
}
```

- [ ] **Step 4: Run test to verify it passes** — Run: `pnpm test -- subAgentClient.test` Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gui): sub-agent transport client seam"`

---

### Task 3: Zustand store (tree + selection + live-event merge)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SubAgents/subAgentStore.ts`
- Test: `crates/vox-gui/ui/src/components/surfaces/SubAgents/subAgentStore.test.ts`

- [ ] **Step 1: Write the failing test**
```ts
import { describe, it, expect, beforeEach } from 'vitest';
import { useSubAgentStore } from './subAgentStore';
import type { SubAgentNode } from './types';

const node = (id: string, depth = 0, children: SubAgentNode[] = []): SubAgentNode => ({
  windowId: id, parentWindowId: null, title: id, skill: null,
  model: { id: 'm', maxTokens: 100, toolCapable: true }, status: 'running', usedTokens: 0, depth, children,
});

describe('subAgentStore', () => {
  beforeEach(() => useSubAgentStore.getState().reset());
  it('setTree stores nodes and toggleExpand flips a node', () => {
    useSubAgentStore.getState().setTree([node('w1', 0, [node('w2', 1)])]);
    expect(useSubAgentStore.getState().tree.length).toBe(1);
    useSubAgentStore.getState().toggleExpand('w1');
    expect(useSubAgentStore.getState().expanded.has('w1')).toBe(true);
  });
  it('pushEvent appends to the selected window event log capped at 200', () => {
    const s = useSubAgentStore.getState();
    s.select('w1');
    for (let i = 0; i < 250; i++) s.pushEvent('w1', { id: i, timestamp_ms: i, kind: { type: 'token_streamed' } });
    expect(useSubAgentStore.getState().eventsByWindow['w1'].length).toBe(200);
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — Run: `pnpm test -- subAgentStore.test` Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**
```ts
import { create } from 'zustand';
import type { SubAgentNode } from './types';

export interface ActivityEvent { id: number; timestamp_ms: number; kind: { type: string; [k: string]: unknown }; }

interface SubAgentState {
  tree: SubAgentNode[];
  expanded: Set<string>;
  selectedWindowId: string | null;
  eventsByWindow: Record<string, ActivityEvent[]>;
  setTree: (t: SubAgentNode[]) => void;
  toggleExpand: (id: string) => void;
  select: (id: string) => void;
  pushEvent: (windowId: string, e: ActivityEvent) => void;
  reset: () => void;
}

export const useSubAgentStore = create<SubAgentState>((set) => ({
  tree: [], expanded: new Set(), selectedWindowId: null, eventsByWindow: {},
  setTree: (t) => set({ tree: t }),
  toggleExpand: (id) => set((s) => {
    const next = new Set(s.expanded);
    next.has(id) ? next.delete(id) : next.add(id);
    return { expanded: next };
  }),
  select: (id) => set({ selectedWindowId: id }),
  pushEvent: (windowId, e) => set((s) => {
    const prev = s.eventsByWindow[windowId] ?? [];
    const next = [...prev, e].slice(-200);
    return { eventsByWindow: { ...s.eventsByWindow, [windowId]: next } };
  }),
  reset: () => set({ tree: [], expanded: new Set(), selectedWindowId: null, eventsByWindow: {} }),
}));
```

- [ ] **Step 4: Run test to verify it passes** — Run: `pnpm test -- subAgentStore.test` Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gui): sub-agent zustand store with capped event log"`

---

### Task 4: SubAgentTree (nested, virtualized, skill + model/token meter)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentTree.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentTree.test.tsx`

- [ ] **Step 1: Write the failing test**
```tsx
// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { SubAgentTree } from './SubAgentTree';
import { useSubAgentStore } from './subAgentStore';
import type { SubAgentNode } from './types';

const tree: SubAgentNode[] = [{
  windowId: 'w1', parentWindowId: null, title: 'planner', skill: 'plan',
  model: { id: 'sonnet', maxTokens: 200000, toolCapable: true }, status: 'running', usedTokens: 1000, depth: 0,
  children: [{ windowId: 'w2', parentWindowId: 'w1', title: 'searcher', skill: 'search',
    model: { id: 'haiku', maxTokens: 8000, toolCapable: true }, status: 'idle', usedTokens: 7600, depth: 1, children: [] }],
}];

describe('SubAgentTree', () => {
  beforeEach(() => { useSubAgentStore.getState().reset(); useSubAgentStore.getState().setTree(tree); });
  it('shows the root with its skill badge', () => {
    render(<SubAgentTree />);
    expect(screen.getByText('planner')).toBeDefined();
    expect(screen.getByText('plan')).toBeDefined();
  });
  it('reveals a nested child after expanding', () => {
    render(<SubAgentTree />);
    expect(screen.queryByText('searcher')).toBeNull();
    fireEvent.click(screen.getByLabelText('expand planner'));
    expect(screen.getByText('searcher')).toBeDefined();
  });
  it('marks an over-budget node', () => {
    useSubAgentStore.getState().toggleExpand('w1');
    render(<SubAgentTree />);
    expect(screen.getByTestId('budget-w2').getAttribute('data-fate')).toBe('warn');
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — Run: `pnpm test -- SubAgentTree.test` Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**
```tsx
import React from 'react';
import { ChevronRight, ChevronDown } from 'lucide-react';
import { useSubAgentStore } from './subAgentStore';
import { flattenTree, tokenFate } from './types';

export function SubAgentTree() {
  const tree = useSubAgentStore((s) => s.tree);
  const expanded = useSubAgentStore((s) => s.expanded);
  const selected = useSubAgentStore((s) => s.selectedWindowId);
  const toggleExpand = useSubAgentStore((s) => s.toggleExpand);
  const select = useSubAgentStore((s) => s.select);
  const rows = flattenTree(tree, expanded);

  return (
    <div role="tree" aria-label="Sub-agent activity">
      {rows.map((r) => {
        const fate = tokenFate(r.node.usedTokens, r.node.model.maxTokens);
        return (
          <div role="treeitem" key={r.windowId} aria-selected={selected === r.windowId}
               style={{ paddingLeft: 8 + r.depth * 16, display: 'flex', gap: 6, alignItems: 'center' }}
               onClick={() => select(r.windowId)}>
            {r.hasChildren ? (
              <button aria-label={`expand ${r.node.title}`} onClick={(e) => { e.stopPropagation(); toggleExpand(r.windowId); }}>
                {expanded.has(r.windowId) ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              </button>
            ) : <span style={{ width: 14 }} />}
            <span>{r.node.title}</span>
            {r.node.skill && <span className="pill">{r.node.skill}</span>}
            <span style={{ opacity: 0.6 }}>{r.node.model.id}</span>
            <span data-testid={`budget-${r.windowId}`} data-fate={fate}>
              {r.node.usedTokens}/{r.node.model.maxTokens}
            </span>
            <span data-status={r.node.status} />
          </div>
        );
      })}
    </div>
  );
}
```
  > NOTE: virtualize with `useVirtualList` (`src/hooks/useVirtualList.ts`) only once the tree
  > exceeds ~200 visible rows; the flat-row model above is already virtualization-ready
  > (windowing the `rows` array). Keep the plain map for v1; do not block this task on it.

- [ ] **Step 4: Run test to verify it passes** — Run: `pnpm test -- SubAgentTree.test` Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gui): nested sub-agent tree with skill badge + model/token meter"`

---

### Task 5: SubAgentContextEditor (editable committed set per node)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentContextEditor.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentContextEditor.test.tsx`

- [ ] **Step 1: Write the failing test**
```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
const setContextMock = vi.fn().mockResolvedValue(undefined);
const getContextMock = vi.fn();
vi.mock('./subAgentClient', () => ({
  getContext: (...a: unknown[]) => getContextMock(...a),
  setContext: (...a: unknown[]) => setContextMock(...a),
}));
import { SubAgentContextEditor } from './SubAgentContextEditor';

const items = [
  { itemId: 'i1', role: 'user', itemKind: 'message', preview: 'use RS256', byteLen: 40, tokenEstimate: 10, pinned: true, fate: 'included' },
  { itemId: 'i2', role: 'tool', itemKind: 'tool_call', preview: 'log dump', byteLen: 4000, tokenEstimate: 1000, pinned: false, fate: 'dropped' },
];

describe('SubAgentContextEditor', () => {
  beforeEach(() => { setContextMock.mockClear(); getContextMock.mockReset(); getContextMock.mockResolvedValue(items); });
  it('renders the committed items for the window', async () => {
    render(<SubAgentContextEditor windowId="w2" maxTokens={8000} />);
    await waitFor(() => expect(screen.getByText('use RS256')).toBeDefined());
  });
  it('removing an item calls setContext without that id', async () => {
    render(<SubAgentContextEditor windowId="w2" maxTokens={8000} />);
    await waitFor(() => expect(screen.getByText('log dump')).toBeDefined());
    fireEvent.click(screen.getByLabelText('remove i2'));
    await waitFor(() => expect(setContextMock).toHaveBeenCalledWith('w2', ['i1']));
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — Run: `pnpm test -- SubAgentContextEditor.test` Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**
```tsx
import React, { useEffect, useState } from 'react';
import { X, Pin } from 'lucide-react';
import { getContext, setContext } from './subAgentClient';
import type { ProjectionItem } from './types';

export function SubAgentContextEditor({ windowId, maxTokens }: { windowId: string; maxTokens: number }) {
  const [items, setItems] = useState<ProjectionItem[]>([]);
  useEffect(() => { let live = true; getContext(windowId).then((r) => { if (live) setItems(r); }).catch(() => {}); return () => { live = false; }; }, [windowId]);

  const used = items.filter((i) => i.fate === 'included' || i.pinned).reduce((a, i) => a + i.tokenEstimate, 0);

  async function persist(next: ProjectionItem[]) {
    setItems(next);
    await setContext(windowId, next.map((i) => i.itemId));
  }
  const remove = (id: string) => persist(items.filter((i) => i.itemId !== id));
  const togglePin = (id: string) => persist(items.map((i) => i.itemId === id ? { ...i, pinned: !i.pinned } : i));

  return (
    <div aria-label={`committed set for ${windowId}`}>
      <div data-testid="budget-bar">{used}/{maxTokens} tok</div>
      <ul>
        {items.map((i) => (
          <li key={i.itemId} data-fate={i.fate} style={{ display: 'flex', gap: 6 }}>
            <button aria-label={`pin ${i.itemId}`} onClick={() => togglePin(i.itemId)}><Pin size={12} /></button>
            <span style={{ opacity: 0.6 }}>{i.role}</span>
            <span>{i.preview}</span>
            <span style={{ opacity: 0.5 }}>{i.tokenEstimate}t</span>
            <button aria-label={`remove ${i.itemId}`} onClick={() => remove(i.itemId)}><X size={12} /></button>
          </li>
        ))}
      </ul>
    </div>
  );
}
```
  > NOTE: drag-reorder via `@dnd-kit/sortable` is a follow-up enhancement; the `setContext`
  > ordered-id contract above already supports reordering. Do not block this task on dnd.

- [ ] **Step 4: Run test to verify it passes** — Run: `pnpm test -- SubAgentContextEditor.test` Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gui): per-node editable committed-set context editor"`

---

### Task 6: SubAgentControls (per-node control actions)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentControls.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentControls.test.tsx`

- [ ] **Step 1: Write the failing test**
```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
const controlMock = vi.fn().mockResolvedValue(undefined);
vi.mock('./subAgentClient', () => ({ control: (...a: unknown[]) => controlMock(...a) }));
import { SubAgentControls } from './SubAgentControls';

describe('SubAgentControls', () => {
  beforeEach(() => controlMock.mockClear());
  it('pause dispatches a pause action for the window', async () => {
    render(<SubAgentControls windowId="w2" status="running" />);
    fireEvent.click(screen.getByLabelText('pause w2'));
    await waitFor(() => expect(controlMock).toHaveBeenCalledWith('w2', { kind: 'pause' }));
  });
  it('overrule sends the typed note', async () => {
    render(<SubAgentControls windowId="w2" status="running" />);
    fireEvent.change(screen.getByLabelText('overrule note'), { target: { value: 'stop, wrong file' } });
    fireEvent.click(screen.getByLabelText('overrule w2'));
    await waitFor(() => expect(controlMock).toHaveBeenCalledWith('w2', { kind: 'overrule', note: 'stop, wrong file' }));
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — Run: `pnpm test -- SubAgentControls.test` Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**
```tsx
import React, { useState } from 'react';
import { control } from './subAgentClient';
import type { SubAgentStatus } from './types';

export function SubAgentControls({ windowId, status }: { windowId: string; status: SubAgentStatus }) {
  const [note, setNote] = useState('');
  return (
    <div role="group" aria-label={`controls for ${windowId}`}>
      {status === 'paused'
        ? <button aria-label={`resume ${windowId}`} onClick={() => control(windowId, { kind: 'resume' })}>Resume</button>
        : <button aria-label={`pause ${windowId}`} onClick={() => control(windowId, { kind: 'pause' })}>Pause</button>}
      <button aria-label={`kill ${windowId}`} onClick={() => control(windowId, { kind: 'kill' })}>Kill</button>
      <input aria-label="overrule note" value={note} onChange={(e) => setNote(e.target.value)} placeholder="overrule…" />
      <button aria-label={`overrule ${windowId}`} onClick={() => control(windowId, { kind: 'overrule', note })}>Overrule</button>
    </div>
  );
}
```
  > NOTE: the real `subagent_control` backend maps `overrule` to the soft-HITL
  > `overrule_task` dispatch + `FeedbackStore` (design §8.1). That wiring is the backend
  > follow-on; this component only needs the typed client call.

- [ ] **Step 4: Run test to verify it passes** — Run: `pnpm test -- SubAgentControls.test` Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gui): per-node sub-agent control actions"`

---

### Task 7: SubAgentActivityStream (live events incl. retrieval pulls)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentActivityStream.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentActivityStream.test.tsx`

- [ ] **Step 1: Write the failing test**
```tsx
// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { SubAgentActivityStream } from './SubAgentActivityStream';
import { useSubAgentStore } from './subAgentStore';

describe('SubAgentActivityStream', () => {
  beforeEach(() => useSubAgentStore.getState().reset());
  it('renders the selected window events, labelling retrieval pulls', () => {
    const s = useSubAgentStore.getState();
    s.pushEvent('w2', { id: 1, timestamp_ms: 1, kind: { type: 'task_started' } });
    s.pushEvent('w2', { id: 2, timestamp_ms: 2, kind: { type: 'context_pull', hash: 'abc', from_window: 'w1' } });
    render(<SubAgentActivityStream windowId="w2" />);
    expect(screen.getByText('task_started')).toBeDefined();
    expect(screen.getByText(/pulled abc from w1/i)).toBeDefined();
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — Run: `pnpm test -- SubAgentActivityStream.test` Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**
```tsx
import React from 'react';
import { useSubAgentStore } from './subAgentStore';

export function SubAgentActivityStream({ windowId }: { windowId: string }) {
  const events = useSubAgentStore((s) => s.eventsByWindow[windowId] ?? []);
  return (
    <ul aria-label={`activity for ${windowId}`} aria-live="polite">
      {events.map((e) => (
        <li key={e.id}>
          {e.kind.type === 'context_pull'
            ? <span>pulled {String(e.kind.hash)} from {String(e.kind.from_window)}</span>
            : <span>{e.kind.type}</span>}
        </li>
      ))}
    </ul>
  );
}
```

- [ ] **Step 4: Run test to verify it passes** — Run: `pnpm test -- SubAgentActivityStream.test` Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gui): live sub-agent activity stream with retrieval-pull markers"`

---

### Task 8: SubAgentGraph (nesting view via @xyflow/react)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentGraph.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentGraph.test.tsx`

- [ ] **Step 1: Write the failing test**
```tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { toFlow } from './SubAgentGraph';
import type { SubAgentNode } from './types';

const tree: SubAgentNode[] = [{
  windowId: 'w1', parentWindowId: null, title: 'root', skill: null,
  model: { id: 'm', maxTokens: 1, toolCapable: true }, status: 'running', usedTokens: 0, depth: 0,
  children: [{ windowId: 'w2', parentWindowId: 'w1', title: 'child', skill: null,
    model: { id: 'm', maxTokens: 1, toolCapable: true }, status: 'idle', usedTokens: 0, depth: 1, children: [] }],
}];

describe('SubAgentGraph.toFlow', () => {
  it('produces a node per window and an edge per parent link', () => {
    const { nodes, edges } = toFlow(tree);
    expect(nodes.map((n) => n.id).sort()).toEqual(['w1', 'w2']);
    expect(edges).toEqual([{ id: 'w1-w2', source: 'w1', target: 'w2' }]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — Run: `pnpm test -- SubAgentGraph.test` Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**
```tsx
import React from 'react';
import { ReactFlow, Background, type Node, type Edge } from '@xyflow/react';
import type { SubAgentNode } from './types';

export function toFlow(tree: SubAgentNode[]): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = [];
  const edges: Edge[] = [];
  const walk = (list: SubAgentNode[]) => {
    for (const n of list) {
      nodes.push({ id: n.windowId, position: { x: n.depth * 200, y: nodes.length * 70 }, data: { label: `${n.title}${n.skill ? ` · ${n.skill}` : ''}` } });
      if (n.parentWindowId) edges.push({ id: `${n.parentWindowId}-${n.windowId}`, source: n.parentWindowId, target: n.windowId });
      walk(n.children);
    }
  };
  walk(tree);
  return { nodes, edges };
}

export function SubAgentGraph({ tree }: { tree: SubAgentNode[] }) {
  const { nodes, edges } = toFlow(tree);
  return (
    <div style={{ height: '100%', minHeight: 240 }}>
      <ReactFlow nodes={nodes} edges={edges} fitView><Background /></ReactFlow>
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes** — Run: `pnpm test -- SubAgentGraph.test` Expected: PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gui): sub-agent nesting graph (xyflow) + pure toFlow mapper"`

---

### Task 9: SubAgentsView + registration

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentsView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`
- Test: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentsView.test.tsx`

- [ ] **Step 1: Write the failing test**
```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
const fetchTreeMock = vi.fn();
vi.mock('./subAgentClient', () => ({
  fetchTree: (...a: unknown[]) => fetchTreeMock(...a),
  getContext: vi.fn().mockResolvedValue([]),
  setContext: vi.fn().mockResolvedValue(undefined),
  control: vi.fn().mockResolvedValue(undefined),
  listenActivity: vi.fn().mockRejectedValue(new Error('not tauri')),
}));
import { SubAgentsView } from './SubAgentsView';

describe('SubAgentsView', () => {
  beforeEach(() => {
    fetchTreeMock.mockReset();
    fetchTreeMock.mockResolvedValue([{ windowId: 'w1', parentWindowId: null, title: 'planner', skill: 'plan',
      model: { id: 'sonnet', maxTokens: 200000, toolCapable: true }, status: 'running', usedTokens: 1000, depth: 0, children: [] }]);
  });
  it('loads the tree and selecting a node shows its context editor', async () => {
    render(<SubAgentsView pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText('planner')).toBeDefined());
    fireEvent.click(screen.getByText('planner'));
    await waitFor(() => expect(screen.getByLabelText('committed set for w1')).toBeDefined());
  });
});
```

- [ ] **Step 2: Run test to verify it fails** — Run: `pnpm test -- SubAgentsView.test` Expected: FAIL.

- [ ] **Step 3: Write minimal implementation** — create `SubAgentsView.tsx`:
```tsx
import React, { useEffect } from 'react';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { useSubAgentStore } from './subAgentStore';
import { fetchTree, listenActivity } from './subAgentClient';
import { SubAgentTree } from './SubAgentTree';
import { SubAgentContextEditor } from './SubAgentContextEditor';
import { SubAgentControls } from './SubAgentControls';
import { SubAgentActivityStream } from './SubAgentActivityStream';
import { flattenTree } from './types';

export function SubAgentsView(_props: SurfaceDecoratorProps) {
  const tree = useSubAgentStore((s) => s.tree);
  const expanded = useSubAgentStore((s) => s.expanded);
  const selected = useSubAgentStore((s) => s.selectedWindowId);
  const setTree = useSubAgentStore((s) => s.setTree);
  const pushEvent = useSubAgentStore((s) => s.pushEvent);

  useEffect(() => { fetchTree().then(setTree).catch(() => {}); }, [setTree]);
  useEffect(() => {
    let un: (() => void) | undefined;
    listenActivity((e) => {
      // Backend does not yet stamp window_id on agent events (audit correction #1):
      // route by window_id when present, else attribute to the selected window.
      const w = (e.kind as { window_id?: string }).window_id
        ?? useSubAgentStore.getState().selectedWindowId;
      if (w) pushEvent(w, e);
    }).then((u) => { un = u; }).catch(() => {});
    return () => un?.();
  }, [pushEvent]);

  const node = selected ? flattenTree(tree, expanded).find((r) => r.windowId === selected)?.node ?? null : null;

  return (
    <div style={{ display: 'flex', gap: 8, height: '100%' }}>
      <div style={{ flex: 1.1, overflow: 'auto' }}><SubAgentTree /></div>
      <div style={{ flex: 1.4, overflow: 'auto' }}>
        {node ? (
          <>
            <SubAgentControls windowId={node.windowId} status={node.status} />
            <SubAgentContextEditor windowId={node.windowId} maxTokens={node.model.maxTokens} />
            <SubAgentActivityStream windowId={node.windowId} />
          </>
        ) : <p>Select a sub-agent</p>}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Register the decorator** — in
  `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`, add the import
  `import { SubAgentsView } from './SubAgents/SubAgentsView';` and add `'sub-agents': SubAgentsView,`
  to the `surfaceDecorators` object.

- [ ] **Step 5: Run test + full gate** — Run: `pnpm test -- SubAgentsView.test && pnpm typecheck`
  Expected: PASS + no type errors.

- [ ] **Step 6: Commit** — `git commit -am "feat(gui): SubAgents surface compose + decorator registration"`

---

### Task 10: Surface-registry SSOT entry + regenerate + final gate

**Files:**
- Modify: `contracts/gui/surface-registry.v1.yaml` (the SSOT — add one entry)
- Regenerate (do NOT hand-edit): `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`

- [ ] **Step 1 (pre-flight, BLOCKING):** confirm the SSOT + the `mesh` template row.
  Run: `rg -n "view_key: mesh" -A6 contracts/gui/surface-registry.v1.yaml`
  Expected: a block with `view_key`, `representation_tier`, `nav_label`, `nav_icon`,
  `nav_group`, `parent_surface`. If the file or that row is absent, STOP and report.

- [ ] **Step 2: Add the `sub-agents` entry to the YAML SSOT**, mirroring `mesh`:
```yaml
- view_key: sub-agents
  cli_group: null
  representation_tier: live_backend
  nav_label: Sub-Agents
  nav_icon: list-tree
  nav_group: compute
  parent_surface: compute
```
  > If `vox ci gui-surface-registry` later fails a *wiring* check (a `live_backend` surface
  > must be reachable in `App.tsx`), the `surfaceDecorators['sub-agents'] = SubAgentsView`
  > entry from Task 9 IS the wiring — confirm that registration landed. Do not change the
  > tier to dodge the gate; report if it still fails after Task 9 is committed.

- [ ] **Step 3: Regenerate the TypeScript registry.**
  Run: `vox ci gui-surface-registry --write`
  Expected: `surfaceRegistry.generated.ts` updated to include `sub-agents`. NEVER edit that
  file by hand.

- [ ] **Step 4: Verify no drift.**
  Run: `vox ci gui-surface-registry`
  Expected: PASS (no drift). If it reports drift, you edited the generated file directly —
  revert and re-run `--write`.

- [ ] **Step 5: Full gate** — from `crates/vox-gui/ui`: `pnpm test && pnpm typecheck`.
  Expected: all green.

- [ ] **Step 6: Commit** — `git commit -am "feat(gui): register sub-agents surface in YAML SSOT + regenerate registry"`

- [ ] **Step 7: Request review** — use `superpowers:requesting-code-review` before declaring done.

---

## Out of scope (explicit follow-ons, do NOT build here)

- Real Tauri backend commands `subagent_tree` / `context_get` / `context_set` /
  `subagent_control` (design chunks 6–7; this plan mocks them).
- `@dnd-kit` drag-reorder polish, `useVirtualList` windowing past ~200 rows, skill-lens
  grouping UI — named enhancements; the data contracts already support them.
- The `ModelProfile`/`TokenEstimator`/projection backend (design §6.1/§6.2).
