---
title: "GUI Intuitiveness Implementation Plan"
description: "Honesty-debt burn-down (needs-you nav, SubAgents wired to the real list_subagent_tree command, dead controls hidden), one useAttentionInbox hook replacing three fragmented polls, and a structured intent panel in the composer serializing into the existing task payload."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-07-02"
training_eligible: false
authored: "2026-07-02"
---

# GUI Intuitiveness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make vox-gui honest and intent-first: (1) burn down the remaining open items from `docs/agents/gui-honesty-triage.md` + the ratified IA blueprint (`needs-you` into nav, SubAgents wired to the real `list_subagent_tree` command, dead sub-agent controls hidden), (2) collapse the three fragmented attention feeds (approvals poll, feedback+hopper poll, per-surface polls) into ONE `useAttentionInbox` hook feeding one "Needs You" surface and one nav badge, (3) upgrade the Chat composer with an optional structured-intent panel (goal / constraints / effort / acceptance criteria) that serializes into the existing `submit_orchestrator_task` payload with zero backend changes.

**Architecture:** All frontend work lives in `crates/vox-gui/ui/src`. Navigation SSOT is `lib/navigation.ts` + the generated `generated/surfaceRegistry.generated.ts` (regenerated from `contracts/gui/surface-registry.v1.yaml` via `vox ci gui-surface-registry --write` — never hand-edit the generated file). Surface dispatch is `components/layout/surfaceComponents.tsx` (`case 'needs-you'` already exists; `sub-agents` renders via `surfaces/decoratorRegistry.ts`). The new `hooks/useAttentionInbox.ts` is owned once by `App.tsx` and prop-drilled through `SurfaceProps` (the established pattern — see `onOpenFeedbackContext`). Intent composition is a pure function in `lib/intentSpec.ts` consumed by `surfaces/Loquela/Loquela.tsx`; the composed intent rides in the existing `description` + `priority` fields of `ChatPayload`, so `App.handleLoquelaSubmit` and the Rust side are untouched. Dashboard's per-row `useAgentApprovals` and Loquela's `InlineApprovals` stay (they poll only while their surface is mounted); the two **App-global** polls are what get consolidated.

**Verified already done — do NOT redo (triage says open, code says fixed):** `ContextWindowMeter.usedTokens` is wired to `budget.used_tokens` (`ChatExecutionRail.tsx:224` + test at `ChatExecutionRail.test.tsx:226`); StreamCard Doubt/Overrule are wired via `App.tsx:947-959` → `voxTransport.doubtTask`/`overruleTask`; the Settings keybinds section is fully interactive (`SettingsView.tsx:959-979,1445-1473` + `useKeybinds`); all 4 SkillsPlugins WIRE+TOAST-FIX items now open a real `SkillDetailPanel` and search results carry Info/Install actions (`SkillsPluginsView.tsx:196-242,445-458`).

**Tech Stack:** React 19 + TypeScript, Tailwind with Limes tokens only (no hardcoded hex/zinc — see `crates/vox-gui/ds/conventions.md`; section headings use the underline pattern `.ds-section-head` / `border-b`, never a cap from above), vitest + @testing-library/react (`pnpm vitest run <file>` from `crates/vox-gui/ui`; **pnpm only, never npm**), Tauri `invoke` IPC, generated surface registry via the Rust `vox` CLI.

---

### Task 1: Add `needs-you` to the Runs nav (blueprint ADD-to-nav, honesty-gate already satisfied by wired `vox_resolve_feedback`/`vox_resolve_approval`)

**Files:**
- `crates/vox-gui/ui/src/lib/navigation.ts`
- `crates/vox-gui/ui/src/lib/navigation.test.ts`
- `contracts/gui/surface-registry.v1.yaml`
- `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` (regenerated, not hand-edited)

**Steps:**

- [ ] Add a failing test to `crates/vox-gui/ui/src/lib/navigation.test.ts` (match the existing test style in that file):

```ts
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';

describe('needs-you attention inbox nav wiring', () => {
  it('resolves needs-you under the runs parent', () => {
    expect(resolveNavigation('needs-you')).toEqual({ parent: 'runs', child: 'needs-you' });
  });
  it('labels needs-you for breadcrumbs', () => {
    expect(labelForNavKey('needs-you')).toBe('Needs You');
  });
  it('registry parents needs-you under runs so ParentSurface shows the tab', () => {
    const entry = SURFACE_REGISTRY.find(e => e.viewKey === 'needs-you');
    expect(entry?.parentSurface).toBe('runs');
  });
});
```

