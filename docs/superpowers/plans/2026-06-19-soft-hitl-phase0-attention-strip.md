# Soft HITL Phase 0 — Attention Strip Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the already-emitted-but-dark `attention_budget` snapshot in the GUI as a compact status strip (focus depth, spent/budget gauge, waiting/blocked counts).

**Architecture:** Pure surfacing. The orchestrator status stream already carries `attention_budget` through `ORCH_STATUS_EVENT` (`crates/vox-gui/src/commands/orchestrator.rs`). No backend change. Add a typed accessor in the GUI's status mapping layer and a React `AttentionStrip` component fed by the existing status subscription.

**Tech Stack:** Rust (Tauri command DTO), TypeScript/React, Tailwind, vitest.

**Spec:** `docs/superpowers/specs/2026-06-19-attention-aware-soft-hitl-design.md` §5.1

---

### Task 1: Type the attention-budget snapshot in the GUI status DTO

The daemon already passes `attention_budget: Option<serde_json::Value>` verbatim. Give the GUI a typed view so the React layer is not parsing raw JSON.

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/orchestratorStatus.ts` (status type used by the `vox://orch-status` listener)
- Test: `crates/vox-gui/ui/src/lib/__tests__/attentionBudget.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect } from 'vitest';
import { parseAttentionBudget } from '../orchestratorStatus';

describe('parseAttentionBudget', () => {
  it('maps a raw snapshot to typed fields', () => {
    const raw = {
      max_attention_ms: 3_600_000,
      spent_ms: 1_800_000,
      interrupt_freq_per_hour: 5.2,
    };
    const b = parseAttentionBudget(raw)!;
    expect(b.spentRatio).toBeCloseTo(0.5, 3);
    expect(b.focusDepth).toBe('focused'); // 3..8/hr
  });

  it('returns null for missing snapshot', () => {
    expect(parseAttentionBudget(null)).toBeNull();
    expect(parseAttentionBudget(undefined)).toBeNull();
  });

  it('classifies focus depth by interrupt frequency', () => {
    expect(parseAttentionBudget({ max_attention_ms: 1, spent_ms: 0, interrupt_freq_per_hour: 1 })!.focusDepth).toBe('ambient');
    expect(parseAttentionBudget({ max_attention_ms: 1, spent_ms: 0, interrupt_freq_per_hour: 9 })!.focusDepth).toBe('deep');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/__tests__/attentionBudget.test.ts`
Expected: FAIL — `parseAttentionBudget` is not exported.

- [ ] **Step 3: Implement the parser**

Add to `crates/vox-gui/ui/src/lib/orchestratorStatus.ts`:

```ts
export type FocusDepth = 'ambient' | 'focused' | 'deep';

export interface AttentionBudgetView {
  maxMs: number;
  spentMs: number;
  spentRatio: number;
  interruptFreqPerHour: number;
  focusDepth: FocusDepth;
}

// Thresholds mirror crates/vox-orchestrator/src/attention/budget.rs focus_depth():
// Ambient <3/hr, Focused 3..8/hr, Deep >=8/hr.
export function parseAttentionBudget(raw: unknown): AttentionBudgetView | null {
  if (!raw || typeof raw !== 'object') return null;
  const r = raw as Record<string, number>;
  const maxMs = Number(r.max_attention_ms ?? 0);
  const spentMs = Number(r.spent_ms ?? 0);
  const freq = Number(r.interrupt_freq_per_hour ?? 0);
  const focusDepth: FocusDepth = freq >= 8 ? 'deep' : freq >= 3 ? 'focused' : 'ambient';
  return {
    maxMs,
    spentMs,
    spentRatio: maxMs > 0 ? spentMs / maxMs : 0,
    interruptFreqPerHour: freq,
    focusDepth,
  };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/__tests__/attentionBudget.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/orchestratorStatus.ts crates/vox-gui/ui/src/lib/__tests__/attentionBudget.test.ts
git commit -m "feat(gui): typed attention-budget view from orch status"
```

---

### Task 2: AttentionStrip component

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/AttentionStrip.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/__tests__/AttentionStrip.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AttentionStrip } from '../AttentionStrip';

