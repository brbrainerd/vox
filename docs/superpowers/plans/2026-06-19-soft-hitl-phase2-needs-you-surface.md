# Soft HITL Phase 2 — Needs You Surface + Blocked Overlay (rev 2)

> 🤖 **EXECUTION TARGET — READ FIRST.** Gemini Flash in Antigravity. See
> `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`.

**Operating Rules (EVERY task):**
1. Atomic + green + committed.
2. Every Step-1 `rg` is a BLOCKING gate; reality differs → STOP and report.
3. Two-strike circuit breaker.
4. Split on overrun (one atomic commit per file/fn).
5. House rules: pnpm package; `npx vitest run <path>` / `npx tsc --noEmit` work; `// @vitest-environment jsdom` as FIRST line of every component test; vox-gui Rust builds **lib-only** (`cargo clippy -p vox-gui --lib`); never `cargo fmt --all`. Reuse design-system components (`Glass`, `Pill`, `Icon`, `EmptyState`, `DataTable`) — do not hand-roll raw Tailwind for interactive controls, and give every button an `aria-label`. No stubs.
6. Tags: `[PARALLEL-SAFE]` / `[SEQUENTIAL]`.

**Goal:** The unified "Needs You" surface (clarifications + doubts, typed actions, click-to-context), a computed **blocked overlay** on the Tasks surface (no hopper mutation), real attention-strip counts, and retirement of the Dashboard `StreamCard` doubt buttons.

**Architecture:** Transport reuses the existing `invoke_mcp_tool` command (like `ApprovalsView`) — no new Tauri commands. Reactivity rides `vox://agent-events` (the `vox://activity-appended` signal has no Rust emitter). "Blocked" is derived: a Tasks row is blocked when its `TaskId` ∈ union of open `FeedbackRequest.gates`; the Rust `HopperTaskDto` gains a `task_id` field for the join.

**Tech Stack:** TypeScript/React, Tailwind, vitest; one small vox-gui Rust change. Depends on Phase 1 (tools `vox_feedback_list`/`vox_resolve_feedback`, events) + Phase 0 (strip).

**Spec:** `docs/superpowers/specs/2026-06-19-attention-aware-soft-hitl-design.md` §5.2, §5.3, §6

---

## Flash Execution Addendum (2026-06-19)