- [ ] Run `pnpm vitest run src/lib/navigation.test.ts` from `crates/vox-gui/ui` — expect the 3 new tests to FAIL (`resolveNavigation('needs-you')` currently returns `{ parent: 'needs-you', child: 'needs-you' }`; registry entry has `parentSurface: null`).
- [ ] In `crates/vox-gui/ui/src/lib/navigation.ts`, add to `PARENT_CHILD_MAP` (after the `policies` entry at line 10): `'needs-you': { parent: 'runs', child: 'needs-you' },` and to `NAV_LABELS` (after `policies: 'Policies',`): `'needs-you': 'Needs You',`.
- [ ] In `contracts/gui/surface-registry.v1.yaml`, change the `needs-you` entry (around line 164): `parent_surface: null` → `parent_surface: runs`.
- [ ] Regenerate the TS registry from the repo root: `cargo run -p vox-cli --bin vox -- ci gui-surface-registry --write` (the generated header documents this exact command as `vox ci gui-surface-registry --write`; use the installed `vox` binary if present). Verify with `git diff crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` that ONLY the `needs-you` row changed (`parentSurface: 'runs'`).
- [ ] Run `pnpm vitest run src/lib/navigation.test.ts` — expect PASS. Also run `pnpm vitest run src/components/layout` to confirm `ParentSurface`/`Sidebar` tests still pass (the Runs sub-tab bar now shows Approvals · Needs You · Policies · Runs from registry order).
- [ ] Commit: `git commit -m "feat(gui): add needs-you attention inbox to Runs nav (blueprint ADD-to-nav)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

---

### Task 2: Wire the SubAgents tree to the real `list_subagent_tree` Tauri command

The frontend `subAgentClient.ts` invokes `subagent_tree` / `context_get` / `context_set` / `subagent_control` — **none of which exist**; `crates/vox-gui/src/main.rs:263` registers only `commands::mission_control::list_subagent_tree`, which returns a **flat `Vec<SubagentTreeNode>`** (`task_id`, `agent_id`, `parent_agent_id?`, `source_task_id?`, `reason` — see `crates/vox-gui/src/commands/mission_control.rs:26-48`), not an `{ is_error, result }` envelope.

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/SubAgents/subAgentClient.ts`
- `crates/vox-gui/ui/src/components/surfaces/SubAgents/subAgentClient.test.ts`
- `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentTree.tsx`
- `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentTree.test.tsx`

**Steps:**

- [ ] Replace the `fetchTree` cases in `subAgentClient.test.ts` with tests for the real command and the edge→tree mapping:

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
import { fetchTree, buildSubAgentTree } from './subAgentClient';

beforeEach(() => invokeMock.mockReset());

describe('fetchTree', () => {
  it('invokes the real list_subagent_tree command (flat edge list, no envelope)', async () => {
    invokeMock.mockResolvedValue([
      { task_id: 10, agent_id: 1, parent_agent_id: null, reason: 'root plan' },
      { task_id: 11, agent_id: 2, parent_agent_id: 1, reason: 'search docs' },
    ]);
    const tree = await fetchTree();
    expect(invokeMock).toHaveBeenCalledWith('list_subagent_tree');
    expect(tree).toHaveLength(1);
    expect(tree[0].windowId).toBe('agent-1');
    expect(tree[0].children[0].windowId).toBe('agent-2');
    expect(tree[0].children[0].depth).toBe(1);
  });
  it('returns [] on a non-array payload', async () => {
    invokeMock.mockResolvedValue(null);
    expect(await fetchTree()).toEqual([]);
  });
});

describe('buildSubAgentTree', () => {
  it('reports unknown token budgets as 0/0, never fabricated numbers', () => {
    const [root] = buildSubAgentTree([{ task_id: 1, agent_id: 7, parent_agent_id: null, reason: 'x' }]);
    expect(root.model.maxTokens).toBe(0);
    expect(root.usedTokens).toBe(0);
    expect(root.title).toContain('#1');
  });
});
```

- [ ] Run `pnpm vitest run src/components/surfaces/SubAgents/subAgentClient.test.ts` — expect FAIL (`buildSubAgentTree` doesn't exist; `fetchTree` invokes `'subagent_tree'` with an envelope unwrap).
- [ ] Rewrite `fetchTree` in `subAgentClient.ts` (keep `SUBAGENT_ACTIVITY_EVENT`/`listenActivity` untouched — `vox://agent-events` is real):

```ts
export interface SubagentTreeEdge {
  task_id: number;
  agent_id: number;
  parent_agent_id?: number | null;
  source_task_id?: number | null;
  reason: string;
}

/** Map the orchestrator's flat delegation edges into the SubAgentNode tree. */
export function buildSubAgentTree(edges: SubagentTreeEdge[]): SubAgentNode[] {
  const byId = new Map<string, SubAgentNode>();
  for (const e of edges) {
    byId.set(`agent-${e.agent_id}`, {
      windowId: `agent-${e.agent_id}`,
      parentWindowId: e.parent_agent_id != null ? `agent-${e.parent_agent_id}` : null,
      title: `task #${e.task_id} · ${e.reason}`,
      skill: null,
      // Honest unknowns: the daemon does not report model/token budgets on this
      // edge list. 0/0 renders as "budget unknown", never a fabricated number.
      model: { id: 'orchestrator', maxTokens: 0, toolCapable: false },
      status: 'running',
      usedTokens: 0,
      depth: 0,
      children: [],
    });
  }
  const roots: SubAgentNode[] = [];
  for (const n of byId.values()) {
    const parent = n.parentWindowId ? byId.get(n.parentWindowId) : undefined;
    if (parent) parent.children.push(n);
    else roots.push(n);
  }
  const stamp = (list: SubAgentNode[], depth: number) => {
    for (const n of list) { n.depth = depth; stamp(n.children, depth + 1); }
  };
  stamp(roots, 0);
  return roots;
}