describe('AttentionStrip', () => {
  it('renders focus depth, spent ratio and counts', () => {
    render(
      <AttentionStrip
        budget={{ maxMs: 3_600_000, spentMs: 1_800_000, spentRatio: 0.5, interruptFreqPerHour: 5.2, focusDepth: 'focused' }}
        waitingQuestions={2}
        blockedTasks={1}
      />
    );
    expect(screen.getByText(/FOCUSED/i)).toBeTruthy();
    expect(screen.getByText(/2 waiting/i)).toBeTruthy();
    expect(screen.getByText(/1 blocked/i)).toBeTruthy();
  });

  it('renders nothing when budget is null', () => {
    const { container } = render(
      <AttentionStrip budget={null} waitingQuestions={0} blockedTasks={0} />
    );
    expect(container.firstChild).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/__tests__/AttentionStrip.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the component**

```tsx
import type { AttentionBudgetView } from '../../lib/orchestratorStatus';

interface AttentionStripProps {
  budget: AttentionBudgetView | null;
  waitingQuestions: number;
  blockedTasks: number;
}

const FOCUS_LABEL: Record<string, string> = { ambient: 'AMBIENT', focused: 'FOCUSED', deep: 'DEEP' };

export function AttentionStrip({ budget, waitingQuestions, blockedTasks }: AttentionStripProps) {
  if (!budget) return null;
  const pct = Math.min(100, Math.round(budget.spentRatio * 100));
  const spentMin = Math.round(budget.spentMs / 60_000);
  const maxMin = Math.round(budget.maxMs / 60_000);
  return (
    <div className="flex items-center gap-3 px-3 py-1.5 bg-[#0c0c0e] border-b border-zinc-800 text-xs text-zinc-400">
      <span className="text-[10px] uppercase font-bold tracking-wider text-zinc-500">Attention</span>
      <span className="px-1.5 py-0.5 rounded bg-amber-400/10 text-amber-300 border border-amber-400/30 text-[10px] font-bold">
        {FOCUS_LABEL[budget.focusDepth]} · {budget.interruptFreqPerHour.toFixed(1)}/hr
      </span>
      <div className="h-1.5 w-40 bg-zinc-800 rounded overflow-hidden">
        <div className="h-full" style={{ width: `${pct}%`, background: 'linear-gradient(90deg,#34d399,#fbbf24)' }} />
      </div>
      <span className="text-zinc-500">spent {spentMin}m / {maxMin}m</span>
      <span className="flex-1" />
      {waitingQuestions > 0 && (
        <span className="px-1.5 py-0.5 rounded bg-amber-400/10 text-amber-300 border border-amber-400/30 text-[10px] font-bold">
          {waitingQuestions} waiting
        </span>
      )}
      {blockedTasks > 0 && (
        <span className="px-1.5 py-0.5 rounded bg-rose-400/10 text-rose-300 border border-rose-400/30 text-[10px] font-bold">
          {blockedTasks} blocked
        </span>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/__tests__/AttentionStrip.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/AttentionStrip.tsx crates/vox-gui/ui/src/components/layout/__tests__/AttentionStrip.test.tsx
git commit -m "feat(gui): AttentionStrip component"
```

---

### Task 3: Wire AttentionStrip into the app shell

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (the top-level shell that already subscribes to `vox://orch-status`)

- [ ] **Step 1: Locate the status subscription**

Run: `cd crates/vox-gui/ui && grep -n "orch-status\|orchestratorStatus\|attention_budget" src/App.tsx`
Expected: a `useState` holding the latest status and a `listen('vox://orch-status', ...)` effect.

- [ ] **Step 2: Derive the budget view and counts, render the strip**

In `App.tsx`, where the status object is held in state, add a derived value and render `<AttentionStrip>` directly above the main surface area:

```tsx
import { AttentionStrip } from './components/layout/AttentionStrip';
import { parseAttentionBudget } from './lib/orchestratorStatus';
// ...
const attentionBudget = parseAttentionBudget(status?.attention_budget);
// waitingQuestions/blockedTasks: 0 for now — Phase 2 wires real counts.
// Render near the top of the shell:
<AttentionStrip budget={attentionBudget} waitingQuestions={0} blockedTasks={0} />
```

(Note for the engineer: the `status` object is whatever the existing `vox://orch-status` listener stores. Use the same variable name already in scope; only add the `attention_budget` read and the JSX line.)

- [ ] **Step 3: Typecheck + build the UI**

Run: `cd crates/vox-gui/ui && npx tsc --noEmit && npx vitest run`
Expected: tsc clean; all vitest suites green.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui): mount AttentionStrip in app shell"
```

---

### Self-review notes
- Spec §5.1 (attention strip) fully covered: focus depth, gauge, waiting/blocked counts (counts stubbed to 0 until Phase 2 — documented inline, not a hidden placeholder).
- Focus-depth thresholds single-sourced in a comment pointing at `budget.rs`.
- No backend change; `attention_budget` field already exists on the status payload.
