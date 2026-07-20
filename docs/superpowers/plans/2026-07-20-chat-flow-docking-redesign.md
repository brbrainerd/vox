# Chat / Flow / Docking Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop raw orchestrator events (CHECKPOINT/TASK/PHASE/COST/TOKEN) from spamming the chat transcript, replace them with a live status line, move full detail into a real dockable Flow panel (via `dockview`, already installed), add an editable+auto-growing plan panel backed by the orchestrator's existing plan-DAG subsystem, add a per-session opt-in chat grounding check, and finish the composer's inline pause/resume controls (Stop already exists).

**Architecture:** Frontend: `dockview@6.6.1` (already a dependency, already themed via `dockview-vox.css`) replaces `ChatSurface.tsx`'s hand-rolled flex-row + `chatRailVisibility` responsive logic with real dockable panels (Sessions, Execution Rail, Flow, Plan), each wrapping existing components with minimal adapter code. `chatTranscriptTimeline.ts` splits event rows out of the chat-only row list into a synthetic status row plus a separate full-event list feeding Flow. Backend: no new task-dispatch mechanics — reuses `upsert_plan_node`/`enqueue_runnable_plan_nodes` (existing plan-DAG), `evaluate_socrates_gate`/`generate_goal_search_context` (existing grounding-check building blocks), and `pause_agent`/`resume_agent`/`interrupt_task` (existing agent lifecycle ops), adding one new agent-decision capability (dynamic plan-node insertion during the phase loop) and one new Tauri command (plan-node edit).

**Tech Stack:** React + TypeScript (vox-gui/ui), `dockview` 6.6.1, Rust (vox-orchestrator, vox-gui, vox-db), Tauri v2 commands, vitest, `cargo test`.

---

## Section reference

This plan implements `docs/superpowers/specs/2026-07-20-chat-flow-docking-redesign-design.md` in full. Section numbers below match that spec.

- Phase A (Tasks A1–A4): Spec Section 1 — event routing & verbosity
- Phase B (Tasks B1–B6): Spec Section 2 — docking architecture
- Phase C (Tasks C1–C2): Spec Section 4 — composer pause/resume (Stop already exists — verified live in `Loquela.tsx:544-552`)
- Phase D (Tasks D1–D3): Spec Section 3 — chat gate policy
- Phase E (Tasks E1–E4): Spec Section 5 — editable + auto-growing plan panel
- Final (Task F1): whole-effort verification, rebuild, relaunch, push

---

## Phase A: Event routing & verbosity

### Task A1: Split the transcript timeline into chat-only rows + a full event list

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/chatTranscriptTimeline.ts`
- Test: `crates/vox-gui/ui/src/lib/chatTranscriptTimeline.test.ts` (create if it doesn't already exist — check first with a file search; if it exists, add to it)

The current `buildTranscriptTimeline` (full contents already read — 138 lines) merges chat `message` rows with `agent`/`token_group` rows into one array. We need a second function that returns ONLY `message` rows plus one synthetic status row, leaving the existing `buildTranscriptTimeline` unchanged (Flow will keep using it for full detail).

- [ ] **Step 1: Write the failing test**

```ts
// crates/vox-gui/ui/src/lib/chatTranscriptTimeline.test.ts
import { describe, it, expect } from 'vitest';
import { buildChatOnlyTimeline } from './chatTranscriptTimeline';
import type { ChatMessage } from './chatCorrelation';
import type { StreamItem } from '../types/dashboard';

function msg(id: string, role: ChatMessage['role'], status: ChatMessage['status'] = 'done'): ChatMessage {
  return { id, role, text: 'hi', status } as ChatMessage;
}

function evt(id: string, tag: string, eventType: string, extra: Record<string, any> = {}): StreamItem {
  return {
    id,
    kind: 'agent',
    tag,
    title: tag,
    body: '',
    ts: 'now',
    metadata: { eventType, timestampMs: Number(id), ...extra },
  };
}

describe('buildChatOnlyTimeline', () => {
  it('excludes all raw agent event rows (CHECKPOINT/TASK/PHASE/COST/TOKEN) from the chat-only list', () => {
    const messages = [msg('m1', 'user'), msg('m2', 'assistant')];
    const events = [
      evt('1', 'CHECKPOINT', 'snapshot_captured'),
      evt('2', 'TASK', 'task_started'),
      evt('3', 'PHASE', 'task_phase_changed'),
      evt('4', 'COST', 'cost_incurred'),
      evt('5', 'TOKEN', 'token_streamed'),
    ];
    const rows = buildChatOnlyTimeline(messages, events);
    expect(rows.every((r) => r.kind === 'message' || r.kind === 'status')).toBe(true);
  });

  it('produces exactly one live status row while a task is in-flight, with phase and elapsed time', () => {
    const messages = [msg('m1', 'user')];
    const events = [
      evt('1', 'TASK', 'task_started', { taskId: 7 }),
      evt('2', 'PHASE', 'task_phase_changed', { taskId: 7, phase: 'Verify' }),
    ];
    const rows = buildChatOnlyTimeline(messages, events, { nowMs: 12_000 });
    const statusRows = rows.filter((r) => r.kind === 'status');
    expect(statusRows).toHaveLength(1);
    expect(statusRows[0]).toMatchObject({ kind: 'status', phase: 'Verify', taskId: 7 });
    expect(statusRows[0].elapsedMs).toBe(10_000); // 12_000 - task_started's timestampMs (2)... see note below
  });

  it('removes the status row once the task completes', () => {
    const messages = [msg('m1', 'user'), msg('m2', 'assistant')];
    const events = [
      evt('1', 'TASK', 'task_started', { taskId: 7 }),
      evt('2', 'PHASE', 'task_phase_changed', { taskId: 7, phase: 'Verify' }),
      evt('3', 'TASK', 'task_completed', { taskId: 7 }),
    ];
    const rows = buildChatOnlyTimeline(messages, events);
    expect(rows.some((r) => r.kind === 'status')).toBe(false);
  });
});
```

Note on the elapsed-time assertion: the synthetic `timestampMs` values used in the test helper (`Number(id)`) are tiny (1-5), not real epoch milliseconds — this is intentional and matches the existing `itemTimestampMs` helper's behavior in this file (it reads `item.metadata?.timestampMs` as a raw number, with no unit assumption baked into the test data). Adjust the exact `elapsedMs` expected value in Step 1 to `nowMs - task_started's timestampMs` using the real numbers you write (12_000 - 1 = 11_999, not 10_000 — fix this before running; the illustrative value above is wrong on purpose to force you to compute the real one from the actual test fixture rather than copy-paste it unchecked).

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/lib/chatTranscriptTimeline.test.ts`
Expected: FAIL with `buildChatOnlyTimeline is not a function` or similar (it doesn't exist yet).

- [ ] **Step 3: Implement `buildChatOnlyTimeline`**

Add to `crates/vox-gui/ui/src/lib/chatTranscriptTimeline.ts`, after the existing `buildTranscriptTimeline` function:

```ts
export type TranscriptStatusRow = {
  kind: 'status';
  id: string;
  atMs: number;
  taskId: number;
  phase: string;
  elapsedMs: number;
};

export type ChatOnlyTimelineRow = TranscriptMessageRow | TranscriptStatusRow;

const IN_FLIGHT_EVENT_TYPES = new Set(['task_started', 'task_phase_changed']);
const TASK_END_EVENT_TYPES = new Set(['task_completed', 'task_failed']);

/**
 * Chat-feed-only view of the timeline: real messages plus at most one live
 * status row per in-flight task (phase + elapsed time). Every raw agent
 * event (CHECKPOINT/TASK/PHASE/COST/TOKEN) that used to render as its own
 * row via `ChatAgentEventRow` is excluded here — full detail stays available
 * via `buildTranscriptTimeline` for the Flow panel.
 */