export async function fetchTree(): Promise<SubAgentNode[]> {
  const edges = await invoke<SubagentTreeEdge[]>('list_subagent_tree');
  return buildSubAgentTree(Array.isArray(edges) ? edges : []);
}
```

- [ ] In `SubAgentTree.tsx`, guard the token chip: render the `tokenFate`/`{usedTokens}/{maxTokens}` chip (lines 28 and 43) only when `r.node.model.maxTokens > 0`; otherwise render nothing for that chip. Add a `SubAgentTree.test.tsx` case first: a node with `maxTokens: 0` must NOT render the text `0/0` (`expect(screen.queryByText('0/0')).toBeNull()`); run it, see FAIL, then implement.
- [ ] Run `pnpm vitest run src/components/surfaces/SubAgents` — expect PASS (update any other SubAgents tests broken by the removed envelope helper as part of this step, keeping their assertions honest to the new shapes).
- [ ] Commit: `git commit -m "fix(gui): wire SubAgents tree to real list_subagent_tree command" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

---

### Task 3: Hide the dead SubAgent controls/context editor and add `sub-agents` to nav resolution

`SubAgentControls` (`subagent_control`) and `SubAgentContextEditor` (`context_get`/`context_set`) call Tauri commands that are not registered in `crates/vox-gui/src/main.rs` — every click silently no-ops. Triage policy: dead + not cheap to wire ⇒ HIDE. The registry already parents `sub-agents` under `compute` (generated line 39), but `navigation.ts` has no entry, so clicking the Compute › Sub-Agents tab loses the tab bar and sidebar highlight.

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentsView.tsx`
- `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentsView.test.tsx`
- `crates/vox-gui/ui/src/lib/navigation.ts`
- `crates/vox-gui/ui/src/lib/navigation.test.ts`
- Delete: `SubAgentControls.tsx`, `SubAgentControls.test.tsx`, `SubAgentContextEditor.tsx`, `SubAgentContextEditor.test.tsx` (plus the now-dead `getContext`/`setContext`/`control` fns and `Envelope`/`call` helper in `subAgentClient.ts`)

**Steps:**

- [ ] Add failing tests. In `navigation.test.ts`: `expect(resolveNavigation('sub-agents')).toEqual({ parent: 'compute', child: 'sub-agents' });` and `expect(labelForNavKey('sub-agents')).toBe('Sub-Agents');`. In `SubAgentsView.test.tsx`: selecting a node must NOT render the overrule input — `expect(screen.queryByLabelText('overrule note')).toBeNull()` (reuse that file's existing store-seeding setup).

> **Coordination note (Plan 2):** if the IA reorg plan (ai-first-plan-2) has already landed, `sub-agents` belongs under `agents`, not `compute` — use `{ parent: 'agents', child: 'sub-agents' }` here and skip the registry edit (Plan 2 Task 2 already reparents it). The assertion above is for the pre-reorg topology only.

- [ ] Run `pnpm vitest run src/lib/navigation.test.ts src/components/surfaces/SubAgents/SubAgentsView.test.tsx` — expect the new cases to FAIL.
- [ ] `navigation.ts`: add `'sub-agents': { parent: 'compute', child: 'sub-agents' },` to `PARENT_CHILD_MAP` (after `mesh`) and `'sub-agents': 'Sub-Agents',` to `NAV_LABELS` (after `mesh: 'Mesh',`). (Lexicon already has `'sub-agents': { en: 'Sub-Agents', la: 'Subagentes' }` — no lexicon change.)
- [ ] `SubAgentsView.tsx`: remove the `SubAgentControls` and `SubAgentContextEditor` imports and their JSX (lines 54-55); the selected-node pane renders only `<SubAgentActivityStream windowId={node.windowId} />` plus the node title. Delete the four component/test files listed above. In `subAgentClient.ts`, delete `getContext`, `setContext`, `control`, and the `Envelope`/`call` helper. Run `pnpm typecheck` and grep `getContext\|setContext\|ControlAction\|ProjectionItem` under `src/` — remove any now-unreferenced types from `SubAgents/types.ts` and their cases from `types.test.ts` / `subAgentStore.ts` if (and only if) the grep shows them orphaned.
- [ ] Run `pnpm vitest run src/components/surfaces/SubAgents src/lib/navigation.test.ts` and `pnpm typecheck` — expect PASS.
- [ ] Commit: `git commit -m "fix(gui): hide dead sub-agent controls; resolve sub-agents nav" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

---

### Task 4: Build `useAttentionInbox` — one hook, one interval, three feeds

Today: `App.tsx:427-444` polls `vox_pending_approvals` every `APPROVALS_POLL_MS` (2 s), `App.tsx:550-595` polls `feedbackList()` + `hopper_list` every 5 s, and `NeedsYouSurface` runs its own 5 s poll. Consolidate the App-global ones.

**Files:**
- `crates/vox-gui/ui/src/hooks/useAttentionInbox.ts` (new)
- `crates/vox-gui/ui/src/hooks/useAttentionInbox.test.ts` (new)
- `crates/vox-gui/ui/src/config/constants.ts`

**Steps:**

- [ ] Write the failing test `src/hooks/useAttentionInbox.test.ts` (model the mocking on `useAgentApprovals.test.ts`):

