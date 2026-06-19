# Soft HITL Phase 2 — Needs You Surface + Blocked Tasks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the unified "Needs You" GUI surface (clarifications + doubts, per-type buttons, Withheld section, click-to-expand-in-chat), render blocked states on the Tasks surface, wire the AttentionStrip's real waiting/blocked counts, and retire the Dashboard `StreamCard` doubt buttons.

**Architecture:** A new `NeedsYou` surface registered in `surfaceRegistry`, fed by the `feedback_list` Tauri command (Phase 1) and refreshed reactively on the `vox://activity-appended`-style events (`FeedbackRequested`/`FeedbackResolved`). Tasks surface maps the new `blocked` state from `HopperTaskDto`. Card click dispatches a chat-scroll intent already used by the Loquela surface.

**Tech Stack:** TypeScript/React, Tailwind, vitest. Depends on Phase 1 backend + Phase 0 strip.

**Spec:** `docs/superpowers/specs/2026-06-19-attention-aware-soft-hitl-design.md` §5.2, §5.3

---

### Task 1: Feedback transport client

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/transport.ts` (add `feedbackList`, `feedbackResolve`, `listenFeedbackChanged`)
- Test: `crates/vox-gui/ui/src/lib/__tests__/feedbackTransport.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect, vi } from 'vitest';
import { normalizeFeedback } from '../transport';