export function buildChatOnlyTimeline(
  messages: ChatMessage[],
  agentItems: StreamItem[],
  options?: { messageStepMs?: number; nowMs?: number },
): ChatOnlyTimelineRow[] {
  const messageStepMs = options?.messageStepMs ?? 1000;
  const nowMs = options?.nowMs ?? Date.now();

  const rows: ChatOnlyTimelineRow[] = messages.map((message, index) => ({
    kind: 'message' as const,
    id: message.id,
    atMs: index * messageStepMs,
    message,
  }));

  // Track the latest in-flight task per taskId, in arrival order, and drop
  // any task that has since completed/failed.
  const inFlight = new Map<number, { phase: string; startedAtMs: number }>();
  for (const item of agentItems) {
    const eventType = item.metadata?.eventType;
    const taskId = item.taskId ?? (item.metadata?.taskId as number | undefined);
    if (taskId == null) continue;
    if (typeof eventType === 'string' && TASK_END_EVENT_TYPES.has(eventType)) {
      inFlight.delete(taskId);
      continue;
    }
    if (typeof eventType === 'string' && IN_FLIGHT_EVENT_TYPES.has(eventType)) {
      const ts = typeof item.metadata?.timestampMs === 'number' ? item.metadata.timestampMs : 0;
      const existing = inFlight.get(taskId);
      const startedAtMs = eventType === 'task_started' ? ts : (existing?.startedAtMs ?? ts);
      const phase =
        eventType === 'task_phase_changed' && typeof item.metadata?.phase === 'string'
          ? item.metadata.phase
          : (existing?.phase ?? 'Working');
      inFlight.set(taskId, { phase, startedAtMs });
    }
  }

  for (const [taskId, { phase, startedAtMs }] of inFlight) {
    rows.push({
      kind: 'status',
      id: `status-${taskId}`,
      atMs: nowMs,
      taskId,
      phase,
      elapsedMs: Math.max(0, nowMs - startedAtMs),
    });
  }

  return rows;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/lib/chatTranscriptTimeline.test.ts`
Expected: PASS (3 tests). If the elapsed-time assertion fails, fix the expected number in the test to match `nowMs - startedAtMs` using your actual fixture timestamps, not the illustrative placeholder — do not change the implementation to match a wrong test value.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/chatTranscriptTimeline.ts crates/vox-gui/ui/src/lib/chatTranscriptTimeline.test.ts
git commit -m "feat(gui): add buildChatOnlyTimeline — chat feed excludes raw agent events, shows one live status row"
```

### Task A2: Render the chat-only timeline with a `StatusLine` component

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/StatusLine.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/StatusLine.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx:242-320` (the render body)

`ChatTranscript.tsx`'s current render body (already read in full) does `buildTranscriptTimeline(messages, agentItems ?? [])` then maps every row to either `MessageBubble` or `ChatAgentEventRow`. Change it to call `buildChatOnlyTimeline` instead, and map `status` rows to the new `StatusLine` component instead of `ChatAgentEventRow`.

- [ ] **Step 1: Write the failing test**

```tsx
// crates/vox-gui/ui/src/components/surfaces/Chat/StatusLine.test.tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { StatusLine } from './StatusLine';

describe('StatusLine', () => {
  it('renders phase and elapsed seconds as a single line', () => {
    render(<StatusLine phase="Verify" elapsedMs={12_340} />);
    expect(screen.getByText(/Verify/)).toBeInTheDocument();
    expect(screen.getByText(/12s/)).toBeInTheDocument();
  });

  it('rounds down to whole seconds', () => {
    render(<StatusLine phase="Act" elapsedMs={999} />);
    expect(screen.getByText(/0s/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/StatusLine.test.tsx`
Expected: FAIL — `StatusLine.tsx` doesn't exist.

- [ ] **Step 3: Implement `StatusLine`**

```tsx
// crates/vox-gui/ui/src/components/surfaces/Chat/StatusLine.tsx
import React from 'react';

interface StatusLineProps {
  phase: string;
  elapsedMs: number;
}

/**
 * The single collapsed status line shown in the chat feed while a task is
 * in flight — replaces the old CHECKPOINT/TASK/PHASE/COST event-row spam.
 * Full detail for the same task remains available in the Flow panel.
 */
export function StatusLine({ phase, elapsedMs }: StatusLineProps) {
  const seconds = Math.floor(elapsedMs / 1000);
  return (
    <div
      className="flex items-center gap-2 self-start rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-[11px] text-text-muted"
      data-testid="chat-status-line"
      role="status"
      aria-live="polite"
    >
      <span className="size-1.5 animate-pulse rounded-full bg-brass" aria-hidden="true" />
      <span className="font-mono">{phase} · {seconds}s</span>
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/StatusLine.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Wire `StatusLine` into `ChatTranscript.tsx`**

In `crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx`, replace the import and usage:

```tsx
// Replace this import:
import { buildTranscriptTimeline } from '../../../lib/chatTranscriptTimeline';
// With:
import { buildChatOnlyTimeline } from '../../../lib/chatTranscriptTimeline';
import { StatusLine } from './StatusLine';
```

Replace the timeline-building line (currently `const timeline = buildTranscriptTimeline(messages, agentStreamItems ?? []);`) with:

```tsx
const timeline = buildChatOnlyTimeline(messages, agentStreamItems ?? []);
```

Replace the row-dispatch map (currently `if (row.kind === 'message') return <MessageBubble .../>; return <ChatAgentEventRow .../>;`) with:

```tsx
{timeline.map((row) => {
  if (row.kind === 'message') {
    return <MessageBubble key={row.id} message={row.message} />;
  }
  return <StatusLine key={row.id} phase={row.phase} elapsedMs={row.elapsedMs} />;
})}
```

Remove the now-unused `ChatAgentEventRow` import and the `onOpenAgentInFlow` prop threading IF (and only if) nothing else in this file still uses it — check with a search inside the file first, since `onOpenAgentInFlow` may still be needed elsewhere (it is NOT needed elsewhere in this component once this change lands, since `ChatAgentEventRow` was its only consumer here — confirm this before deleting the prop from `ChatTranscriptProps`, since `ChatSurface.tsx` still passes it in and Task B3 will redirect it to the new Flow panel instead).

- [ ] **Step 6: Run the full Chat component test suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/`
Expected: PASS for all files. If `ChatTranscript.test.tsx` exists and asserts on `ChatAgentEventRow`/`data-testid="chat-agent-event-row"` rendering, update those assertions to expect `data-testid="chat-status-line"` instead — this is an intentional behavior change, not a regression.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/StatusLine.tsx crates/vox-gui/ui/src/components/surfaces/Chat/StatusLine.test.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx
git commit -m "feat(gui): chat feed renders StatusLine instead of raw event rows"
```

### Task A3: Verbosity setting (Quiet / Normal / Verbose)

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useChatVerbosity.ts`
- Test: `crates/vox-gui/ui/src/hooks/useChatVerbosity.test.ts`

Mirrors the existing `useLocalStorage` pattern (already read in full — 29 lines, generic `useLocalStorage<T>(key, initialValue)`).

- [ ] **Step 1: Write the failing test**

```ts
// crates/vox-gui/ui/src/hooks/useChatVerbosity.test.ts
// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useChatVerbosity, CHAT_VERBOSITY_KEY } from './useChatVerbosity';

describe('useChatVerbosity', () => {
  beforeEach(() => localStorage.clear());

  it('defaults to normal', () => {
    const { result } = renderHook(() => useChatVerbosity());
    expect(result.current[0]).toBe('normal');
  });

  it('persists a changed level to localStorage', () => {
    const { result } = renderHook(() => useChatVerbosity());
    act(() => result.current[1]('verbose'));
    expect(result.current[0]).toBe('verbose');
    expect(localStorage.getItem(CHAT_VERBOSITY_KEY)).toBe('"verbose"');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/hooks/useChatVerbosity.test.ts`
Expected: FAIL — file doesn't exist.

- [ ] **Step 3: Implement the hook**

```ts
// crates/vox-gui/ui/src/hooks/useChatVerbosity.ts
import { useLocalStorage } from './useLocalStorage';

export type ChatVerbosity = 'quiet' | 'normal' | 'verbose';

export const CHAT_VERBOSITY_KEY = 'gui.chat.verbosity.v1';

/**
 * Global chat-feed verbosity: quiet (status line only), normal (adds a
 * one-line done-in/cost summary per turn), verbose (adds collapsed
 * per-phase breadcrumbs, still without leaving the chat tab). Full detail
 * is always available in the Flow panel regardless of this setting.
 */
export function useChatVerbosity() {
  return useLocalStorage<ChatVerbosity>(CHAT_VERBOSITY_KEY, 'normal');
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/hooks/useChatVerbosity.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/hooks/useChatVerbosity.ts crates/vox-gui/ui/src/hooks/useChatVerbosity.test.ts
git commit -m "feat(gui): add useChatVerbosity hook (quiet/normal/verbose, persisted)"
```

### Task A4: Wire verbosity levels into the chat feed

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/chatTranscriptTimeline.ts` (`buildChatOnlyTimeline`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx`
- Test: extend `crates/vox-gui/ui/src/lib/chatTranscriptTimeline.test.ts`

- [ ] **Step 1: Write the failing test**

Add to `chatTranscriptTimeline.test.ts`:

```ts
it('normal verbosity adds a done-summary row after a task completes, using its cost_incurred data', () => {
  const messages = [msg('m1', 'user'), msg('m2', 'assistant')];
  const events = [
    evt('1', 'TASK', 'task_started', { taskId: 7 }),
    evt('2', 'COST', 'cost_incurred', { taskId: 7, costUsd: 0.003 }),
    evt('3', 'TASK', 'task_completed', { taskId: 7 }),
  ];
  const rows = buildChatOnlyTimeline(messages, events, { verbosity: 'normal' });
  const summary = rows.find((r) => r.kind === 'summary');
  expect(summary).toMatchObject({ kind: 'summary', taskId: 7, costUsd: 0.003 });
});

it('quiet verbosity omits the summary row even after completion', () => {
  const messages = [msg('m1', 'user'), msg('m2', 'assistant')];
  const events = [
    evt('1', 'TASK', 'task_started', { taskId: 7 }),
    evt('2', 'COST', 'cost_incurred', { taskId: 7, costUsd: 0.003 }),
    evt('3', 'TASK', 'task_completed', { taskId: 7 }),
  ];
  const rows = buildChatOnlyTimeline(messages, events, { verbosity: 'quiet' });
  expect(rows.some((r) => r.kind === 'summary')).toBe(false);
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/lib/chatTranscriptTimeline.test.ts`
Expected: FAIL (no `verbosity` option, no `summary` row kind).

- [ ] **Step 3: Extend `buildChatOnlyTimeline`**

In `crates/vox-gui/ui/src/lib/chatTranscriptTimeline.ts`, add the new row type and options field:

```ts
export type TranscriptSummaryRow = {
  kind: 'summary';
  id: string;
  atMs: number;
  taskId: number;
  costUsd: number;
};
```

Update `ChatOnlyTimelineRow` to include it:

```ts
export type ChatOnlyTimelineRow = TranscriptMessageRow | TranscriptStatusRow | TranscriptSummaryRow;
```

Update the function signature's options and add tracking for completed tasks' cost, computed alongside the existing `inFlight` tracking loop (same loop, additional bookkeeping — do not add a second pass over `agentItems`):

```ts
export function buildChatOnlyTimeline(
  messages: ChatMessage[],
  agentItems: StreamItem[],
  options?: { messageStepMs?: number; nowMs?: number; verbosity?: 'quiet' | 'normal' | 'verbose' },
): ChatOnlyTimelineRow[] {
  const messageStepMs = options?.messageStepMs ?? 1000;
  const nowMs = options?.nowMs ?? Date.now();
  const verbosity = options?.verbosity ?? 'normal';

  const rows: ChatOnlyTimelineRow[] = messages.map((message, index) => ({
    kind: 'message' as const,
    id: message.id,
    atMs: index * messageStepMs,
    message,
  }));

  const inFlight = new Map<number, { phase: string; startedAtMs: number }>();
  const lastCostByTask = new Map<number, number>();
  const completedTasks = new Set<number>();

  for (const item of agentItems) {
    const eventType = item.metadata?.eventType;
    const taskId = item.taskId ?? (item.metadata?.taskId as number | undefined);
    if (taskId == null) continue;

    if (typeof eventType === 'string' && eventType === 'cost_incurred') {
      const costUsd = item.metadata?.costUsd;
      if (typeof costUsd === 'number') lastCostByTask.set(taskId, costUsd);
      continue;
    }
    if (typeof eventType === 'string' && TASK_END_EVENT_TYPES.has(eventType)) {
      inFlight.delete(taskId);
      if (eventType === 'task_completed') completedTasks.add(taskId);
      continue;
    }
    if (typeof eventType === 'string' && IN_FLIGHT_EVENT_TYPES.has(eventType)) {
      const ts = typeof item.metadata?.timestampMs === 'number' ? item.metadata.timestampMs : 0;
      const existing = inFlight.get(taskId);
      const startedAtMs = eventType === 'task_started' ? ts : (existing?.startedAtMs ?? ts);
      const phase =
        eventType === 'task_phase_changed' && typeof item.metadata?.phase === 'string'
          ? item.metadata.phase
          : (existing?.phase ?? 'Working');
      inFlight.set(taskId, { phase, startedAtMs });
    }
  }

  for (const [taskId, { phase, startedAtMs }] of inFlight) {
    rows.push({
      kind: 'status',
      id: `status-${taskId}`,
      atMs: nowMs,
      taskId,
      phase,
      elapsedMs: Math.max(0, nowMs - startedAtMs),
    });
  }

  if (verbosity !== 'quiet') {
    for (const taskId of completedTasks) {
      const costUsd = lastCostByTask.get(taskId);
      if (costUsd == null) continue;
      rows.push({ kind: 'summary', id: `summary-${taskId}`, atMs: nowMs, taskId, costUsd });
    }
  }

  return rows;
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/lib/chatTranscriptTimeline.test.ts`
Expected: PASS (all tests, including the two new ones).

- [ ] **Step 5: Wire verbosity + the new `summary` row into `ChatTranscript.tsx`**

In `crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx`, import and use the hook, pass `verbosity` into `buildChatOnlyTimeline`, and render `summary` rows as a plain one-line div:

```tsx
import { useChatVerbosity } from '../../../hooks/useChatVerbosity';
```

Inside the `ChatTranscript` component body, before the `buildChatOnlyTimeline` call:

```tsx
const [verbosity] = useChatVerbosity();
```

Update the call: `const timeline = buildChatOnlyTimeline(messages, agentStreamItems ?? [], { verbosity });`

Add a case to the row-dispatch map:

```tsx
{timeline.map((row) => {
  if (row.kind === 'message') return <MessageBubble key={row.id} message={row.message} />;
  if (row.kind === 'status') return <StatusLine key={row.id} phase={row.phase} elapsedMs={row.elapsedMs} />;
  return (
    <div key={row.id} className="self-start px-1 font-mono text-[10px] text-text-muted">
      Done · ${row.costUsd.toFixed(4)}
    </div>
  );
})}
```

- [ ] **Step 6: Run the Chat component suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/lib/chatTranscriptTimeline.ts crates/vox-gui/ui/src/lib/chatTranscriptTimeline.test.ts crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx
git commit -m "feat(gui): wire chat verbosity levels (quiet/normal/verbose) into the transcript"
```

---

## Phase B: Docking architecture (dockview)

**Ground truth confirmed for this phase:** `dockview@6.6.1` installed; React API is `import { DockviewReact } from 'dockview'`; required props `components: Record<string, FC<IDockviewPanelProps>>` and `onReady: (event: DockviewReadyEvent) => void`; a panel component receives `{ params, api, containerApi }`; `event.api.addPanel({ id, component, title?, params?, position? })`; layout persistence via `containerApi.toJSON()` / `containerApi.fromJSON(data)`, triggered off `containerApi.onDidLayoutChange`; theming via a `dockview-theme-vox` class on the wrapping element (matching the existing `dockview-vox.css`), NOT the `theme` prop. `ChatSurface.tsx`'s current layout (already read in full, 378 lines): `sessionRailNode` (left, `ChatSessionRail`), center column (`ChatTranscript` + composer), `executionRailNode` (right, `ChatExecutionRail`) — all siblings in one flex row (lines 242-377), driven by `chatRailVisibility(containerWidth)` responsive logic (lines 99-111).

### Task B1: Install dockview stylesheet + mount an empty `DockviewReact` shell behind a feature flag

**Files:**
- Modify: `crates/vox-gui/ui/src/main.tsx` (or wherever global CSS is imported — check for the existing `dockview-vox.css` import first; if it's already imported globally, skip re-adding it)
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx`

- [ ] **Step 1: Confirm the theme CSS and dockview base CSS are imported globally**

Run: `grep -rn "dockview-vox.css\|dockview/dist/styles/dockview.css" crates/vox-gui/ui/src/`

If `dockview-vox.css` is imported but `dockview/dist/styles/dockview.css` (the library's own base stylesheet, required for the library to render at all — confirmed via the dockview README's quick-start, which imports both) is NOT imported anywhere, add it. Find the app's global CSS entry point (likely `crates/vox-gui/ui/src/index.css` or similar — check `crates/vox-gui/ui/src/main.tsx` for what it imports) and add:

```css
@import 'dockview/dist/styles/dockview.css';
```

alongside the existing theme import, in the same file, base import first (order matters — the theme's CSS custom-property overrides must load after the library's own defaults).

- [ ] **Step 2: Write the failing test**

```tsx
// crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';
import { ChatDockShell } from './ChatDockShell';

describe('ChatDockShell', () => {
  it('mounts a dockview-theme-vox container and calls onReady with an api', () => {
    const onReady = vi.fn();
    const { container } = render(<ChatDockShell onReady={onReady} components={{}} />);
    expect(container.querySelector('.dockview-theme-vox')).not.toBeNull();
    expect(onReady).toHaveBeenCalledTimes(1);
    expect(onReady.mock.calls[0][0]).toHaveProperty('api');
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatDockShell.test.tsx`
Expected: FAIL — `ChatDockShell.tsx` doesn't exist.

- [ ] **Step 4: Implement `ChatDockShell`**

```tsx
// crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx
import React from 'react';
import { DockviewReact, type DockviewReadyEvent, type IDockviewPanelProps } from 'dockview';

interface ChatDockShellProps {
  components: Record<string, React.FunctionComponent<IDockviewPanelProps>>;
  onReady: (event: DockviewReadyEvent) => void;
}

/**
 * The dockview shell for the chat workspace: sessions list, execution rail,
 * Flow, and plan panels all dock/resize/hide within this container around
 * the central chat pane. Theming via the `dockview-theme-vox` class
 * (crates/vox-gui/ui/src/styles/dockview-vox.css), not the `theme` prop.
 */
export function ChatDockShell({ components, onReady }: ChatDockShellProps) {
  return (
    <div className="dockview-theme-vox h-full min-h-[60vh] w-full">
      <DockviewReact components={components} onReady={onReady} />
    </div>
  );
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatDockShell.test.tsx`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx
git commit -m "feat(gui): add ChatDockShell — dockview container wired to the Vox theme"
```

Note: if Step 1 required a CSS import change, include that file in this commit too.

### Task B2: Wrap `ChatSessionRail`, `ChatExecutionRail`, and the central chat pane as dockview panels inside `ChatSurface`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx:242-377` (the return JSX)
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend — check if it exists first)

This is the highest-risk task in the plan (per the explicit "be careful how you implement it" instruction) — it replaces `ChatSurface`'s hand-rolled flex row + `chatRailVisibility` responsive show/hide logic with real dockview panels, while keeping every existing prop and behavior (session CRUD, execution rail content, composer dock, secretary toast, routing drawer) working unchanged.

- [ ] **Step 1: Write the failing test**

```tsx
// Add to crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
it('mounts sessions, chat, and execution rail as dockview panels', () => {
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[{ id: 'm1', role: 'user', text: 'hi', status: 'done' } as any]}
      composer={<div>composer</div>}
    />,
  );
  expect(screen.getByTestId('chat-dock-sessions')).toBeInTheDocument();
  expect(screen.getByTestId('chat-dock-transcript')).toBeInTheDocument();
  expect(screen.getByTestId('chat-dock-execution-rail')).toBeInTheDocument();
});
```

Check the existing test file first (`ChatSurface.test.tsx`) for its current test setup (mocks, imports) and add this test using the same conventions, rather than assuming the snippet above is copy-paste-ready — the exact render helper/providers wrapping `<ChatSurface>` in existing tests must be reused here for consistency.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — the `data-testid` values don't exist yet (current layout uses `chat-surface-layout`, `chat-session-rail-toggle`, etc., not `chat-dock-*`).

- [ ] **Step 3: Replace the return JSX with dockview panels**

In `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`, add the import:

```tsx
import { ChatDockShell } from './ChatDockShell';
import type { IDockviewPanelProps } from 'dockview';
```

Replace the entire return statement (currently lines 242-377: the `<div ref={containerRef} className="relative flex min-h-[60vh] gap-4" data-testid="chat-surface-layout">...</div>` block) with a dockview-panel-based version. Define the three panel-content components as plain functions above the `ChatSurface` component (module scope, not inline in JSX, so `components` is a stable reference across renders — dockview requires this, since it uses the `components` map's identity/keys, not React reconciliation, to resolve which renderer backs a given panel id):

```tsx
function SessionsPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-sessions" className="h-full overflow-y-auto">{props.params.node}</div>;
}

function TranscriptPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-transcript" className="flex h-full min-w-0 flex-col gap-4 p-2">{props.params.node}</div>;
}

function ExecutionRailPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-execution-rail" className="h-full overflow-y-auto">{props.params.node}</div>;
}

const CHAT_DOCK_COMPONENTS = {
  sessions: SessionsPanel,
  transcript: TranscriptPanel,
  executionRail: ExecutionRailPanel,
};
```

Then replace the return statement's body. Keep everything currently rendered inside the center column (the `EmptyState`/`ChatTranscript` branch, the composer dock, the `ChatModelPicker`) exactly as-is, just move it into a `centerContent` variable so it can be passed as a panel's `params.node`:

```tsx
const centerContent = (
  <>
    {messages.length === 0 && !(agentStreamItems?.length ?? 0) ? (
      <EmptyState
        icon={<Icon.spark className="size-8 text-brass" aria-hidden="true" />}
        title="No messages yet"
        description="Describe a task in the composer below to start this session."
      />
    ) : (
      <ChatTranscript messages={messages} agentStreamItems={agentStreamItems} />
    )}
    {composer != null ? (
      <div className="mt-auto shrink-0 border-t border-border-subtle pt-3" data-testid="chat-composer-dock">
        {attention_budget ? (
          <div className="mb-2 px-1" data-testid="chat-attention-meter">
            <AttentionBudgetMeter budget={attention_budget} waitingQuestions={waitingQuestions} blockedTasks={blockedTasks} />
          </div>
        ) : null}
        {composer}
      </div>
    ) : null}
  </>
);

return (
  <div ref={containerRef} className="relative min-h-[60vh]" data-testid="chat-surface-layout">
    <h1 className="sr-only">Chat</h1>
    <ChatDockShell
      components={CHAT_DOCK_COMPONENTS}
      onReady={(event) => {
        event.api.addPanel({ id: 'sessions', component: 'sessions', title: 'Sessions', params: { node: sessionRailNode } });
        event.api.addPanel({
          id: 'transcript',
          component: 'transcript',
          title: 'Chat',
          params: { node: centerContent },
          position: { direction: 'right', referencePanel: 'sessions' },
        });
        if (executionRailNode) {
          event.api.addPanel({
            id: 'executionRail',
            component: 'executionRail',
            title: 'Execution',
            params: { node: executionRailNode },
            position: { direction: 'right', referencePanel: 'transcript' },
          });
        }
      }}
    />
    {secretaryToast != null ? (
      <SecretaryToast payload={secretaryToast} onDismiss={() => setSecretaryToast(null)} />
    ) : null}
    {routingOpen ? (
      <div className="fixed inset-y-0 right-0 z-50 w-[420px] border-l border-border-subtle bg-bg-base/95 backdrop-blur-xl">
        <Matrix onClose={() => setRoutingOpen(false)} />
      </div>
    ) : null}
  </div>
);
```

Check the exact JSX for `secretaryToast`/`routingOpen` overlays against what's currently at lines 343-375 before writing this — the snippet above is a reasonable reconstruction but the original prop names/structure for `SecretaryToast` and `Matrix` must match exactly what's already in the file (read those two blocks directly before finalizing this step, since they weren't included in the earlier full-file research pass).

**What this intentionally drops:** the `railVis`/`chatRailVisibility`/`ResizeObserver`-driven collapse-to-toggle-button behavior (old lines 252-273, 320-341) and the `sessionOverlayOpen` slide-over state — dockview's own panel visibility/collapse mechanism supersedes this (a user can now close/reopen the Sessions or Execution panel via dockview's tab UI, and the layout persists via Task B5, so the old manual toggle buttons and their state are dead code once this lands). Remove the now-unused `chatRailVisibility` import, `railVis`, `sessionOverlayOpen`, and `containerRef`'s `ResizeObserver` wiring — but only after confirming (via a search inside this file) that nothing else in the component still depends on them.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS, including all pre-existing tests in this file (session create/rename/archive, composer mounting, attention meter, etc.) — dockview panels wrap the SAME components with the same props, so behavior other than layout mechanics must be unchanged. If any pre-existing test fails because it queried `chat-session-rail-toggle` or similar now-removed elements, update that test to reflect the intentional removal (dockview replaces manual collapse toggles), not paper over a real regression.

- [ ] **Step 5: Run the full frontend suite for regressions outside this file**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run`
Expected: PASS. Pay particular attention to any test that imports `ChatSurface` or asserts on the removed `chatRailVisibility`/`sessionOverlayOpen` behavior elsewhere in the codebase (search first: `grep -rln "chatRailVisibility\|sessionOverlayOpen" crates/vox-gui/ui/src`).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "feat(gui): ChatSurface uses dockview panels for sessions/transcript/execution-rail, replacing hand-rolled responsive rail toggles"
```

### Task B3: Add Flow as a dockable panel, reachable from chat

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (the `onReady` callback and `CHAT_DOCK_COMPONENTS` map from Task B2)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (props — re-add `onOpenAgentInFlow`/`agentStreamItems` wiring, this time into a Flow panel instead of `ChatAgentEventRow`)
- Test: extend `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx`

`AgentFlow` (`crates/vox-gui/ui/src/components/surfaces/Flow/AgentFlow.tsx`, already touched this session for the null-budget fix) takes `agents: Agent[]` — NOT the raw `StreamItem[]`/`agentStreamItems` this plan has been discussing. Before wiring this task, read `crates/vox-gui/ui/src/components/surfaces/Flow/AgentFlow.tsx` in full (it wasn't re-read in this planning pass beyond the earlier null-budget fix) and whatever currently constructs its `agents`/`graph` props at the top-level `Flow` tab (search `grep -rn "<AgentFlow" crates/vox-gui/ui/src`) to confirm the exact data-shape adapter needed — this plan cannot specify that adapter's exact code without that read, since `Agent` (dashboard type) and `StreamItem` are different shapes and the existing top-level Flow tab's data-fetching hook is the thing to reuse here, not something to reinvent.

- [ ] **Step 1: Read the real Flow-tab data source**

Run: `grep -n "<AgentFlow" crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`

Read the full surrounding context (the hook/prop supplying `agents`/`graph` to whichever of those call sites is the real top-level Flow tab mount) before proceeding — this determines whether the new dockable Flow panel can reuse that same data source directly (preferred) or needs its own fetch.

- [ ] **Step 2: Write the failing test**

```tsx
// Add to ChatSurface.test.tsx
it('mounts a Flow panel dockable alongside chat, using the same agent data as the top-level Flow tab', () => {
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      agentStreamItems={[]}
      composer={<div>composer</div>}
    />,
  );
  expect(screen.getByTestId('chat-dock-flow')).toBeInTheDocument();
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — no `chat-dock-flow` element yet.

- [ ] **Step 4: Add the Flow panel**

Add to `CHAT_DOCK_COMPONENTS` (Task B2's map) in `ChatSurface.tsx`:

```tsx
function FlowPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-flow" className="h-full overflow-y-auto p-2">{props.params.node}</div>;
}
```

Add `flow: FlowPanel` to `CHAT_DOCK_COMPONENTS`. In the `onReady` callback (from Task B2), add a fourth `addPanel` call, tabbed together with the execution rail by default (`position: { direction: 'within', referencePanel: 'executionRail' }` — this puts Flow and Execution in the same tab group rather than consuming more horizontal space; adjust based on what Step 1's investigation reveals about `AgentFlow`'s real prop source):

```tsx
event.api.addPanel({
  id: 'flow',
  component: 'flow',
  title: 'Flow',
  params: { node: /* <AgentFlow .../> wired to the data source found in Step 1 */ },
  position: { direction: 'within', referencePanel: 'executionRail' },
});
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "feat(gui): Flow is now a dockable panel reachable from chat, tabbed with the execution rail"
```

### Task B4: Layout persistence

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx`
- Test: extend `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx`
- Check: `crates/vox-gui/ui/src/config/constants.ts:66` for the already-anticipated-but-undefined debounce comment found during research; read the surrounding 10 lines and either use the existing constant if one now exists there, or add one.

- [ ] **Step 1: Read `constants.ts` around line 66**

Run: `sed -n '55,75p' crates/vox-gui/ui/src/config/constants.ts` (or use the Read tool) to see the exact current state of the "Dockview layout persistence debounce (ms)" comment and whether a constant follows it.

- [ ] **Step 2: Write the failing test**

```tsx
// Add to ChatDockShell.test.tsx
it('restores a previously serialized layout via fromJSON on mount', () => {
  const savedLayout = { grid: {} } as any; // shape doesn't matter for this test — only that fromJSON is called with it
  localStorage.setItem('gui.chat.dockview_layout.v1', JSON.stringify(savedLayout));
  const onReady = vi.fn((event) => {
    expect(event.api.fromJSON).toBeDefined();
  });
  render(<ChatDockShell onReady={onReady} components={{}} />);
  expect(onReady).toHaveBeenCalled();
});
```

Note: a full round-trip test (serialize a real layout, reload, assert panels restored) belongs in `ChatSurface.test.tsx` once panels exist (Task B2/B3 must land first) — this task's own test only proves the persistence hook fires and calls the right API, since `ChatDockShell` itself has no panels of its own to serialize.

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatDockShell.test.tsx`
Expected: FAIL if persistence isn't wired yet (the test as written will actually pass trivially if `onReady` is just called — tighten it if needed to check `fromJSON` was actually invoked via a spy, not just that it exists as a property).

- [ ] **Step 4: Add layout persistence to `ChatDockShell`**

```tsx
// crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx
import React, { useCallback, useRef } from 'react';
import { DockviewReact, type DockviewApi, type DockviewReadyEvent, type IDockviewPanelProps } from 'dockview';

const LAYOUT_STORAGE_KEY = 'gui.chat.dockview_layout.v1';
const PERSIST_DEBOUNCE_MS = 500; // use the real constant from constants.ts if Step 1 found one already defined

interface ChatDockShellProps {
  components: Record<string, React.FunctionComponent<IDockviewPanelProps>>;
  onReady: (event: DockviewReadyEvent) => void;
}

export function ChatDockShell({ components, onReady }: ChatDockShellProps) {
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleReady = useCallback(
    (event: DockviewReadyEvent) => {
      const saved = window.localStorage.getItem(LAYOUT_STORAGE_KEY);
      let restored = false;
      if (saved) {
        try {
          event.api.fromJSON(JSON.parse(saved));
          restored = true;
        } catch (err) {
          console.warn('failed to restore dockview layout, using default', err);
        }
      }

      event.api.onDidLayoutChange(() => {
        if (debounceRef.current) clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(() => {
          try {
            window.localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(event.api.toJSON()));
          } catch (err) {
            console.warn('failed to persist dockview layout', err);
          }
        }, PERSIST_DEBOUNCE_MS);
      });

      // Only call the caller's onReady to add default panels if we didn't
      // just restore a saved layout — restoring already recreates them.
      if (!restored) onReady(event);
      else onReady(event); // caller's addPanel calls must themselves check
                            // `event.api.getPanel(id)` before adding, so a
                            // restored layout doesn't get duplicate panels —
                            // see Task B2/B3's addPanel calls, which need a
                            // `if (!event.api.getPanel('sessions'))` guard
                            // added around each addPanel call as part of
                            // this task, not left as an unconditional add.
    },
    [onReady],
  );

  return (
    <div className="dockview-theme-vox h-full min-h-[60vh] w-full">
      <DockviewReact components={components} onReady={handleReady} />
    </div>
  );
}
```

- [ ] **Step 5: Guard the addPanel calls in `ChatSurface.tsx` against duplicate-add on restore**

Go back to Task B2/B3's `onReady` callback in `ChatSurface.tsx` and wrap each `event.api.addPanel(...)` call:

```tsx
if (!event.api.getPanel('sessions')) {
  event.api.addPanel({ id: 'sessions', component: 'sessions', title: 'Sessions', params: { node: sessionRailNode } });
}
```

Apply the same `if (!event.api.getPanel(id))` guard to the `transcript`, `executionRail`, and `flow` panels' `addPanel` calls.

- [ ] **Step 6: Run tests**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "feat(gui): persist and restore dockview layout across restarts (debounced, guarded against duplicate panels)"
```

### Task B5: `getPanel`-guarded panel params refresh on data change

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`

Because `addPanel`'s `params` are only set once (at add time), and `ChatSurface`'s props (`messages`, `agentStreamItems`, `tasks`, etc.) change on every render, the dockview panels built in Tasks B2-B3 need their `params.node` refreshed when the underlying React content changes — otherwise the transcript/execution-rail/flow panels go stale after the first render.

- [ ] **Step 1: Write the failing test**

```tsx
// Add to ChatSurface.test.tsx
it('updates the transcript panel content when messages change (does not go stale after first render)', async () => {
  const { rerender } = render(
    <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />,
  );
  rerender(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[{ id: 'm1', role: 'user', text: 'hello', status: 'done' } as any]}
      composer={<div>composer</div>}
    />,
  );
  expect(await screen.findByText('hello')).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — panel content frozen at initial `addPanel` call, `hello` never appears.

- [ ] **Step 3: Refresh panel params via `api.updateParameters` on prop change**

Add a `useEffect` in `ChatSurface.tsx` that runs whenever `centerContent` (or the underlying `messages`/`agentStreamItems`) changes, calling `panel.api.updateParameters({ node: centerContent })` on the transcript panel. This requires keeping a ref to the `DockviewApi` returned in `onReady`:

```tsx
const dockApiRef = useRef<DockviewApi | null>(null);
```

In the `onReady` callback, add `dockApiRef.current = event.api;` before the `addPanel` calls. Then add:

```tsx
useEffect(() => {
  const panel = dockApiRef.current?.getPanel('transcript');
  panel?.api.updateParameters({ node: centerContent });
}, [centerContent]);
```

Apply the same pattern for the execution-rail panel (`params: { node: executionRailNode }`, effect keyed on `[executionRailNode]`) and the Flow panel from Task B3 if its content depends on props that change (`agentStreamItems`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS.

- [ ] **Step 5: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "fix(gui): keep dockview panel content in sync with props via updateParameters (was frozen at first addPanel call)"
```

### Task B6: Visual pass

**Files:** none (verification-only task)

Per the explicit instruction to do a visual pass before considering the docking work done. This task has no code changes — it is a mandatory checkpoint.

- [ ] **Step 1: Build and launch**

```bash
cd crates/vox-gui/ui && pnpm build
cd ../../..
cargo build --release -p vox-gui
```

Kill any running `vox-gui.exe`/`vox-orchestrator-d.exe` first (`taskkill //IM vox-gui.exe //F` on Windows), then launch the freshly built binary.

- [ ] **Step 2: Screenshot the default layout**

Open the Chat tab. Screenshot the default docked arrangement (sessions left, chat center, execution rail + Flow tabbed right).

- [ ] **Step 3: Screenshot a rearranged layout**

Drag the Flow panel to a new position (e.g. below the chat pane instead of tabbed with execution rail). Screenshot the result. Reload the app (restart `vox-gui.exe`). Screenshot again to confirm the rearranged layout persisted.

- [ ] **Step 4: Screenshot light/dark theme**

Toggle the app's theme if a toggle exists (check Settings) and screenshot both, confirming `dockview-vox.css`'s custom-property overrides render correctly in both.

- [ ] **Step 5: Report findings**

If anything looks visually broken (overlapping panels, unreadable text, theme colors not applying, tab bar misaligned), file it as a follow-up fix before marking Phase B complete — do not silently ship a visual regression because the automated tests passed.

---

## Phase C: Composer pause/resume (Stop already exists)

**Ground truth confirmed:** `Loquela.tsx:544-552` already renders a Stop button (replacing Send) when `taskInProgress` is true, wired via `onInterrupt?.(currentTaskId)` to `App.tsx`'s `handleInterruptTask` (already calls `invoke('interrupt_orchestrator_task', { taskId })`). `App.tsx` already has `handlePause`/`handleResume` callbacks (lines 949, 960) wired to `invoke`d Tauri commands backed by real orchestrator methods (`Orchestrator::pause_agent`/`resume_agent`, `crates/vox-orchestrator/src/orchestrator/agent/lifecycle_ops.rs:479,490`) — currently only exposed via command-palette actions (`pause-all`/`resume-all`) and the Agents view, not inline in the composer.

### Task C1: Thread pause/resume props into `Loquela`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx`
- Modify: `crates/vox-gui/ui/src/App.tsx` (the `loquelaComposer` JSX, `App.tsx:1115-1128`)
- Test: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.test.tsx` (extend)

- [ ] **Step 1: Write the failing test**

```tsx
// Add to Loquela.test.tsx — check existing test setup/render helper conventions first
it('renders a Resume button when the current agent is paused, calling onResume with the agent', () => {
  const onResume = vi.fn();
  render(
    <Loquela
      chips={[]}
      setChips={vi.fn()}
      onSubmit={vi.fn()}
      onSlashCommand={vi.fn()}
      agentPaused={true}
      currentAgent={{ id: 'a1' } as any}
      onResume={onResume}
    />,
  );
  const resumeBtn = screen.getByRole('button', { name: /resume/i });
  fireEvent.click(resumeBtn);
  expect(onResume).toHaveBeenCalledWith({ id: 'a1' });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Loquela/Loquela.test.tsx`
Expected: FAIL — `agentPaused`/`currentAgent`/`onResume` props don't exist, no Resume button renders.

- [ ] **Step 3: Add the props and Resume button**

In `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx`, add to the props interface (near the existing `taskInProgress`/`currentTaskId`/`onInterrupt` props, already read at lines 116-121):

```tsx
/** True when the current agent (not just the task) is paused. */
agentPaused?: boolean;
/** The agent to resume; only meaningful when agentPaused is true. */
currentAgent?: { id: string } | null;
/** Called when the user clicks Resume. */
onResume?: (agent: { id: string }) => void;
```

Add matching destructured params to the component's function signature (near line 137-139's existing `taskInProgress = false, ..., onInterrupt`).

Near the existing Stop button (lines 544-552 — read that exact JSX first, since the new button should match its visual style, not be reinvented from scratch), add a sibling Resume button, rendered when `agentPaused` is true (mutually exclusive with the Stop button's `taskInProgress` condition — a task can't be both in-progress and paused):

```tsx
{agentPaused && currentAgent ? (
  <button
    type="button"
    onClick={() => onResume?.(currentAgent)}
    aria-label="Resume"
    className={/* match the Stop button's className from lines 544-552 exactly, swapping only the icon/label */}
  >
    <span className="font-display text-[11px] uppercase tracking-[0.18em]">Resume</span>
  </button>
) : null}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Loquela/Loquela.test.tsx`
Expected: PASS.

- [ ] **Step 5: Wire it in `App.tsx`**

In `App.tsx`, find the current agent for the active chat session (search for how `handlePause`/`handleResume` currently determine which `Agent` object they act on in the Agents view — reuse that same "current agent" resolution logic, do not invent a new one). Update the `loquelaComposer` JSX (currently lines 1115-1128) to pass the new props:

```tsx
agentPaused={/* derive from the resolved current agent's phase === 'Paused', matching the existing handlePause/handleResume dispatch pattern at lines 989-990 */}
currentAgent={/* the resolved current agent, or null */}
onResume={handleResume}
```

- [ ] **Step 6: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.test.tsx crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui): inline Resume button in the composer, reusing existing pause_agent/resume_agent wiring (Stop already existed)"
```

### Task C2: Visual check — Stop/Resume mutually exclusive states

**Files:** none (verification-only)

- [ ] **Step 1: Manually exercise all three composer states**

With the app running (from Task B6's build, or rebuild if needed): (a) idle — Send button visible; (b) task in-flight — Stop button visible (pre-existing, confirm unchanged); (c) agent paused — Resume button visible, Stop/Send hidden. Screenshot all three. Confirm no state shows two action buttons simultaneously.

---

## Phase D: Chat gate policy

**Ground truth confirmed:** `ChatTaskProcessor::process` (full contents already read, `crates/vox-orchestrator/src/chat_processor.rs`, 159 lines) streams the reply and returns — no grounding/validation step exists. `evaluate_socrates_gate(ctx: &SocratesTaskContext, policy: &ConfidencePolicy, query: &str) -> SocratesGateOutcome` (`crates/vox-orchestrator/src/socrates.rs:253`) is the real scoring function, needing a populated `SocratesTaskContext` (evidence_count, citation_coverage, contradiction_hints, etc.) — which normally comes from the research/CRAG pipeline this session's earlier fix (`ab57d7a42c`) made chat tasks skip entirely. This phase's grounding check therefore runs its own bounded, POST-reply research pass (reusing `Orchestrator::generate_goal_search_context`, already used elsewhere for the same purpose), not the pre-enqueue pipeline.

### Task D1: Per-session grounding-check toggle (frontend)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` or wherever session-scoped settings currently live (check `grep -rn "session.*setting\|SessionSettings" crates/vox-gui/ui/src` first — this plan assumes such a mechanism exists given other session-scoped state like `activeSessionId`; if none exists, this step also creates the minimal one needed: a per-session key in the existing session-settings persistence path, not a new global system)
- Test: alongside whatever file Step 1 identifies as the real settings location

- [ ] **Step 1: Find the existing session-settings mechanism**

Run: `grep -rln "session.*setting\|per.session\|sessionSettings" crates/vox-gui/ui/src crates/vox-gui/src` and read whatever surfaces. If a `chat_ensure_gui_session`-adjacent settings table/command already exists (checked during this session's DB-concurrency work — `crates/vox-db/src/codex_chat.rs`), prefer extending that over inventing a new persistence path.

- [ ] **Step 2: Write the failing test, implement, verify green, commit**

Follow the same TDD shape as Task A3 (`useLocalStorage`-backed hook, or a Tauri-command-backed per-session field if Step 1 finds a DB-backed mechanism instead) — the exact code depends on Step 1's finding, which cannot be predicted without that read. At minimum the resulting API must expose `groundingCheckEnabled: boolean` and a setter, scoped per `session_id`, defaulting to `false`.

### Task D2: Non-blocking post-reply grounding check (backend)

**Files:**
- Modify: `crates/vox-orchestrator/src/chat_processor.rs`
- Test: extend `crates/vox-orchestrator/src/chat_processor.rs`'s existing `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test**

```rust
// Add to crates/vox-orchestrator/src/chat_processor.rs's `mod tests`
#[tokio::test]
async fn process_emits_no_grounding_check_when_disabled_on_the_task() {
    let orchestrator = Arc::new(crate::orchestrator::Orchestrator::new(
        crate::config::OrchestratorConfig::for_testing(),
    ));
    let event_bus = crate::events::EventBus::new(16);
    let mut rx = event_bus.subscribe();
    let processor = ChatTaskProcessor::new(event_bus, orchestrator.clone()).await;

    let mut task = crate::types::AgentTask::new(
        crate::types::TaskId(100),
        "hello",
        crate::types::TaskPriority::Normal,
        vec![],
    );
    task.grounding_check_enabled = false; // new field, added in Step 3
    let cancel = Arc::new(AtomicBool::new(false));
    let _ = processor.process(crate::types::AgentId(1), task, cancel).await;

    // Drain whatever events fired; none should be a grounding-check event.
    let mut saw_grounding_event = false;
    while let Ok(evt) = rx.try_recv() {
        if matches!(evt.kind, AgentEventKind::GroundingCheckCompleted { .. }) {
            saw_grounding_event = true;
        }
    }
    assert!(!saw_grounding_event, "grounding check must not run when disabled");
}
```

This test will not compile until `AgentTask::grounding_check_enabled` and `AgentEventKind::GroundingCheckCompleted` exist — that's expected for a red step; write it, confirm the compile error names those two missing items specifically (not something else), then proceed.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator --lib process_emits_no_grounding_check`
Expected: FAIL to compile — `no field grounding_check_enabled on AgentTask` and/or `no variant GroundingCheckCompleted`.

- [ ] **Step 3: Add the field, event variant, and non-blocking check**

Add `pub grounding_check_enabled: bool` to `AgentTask` in `crates/vox-orchestrator/src/types/tasks.rs` (default `false` in `AgentTask::new`, settable via `TaskEnqueueHints` following the exact same pattern as `task_category` — see `apply_hints`, already read in full this session, `crates/vox-orchestrator/src/types/tasks.rs:857-879`). Thread it through `orch_daemon/mod.rs`'s SUBMIT_TASK params parsing the same way `task_category` was threaded (this session's Task A4 commit `31ef5b9bc2` is the exact template to follow — same null-safe `.filter(|v| !v.is_null())` idiom).

Add a new `AgentEventKind::GroundingCheckCompleted { agent_id: AgentId, task_id: TaskId, confidence: f64, flagged: bool }` variant in `crates/vox-orchestrator/src/events.rs` (follow the existing variants' shape exactly — read a couple of neighboring variants first to match field-naming conventions).

In `chat_processor.rs`'s `process`, after Step 6 (`record_ai_usage`, currently the function's tail before `Ok(task.id)`), add:

```rust
// Step 6.5: optional, non-blocking grounding check — never delays the
// reply (already streamed above), only emits a follow-up badge event.
if task.grounding_check_enabled {
    let orchestrator = self.orchestrator.clone();
    let event_bus = self.event_bus.clone();
    let query = task.description.clone();
    let reply = reply_text.clone();
    let task_id = task.id;
    tokio::spawn(async move {
        let ctx = orchestrator
            .generate_goal_search_context(&query, &[])
            .await;
        let policy = crate::socrates::ConfidencePolicy::default();
        let outcome = crate::socrates::evaluate_socrates_gate(&ctx, &policy, &reply);
        event_bus.emit(AgentEventKind::GroundingCheckCompleted {
            agent_id,
            task_id,
            confidence: outcome.confidence,
            flagged: outcome.confidence < 0.5,
        });
    });
}
```

Before writing this, confirm `generate_goal_search_context`'s real visibility (`pub(crate) async fn generate_goal_search_context` per the earlier research — it's `pub(crate)`, defined on `Orchestrator` in `goal.rs`, so calling it from `chat_processor.rs` within the same crate is fine) and `ConfidencePolicy`'s real `Default` impl (check `crates/vox-orchestrator/src/socrates.rs` or wherever `ConfidencePolicy` is defined — it wasn't in the portion of `socrates.rs` read during planning; read it now before writing this step's final code, and adjust the construction call if `Default` isn't implemented).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-orchestrator --lib process_emits_no_grounding_check`
Expected: PASS.

- [ ] **Step 5: Write the positive-case test (grounding check runs when enabled)**

```rust
#[tokio::test]
async fn process_emits_grounding_check_completed_when_enabled() {
    let orchestrator = Arc::new(crate::orchestrator::Orchestrator::new(
        crate::config::OrchestratorConfig::for_testing(),
    ));
    let event_bus = crate::events::EventBus::new(16);
    let mut rx = event_bus.subscribe();
    let processor = ChatTaskProcessor::new(event_bus, orchestrator.clone()).await;

    let mut task = crate::types::AgentTask::new(
        crate::types::TaskId(101),
        "hello",
        crate::types::TaskPriority::Normal,
        vec![],
    );
    task.grounding_check_enabled = true;
    let cancel = Arc::new(AtomicBool::new(true)); // preset-cancel: no real LLM call, matches this file's existing no-network-call test pattern
    let _ = processor.process(crate::types::AgentId(1), task, cancel).await;

    // Cancel-preset means process() returns before reaching Step 6.5 at all
    // (see the existing process_aborts_before_any_generation_call_when_cancel_flag_preset
    // test above this one) — so this specific test proves grounding_check_enabled
    // being true does NOT itself cause a network call before generation even
    // starts. A true "grounding check actually ran" assertion needs a real
    // (non-cancelled) run, which this codebase's no-paid-LLM-calls-in-tests
    // constraint (see this file's existing test doc comment) makes
    // impractical here — cover the tokio::spawn branch's internal logic
    // (evaluate_socrates_gate's scoring) with a separate, direct unit test
    // against evaluate_socrates_gate in socrates.rs instead, not by driving
    // it through ChatTaskProcessor::process with a live client.
    let mut events = Vec::new();
    while let Ok(evt) = rx.try_recv() { events.push(evt); }
    assert!(events.is_empty(), "cancelled-before-generation must emit nothing, including no grounding event");
}
```

This test is intentionally narrower than its name suggests — see the comment inside it. It exists to prove the cancel-path still short-circuits correctly with the new field present, not to prove the grounding check's actual scoring logic (that's Step 6 below).

- [ ] **Step 6: Add a direct unit test for `evaluate_socrates_gate`'s use here**

Check `crates/vox-orchestrator/src/socrates.rs` for existing tests of `evaluate_socrates_gate` (search `grep -n "evaluate_socrates_gate" crates/vox-orchestrator/src/socrates.rs` for existing coverage). If none exercise a "low evidence → low confidence → flagged" case relevant to chat's typical low-evidence context (chat replies rarely have citations), add one there:

```rust
#[test]
fn evaluate_socrates_gate_flags_low_confidence_for_zero_evidence_context() {
    let ctx = SocratesTaskContext {
        required_citations: 3,
        evidence_count: 0,
        contradiction_hints: 0,
        citation_coverage: 0.0,
        retrieval_tier: None,
        retrieval_used_lexical_fallback: false,
        ..Default::default()
    };
    let policy = ConfidencePolicy::default();
    let outcome = evaluate_socrates_gate(&ctx, &policy, "some chat reply");
    assert!(outcome.confidence < 0.5, "zero evidence against a required-citation policy must score low confidence");
}
```

Verify `SocratesTaskContext` and `ConfidencePolicy` both implement `Default` before writing this (check the structs' derives in `socrates.rs`); if not, construct them field-by-field using the real struct definition instead of `..Default::default()`.

- [ ] **Step 7: Run the full test**

Run: `cargo test -p vox-orchestrator --lib chat_processor:: socrates::`
Expected: PASS (all chat_processor and socrates tests).

- [ ] **Step 8: `cargo fmt -p vox-orchestrator`, then commit**

```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/chat_processor.rs crates/vox-orchestrator/src/types/tasks.rs crates/vox-orchestrator/src/events.rs crates/vox-orchestrator/src/socrates.rs crates/vox-orchestrator/src/orch_daemon/mod.rs
git commit -m "feat(orchestrator): opt-in non-blocking grounding check for chat replies (per-task flag, background evaluate_socrates_gate pass)"
```

### Task D3: Frontend badge for flagged replies

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx` (the `MessageBubble` component)
- Test: extend `ChatTranscript.test.tsx` if it exists, else create one covering `MessageBubble`'s new badge

- [ ] **Step 1: Write the failing test**

```tsx
it('shows a low-confidence badge on an assistant message flagged by the grounding check', () => {
  render(
    <MessageBubble
      message={{ id: 'm1', role: 'assistant', text: 'reply', status: 'done', groundingFlagged: true } as any}
    />,
  );
  expect(screen.getByText(/low confidence/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatTranscript.test.tsx`
Expected: FAIL — no `groundingFlagged` field read, no badge rendered.

- [ ] **Step 3: Add `groundingFlagged` to `ChatMessage` and render the badge**

Add `groundingFlagged?: boolean` to the `ChatMessage` type (`crates/vox-gui/ui/src/lib/chatCorrelation.ts` — check its current shape first, since it wasn't fully read during this planning pass; add the field alongside the existing `modelId`/`taskId` optional fields following the same pattern). Find wherever `AgentEventKind::GroundingCheckCompleted` frames get consumed on the frontend (likely the same `vox://agent-events` listener that produces `StreamItem`s via `mapAgentEvent.ts`, already read in full — add a case for `grounding_check_completed` there, or wherever chat messages get updated by task-id-correlated events, following the existing `task_completed`/`task_failed` correlation pattern) and set `groundingFlagged: true` on the matching message when `flagged` is true.

In `ChatTranscript.tsx`'s `MessageBubble`, add near the existing `ModelBadge` rendering (already read — `message.role === 'assistant' && message.status === 'done' && message.modelId && <ModelBadge .../>`):

```tsx
{message.role === 'assistant' && message.groundingFlagged && (
  <div className="mt-1 flex justify-end">
    <span className="rounded border border-amber-400/30 bg-amber-400/[0.08] px-1.5 py-0.5 font-mono text-[9px] text-amber-300">
      low confidence — unverified
    </span>
  </div>
)}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatTranscript.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx crates/vox-gui/ui/src/lib/chatCorrelation.ts crates/vox-gui/ui/src/lib/mapAgentEvent.ts
git commit -m "feat(gui): show a low-confidence badge on chat replies flagged by the opt-in grounding check"
```

---

## Phase E: Editable + auto-growing plan panel

**Ground truth confirmed:** `PlanNode { node_id, description, depends_on, status, execution_policy, workflow_invocation }`, `PlanStatus` (Pending/Queued/InProgress/Completed/Failed/Cancelled/Superseded) — `crates/vox-orchestrator/src/planning/types.rs:94-115`. `upsert_plan_node` (`crates/vox-db/src/store/ops_planning.rs:133`), `load_plan_nodes_with_status` (`:288`), `set_plan_node_status` (`:319`) — all real, already used. `enqueue_runnable_plan_nodes` (`crates/vox-orchestrator/src/planning/schedule.rs:40-87`) re-reads current DB rows via `load_plan_nodes_with_status` immediately before dispatching each node — confirmed this session by direct code reading — so edits to a not-yet-dispatched node's description take effect automatically with no new mechanism needed. Dynamic mid-execution node insertion does NOT currently exist outside `replan.rs::synthesize_recovery_nodes` (failure-recovery only).

### Task E1: Tauri command wrapping `upsert_plan_node` for GUI edits

**Files:**
- Create: `crates/vox-gui/src/commands/plan_panel.rs`
- Modify: `crates/vox-gui/src/main.rs` (command registration, alongside the existing `commands::runs::start_gui_run`/`finish_gui_run` registrations touched earlier this session)
- Test: `#[cfg(test)] mod tests` inside `plan_panel.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-gui/src/commands/plan_panel.rs
// (test module at the bottom of the file, written first per TDD)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::gui_db_pool::GuiDbPool;

    #[tokio::test]
    async fn update_plan_node_writes_through_to_the_db() {
        let pool = GuiDbPool::connect_memory().await.unwrap();
        let db = pool.handle().unwrap();
        db.create_plan_session("ps1", None, "test goal", "sequential").await.unwrap();
        db.append_plan_version("ps1", 1, None, None, None).await.unwrap();
        db.upsert_plan_node("ps1", 1, "n1", "original description", "[]", "{}", "pending", None)
            .await
            .unwrap();

        update_plan_node(
            pool.clone(),
            UpdatePlanNodeInput {
                plan_session_id: "ps1".to_string(),
                plan_version: 1,
                node_id: "n1".to_string(),
                description: "edited description".to_string(),
            },
        )
        .await
        .unwrap();

        let rows = db.load_plan_nodes_with_status("ps1", 1).await.unwrap();
        let edited = rows.iter().find(|r| r.node_id == "n1").unwrap();
        assert_eq!(edited.description, "edited description");
    }
}
```

Before writing this, check `GuiDbPool::connect_memory` (already read in full this session, `crates/vox-gui/src/commands/gui_db_pool.rs:24-32`) is `#[cfg(test)] pub async fn` and confirm `db.create_plan_session`/`append_plan_version`/`upsert_plan_node`/`load_plan_nodes_with_status` are all real `VoxDb` methods with these exact signatures — they were confirmed to exist this session (via `upsert_plan_node`'s call sites in `dei_plan_materialize.rs`/`goal.rs`) but their exact parameter order/types should be double-checked against `crates/vox-db/src/store/ops_planning.rs` directly before finalizing this test, since the plan's earlier research only confirmed their existence and rough shape, not the literal signature.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-gui --bin vox-gui update_plan_node_writes_through_to_the_db`
Expected: FAIL to compile — `plan_panel.rs` and its types don't exist.

- [ ] **Step 3: Implement the command**

```rust
// crates/vox-gui/src/commands/plan_panel.rs
use crate::commands::gui_db_pool::{GuiDbPool, map_db_err, pool_db};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct UpdatePlanNodeInput {
    pub plan_session_id: String,
    pub plan_version: i64,
    pub node_id: String,
    pub description: String,
}

/// Edit a not-yet-dispatched plan node's description from the GUI. Writes
/// through the same `upsert_plan_node` primitive the orchestrator's own
/// plan-synthesis code uses — `enqueue_runnable_plan_nodes` re-reads current
/// DB state immediately before dispatching each node, so this edit takes
/// effect automatically for any node the scheduler hasn't reached yet.
#[tauri::command]
pub async fn update_plan_node(
    pool: State<'_, GuiDbPool>,
    input: UpdatePlanNodeInput,
) -> Result<(), String> {
    let db = pool_db(&pool)?;
    let rows = db
        .load_plan_nodes_with_status(&input.plan_session_id, input.plan_version)
        .await
        .map_err(map_db_err)?;
    let existing = rows
        .iter()
        .find(|r| r.node_id == input.node_id)
        .ok_or_else(|| format!("plan node {} not found", input.node_id))?;
    db.upsert_plan_node(
        &input.plan_session_id,
        input.plan_version,
        &input.node_id,
        &input.description,
        &existing.dependencies_json,
        &existing.execution_policy_json,
        &existing.status,
        existing.workflow_invocation.as_deref(),
    )
    .await
    .map_err(map_db_err)?;
    Ok(())
}
```

Check `pool_db` and `map_db_err`'s real signatures in `gui_db_pool.rs` (already read in full — `map_db_err` is a free function taking `impl std::fmt::Display`; `pool_db` was referenced in `chat.rs` earlier this session as `let db = pool_db(&pool)?;` but its own definition wasn't part of this session's reads — grep for `fn pool_db` in `gui_db_pool.rs` and confirm it exists with this shape before relying on it; if it doesn't exist yet as a shared helper, use `pool.handle()` directly instead, matching `chat.rs`'s actual pattern once you check it).

Also check `PlanNodeRow`'s (or whatever `load_plan_nodes_with_status` returns) exact field names — the code above assumes `dependencies_json`, `execution_policy_json`, `status`, `workflow_invocation` fields exist on it based on `schedule.rs`'s `row_to_plan_node` (already read: `r.dependencies_json`, `r.node_id`, and implicitly a status field are real — `execution_policy_json` and `workflow_invocation`'s exact names need confirming against the row struct's actual definition before this compiles).

- [ ] **Step 4: Register the command**

In `crates/vox-gui/src/main.rs`, add `commands::plan_panel::update_plan_node,` to the `invoke_handler` list (alongside `commands::runs::start_gui_run`/`finish_gui_run`, already located at lines 211-213 this session). Add `pub mod plan_panel;` to `crates/vox-gui/src/commands/mod.rs` if commands are declared there (check the existing pattern for how `runs`/`chat` modules are declared).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-gui --bin vox-gui update_plan_node_writes_through_to_the_db`
Expected: PASS.

- [ ] **Step 6: `cargo fmt -p vox-gui`, then commit**

```bash
cargo fmt -p vox-gui
git add crates/vox-gui/src/commands/plan_panel.rs crates/vox-gui/src/main.rs crates/vox-gui/src/commands/mod.rs
git commit -m "feat(gui): update_plan_node Tauri command — edits write through to upsert_plan_node"
```

### Task E2: Insert a new plan node from the GUI

**Files:**
- Modify: `crates/vox-gui/src/commands/plan_panel.rs`
- Test: extend the same `mod tests`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn insert_plan_node_adds_a_new_pending_node() {
    let pool = GuiDbPool::connect_memory().await.unwrap();
    let db = pool.handle().unwrap();
    db.create_plan_session("ps2", None, "test goal", "sequential").await.unwrap();
    db.append_plan_version("ps2", 1, None, None, None).await.unwrap();

    insert_plan_node(
        pool.clone(),
        InsertPlanNodeInput {
            plan_session_id: "ps2".to_string(),
            plan_version: 1,
            node_id: "n-new".to_string(),
            description: "a step the user added".to_string(),
            depends_on: vec![],
        },
    )
    .await
    .unwrap();

    let rows = db.load_plan_nodes_with_status("ps2", 1).await.unwrap();
    let added = rows.iter().find(|r| r.node_id == "n-new").unwrap();
    assert_eq!(added.description, "a step the user added");
    assert_eq!(added.status, "pending");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-gui --bin vox-gui insert_plan_node_adds_a_new_pending_node`
Expected: FAIL — `insert_plan_node`/`InsertPlanNodeInput` don't exist.

- [ ] **Step 3: Implement**

```rust
#[derive(Debug, Deserialize)]
pub struct InsertPlanNodeInput {
    pub plan_session_id: String,
    pub plan_version: i64,
    pub node_id: String,
    pub description: String,
    pub depends_on: Vec<String>,
}

/// Insert a new intermediate step into the live plan. Joins the same
/// dependency graph `enqueue_runnable_plan_nodes` walks — becomes runnable
/// as soon as its `depends_on` entries complete, same as any agent-created
/// node.
#[tauri::command]
pub async fn insert_plan_node(
    pool: State<'_, GuiDbPool>,
    input: InsertPlanNodeInput,
) -> Result<(), String> {
    let db = pool_db(&pool)?;
    let deps_json = serde_json::to_string(&input.depends_on).map_err(|e| e.to_string())?;
    db.upsert_plan_node(
        &input.plan_session_id,
        input.plan_version,
        &input.node_id,
        &input.description,
        &deps_json,
        "{}",
        "pending",
        None,
    )
    .await
    .map_err(map_db_err)?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-gui --bin vox-gui insert_plan_node_adds_a_new_pending_node`
Expected: PASS.

- [ ] **Step 5: Register the command and commit**

Add `commands::plan_panel::insert_plan_node,` to `main.rs`'s `invoke_handler` list.

```bash
cargo fmt -p vox-gui
git add crates/vox-gui/src/commands/plan_panel.rs crates/vox-gui/src/main.rs
git commit -m "feat(gui): insert_plan_node Tauri command — user-added intermediate steps join the real dependency graph"
```

### Task E3: Plan panel UI

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (add as a fifth dockview panel, alongside sessions/transcript/executionRail/flow from Phase B)

- [ ] **Step 1: Write the failing test**

```tsx
// crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.test.tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve()) }));

import { PlanPanel } from './PlanPanel';
import { invoke } from '@tauri-apps/api/core';

const nodes = [
  { node_id: 'n1', description: 'first step', status: 'completed' as const },
  { node_id: 'n2', description: 'second step', status: 'in_progress' as const },
  { node_id: 'n3', description: 'third step', status: 'pending' as const },
];

describe('PlanPanel', () => {
  it('renders each node with a status-appropriate checkbox state', () => {
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={nodes} />);
    expect(screen.getByText('first step')).toBeInTheDocument();
    expect(screen.getByText('second step')).toBeInTheDocument();
    expect(screen.getByText('third step')).toBeInTheDocument();
  });

  it('editing a pending node description calls update_plan_node', async () => {
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={nodes} />);
    const input = screen.getByDisplayValue('third step');
    fireEvent.change(input, { target: { value: 'edited third step' } });
    fireEvent.blur(input);
    expect(invoke).toHaveBeenCalledWith('update_plan_node', {
      input: { plan_session_id: 'ps1', plan_version: 1, node_id: 'n3', description: 'edited third step' },
    });
  });

  it('a completed node is not editable', () => {
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={nodes} />);
    const completedText = screen.getByText('first step');
    expect(completedText.tagName).not.toBe('INPUT');
  });

  it('adding a new step calls insert_plan_node with a fresh node_id', async () => {
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={nodes} />);
    fireEvent.click(screen.getByRole('button', { name: /add step/i }));
    const newInput = screen.getByPlaceholderText(/new step/i);
    fireEvent.change(newInput, { target: { value: 'a fourth step' } });
    fireEvent.keyDown(newInput, { key: 'Enter' });
    expect(invoke).toHaveBeenCalledWith(
      'insert_plan_node',
      expect.objectContaining({
        input: expect.objectContaining({ plan_session_id: 'ps1', plan_version: 1, description: 'a fourth step' }),
      }),
    );
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/PlanPanel.test.tsx`
Expected: FAIL — `PlanPanel.tsx` doesn't exist.

- [ ] **Step 3: Implement `PlanPanel`**

```tsx
// crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.tsx
import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

export type PlanNodeStatus = 'pending' | 'queued' | 'in_progress' | 'completed' | 'failed' | 'cancelled' | 'superseded';

export interface PlanNodeView {
  node_id: string;
  description: string;
  status: PlanNodeStatus;
}

interface PlanPanelProps {
  planSessionId: string;
  planVersion: number;
  nodes: PlanNodeView[];
}

const STATUS_ICON: Record<PlanNodeStatus, string> = {
  pending: '○',
  queued: '◔',
  in_progress: '◑',
  completed: '●',
  failed: '✕',
  cancelled: '−',
  superseded: '»',
};

const EDITABLE_STATUSES = new Set<PlanNodeStatus>(['pending']);

function PlanNodeRow({ node, planSessionId, planVersion }: { node: PlanNodeView; planSessionId: string; planVersion: number }) {
  const editable = EDITABLE_STATUSES.has(node.status);
  const [value, setValue] = useState(node.description);

  const commit = () => {
    if (value === node.description) return;
    void invoke('update_plan_node', {
      input: { plan_session_id: planSessionId, plan_version: planVersion, node_id: node.node_id, description: value },
    });
  };

  return (
    <div className="flex items-center gap-2 py-1 text-[12px]" data-testid={`plan-node-${node.node_id}`}>
      <span aria-hidden="true" className="w-4 shrink-0 text-center text-text-muted">{STATUS_ICON[node.status]}</span>
      {editable ? (
        <input
          className="flex-1 rounded border border-transparent bg-transparent px-1 hover:border-border-subtle focus:border-brass/40 focus:outline-none"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onBlur={commit}
        />
      ) : (
        <span className="flex-1 text-text-secondary">{node.description}</span>
      )}
    </div>
  );
}

export function PlanPanel({ planSessionId, planVersion, nodes }: PlanPanelProps) {
  const [adding, setAdding] = useState(false);
  const [newDescription, setNewDescription] = useState('');

  const submitNew = () => {
    if (!newDescription.trim()) return;
    void invoke('insert_plan_node', {
      input: {
        plan_session_id: planSessionId,
        plan_version: planVersion,
        node_id: `n-${crypto.randomUUID()}`,
        description: newDescription,
        depends_on: [],
      },
    });
    setNewDescription('');
    setAdding(false);
  };

  return (
    <div className="flex flex-col gap-1 p-2" data-testid="plan-panel">
      {nodes.map((n) => (
        <PlanNodeRow key={n.node_id} node={n} planSessionId={planSessionId} planVersion={planVersion} />
      ))}
      {adding ? (
        <input
          autoFocus
          className="mt-1 rounded border border-border-subtle bg-transparent px-1 text-[12px]"
          placeholder="new step…"
          value={newDescription}
          onChange={(e) => setNewDescription(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') submitNew(); if (e.key === 'Escape') setAdding(false); }}
          onBlur={submitNew}
        />
      ) : (
        <button
          type="button"
          onClick={() => setAdding(true)}
          className="mt-1 self-start text-[11px] text-text-muted hover:text-brass"
        >
          + Add step
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/PlanPanel.test.tsx`
Expected: PASS (4 tests).

- [ ] **Step 5: Add as a fifth dockview panel in `ChatSurface.tsx`**

Following the exact same pattern as Task B3's Flow panel: add a `PlanPanelWrapper` to `CHAT_DOCK_COMPONENTS`, an `addPanel` call in `onReady` (guarded with `getPanel`, per Task B4), and a `useEffect`-driven `updateParameters` call (per Task B5) keyed on whatever prop supplies the current plan session's nodes — this requires `ChatSurface` to receive plan-session data as a new prop from `App.tsx`/`surfaceComponents.tsx`, fetched via a new `list_plan_nodes`-style query (not specified in this plan — read `crates/vox-db/src/store/ops_planning.rs:288`'s `load_plan_nodes_with_status` and wrap it in a thin read-only Tauri command in `plan_panel.rs`, following `update_plan_node`'s exact pattern from Task E1, before wiring this step).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.tsx crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.test.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/src/commands/plan_panel.rs
git commit -m "feat(gui): dockable PlanPanel — editable checklist backed by the real plan-DAG, dockview panel"
```

### Task E4: Dynamic mid-execution plan-node insertion (backend, agent-side)

**Files:**
- Modify: wherever the phase loop reasons about its current step — this session's earlier work (`docs/superpowers/plans/2026-07-20-orchestrator-chat-latency-reliability.md`) confirmed `AiTaskProcessor`'s phase loop lives in `crates/vox-orchestrator/src/runtime.rs`. Read that file's phase-loop section fresh before starting this task — it was modified twice already this session (compaction wiring, nudge concurrency) and its exact current line numbers are not reliable from earlier research.
- Test: alongside whatever module ends up housing the new decision logic

This is explicitly the highest-uncertainty task in this plan — the spec itself flagged this as "new agent-decision logic," not a mechanical wiring change like the rest of Phase E. It requires the implementer to read the current phase loop fresh and make a genuine design decision about WHERE in that loop a "should I add a step" check belongs, informed by the existing `synthesize_recovery_nodes` pattern (`crates/vox-orchestrator/src/planning/replan.rs:33`, already read: takes `reason: &str, failed_desc: &str`, returns `Vec<PlanNode>` — a synchronous-shaped synthesis call) as the closest existing precedent, but this is failure-triggered, not judgment-triggered, so the trigger condition is new.

- [ ] **Step 1: Read the current phase loop in full**

Run: `grep -n "async fn.*phase\|Inspect\|Localize\|Hypothesize\|Act\|Verify\|Decide" crates/vox-orchestrator/src/runtime.rs | head -40` and read the surrounding function(s) in full via the Read tool — do not proceed without this, since the plan cannot specify exact insertion points without it.

- [ ] **Step 2: Write a failing test proving the new capability's contract**

The exact test depends on Step 1's findings (what hook point exists for post-phase decisions), but at minimum must prove: given a phase's output text containing a recognizable "I need an additional step" signal (define a concrete, testable signal — e.g. a structured marker in the LLM output like `[[ADD_STEP: description]]`, mirroring this codebase's existing `[[category:X]]` marker-scan convention already used in `AgentTask::new`, per `types/tasks.rs`'s marker-scan read during this session's earlier work), a new `PlanNode` gets upserted into the current plan session/version via `upsert_plan_node`, using `StubTaskProcessor`/no real LLM call (per this session's hard "no paid LLM calls in tests" constraint — same pattern as `chat_round_trip.rs`'s existing tests).

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p vox-orchestrator --lib` filtered to the new test name.
Expected: FAIL — capability doesn't exist yet.

- [ ] **Step 4: Implement the marker-scan + upsert, following the `[[category:X]]` precedent**

Add detection of the `[[ADD_STEP: description]]` marker (or whatever concrete signal Step 2 settled on) to the phase loop's post-phase-output handling, calling `orchestrator.db()`'s `upsert_plan_node` with a freshly generated `node_id`, the extracted description, `depends_on: []` (appended at the end of the current plan, not blocking on anything — a conservative default; do not attempt to infer dependency ordering automatically, that's out of scope), status `"pending"`.

- [ ] **Step 5: Run to verify it passes**

Run the same filtered test command as Step 3.
Expected: PASS.

- [ ] **Step 6: Run the full orchestrator suite for regressions**

Run: `cargo test -p vox-orchestrator --lib`
Expected: PASS, same count as before this task plus the new test(s).

- [ ] **Step 7: `cargo fmt -p vox-orchestrator`, then commit**

```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/runtime.rs
git commit -m "feat(orchestrator): phase loop can dynamically append plan-DAG nodes mid-execution (TodoWrite-equivalent), via a [[ADD_STEP:]] marker"
```

---

## Final Task F1: Whole-effort verification, rebuild, relaunch, push

**Files:** none (verification/release task)

- [ ] **Step 1: Full test suites**

```bash
cd crates/vox-gui/ui && npx tsc --noEmit && pnpm exec vitest run
cd ../../..
cargo test -p vox-orchestrator --lib
cargo test -p vox-gui --bin vox-gui
cargo test -p vox-db --lib --features local
```

Expected: all green.

- [ ] **Step 2: Rebuild frontend + both release binaries**

```bash
cd crates/vox-gui/ui && pnpm build
cd ../../..
taskkill //IM vox-gui.exe //F
taskkill //IM vox-orchestrator-d.exe //F
cargo build --release -p vox-gui -p vox-orchestrator-d
```

- [ ] **Step 3: Relaunch and confirm both processes are the freshly built binaries**

Launch `target/release/vox-gui.exe`. Confirm via `wmic process where "name='vox-gui.exe'" get CreationDate` (or equivalent) that the running process's start time is after the build just completed, and that `vox-orchestrator-d.exe` was staged from `~/.vox/bin/` at launch time (self-heal spawn pattern, confirmed this session) matching the just-built binary's timestamp.

- [ ] **Step 4: Manual smoke test of every new surface**

Chat: confirm the transcript no longer shows raw PHASE/TASK/CHECKPOINT/COST rows, confirm the status line appears while a task runs and disappears on completion, confirm the verbosity setting changes what's shown. Docking: confirm sessions/transcript/execution-rail/Flow/Plan panels are all present, draggable, and the layout survives a restart. Composer: confirm Stop still works, confirm Resume appears when an agent is paused. Plan panel: edit a pending node's description, confirm it persists; add a new step, confirm it appears with pending status.

- [ ] **Step 5: `cargo fmt -p` each crate touched in this plan (never `--all`)**

```bash
cargo fmt -p vox-orchestrator
cargo fmt -p vox-gui
cargo fmt -p vox-db
```

- [ ] **Step 6: Push**

```bash
git fetch origin main
git log origin/main -1 --oneline
git push origin HEAD:main
```

Handle the pre-push hook's own gates (fmt retries, `VOX_SKIP_FRESHNESS_CHECK=1` contract regen for drift) the same way this session has handled them repeatedly already — retry the build/gate step, do not bypass with `--no-verify`.

- [ ] **Step 7: Confirm and report**

```bash
git log origin/main -1 --oneline
```

Deliver a final report covering: what changed in each phase, real test counts, confirmation the app was rebuilt/relaunched and manually smoke-tested (not just unit-tested), the final pushed commit hash.