```ts
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...a: unknown[]) => invokeMock(...a),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('../transport', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../transport')>()),
  feedbackList: vi.fn().mockResolvedValue({
    needsYou: [{ feedbackId: 'F-1', kind: 'doubt', prompt: 'p', options: [], gates: [7], doubtedTaskId: 7, surface: 'needs_you', infoGainBits: 1 }],
    withheld: [],
  }),
  listenFeedbackChanged: vi.fn().mockResolvedValue(() => {}),
  voxTransport: { invokeMcpTool: vi.fn().mockResolvedValue({ tool: 'vox_pending_approvals', is_error: false, result: { approvals: [{ approval_id: 'A-1', tool: 'bash', summary: 's', requested_at_ms: 0 }] } }) },
}));

import { useAttentionInbox } from './useAttentionInbox';
import { voxTransport } from '../transport';

beforeEach(() => {
  invokeMock.mockImplementation((cmd: string) =>
    cmd === 'hopper_list' ? Promise.resolve([{ item_id: 'h1', intent: 'x', priority: 1, state: 'blocked', task_id: 7 }]) : Promise.resolve(null));
});
afterEach(() => vi.restoreAllMocks());

describe('useAttentionInbox', () => {
  it('aggregates approvals + feedback + blocked hopper tasks with one total', async () => {
    const { result } = renderHook(() => useAttentionInbox());
    await waitFor(() => expect(result.current.totalCount).toBe(2)); // 1 approval + 1 needsYou
    expect(result.current.approvals).toHaveLength(1);
    expect(result.current.needsYou).toHaveLength(1);
    expect(result.current.blockedTasksCount).toBe(1); // task 7 gated by F-1
  });

  it('resolveApproval calls vox_resolve_approval then drops the row', async () => {
    const { result } = renderHook(() => useAttentionInbox());
    await waitFor(() => expect(result.current.approvals).toHaveLength(1));
    await act(() => result.current.resolveApproval('A-1', 'approved'));
    expect(voxTransport.invokeMcpTool).toHaveBeenCalledWith('vox_resolve_approval', { approval_id: 'A-1', outcome: 'approved' });
  });
});
```

- [ ] Run `pnpm vitest run src/hooks/useAttentionInbox.test.ts` — expect FAIL (module not found).
- [ ] Add `export const ATTENTION_POLL_MS = 5000;` to `src/config/constants.ts` (below `APPROVALS_POLL_MS`, comment: single interval for the unified Needs-You inbox).
- [ ] Implement `src/hooks/useAttentionInbox.ts`:

```ts
import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { voxTransport, feedbackList, feedbackResolve, listenFeedbackChanged, type FeedbackRow } from '../transport';
import { parsePendingApprovals, type McpInvokeResult, type PendingApprovalRow } from '../lib/mcpToolResult';
import { ATTENTION_POLL_MS } from '../config/constants';

interface HopperTaskDto { item_id: string; intent: string; priority: number; state: string; task_id: number; }

export interface AttentionInbox {
  approvals: PendingApprovalRow[];
  needsYou: FeedbackRow[];
  withheld: FeedbackRow[];
  blockedTasksCount: number;
  /** Items awaiting a human decision: pending approvals + needs-you feedback. */
  totalCount: number;
  refresh(): Promise<void>;
  resolveApproval(approvalId: string, outcome: 'approved' | 'rejected'): Promise<void>;
  resolveFeedback(feedbackId: string, action: Record<string, unknown>): Promise<void>;
}

export function useAttentionInbox(): AttentionInbox {
  const [approvals, setApprovals] = useState<PendingApprovalRow[]>([]);
  const [needsYou, setNeedsYou] = useState<FeedbackRow[]>([]);
  const [withheld, setWithheld] = useState<FeedbackRow[]>([]);
  const [blockedTasksCount, setBlockedTasksCount] = useState(0);

  const refresh = useCallback(async () => {
    const [approvalRes, feedback, tasks] = await Promise.all([
      voxTransport.invokeMcpTool('vox_pending_approvals', {}).catch(() => null),
      feedbackList().catch(() => ({ needsYou: [] as FeedbackRow[], withheld: [] as FeedbackRow[] })),
      invoke<HopperTaskDto[]>('hopper_list').catch(() => [] as HopperTaskDto[]),
    ]);
    setApprovals(approvalRes ? parsePendingApprovals(approvalRes as McpInvokeResult) : []);
    setNeedsYou(feedback.needsYou);
    setWithheld(feedback.withheld ?? []);
    const gates = new Set<number>(feedback.needsYou.flatMap((f) => f.gates ?? []));
    setBlockedTasksCount(tasks.filter((t) => gates.has(t.task_id)).length);
  }, []);

  useEffect(() => {
    refresh();
    let unFeedback: (() => void) | null = null;
    let unTasks: (() => void) | null = null;
    listenFeedbackChanged(() => { refresh(); }).then((u) => { unFeedback = u; }).catch(() => {});
    listen<void>('vox://tasks-changed', () => { refresh(); }).then((u) => { unTasks = u; }).catch(() => {});
    const id = setInterval(refresh, ATTENTION_POLL_MS);
    return () => { unFeedback?.(); unTasks?.(); clearInterval(id); };
  }, [refresh]);

  const resolveApproval = useCallback(async (approvalId: string, outcome: 'approved' | 'rejected') => {
    await voxTransport.invokeMcpTool('vox_resolve_approval', { approval_id: approvalId, outcome });
    setApprovals((prev) => prev.filter((a) => a.approval_id !== approvalId));
    await refresh();
  }, [refresh]);

  const resolveFeedback = useCallback(async (feedbackId: string, action: Record<string, unknown>) => {
    await feedbackResolve(feedbackId, action);
    await refresh();
  }, [refresh]);

  return { approvals, needsYou, withheld, blockedTasksCount, totalCount: approvals.length + needsYou.length, refresh, resolveApproval, resolveFeedback };
}
```