**Global gates:**
- **Real paths:** transport is `crates/vox-gui/ui/src/transport.ts` (NOT `lib/transport.ts`). Status types are `crates/vox-gui/ui/src/types/tauri.ts`. There is no `lib/orchestratorStatus.ts`.
- **Transport pattern:** call `invoke('invoke_mcp_tool', { tool: 'vox_feedback_list', args: {} })` exactly as `ApprovalsView.tsx` does for `vox_pending_approvals`. Do NOT add new `#[tauri::command]`s and do NOT use `VoxDb::connect_canonical` (that reads a different hopper instance than the daemon's in-memory `FeedbackStore`).
- **Reactivity:** subscribe to `vox://agent-events` (`AGENT_EVENTS_EVENT`) and refresh when `frame.kind.type` ∈ `{feedback_requested, feedback_resolved}` (snake_case, from Phase 1). Do NOT subscribe to `vox://activity-appended` (dead — no Rust emitter).
- **Surface registration is 4 places:** (1) the `View` string-union in `App.tsx`; (2) `contracts/gui/surface-registry.v1.yaml` (snake_case `view_key`, NOT a TS source); (3) `vox ci gui-surface-registry --write` to regen `surfaceRegistry.generated.ts` (never hand-edit); (4) the `childRenderer` switch in `surfaceComponents.tsx`. The `gui-surface-registry` CI gate FAILS if the literal `'needs-you'` is absent from `App.tsx`.
- **Doubts vs clarifications:** doubts are pinned top (actionable; `info_gain_bits == 0` by construction — never sort by it); clarifications sorted by info gain below.

**Mandatory pre-flight:**
```
rg -n "invoke_mcp_tool|vox_pending_approvals|AGENT_EVENTS_EVENT|setInterval" crates/vox-gui/ui/src/components/surfaces/Approvals/ApprovalsView.tsx
rg -n "ACTIVITY_APPENDED_EVENT|AGENT_EVENTS_EVENT|listenActivityAppended|invoke" crates/vox-gui/ui/src/transport.ts
rg -n "interface TaskRow|interface GroupedTasks|function groupTasks" crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts
rg -n "dto.state|lifecycle|hopper_list" crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx
rg -n "type View|View =|navigateTo|childRenderer|case 'activity'" crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx
rg -n "view_key: activity|view_key: approvals" contracts/gui/surface-registry.v1.yaml
rg -n "Icon\\.|export const Icon" crates/vox-gui/ui/src/components/ui/Icons.tsx
rg -n "HopperTaskDto|hopper_item_to_dto|stable_hash" crates/vox-gui/src/commands/orchestrator.rs crates/vox-orchestrator/src/orchestrator/dispatch.rs
```
Expected: `ApprovalsView` uses `invoke('invoke_mcp_tool', {tool, args})` + `setInterval` poll; `AGENT_EVENTS_EVENT` exists in transport.ts; `TaskRow` has `id`/`lifecycle` (no `item_id`/`state`); `GroupedTasks = {inProgress, queued}`; `Icon` has no `bell` (use `alert` or `eye`); `stable_hash` in dispatch.rs (~:19) computes `TaskId` from `item_id.0`.

**Task-split table:**

| Task | Touches | Tag |
|---|---|---|
| 1 — transport helpers | `transport.ts` (+test) | [PARALLEL-SAFE] |
| 2 — FeedbackCard | `NeedsYou/FeedbackCard.tsx` (+test) | [PARALLEL-SAFE] |
| 3 — NeedsYouSurface | `NeedsYou/NeedsYouSurface.tsx` (+test) | [SEQUENTIAL after 1,2] |
| 4 — `task_id` on HopperTaskDto (Rust) | `vox-gui/src/commands/orchestrator.rs`, expose hash in vox-orchestrator | [PARALLEL-SAFE] |
| 5 — blocked overlay on Tasks | `tasksHelpers.ts`, `TasksView.tsx` (+test) | [SEQUENTIAL] |
| 6 — register surface | `App.tsx`, `surface-registry.v1.yaml`, `surfaceComponents.tsx` | [SEQUENTIAL] |
| 7 — chat-focus + retire Dashboard buttons + strip counts | `App.tsx`, `Dashboard/StreamCard.tsx`, `Dashboard/Dashboard.tsx` | [SEQUENTIAL] |

---

### Task 1 — Feedback transport helpers [PARALLEL-SAFE]

**Files:** Modify `crates/vox-gui/ui/src/transport.ts`; test `crates/vox-gui/ui/src/__tests__/feedbackTransport.test.ts`.

- [ ] **Step 1 (gate):** confirm `invoke_mcp_tool` usage + `AGENT_EVENTS_EVENT` per pre-flight.

- [ ] **Step 2: Write the failing test**
```ts
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { normalizeFeedback } from '../transport';
describe('normalizeFeedback', () => {
  it('splits needs_you from withheld and pins doubts first', () => {
    const raw = { needs_you: [
      { feedback_id:'F-1', kind:'clarification', prompt:'q', options:['a'], gates:[7], doubted_task_id:null, surface:'needs_you', info_gain_bits:0.8 },
      { feedback_id:'F-2', kind:'doubt', prompt:'d', options:[], gates:[], doubted_task_id:9, surface:'needs_you', info_gain_bits:0 },
    ], withheld: [
      { feedback_id:'F-3', kind:'clarification', prompt:'low', options:[], gates:[], doubted_task_id:null, surface:'withheld', info_gain_bits:0.05 },
    ]};
    const { needsYou, withheld } = normalizeFeedback(raw);
    expect(needsYou[0].feedbackId).toBe('F-2');   // doubt pinned first
    expect(needsYou[1].feedbackId).toBe('F-1');   // clarification by gain
    expect(withheld.map(r=>r.feedbackId)).toEqual(['F-3']);
  });
});
```

- [ ] **Step 3:** run → FAIL. **Step 4: Implement** (standalone exports near `listenActivityAppended`; imports already present at top of file):
```ts
export interface FeedbackRow {
  feedbackId: string;
  kind: 'clarification' | 'doubt';
  prompt: string; options: string[]; gates: number[];
  doubtedTaskId: number | null;
  surface: 'needs_you' | 'withheld';
  infoGainBits: number;
}
const toRow = (r: any): FeedbackRow => ({
  feedbackId: r.feedback_id, kind: r.kind, prompt: r.prompt, options: r.options ?? [],
  gates: r.gates ?? [], doubtedTaskId: r.doubted_task_id ?? null, surface: r.surface,
  infoGainBits: r.info_gain_bits ?? 0,
});
export function normalizeFeedback(raw: any): { needsYou: FeedbackRow[]; withheld: FeedbackRow[] } {
  const ny = (raw?.needs_you ?? []).map(toRow).sort((a: FeedbackRow, b: FeedbackRow) => {
    if (a.kind !== b.kind) return a.kind === 'doubt' ? -1 : 1; // doubts pinned top
    return b.infoGainBits - a.infoGainBits;
  });
  return { needsYou: ny, withheld: (raw?.withheld ?? []).map(toRow) };
}
export async function feedbackList() {
  const res = await invoke<string>('invoke_mcp_tool', { tool: 'vox_feedback_list', args: {} });
  return normalizeFeedback(JSON.parse(res)); // ToolResult json -> {needs_you, withheld}
}
// action: { action:'answer', option, text } | { action:'skip' } | { action:'overrule' } | { action:'let_verify' }
export async function feedbackResolve(feedbackId: string, action: Record<string, unknown>) {
  await invoke('invoke_mcp_tool', { tool: 'vox_resolve_feedback', args: { feedback_id: feedbackId, action } });
}
export function listenFeedbackChanged(onChange: () => void): Promise<UnlistenFn> {
  return listen<any>(AGENT_EVENTS_EVENT, (e) => {
    const t = e?.payload?.kind?.type;
    if (t === 'feedback_requested' || t === 'feedback_resolved') onChange();
  });
}
```
(Adjust `vox_feedback_list` result unwrapping to the real `ToolResult` shape from the Phase-1 `feedback_list` handler — it returns `ToolResult::ok(json!({...})).to_json()`; parse accordingly. Confirm `AGENT_EVENTS_EVENT` is exported from transport.ts; if not, import from its module.)

- [ ] **Step 5:** run → PASS. `npx tsc --noEmit`. **Step 6: Commit** `feat(gui): feedback transport via invoke_mcp_tool + agent-events`.

---

### Task 2 — FeedbackCard (typed actions, design system) [PARALLEL-SAFE]

**Files:** Create `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx`; test alongside.

- [ ] **Step 1 (gate):** `rg -n "phase=|tone=|export function Pill|Icon\\.gavel|Icon\\.doubt" crates/vox-gui/ui/src/components/ui/Pill.tsx crates/vox-gui/ui/src/components/ui/Icons.tsx` — confirm `Pill` props (`phase` tones incl. `Doubted`/`Verifying`) and that `Icon.gavel`/`Icon.doubt` exist.

- [ ] **Step 2: Write the failing test**
```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FeedbackCard } from '../FeedbackCard';
const clar = { feedbackId:'F-1', kind:'clarification' as const, prompt:'schema?', options:['In hopper','Separate'], gates:[7], doubtedTaskId:null, surface:'needs_you' as const, infoGainBits:0.8 };
const doubt = { feedbackId:'F-2', kind:'doubt' as const, prompt:'suspect', options:[], gates:[], doubtedTaskId:9, surface:'needs_you' as const, infoGainBits:0 };
describe('FeedbackCard', () => {
  it('clarification: option click resolves with answer action', () => {
    const onResolve = vi.fn();
    render(<FeedbackCard row={clar} onResolve={onResolve} onOpenContext={()=>{}} />);
    fireEvent.click(screen.getByText('Separate'));
    expect(onResolve).toHaveBeenCalledWith('F-1', { action:'answer', option:1, text:null });
  });
  it('doubt: overrule resolves with overrule action', () => {
    const onResolve = vi.fn();
    render(<FeedbackCard row={doubt} onResolve={onResolve} onOpenContext={()=>{}} />);
    fireEvent.click(screen.getByLabelText(/overrule/i));
    expect(onResolve).toHaveBeenCalledWith('F-2', { action:'overrule' });
  });
});
```

- [ ] **Step 3:** run → FAIL. **Step 4: Implement** with `Glass`/`Pill`/`Icon`; every button has `aria-label`:
```tsx
import { Glass } from '../../ui/Glass';
import { Pill } from '../../ui/Pill';
import { Icon } from '../../ui/Icons';
import type { FeedbackRow } from '../../../transport';
interface Props {
  row: FeedbackRow;
  onResolve: (id: string, action: Record<string, unknown>) => void;
  onOpenContext: (id: string) => void;
}
export function FeedbackCard({ row, onResolve, onOpenContext }: Props) {
  const isDoubt = row.kind === 'doubt';
  return (
    <Glass className="p-3 border-b border-zinc-800">
      <div className="flex items-center gap-2 mb-1">
        <Pill phase={isDoubt ? 'Doubted' : 'Verifying'}>{isDoubt ? 'Doubt' : 'Clarification'} · {row.feedbackId}</Pill>
        {row.gates.length > 0 && <span className="text-[11px] text-zinc-500">parks {row.gates.length} task{row.gates.length>1?'s':''}</span>}
      </div>
      <button className="text-xs text-zinc-200 mb-2 text-left block w-full" aria-label="Open context" onClick={() => onOpenContext(row.feedbackId)}>{row.prompt}</button>
      <div className="flex gap-1.5 flex-wrap">
        {isDoubt ? (<>
          <button aria-label="Overrule the doubt" className="text-[11px] font-semibold px-2.5 py-1 rounded border border-emerald-400/30 text-emerald-300 bg-emerald-400/10 inline-flex items-center gap-1" onClick={() => onResolve(row.feedbackId, { action:'overrule' })}><Icon.gavel className="size-3.5" />Overrule</button>
          <button aria-label="Let the agent verify" className="text-[11px] font-semibold px-2.5 py-1 rounded border border-zinc-700 text-zinc-400" onClick={() => onResolve(row.feedbackId, { action:'let_verify' })}>Let it verify</button>
        </>) : (<>
          {row.options.map((opt, i) => (
            <button key={i} aria-label={`Answer: ${opt}`} className="text-[11px] font-semibold px-2.5 py-1 rounded border border-emerald-400/30 text-emerald-300 bg-emerald-400/10" onClick={() => onResolve(row.feedbackId, { action:'answer', option:i, text:null })}>{opt}</button>
          ))}
          <button aria-label="Answer in free text" className="text-[11px] font-semibold px-2.5 py-1 rounded border border-zinc-700 text-zinc-400" onClick={() => onOpenContext(row.feedbackId)}>✎ Answer…</button>
          <button aria-label="Skip this question" className="text-[11px] font-semibold px-2.5 py-1 rounded border border-zinc-700 text-zinc-400" onClick={() => onResolve(row.feedbackId, { action:'skip' })}>Skip</button>
        </>)}
      </div>
    </Glass>
  );
}
```
(If `Pill`/`Glass`/`Icon` APIs differ from the gate output, adapt to the real signatures — do not invent props.)

- [ ] **Step 5:** run → PASS. **Step 6: Commit** `feat(gui): FeedbackCard (typed actions, design-system components)`.

---

### Task 3 — NeedsYouSurface [SEQUENTIAL after 1,2]

**Files:** Create `crates/vox-gui/ui/src/components/surfaces/NeedsYou/NeedsYouSurface.tsx`; test alongside.

- [ ] **Step 1: Write the failing test** (mock `transport`):
```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { NeedsYouSurface } from '../NeedsYouSurface';
import * as transport from '../../../../transport';
beforeEach(() => {
  vi.spyOn(transport, 'feedbackList').mockResolvedValue({
    needsYou: [{ feedbackId:'F-1', kind:'clarification', prompt:'schema?', options:['a'], gates:[7], doubtedTaskId:null, surface:'needs_you', infoGainBits:0.8 }],
    withheld: [{ feedbackId:'F-9', kind:'clarification', prompt:'low', options:[], gates:[], doubtedTaskId:null, surface:'withheld', infoGainBits:0.05 }],
  });
  vi.spyOn(transport, 'listenFeedbackChanged').mockResolvedValue(() => {});
});
describe('NeedsYouSurface', () => {
  it('lists open items + withheld section', async () => {
    render(<NeedsYouSurface onOpenContext={()=>{}} pushToast={()=>{}} />);
    await waitFor(() => expect(screen.getByText('schema?')).toBeTruthy());
    expect(screen.getByText(/Withheld by policy/i)).toBeTruthy();
  });
  it('empty state when nothing needs you', async () => {
    (transport.feedbackList as any).mockResolvedValue({ needsYou: [], withheld: [] });
    render(<NeedsYouSurface onOpenContext={()=>{}} pushToast={()=>{}} />);
    await waitFor(() => expect(screen.getByText(/Nothing needs you/i)).toBeTruthy());
  });
});
```

- [ ] **Step 2:** run → FAIL. **Step 3: Implement** using `EmptyState` for the empty case; `feedbackList()` already returns sorted/partitioned rows (doubts pinned), so render `needsYou` in order. `refresh` on mount + on `listenFeedbackChanged`. `handleResolve(id, action)` calls `transport.feedbackResolve(id, action)` then `refresh()`. Withheld in a collapsible `<details>`.

- [ ] **Step 4:** run → PASS. **Step 5: Commit** `feat(gui): NeedsYouSurface (reactive, doubts pinned, withheld section)`.

---

### Task 4 — `task_id` on HopperTaskDto [PARALLEL-SAFE]

So the GUI can join hopper rows to feedback gates. `TaskId = stable_hash(item_id.0)` — the same hash the dispatcher uses.

**Files:** `crates/vox-orchestrator/src/orchestrator/dispatch.rs` (expose the hash), `crates/vox-gui/src/commands/orchestrator.rs` (DTO).

- [ ] **Step 1 (gate):** `rg -n "fn stable_hash|TaskId(stable_hash" crates/vox-orchestrator/src/orchestrator/dispatch.rs` — confirm the hash fn and its visibility.

- [ ] **Step 2:** Make the hash reusable: add `pub fn task_id_for_hopper_id(item_id: &str) -> u64 { stable_hash(item_id) }` (or `pub(crate)` + a re-export) in vox-orchestrator, returning the `u64` inside `TaskId`. Test: assert it equals the dispatcher's value for a known id.

- [ ] **Step 3:** Add `pub task_id: u64` to `HopperTaskDto` and set it in `hopper_item_to_dto`: `task_id: vox_orchestrator::orchestrator::task_id_for_hopper_id(&item.item_id.0)`. Update the existing `hopper_tests` in that file.

- [ ] **Step 4:** `cargo test -p vox-orchestrator stable_hash && cargo clippy -p vox-gui --lib -- -D warnings`. fmt both. **Step 5: Commit** `feat(gui): expose task_id on HopperTaskDto for blocked overlay`.

---

### Task 5 — Blocked overlay on the Tasks surface [SEQUENTIAL]

**Files:** `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts`, `TasksView.tsx`; test alongside.

- [ ] **Step 1 (gate):** confirm the REAL `TaskRow` shape (`id`, `description`, `priority`, `lifecycle` — NO `item_id`/`state`) and `GroupedTasks = {inProgress, queued}` per pre-flight. Confirm the DTO→lifecycle mapping in `TasksView.tsx` (`dto.state === 'assigned' ? 'in_progress' : ...`).

- [ ] **Step 2: Write the failing test** (real shape):
```ts
import { describe, it, expect } from 'vitest';
import { groupTasks } from '../tasksHelpers';
describe('groupTasks with blocked', () => {
  it('separates blocked lifecycle into its own bucket', () => {
    const rows = [
      { id:'H-1', description:'a', priority:'normal', lifecycle:'in_progress' },
      { id:'H-2', description:'b', priority:'normal', lifecycle:'queued' },
      { id:'H-3', description:'c', priority:'normal', lifecycle:'blocked' },
    ] as any[];
    const g = groupTasks(rows);
    expect(g.blocked.map((r:any)=>r.id)).toEqual(['H-3']);
    expect(g.inProgress.map((r:any)=>r.id)).toEqual(['H-1']);
    expect(g.queued.map((r:any)=>r.id)).toEqual(['H-2']);
  });
});
```

- [ ] **Step 3:** run → FAIL. **Step 4: Implement**
  - Extend `GroupedTasks` to `{ inProgress: TaskRow[]; queued: TaskRow[]; blocked: TaskRow[] }`; `groupTasks` filters `lifecycle==='blocked'` into `blocked`, `'in_progress'` into `inProgress`, and the rest (excluding blocked) into `queued`.
  - In `TasksView.tsx`: (a) fetch the open feedback gate set (`feedbackList()` → flatten `needsYou` gates into a `Set<number>`); (b) when mapping each `HopperTaskDto`, set `lifecycle: gateSet.has(dto.task_id) ? 'blocked' : (dto.state === 'assigned' ? 'in_progress' : dto.state === 'inbox' ? 'queued' : dto.state === 'done' ? 'completed' : 'unknown')`; (c) render a "Blocked" section (dimmed `opacity-55`, caption "⛔ waiting on Needs You") with a show/hide filter. Refresh the gate set on `listenFeedbackChanged`.

- [ ] **Step 5:** `npx vitest run src/components/surfaces/Tasks && npx tsc --noEmit` → green. **Step 6: Commit** `feat(gui): blocked overlay on Tasks (derived from feedback gates)`.

---

### Task 6 — Register the Needs You surface [SEQUENTIAL]

**Files:** `crates/vox-gui/ui/src/App.tsx`, `contracts/gui/surface-registry.v1.yaml`, `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`.

- [ ] **Step 1 (gate):** `rg -n "type View|View =" crates/vox-gui/ui/src/App.tsx` and read an existing curated entry (`approvals`) in `contracts/gui/surface-registry.v1.yaml`.

- [ ] **Step 2:** Add `'needs-you'` to the `View` union in `App.tsx` (this also satisfies the `gui-surface-registry` wiring gate, which requires the literal in `App.tsx`).

- [ ] **Step 3:** Add to `contracts/gui/surface-registry.v1.yaml` (snake_case, mirror `approvals`):
```yaml
- view_key: needs-you
  cli_group: null
  representation_tier: live_backend
  nav_label: Needs You
  nav_icon: alert          # 'bell' is NOT a valid Icon key — use alert/eye
  nav_group: operate
  parent_surface: null
  notes: Unified soft-HITL feedback inbox (clarifications + doubts)
```

- [ ] **Step 4:** `vox ci gui-surface-registry --write` → confirm `surfaceRegistry.generated.ts` gains the entry. Do NOT hand-edit the generated file.

- [ ] **Step 5:** Add to the `childRenderer` switch in `surfaceComponents.tsx` (mirror `case 'activity'`):
```tsx
case 'needs-you':
  return <NeedsYouSurface onOpenContext={props.onOpenFeedbackContext!} pushToast={props.pushToast} />;
```
Add `onOpenFeedbackContext?: (id: string) => void;` to `SurfaceProps` (same file).

- [ ] **Step 6:** `vox ci gui-surface-registry` (non-write, the drift+wiring gate) → PASS; `npx tsc --noEmit`. **Step 7: Commit** `feat(gui): register Needs You surface (View union + yaml + childRenderer)`.

---

### Task 7 — Chat-focus, retire Dashboard doubt buttons, real strip counts [SEQUENTIAL]

**Files:** `crates/vox-gui/ui/src/App.tsx`, `Dashboard/StreamCard.tsx`, `Dashboard/Dashboard.tsx`; tests alongside StreamCard.

- [ ] **Step 1: Write the failing test** (buttons gone):
```tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { StreamCard } from '../StreamCard';
describe('StreamCard after doubt migration', () => {
  it('no longer renders doubt or overrule controls', () => {
    render(<StreamCard item={{ id:'E-1', title:'t', kind:'in-progress' } as any} />);
    expect(screen.queryByTitle(/doubt/i)).toBeNull();
    expect(screen.queryByTitle(/overrule/i)).toBeNull();
  });
});
```

- [ ] **Step 2:** run → FAIL (buttons present). **Step 3:** Remove the `onDoubt`/`onOverrule` props + ❓/⚖️ buttons from `StreamCard.tsx`; drop the prop forwarding in `Dashboard.tsx` and the `handleDoubt`/`handleOverrule` Dashboard wiring in `App.tsx`. (The backend doubt path stays — only the Dashboard UI moves to Needs You.) Update any StreamCard test asserting the buttons existed.

- [ ] **Step 4: Chat-focus handler + real counts** in `App.tsx`:
```tsx
const [focusedFeedbackId, setFocusedFeedbackId] = useState<string | null>(null);
const onOpenFeedbackContext = useCallback((feedbackId: string) => {
  navigateTo('chat');                 // real nav callback + real view key
  setFocusedFeedbackId(feedbackId);   // ChatSurface reads this and scrolls/highlights (net-new wiring)
}, [navigateTo]);
```
Thread `focusedFeedbackId` into `ChatSurface` and add a `useEffect` + ref there that `scrollIntoView`s the matching message (net-new — there is no existing scroll-to-thread helper). For the attention strip (Phase 0 stubbed counts to 0): hold the latest `feedbackList()` result + a `hopper_list` snapshot in `App.tsx`; pass `waitingQuestions={needsYou.length}` and `blockedTasks={<count of hopper rows whose task_id ∈ open gate set>}` to `<AttentionStrip>`.

- [ ] **Step 5:** `npx vitest run && npx tsc --noEmit` → all green. **Step 6: Commit** `feat(gui): chat-focus for feedback, real strip counts, retire Dashboard doubt buttons`.

---

### Self-review notes (vs spec rev 2)
- §5.2 Needs You: Tasks 2, 3, 6 — typed actions, doubts pinned, withheld section, design-system components, registration in all 4 places. ✓
- §5.3 blocked overlay: Tasks 4, 5 — `task_id` join, derived `lifecycle: 'blocked'`, no hopper mutation. ✓
- §6 transport/reactivity: Task 1 — `invoke_mcp_tool` + `vox://agent-events`. ✓
- Retire Dashboard buttons + real counts: Task 7. ✓
- Audit corrections: real paths (`transport.ts`, no `lib/`), `navigateTo('chat')` not `setActiveSurface('loquela')`, `focusedFeedbackId` is net-new (not a fictional helper), `Icon.alert` not `bell`, real `TaskRow`/`groupTasks` shape, jsdom pragma, lib-only vox-gui build. ✓
- Type consistency: `FeedbackRow`, `feedbackList`/`feedbackResolve`/`listenFeedbackChanged`/`normalizeFeedback`, `onOpenFeedbackContext`, `GroupedTasks.blocked`, `HopperTaskDto.task_id` consistent across tasks and with Phase 1's tool outputs.
```