describe('normalizeFeedback', () => {
  it('splits needs_you from withheld', () => {
    const rows = [
      { feedback_id: 'F-1', kind: 'clarification', prompt: 'q', options: ['a'], gates: ['H-1'], surface: 'needs_you', info_gain_bits: 0.8 },
      { feedback_id: 'F-2', kind: 'doubt', prompt: 'd', options: [], gates: ['H-2'], surface: 'needs_you', info_gain_bits: 0 },
      { feedback_id: 'F-3', kind: 'clarification', prompt: 'low', options: [], gates: [], surface: 'withheld', info_gain_bits: 0.05 },
    ];
    const { needsYou, withheld } = normalizeFeedback(rows);
    expect(needsYou.map(r => r.feedbackId)).toEqual(['F-1', 'F-2']);
    expect(withheld.map(r => r.feedbackId)).toEqual(['F-3']);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/__tests__/feedbackTransport.test.ts`
Expected: FAIL — `normalizeFeedback` not exported.

- [ ] **Step 3: Implement**

In `transport.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface FeedbackRow {
  feedbackId: string;
  kind: 'clarification' | 'doubt';
  prompt: string;
  options: string[];
  gates: string[];
  surface: 'needs_you' | 'withheld';
  infoGainBits: number;
}

export function normalizeFeedback(raw: any[]): { needsYou: FeedbackRow[]; withheld: FeedbackRow[] } {
  const rows: FeedbackRow[] = (raw ?? []).map((r) => ({
    feedbackId: r.feedback_id, kind: r.kind, prompt: r.prompt,
    options: r.options ?? [], gates: r.gates ?? [], surface: r.surface,
    infoGainBits: r.info_gain_bits ?? 0,
  }));
  return {
    needsYou: rows.filter((r) => r.surface === 'needs_you'),
    withheld: rows.filter((r) => r.surface === 'withheld'),
  };
}

export async function feedbackList(): Promise<{ needsYou: FeedbackRow[]; withheld: FeedbackRow[] }> {
  const raw = await invoke<any[]>('feedback_list');
  return normalizeFeedback(raw);
}

export async function feedbackResolve(feedbackId: string, chosenOption: number | null, freeText: string | null): Promise<void> {
  await invoke('feedback_resolve', { feedbackId, chosenOption, freeText });
}

// Feedback changes ride the activity-appended signal; subscribe the same way.
export function listenFeedbackChanged(onChange: () => void): Promise<UnlistenFn> {
  return listen<void>('vox://activity-appended', () => onChange());
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/__tests__/feedbackTransport.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/transport.ts crates/vox-gui/ui/src/lib/__tests__/feedbackTransport.test.ts
git commit -m "feat(gui): feedback transport client"
```

---

### Task 2: FeedbackCard component (per-type buttons)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { FeedbackCard } from '../FeedbackCard';

const clar = { feedbackId: 'F-1', kind: 'clarification' as const, prompt: 'schema?', options: ['In hopper', 'Separate'], gates: ['H-1'], surface: 'needs_you' as const, infoGainBits: 0.8 };
const doubt = { feedbackId: 'F-2', kind: 'doubt' as const, prompt: 'suspect', options: [], gates: ['H-2'], surface: 'needs_you' as const, infoGainBits: 0 };

describe('FeedbackCard', () => {
  it('clarification renders option buttons + answer + skip', () => {
    render(<FeedbackCard row={clar} onResolve={() => {}} onOpenContext={() => {}} />);
    expect(screen.getByText('In hopper')).toBeTruthy();
    expect(screen.getByText('Separate')).toBeTruthy();
    expect(screen.getByText(/Answer/i)).toBeTruthy();
  });

  it('doubt renders overrule + answer + let-verify (no option buttons)', () => {
    render(<FeedbackCard row={doubt} onResolve={() => {}} onOpenContext={() => {}} />);
    expect(screen.getByText(/Overrule/i)).toBeTruthy();
    expect(screen.getByText(/verify/i)).toBeTruthy();
  });

  it('clicking an option calls onResolve with its index', () => {
    const onResolve = vi.fn();
    render(<FeedbackCard row={clar} onResolve={onResolve} onOpenContext={() => {}} />);
    fireEvent.click(screen.getByText('Separate'));
    expect(onResolve).toHaveBeenCalledWith('F-1', 1, null);
  });

  it('clicking the card body opens context', () => {
    const onOpenContext = vi.fn();
    render(<FeedbackCard row={clar} onResolve={() => {}} onOpenContext={onOpenContext} />);
    fireEvent.click(screen.getByText('schema?'));
    expect(onOpenContext).toHaveBeenCalledWith('F-1');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

```tsx
import type { FeedbackRow } from '../../../lib/transport';

interface Props {
  row: FeedbackRow;
  onResolve: (feedbackId: string, chosenOption: number | null, freeText: string | null) => void;
  onOpenContext: (feedbackId: string) => void;
}

const EDGE = { clarification: 'border-l-amber-400', doubt: 'border-l-rose-400' } as const;

export function FeedbackCard({ row, onResolve, onOpenContext }: Props) {
  const isDoubt = row.kind === 'doubt';
  return (
    <div className={`p-3 border-b border-zinc-800 border-l-2 ${EDGE[row.kind]} hover:bg-white/[0.02]`}>
      <div className="flex items-center gap-2 mb-1">
        <span className={`text-[9px] font-bold uppercase tracking-wide px-1.5 py-0.5 rounded border ${isDoubt ? 'bg-rose-400/10 text-rose-300 border-rose-400/30' : 'bg-amber-400/10 text-amber-300 border-amber-400/30'}`}>
          {isDoubt ? 'Doubt' : 'Clarification'} · {row.feedbackId}
        </span>
        {row.gates.length > 0 && <span className="text-[11px] text-zinc-500">gates {row.gates.length} task{row.gates.length > 1 ? 's' : ''}</span>}
        <span className="ml-auto text-[11px] text-zinc-600">context ›</span>
      </div>
      <button className="text-xs text-zinc-200 mb-2 text-left block w-full" onClick={() => onOpenContext(row.feedbackId)}>
        {row.prompt}
      </button>
      <div className="flex gap-1.5 flex-wrap">
        {isDoubt ? (
          <>
            <Btn tone="ok" onClick={() => onResolve(row.feedbackId, null, 'overrule')}>⚖️ Overrule</Btn>
            <Btn tone="warn" onClick={() => onOpenContext(row.feedbackId)}>✎ Answer the conflict</Btn>
            <Btn tone="ghost" onClick={() => onResolve(row.feedbackId, null, 'let-verify')}>Let it verify</Btn>
          </>
        ) : (
          <>
            {row.options.map((opt, i) => (
              <Btn key={i} tone="ok" onClick={() => onResolve(row.feedbackId, i, null)}>{opt}</Btn>
            ))}
            <Btn tone="ghost" onClick={() => onOpenContext(row.feedbackId)}>✎ Answer…</Btn>
            <Btn tone="ghost" onClick={() => onResolve(row.feedbackId, null, 'skip')}>Skip</Btn>
          </>
        )}
      </div>
    </div>
  );
}

function Btn({ tone, onClick, children }: { tone: 'ok' | 'warn' | 'ghost'; onClick: () => void; children: React.ReactNode }) {
  const cls = tone === 'ok' ? 'bg-emerald-400/10 text-emerald-300 border-emerald-400/30'
    : tone === 'warn' ? 'bg-amber-400/10 text-amber-300 border-amber-400/30'
    : 'bg-transparent text-zinc-400 border-zinc-700';
  return <button className={`text-[11px] font-semibold px-2.5 py-1 rounded border ${cls}`} onClick={onClick}>{children}</button>;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx
git commit -m "feat(gui): FeedbackCard with per-type response buttons"
```

---

### Task 3: NeedsYouSurface (list + Withheld + reactive refresh)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/NeedsYouSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/NeedsYouSurface.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { NeedsYouSurface } from '../NeedsYouSurface';
import * as transport from '../../../../lib/transport';

beforeEach(() => {
  vi.spyOn(transport, 'feedbackList').mockResolvedValue({
    needsYou: [{ feedbackId: 'F-1', kind: 'clarification', prompt: 'schema?', options: ['a'], gates: ['H-1'], surface: 'needs_you', infoGainBits: 0.8 }],
    withheld: [{ feedbackId: 'F-9', kind: 'clarification', prompt: 'low', options: [], gates: [], surface: 'withheld', infoGainBits: 0.05 }],
  });
  vi.spyOn(transport, 'listenFeedbackChanged').mockResolvedValue(() => {});
});

describe('NeedsYouSurface', () => {
  it('lists open needs-you items and a withheld section', async () => {
    render(<NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText('schema?')).toBeTruthy());
    expect(screen.getByText(/Withheld by policy/i)).toBeTruthy();
  });

  it('shows empty state when nothing needs the user', async () => {
    (transport.feedbackList as any).mockResolvedValue({ needsYou: [], withheld: [] });
    render(<NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText(/Nothing needs you/i)).toBeTruthy());
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/NeedsYou/__tests__/NeedsYouSurface.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

```tsx
import { useCallback, useEffect, useState } from 'react';
import { feedbackList, feedbackResolve, listenFeedbackChanged, type FeedbackRow } from '../../../lib/transport';
import { FeedbackCard } from './FeedbackCard';

interface Props {
  onOpenContext: (feedbackId: string) => void;
  pushToast: (t: { tone: string; title: string; body?: string }) => void;
}

export function NeedsYouSurface({ onOpenContext, pushToast }: Props) {
  const [needsYou, setNeedsYou] = useState<FeedbackRow[]>([]);
  const [withheld, setWithheld] = useState<FeedbackRow[]>([]);

  const refresh = useCallback(async () => {
    try {
      const { needsYou, withheld } = await feedbackList();
      // Sort by attention value (info gain), highest first.
      setNeedsYou([...needsYou].sort((a, b) => b.infoGainBits - a.infoGainBits));
      setWithheld(withheld);
    } catch (e) {
      pushToast({ tone: 'warn', title: 'Feedback query failed', body: String(e) });
    }
  }, [pushToast]);

  useEffect(() => { refresh(); }, [refresh]);
  useEffect(() => {
    const p = listenFeedbackChanged(refresh);
    return () => { p.then((un) => un()); };
  }, [refresh]);

  const handleResolve = useCallback(async (id: string, choice: number | null, text: string | null) => {
    await feedbackResolve(id, choice, text);
    refresh();
  }, [refresh]);

  return (
    <div className="flex flex-col h-full gap-3 p-4 text-zinc-300">
      <h2 className="text-lg font-semibold tracking-wider text-zinc-100">🙋 Needs You</h2>
      {needsYou.length === 0 ? (
        <p className="text-xs text-zinc-500">Nothing needs you right now.</p>
      ) : (
        <div className="flex flex-col">
          {needsYou.map((r) => (
            <FeedbackCard key={r.feedbackId} row={r} onResolve={handleResolve} onOpenContext={onOpenContext} />
          ))}
        </div>
      )}
      {withheld.length > 0 && (
        <div className="mt-2">
          <div className="text-[10px] uppercase font-bold tracking-wider text-zinc-500 mb-1">Withheld by policy</div>
          {withheld.map((r) => (
            <div key={r.feedbackId} className="text-[11px] text-zinc-500">▸ {r.prompt} ({r.infoGainBits.toFixed(2)} bits)</div>
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/NeedsYou/__tests__/NeedsYouSurface.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/NeedsYou/
git commit -m "feat(gui): NeedsYouSurface — unified feedback inbox"
```

---

### Task 4: Register the surface + route it

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` (add `case 'needs-you'`)
- Modify: surface registry source + regenerate (`vox ci gui-surface-registry --write`)

- [ ] **Step 1: Add the registry entry (source, then regenerate)**

Add a `needs-you` surface to the registry source (find it: `grep -rn "viewKey: 'activity'" crates/vox-gui/ui/src`). Mirror the `activity` entry: `{ viewKey: 'needs-you', tier: 'live_backend', navLabel: 'Needs You', navIcon: 'bell', navGroup: 'operate' }`.

- [ ] **Step 2: Regenerate the registry**

Run: `vox ci gui-surface-registry --write`
Expected: `surfaceRegistry.generated.ts` now contains the `needs-you` entry. (Per project memory, the generated file is SSOT — never hand-edit it.)

- [ ] **Step 3: Route the component**

In `surfaceComponents.tsx` switch, add:

```tsx
case 'needs-you':
  return <NeedsYouSurface onOpenContext={props.onOpenFeedbackContext!} pushToast={props.pushToast} />;
```

Add `onOpenFeedbackContext?: (feedbackId: string) => void;` to `SurfaceProps` and thread it from `App.tsx` (Task 6 implements the handler).

- [ ] **Step 4: Typecheck**

Run: `cd crates/vox-gui/ui && npx tsc --noEmit`
Expected: clean (a temporary `onOpenFeedbackContext` no-op is fine until Task 6).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts crates/vox-gui/ui/src/<registry-source>
git commit -m "feat(gui): register + route Needs You surface"
```

---

### Task 5: Blocked state on the Tasks surface

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts` (recognize `blocked`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` (render dimmed + caption + filter)
- Test: `crates/vox-gui/ui/src/components/surfaces/Tasks/__tests__/tasksHelpers.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect } from 'vitest';
import { groupTasks } from '../tasksHelpers';

describe('groupTasks with blocked', () => {
  it('separates blocked from queued and in-progress', () => {
    const rows = [
      { item_id: 'H-1', intent: 'a', priority: 1, state: 'assigned' },
      { item_id: 'H-2', intent: 'b', priority: 1, state: 'inbox' },
      { item_id: 'H-3', intent: 'c', priority: 1, state: 'blocked' },
    ] as any[];
    const g = groupTasks(rows);
    expect(g.blocked.map((r) => r.item_id)).toEqual(['H-3']);
    expect(g.inProgress.map((r) => r.item_id)).toEqual(['H-1']);
    expect(g.queued.map((r) => r.item_id)).toEqual(['H-2']);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Tasks/__tests__/tasksHelpers.test.ts`
Expected: FAIL — `groupTasks` has no `blocked` group.

- [ ] **Step 3: Extend `groupTasks`**

In `tasksHelpers.ts`, add a `blocked` bucket to the grouping return type and put `state === 'blocked'` rows there (in-progress = `assigned`, queued = `inbox`, blocked = `blocked`).

- [ ] **Step 4: Render in TasksView**

In `TasksView.tsx`, add a "Blocked" section that renders blocked rows with `className="opacity-55"` and a caption "⛔ blocked — waiting on Needs You", plus a checkbox filter to hide/show the blocked group.

- [ ] **Step 5: Run test + typecheck**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Tasks && npx tsc --noEmit`
Expected: PASS; tsc clean.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Tasks/
git commit -m "feat(gui): render blocked tasks (dimmed + caption + filter)"
```

---

### Task 6: Click-to-expand-in-chat + real strip counts; retire Dashboard doubt buttons

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (handler + counts)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/StreamCard.tsx` (remove doubt/overrule buttons)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` (drop `onDoubt`/`onOverrule` props)
- Test: `crates/vox-gui/ui/src/components/surfaces/Dashboard/__tests__/StreamCard.test.tsx`

- [ ] **Step 1: Write the failing test (buttons gone)**

```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { StreamCard } from '../StreamCard';

describe('StreamCard after doubt migration', () => {
  it('no longer renders doubt or overrule controls', () => {
    render(<StreamCard item={{ id: 'E-1', title: 't', kind: 'in-progress' } as any} />);
    expect(screen.queryByTitle(/doubt/i)).toBeNull();
    expect(screen.queryByTitle(/overrule/i)).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Dashboard/__tests__/StreamCard.test.tsx`
Expected: FAIL — buttons still present.

- [ ] **Step 3: Remove the doubt/overrule buttons + props**

Delete the ❓/⚖️ button JSX and the `onDoubt`/`onOverrule` props from `StreamCard.tsx`; remove the prop forwarding in `Dashboard.tsx` and the `handleDoubt`/`handleOverrule` wiring in `App.tsx` that fed the Dashboard (the backend doubt path stays — only the Dashboard UI moves to Needs You). Update any other `StreamCard` test that asserted the buttons existed.

- [ ] **Step 4: Implement chat-expand handler + real counts**

In `App.tsx`:

```tsx
const onOpenFeedbackContext = useCallback((feedbackId: string) => {
  // Reuse the Loquela/chat scroll intent already used for inline approvals.
  setActiveSurface('loquela');
  scrollChatToFeedback(feedbackId); // existing chat ref helper; if absent, set a `focusedFeedbackId` state the chat surface reads
}, []);
```

Wire real counts into the AttentionStrip (Phase 0 stubbed them to 0): hold the latest `feedbackList()` result in `App.tsx` state and pass `waitingQuestions={needsYou.length}` and `blockedTasks={blockedCount}` (count of `hopper_list` rows with `state === 'blocked'`).

- [ ] **Step 5: Run all UI tests + typecheck + lib build**

Run: `cd crates/vox-gui/ui && npx vitest run && npx tsc --noEmit`
Expected: all green; tsc clean.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/Dashboard/
git commit -m "feat(gui): chat-expand for feedback, real strip counts, retire Dashboard doubt buttons"
```

---

### Self-review notes
- Spec §5.2 (Needs You): Tasks 2, 3, 4 — unified list, per-type buttons, Withheld section, click-to-chat, registry entry. ✓
- Spec §5.3 (Tasks blocked states): Task 5 — dimmed + caption + filter. ✓
- "Retire Dashboard StreamCard buttons" (spec §5.2): Task 6. ✓
- AttentionStrip real counts (closes Phase 0's documented stub): Task 6. ✓
- Type consistency: `FeedbackRow`, `feedbackList`/`feedbackResolve`/`listenFeedbackChanged`, `onOpenContext`/`onOpenFeedbackContext`, `groupTasks().blocked` consistent across tasks.
- Caveats flagged inline (registry source path via grep; chat-scroll helper may need a `focusedFeedbackId` state if no ref helper exists) — guarded, not placeholders.