- [ ] Run `pnpm vitest run src/hooks/useAttentionInbox.test.ts` — expect PASS.
- [ ] Commit: `git commit -m "feat(gui): useAttentionInbox — one hook/interval for approvals, doubts, blocked tasks" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

---

### Task 5: Adopt the hook in App.tsx and unify the nav badge

**Files:**
- `crates/vox-gui/ui/src/App.tsx`
- `crates/vox-gui/ui/src/components/layout/AppShell.tsx`
- `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`
- `crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx` / `AppShell.test.tsx`
- `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`

**Steps:**

- [ ] Add/adjust a failing Sidebar test (in `Sidebar.test.tsx`, matching its render helpers): rename the badge prop and assert the aria copy — render with `needsYouCount={3}` and expect `screen.getByLabelText('Runs and Approvals, 3 items need you')`. Run `pnpm vitest run src/components/layout/Sidebar.test.tsx` — FAIL (prop is `approvalsPending`, aria says "pending").
- [ ] `Sidebar.tsx`: rename prop `approvalsPending` → `needsYouCount` (interface line 79, destructure line 93, badge expr line 169, aria lines 171-176; new aria string: `` `Runs and Approvals, ${needsYouCount} items need you` ``). `AppShell.tsx`: rename the pass-through prop the same way (lines 27, 61, 98); keep feeding `TopHud pendingApprovals` from a new explicit `pendingApprovals` prop (line 120) so the HUD tile stays approvals-only and honest.
- [ ] `App.tsx`: delete the approvals-badge effect (lines 426-444) with its `approvalsPending` state (line 234), and delete `refreshAttentionCounts` + its effect and the `needsYouCount`/`blockedTasksCount` states (lines 546-595). Add `const attention = useAttentionInbox();` near the other hooks. Feed: `AppShell needsYouCount={attention.totalCount} pendingApprovals={attention.approvals.length}`; `AttentionStrip waitingQuestions={attention.needsYou.length} blockedTasks={attention.blockedTasksCount}` (line 1161); add `attention` to `surfaceProps` (line 1095 block).
- [ ] `surfaceComponents.tsx`: add `attention?: AttentionInbox;` to `SurfaceProps` (import the type from `../../hooks/useAttentionInbox`) and pass `attention={props.attention}` in `case 'needs-you'` (line 169-170). Leave `NeedsYouSurface`'s existing props working — Task 6 consumes it.
- [ ] Run `pnpm vitest run src/components/layout src/App.test.tsx` and `pnpm typecheck` — expect PASS (fix any test renders that passed the old prop names).
- [ ] Commit: `git commit -m "refactor(gui): single attention poll in App; unified Needs-You nav badge" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

> **Coordination note (Plan 2):** Plan 2 Task 4 also touches the Sidebar aria-label block (its version says "Review, N pending approvals"). Whichever plan lands second reconciles the aria copy to: `` `Review, ${needsYouCount} items need you` `` with the `needsYouCount` prop name from this task.

---

### Task 6: Upgrade NeedsYouSurface into the unified inbox (approvals + doubts/questions + withheld) with Limes tokens

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/NeedsYou/NeedsYouSurface.tsx`
- `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx`
- `crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/NeedsYouSurface.test.tsx`

**Steps:**

- [ ] Extend `__tests__/NeedsYouSurface.test.tsx` (keep the existing two tests; they must stay green via the legacy self-fetch path). Add:

```ts
const attention = {
  approvals: [{ approval_id: 'A-1', tool: 'bash', summary: 'rm -rf build', requested_at_ms: 0 }],
  needsYou: [], withheld: [], blockedTasksCount: 0, totalCount: 1,
  refresh: vi.fn(), resolveApproval: vi.fn().mockResolvedValue(undefined), resolveFeedback: vi.fn().mockResolvedValue(undefined),
};

it('renders an Approvals section from the shared inbox and resolves inline', async () => {
  render(<LanguageProvider><NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} attention={attention} /></LanguageProvider>);
  expect(await screen.findByText('rm -rf build')).toBeTruthy();
  fireEvent.click(screen.getByRole('button', { name: /approve rm -rf build|^approve$/i }));
  await waitFor(() => expect(attention.resolveApproval).toHaveBeenCalledWith('A-1', 'approved'));
});

it('does not start its own poll when the shared inbox is provided', async () => {
  const spy = vi.spyOn(transport, 'feedbackList');
  render(<LanguageProvider><NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} attention={{ ...attention, approvals: [] }} /></LanguageProvider>);
  await waitFor(() => expect(screen.getByText(/Nothing needs you/i)).toBeTruthy());
  expect(spy).not.toHaveBeenCalled();
});
```

- [ ] Run `pnpm vitest run src/components/surfaces/NeedsYou` — expect the new cases to FAIL (no `attention` prop).
- [ ] Implement in `NeedsYouSurface.tsx`: add `attention?: AttentionInbox` to `Props`. When `attention` is provided, source `needsYou`/`withheld`/`approvals` from it and skip the internal `refresh` effect entirely (no `feedbackList` call, no interval, no listener — the App owns polling); `handleResolve` delegates to `attention.resolveFeedback` (keep the same toasts). When absent (embedded mini-render / legacy), keep the existing self-fetch path unchanged. Render order inside the scroll area: **Approvals** section (only when non-empty) → **Questions & doubts** (existing cards / empty state) → **Withheld by policy** details. Approvals rows reuse the row markup pattern from `InlineApprovals.tsx:111-143` (tool + summary + Reject/Approve buttons calling `attention.resolveApproval`, `aria-label={\`Approve ${a.summary}\`}` / `Reject …`, with the same ok/warn toasts).
- [ ] Limes restyle in the same file (tokens only, per `ds/conventions.md`): root `bg-zinc-950/80` → remove (transparent); header `border-b border-white/[0.08]` stays a **heading underline** but becomes `border-b border-border-subtle` with `text-text-primary`; each new section heading uses `<h3 className="ds-section-head">` (the underline pattern already used by `MissionControlPanel.tsx:239`) — never a `border-t` capping from above; loading text `text-zinc-500` → `text-text-muted`; withheld `border-zinc-800/80 bg-zinc-900/10` → `border-border-subtle bg-overlay-subtle`, summary text → `text-text-muted hover:text-text-secondary`. In `FeedbackCard.tsx`: `border-zinc-800` → `border-border-subtle`, `text-zinc-500` → `text-text-muted`, `text-zinc-200` → `text-text-secondary`, `border-zinc-700 text-zinc-400 hover:bg-white/[0.02]` → `border-border-subtle text-text-muted hover:bg-overlay-subtle` (leave the emerald action tones — they match the app-wide ok-tone pattern, e.g. `SkillsPluginsView.tsx:605`).
- [ ] Run `pnpm vitest run src/components/surfaces/NeedsYou` — expect PASS (all four tests, old and new).
- [ ] Commit: `git commit -m "feat(gui): NeedsYou is the unified attention inbox (approvals + doubts), Limes tokens" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

---

### Task 7: `lib/intentSpec.ts` — pure intent serialization

Backend contract (verified): `submit_orchestrator_task` takes `description` + `priority` where priority parses `"urgent" | "normal" | "background"` (`crates/vox-gui/src/commands/control_plane.rs:55-57`). Intent rides entirely in those two existing fields — no `ChatPayload`/Rust change.

**Files:**
- `crates/vox-gui/ui/src/lib/intentSpec.ts` (new)
- `crates/vox-gui/ui/src/lib/intentSpec.test.ts` (new)

**Steps:**

- [ ] Write the failing test `src/lib/intentSpec.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { EMPTY_INTENT, hasIntent, composeDescription, effortToPriority, type IntentFields } from './intentSpec';

const intent = (over: Partial<IntentFields>): IntentFields => ({ ...EMPTY_INTENT, ...over });

describe('intentSpec', () => {
  it('plain text passes through untouched when no intent fields are set', () => {
    expect(composeDescription('fix the login bug', EMPTY_INTENT)).toBe('fix the login bug');
    expect(hasIntent(EMPTY_INTENT)).toBe(false);
  });
  it('appends markdown sections for filled fields only', () => {
    const d = composeDescription('fix login', intent({ goal: 'users stay signed in', acceptance: 'refresh keeps session' }));
    expect(d).toBe('fix login\n\n## Goal\nusers stay signed in\n\n## Acceptance criteria\nrefresh keeps session');
    expect(d).not.toContain('## Constraints');
  });
  it('goal alone can carry the task (empty free text)', () => {
    expect(composeDescription('', intent({ goal: 'ship dark mode' }))).toBe('ship dark mode');
  });
  it('maps effort onto the backend TaskPriority strings', () => {
    expect(effortToPriority('urgent')).toBe('urgent');
    expect(effortToPriority('background')).toBe('background');
    expect(effortToPriority('')).toBeNull();
  });
});
```

- [ ] Run `pnpm vitest run src/lib/intentSpec.test.ts` — FAIL (module not found).
- [ ] Implement `src/lib/intentSpec.ts`:

```ts
/** Structured-intent fields for the composer. Effort maps 1:1 onto the
 *  orchestrator TaskPriority strings accepted by submit_orchestrator_task
 *  (crates/vox-gui/src/commands/control_plane.rs). */
export type Effort = '' | 'background' | 'normal' | 'urgent';

export interface IntentFields {
  goal: string;
  constraints: string;
  acceptance: string;
  effort: Effort;
}

export const EMPTY_INTENT: IntentFields = { goal: '', constraints: '', acceptance: '', effort: '' };

export function hasIntent(i: IntentFields): boolean {
  return Boolean(i.goal.trim() || i.constraints.trim() || i.acceptance.trim() || i.effort);
}

function section(heading: string, value: string): string {
  return value.trim() ? `\n\n## ${heading}\n${value.trim()}` : '';
}

/** Compose the task description: free text first; goal promotes to the head
 *  line when free text is empty (goal-only submits stay valid). */
export function composeDescription(text: string, i: IntentFields): string {
  const head = text.trim() || i.goal.trim();
  const goalSection = text.trim() && i.goal.trim() ? section('Goal', i.goal) : '';
  return `${head}${goalSection}${section('Constraints', i.constraints)}${section('Acceptance criteria', i.acceptance)}`;
}

export function effortToPriority(effort: Effort): string | null {
  return effort === '' ? null : effort;
}
```

- [ ] Run `pnpm vitest run src/lib/intentSpec.test.ts` — expect PASS.
- [ ] Commit: `git commit -m "feat(gui): intentSpec — serialize structured intent into task description/priority" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

---

### Task 8: `IntentPanel` component (collapsible fields, tokens only)

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/Loquela/IntentPanel.tsx` (new)
- `crates/vox-gui/ui/src/components/surfaces/Loquela/IntentPanel.test.tsx` (new)

**Steps:**

- [ ] Write the failing test `IntentPanel.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { IntentPanel } from './IntentPanel';
import { EMPTY_INTENT } from '../../../lib/intentSpec';

describe('IntentPanel', () => {
  it('exposes labelled fields for goal, constraints, acceptance and effort', () => {
    render(<IntentPanel intent={EMPTY_INTENT} onChange={() => {}} />);
    expect(screen.getByLabelText('Goal')).toBeDefined();
    expect(screen.getByLabelText('Constraints')).toBeDefined();
    expect(screen.getByLabelText('Acceptance criteria')).toBeDefined();
    expect(screen.getByLabelText('Effort')).toBeDefined();
  });
  it('reports field edits upward as partial patches', () => {
    const onChange = vi.fn();
    render(<IntentPanel intent={EMPTY_INTENT} onChange={onChange} />);
    fireEvent.change(screen.getByLabelText('Goal'), { target: { value: 'ship dark mode' } });
    expect(onChange).toHaveBeenCalledWith({ goal: 'ship dark mode' });
    fireEvent.change(screen.getByLabelText('Effort'), { target: { value: 'urgent' } });
    expect(onChange).toHaveBeenCalledWith({ effort: 'urgent' });
  });
});
```

- [ ] Run `pnpm vitest run src/components/surfaces/Loquela/IntentPanel.test.tsx` — FAIL.
- [ ] Implement `IntentPanel.tsx` (styling follows the composer's existing input treatment — see the add-source input in `SkillsPluginsView.tsx:510-517`; tokens only, no zinc/hex):

```tsx
import React from 'react';
import type { Effort, IntentFields } from '../../../lib/intentSpec';

interface IntentPanelProps {
  intent: IntentFields;
  onChange: (patch: Partial<IntentFields>) => void;
}

const FIELD_CLS =
  'w-full rounded-md border border-border-subtle bg-overlay-subtle px-2 py-1.5 text-[12px] text-text-primary placeholder:text-text-muted focus:outline-none focus:ring-1 focus:ring-brass/40';
const LABEL_CLS = 'font-display text-[9px] uppercase tracking-[0.22em] text-text-muted';

export function IntentPanel({ intent, onChange }: IntentPanelProps) {
  return (
    <div className="mt-2 grid grid-cols-1 gap-2 border-t border-border-subtle pt-2 sm:grid-cols-2" data-testid="intent-panel">
      <label className="flex flex-col gap-1 sm:col-span-2">
        <span className={LABEL_CLS}>Goal</span>
        <input aria-label="Goal" value={intent.goal} placeholder="What outcome should the agent achieve?"
          onChange={(e) => onChange({ goal: e.target.value })} className={FIELD_CLS} />
      </label>
      <label className="flex flex-col gap-1">
        <span className={LABEL_CLS}>Constraints</span>
        <textarea aria-label="Constraints" rows={2} value={intent.constraints} placeholder="Boundaries: don't touch X, keep API stable…"
          onChange={(e) => onChange({ constraints: e.target.value })} className={`${FIELD_CLS} resize-none`} />
      </label>
      <label className="flex flex-col gap-1">
        <span className={LABEL_CLS}>Acceptance criteria</span>
        <textarea aria-label="Acceptance criteria" rows={2} value={intent.acceptance} placeholder="How you'll judge the work is done"
          onChange={(e) => onChange({ acceptance: e.target.value })} className={`${FIELD_CLS} resize-none`} />
      </label>
      <label className="flex flex-col gap-1">
        <span className={LABEL_CLS}>Effort</span>
        <select aria-label="Effort" value={intent.effort}
          onChange={(e) => onChange({ effort: e.target.value as Effort })} className={FIELD_CLS}>
          <option value="">default</option>
          <option value="background">background — when idle</option>
          <option value="normal">normal</option>
          <option value="urgent">urgent — jump the queue</option>
        </select>
      </label>
    </div>
  );
}
```

- [ ] Run `pnpm vitest run src/components/surfaces/Loquela/IntentPanel.test.tsx` — expect PASS.
- [ ] Commit: `git commit -m "feat(gui): IntentPanel — structured intent fields for the composer" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

---

### Task 9: Integrate the intent panel into Loquela (additive — plain text unchanged)

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx`
- `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.test.tsx`

**Steps:**

- [ ] Add failing tests to `Loquela.test.tsx` (reuse its `renderLoquela` helper; note the existing module mocks at the top of the file):

```tsx
it('intent panel is collapsed by default and toggles open', () => {
  renderLoquela();
  expect(screen.queryByLabelText('Goal')).toBeNull();
  const toggle = screen.getByRole('button', { name: /structured intent/i });
  expect(toggle.getAttribute('aria-expanded')).toBe('false');
  fireEvent.click(toggle);
  expect(screen.getByLabelText('Goal')).toBeDefined();
});

it('serializes intent fields into the submitted description and priority', () => {
  const onSubmit = vi.fn();
  renderLoquela({ onSubmit });
  fireEvent.click(screen.getByRole('button', { name: /structured intent/i }));
  fireEvent.change(screen.getByLabelText('Goal'), { target: { value: 'ship dark mode' } });
  fireEvent.change(screen.getByLabelText('Acceptance criteria'), { target: { value: 'toggle persists' } });
  fireEvent.change(screen.getByLabelText('Effort'), { target: { value: 'urgent' } });
  const ta = screen.getByLabelText('Task composer');
  fireEvent.change(ta, { target: { value: 'add a theme switch' } });
  fireEvent.keyDown(ta, { key: 'Enter' });
  const payload = onSubmit.mock.calls[0][0];
  expect(payload.description).toContain('add a theme switch');
  expect(payload.description).toContain('## Goal\nship dark mode');
  expect(payload.description).toContain('## Acceptance criteria\ntoggle persists');
  expect(payload.priority).toBe('urgent');
});

it('goal alone is submittable without free text', () => {
  const onSubmit = vi.fn();
  renderLoquela({ onSubmit });
  fireEvent.click(screen.getByRole('button', { name: /structured intent/i }));
  fireEvent.change(screen.getByLabelText('Goal'), { target: { value: 'ship dark mode' } });
  fireEvent.click(screen.getByRole('button', { name: /run/i }));
  expect(onSubmit.mock.calls[0][0].description).toBe('ship dark mode');
});
```

- [ ] Run `pnpm vitest run src/components/surfaces/Loquela/Loquela.test.tsx` — new cases FAIL.
- [ ] Implement in `Loquela.tsx`: import `IntentPanel` and `{ EMPTY_INTENT, hasIntent, composeDescription, effortToPriority, type IntentFields }` from `../../../lib/intentSpec`. Add state `const [intent, setIntent] = useState<IntentFields>(EMPTY_INTENT); const [intentOpen, setIntentOpen] = useState(false);`. Update `send()` (line 404): `const canSend = !!text.trim() || !!intent.goal.trim(); if (!canSend) return;` then build `description: composeDescription(text, intent)` and add `priority: effortToPriority(intent.effort)` to the payload; after submit also `setIntent(EMPTY_INTENT)`. Update the Run button's `disabled`/tone condition (line 568-570) from `!text.trim()` to `!canSend` (hoist `canSend` above the JSX). Render `{intentOpen && <IntentPanel intent={intent} onChange={(p) => setIntent((i) => ({ ...i, ...p }))} />}` directly under the textarea row (after the closing `</div>` of the `relative flex items-end gap-2` block, inside the Glass). Add the toggle in the footer controls row (next to the tier button, line ~600):

```tsx
<button type="button" aria-label="Structured intent" aria-expanded={intentOpen}
  onClick={() => setIntentOpen((o) => !o)}
  className={`inline-flex items-center gap-1 rounded-md border px-2 py-1 transition ${
    hasIntent(intent) || intentOpen
      ? 'border-brass/40 bg-brass/10 text-brass'
      : 'border-border-subtle bg-overlay-subtle text-text-secondary hover:border-white/20'
  }`}>
  <Icon.list className="size-3" aria-hidden="true" /> Intent{hasIntent(intent) ? ' ·' : ''}
</button>
```

  (If `Icon.list` does not exist in `components/ui/Icons.tsx`, use an icon that does — check the file; do not add a new icon.)
- [ ] Run `pnpm vitest run src/components/surfaces/Loquela/Loquela.test.tsx` — expect ALL Loquela tests PASS (old + new).
- [ ] Commit: `git commit -m "feat(gui): structured intent panel in the composer — goal/constraints/effort/acceptance" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

---

### Task 10: Full verification + close the ledger

**Files:**
- `docs/agents/gui-honesty-triage.md`

**Steps:**

- [ ] From `crates/vox-gui/ui` run the full suite exactly as CI does: `pnpm test` (this is `vitest run`) — expect 0 failures. Then `pnpm typecheck` — expect clean.
- [ ] Grep for regressions in what was consolidated: `grep -rn "vox_pending_approvals" crates/vox-gui/ui/src --include="*.tsx" --include="*.ts" | grep -v test` must show only `useAttentionInbox.ts`, `useAgentApprovals.ts` (Dashboard rows), `ApprovalsView.tsx`, and `InlineApprovals.tsx` — App.tsx must be gone from the list.
- [ ] Update `docs/agents/gui-honesty-triage.md`: append a dated "Burn-down status (2026-07-02)" section marking each WIRE/HIDE/TOAST-FIX row with its resolution — SkillsPlugins ×4 done (detail panel), ContextWindowMeter WIRED (superseded HIDE), StreamCard Doubt/Overrule WIRED (superseded HIDE), Keybinds WIRED (superseded HIDE), needs-you ADD-to-nav done (Task 1), sub-agents wired-tree + hidden dead controls (Tasks 2-3).
- [ ] Commit: `git commit -m "docs: record honesty-triage burn-down; verify full vox-gui suite green" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"`

---

### Critical Files for Implementation
- `crates/vox-gui/ui/src/App.tsx` (owns the polls being consolidated at lines 234, 426-444, 546-595; submit path `handleLoquelaSubmit` at 709)
- `crates/vox-gui/ui/src/hooks/useAttentionInbox.ts` (new — the single attention feed)
- `crates/vox-gui/ui/src/components/surfaces/NeedsYou/NeedsYouSurface.tsx` (becomes the unified inbox)
- `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx` (composer integration; `send()` at line 404)
- `crates/vox-gui/ui/src/lib/navigation.ts` (nav SSOT for `needs-you` / `sub-agents`)
