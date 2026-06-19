# Soft HITL Phase 0 — Attention Strip Implementation Plan (rev 2)

> 🤖 **EXECUTION TARGET — READ FIRST.** This plan is written for Gemini Flash in
> Antigravity. Flash has ~48% unaided in-IDE completion, no mid-task checkpoint,
> and a hard quota cutoff. See
> `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`.
> Follow the Operating Rules and the Flash Execution Addendum exactly.

**Operating Rules (apply to EVERY task):**
1. Each task is **atomic + green + committed**: tests pass before you commit; never leave a broken tree.
2. **Verify before use.** Every Step-1 `rg` is a BLOCKING gate — run it, paste the output, and if reality differs from the plan, STOP and report rather than guessing.
3. **Two-strike circuit breaker.** If a step fails twice, stop and report; do not thrash.
4. **Split on overrun.** If an implement step would touch >1 file or add >1 new function, make one atomic green commit per sub-bullet.
5. **House rules:** this package is pnpm-managed; `npx vitest run <path>` and `npx tsc --noEmit` work. Add `// @vitest-environment jsdom` as the FIRST line of every new component test. Never run `cargo fmt --all` (Windows arg-limit). No stubs.
6. **Tags:** `[PARALLEL-SAFE]` tasks touch disjoint files and may run in parallel subagents; `[SEQUENTIAL]` tasks must not share a file with a concurrent subagent.

**Goal:** Surface attention metrics in a persistent top status strip — reusing the existing `AttentionBudgetMeter` — and add waiting-questions + blocked-tasks counts.

**Architecture:** Pure GUI. The snapshot is ALREADY typed (`AttentionBudgetSnapshot`, `crates/vox-gui/ui/src/types/tauri.ts`) and ALREADY rendered by `AttentionBudgetMeter.tsx` on the Dashboard. Phase 0 composes that meter into a top strip and adds two count props. No backend change, no new parser/type.

**Tech Stack:** TypeScript/React, Tailwind, vitest. Reuses `AttentionBudgetMeter`, `Pill`, `Icon`.

**Spec:** `docs/superpowers/specs/2026-06-19-attention-aware-soft-hitl-design.md` §5.1, §7

---

## Flash Execution Addendum (2026-06-19)

**Global gates:**
- The attention budget is NOT dark data. `crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.tsx` already renders focus depth + a `role="meter"` gauge and is mounted on the Dashboard (`Dashboard.tsx`, threaded via `App.tsx` `orchQuery.data?.attention_budget`). REUSE it; do not write a second parser.
- The status is read via the react-query hook: `const orchQuery = useOrchestratorStatus()` in `App.tsx` (~:198), value at `orchQuery.data?.attention_budget` (~:1002). There is NO `useState` + `listen('vox://orch-status')` in `App.tsx` — that lives in `hooks/useOrchestratorStatus.ts`.

**Mandatory pre-flight (run, paste output, confirm before any code):**
```
rg -n "AttentionBudgetSnapshot|focus" crates/vox-gui/ui/src/types/tauri.ts
rg -n "AttentionBudgetMeter" crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.tsx
rg -n "AttentionBudgetMeter|attention_budget" crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx
rg -n "useOrchestratorStatus|orchQuery|attention_budget" crates/vox-gui/ui/src/App.tsx
```
Expected: `AttentionBudgetSnapshot` interface with `max_attention_ms`, `spent_ms`, `interrupt_freq_per_hour`; `AttentionBudgetMeter` component taking a `budget` prop; Dashboard mounting it; `App.tsx` reading `orchQuery.data?.attention_budget`.

**Task-split table:**

| Task | Touches | Tag |
|---|---|---|
| 1 — count props on `AttentionBudgetMeter` | `AttentionBudgetMeter.tsx` (+test) | [PARALLEL-SAFE] |
| 2 — `AttentionStrip` wrapper | `layout/AttentionStrip.tsx` (+test) | [PARALLEL-SAFE] |
| 3 — mount in shell | `App.tsx` | [SEQUENTIAL] |

---

### Task 1 — Add optional count props to AttentionBudgetMeter [PARALLEL-SAFE]

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/__tests__/AttentionBudgetMeter.counts.test.tsx`

- [ ] **Step 1 (gate):** `rg -n "export function AttentionBudgetMeter|interface .*Props|role=\"meter\"" crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.tsx` — paste the props interface and the meter markup. Confirm the existing `budget` prop name and the focus-depth helper.

- [ ] **Step 2: Write the failing test**

```tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AttentionBudgetMeter } from '../AttentionBudgetMeter';

const snap = { max_attention_ms: 3_600_000, spent_ms: 1_800_000, interrupt_freq_per_hour: 5.2,
  total_requests: 0, auto_approved: 0, rejected: 0, last_interrupt_ms: 0, inbox_suppressed_count: 0 };

describe('AttentionBudgetMeter counts', () => {
  it('renders waiting + blocked counts when provided', () => {
    render(<AttentionBudgetMeter budget={snap} waitingQuestions={2} blockedTasks={1} />);
    expect(screen.getByText(/2 waiting/i)).toBeTruthy();
    expect(screen.getByText(/1 blocked/i)).toBeTruthy();
  });
  it('omits count chips when zero / undefined', () => {
    const { container } = render(<AttentionBudgetMeter budget={snap} />);
    expect(container.textContent).not.toMatch(/waiting/i);
  });
});
```

- [ ] **Step 3:** `npx vitest run src/components/surfaces/__tests__/AttentionBudgetMeter.counts.test.tsx` → FAIL (props unknown).

- [ ] **Step 4: Add the two optional props** to the meter's props interface and render chips with `<Pill>` (reuse the existing import; if `Pill` is not imported, add `import { Pill } from '../ui/Pill'`):

```tsx
// add to the props interface:
waitingQuestions?: number;
blockedTasks?: number;
// in the render, after the existing gauge, append:
{!!waitingQuestions && <Pill phase="Focused">{waitingQuestions} waiting</Pill>}
{!!blockedTasks && <Pill phase="Doubted">{blockedTasks} blocked</Pill>}
```
(If `Pill`'s API is `tone`/`label` rather than `phase`/children, adapt to the real signature from the Step-1 gate output — do NOT invent a prop.)

- [ ] **Step 5:** `npx vitest run src/components/surfaces/__tests__/AttentionBudgetMeter.counts.test.tsx` → PASS. Then `npx tsc --noEmit`.

- [ ] **Step 6: Commit** `git commit -m "feat(gui): waiting/blocked count chips on AttentionBudgetMeter"`

---

### Task 2 — AttentionStrip wrapper [PARALLEL-SAFE]

A thin top-bar wrapper composing `AttentionBudgetMeter` in a compact horizontal layout.

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/AttentionStrip.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/__tests__/AttentionStrip.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { AttentionStrip } from '../AttentionStrip';

const snap = { max_attention_ms: 3_600_000, spent_ms: 1_800_000, interrupt_freq_per_hour: 5.2,
  total_requests: 0, auto_approved: 0, rejected: 0, last_interrupt_ms: 0, inbox_suppressed_count: 0 };

describe('AttentionStrip', () => {
  it('renders nothing when budget is null', () => {
    const { container } = render(<AttentionStrip budget={null} waitingQuestions={0} blockedTasks={0} />);
    expect(container.firstChild).toBeNull();
  });
  it('renders the meter when budget present', () => {
    const { container } = render(<AttentionStrip budget={snap} waitingQuestions={1} blockedTasks={0} />);
    expect(container.textContent).toMatch(/waiting/i);
  });
});
```

- [ ] **Step 2:** run → FAIL (module missing).

- [ ] **Step 3: Implement**

```tsx
import type { AttentionBudgetSnapshot } from '../../types/tauri';
import { AttentionBudgetMeter } from '../surfaces/AttentionBudgetMeter';

interface AttentionStripProps {
  budget: AttentionBudgetSnapshot | null | undefined;
  waitingQuestions: number;
  blockedTasks: number;
}

export function AttentionStrip({ budget, waitingQuestions, blockedTasks }: AttentionStripProps) {
  if (!budget) return null;
  return (
    <div className="flex items-center gap-3 px-3 py-1.5 bg-[#0c0c0e] border-b border-zinc-800">
      <AttentionBudgetMeter budget={budget} waitingQuestions={waitingQuestions} blockedTasks={blockedTasks} />
    </div>
  );
}
```
(Confirm the `AttentionBudgetSnapshot` import path against the Step-1 gate of Task 1; it is in `types/tauri.ts`.)

- [ ] **Step 4:** run → PASS. `npx tsc --noEmit`.

- [ ] **Step 5: Commit** `git commit -m "feat(gui): AttentionStrip top-bar wrapper"`

---

### Task 3 — Mount AttentionStrip in the app shell [SEQUENTIAL]

**Files:** Modify `crates/vox-gui/ui/src/App.tsx`

- [ ] **Step 1 (gate):** `rg -n "useOrchestratorStatus|orchQuery|attention_budget|return \\(" crates/vox-gui/ui/src/App.tsx | head -40` — confirm `orchQuery.data?.attention_budget` is the live value and locate the shell's top-level JSX return.

- [ ] **Step 2: Render the strip** at the top of the shell:

```tsx
import { AttentionStrip } from './components/layout/AttentionStrip';
// inside the component body:
const attentionBudget = orchQuery.data?.attention_budget ?? null;
// waitingQuestions/blockedTasks: 0 in Phase 0 — Phase 2 Task 6 wires real values.
// at the top of the shell JSX:
<AttentionStrip budget={attentionBudget} waitingQuestions={0} blockedTasks={0} />
```

- [ ] **Step 3:** `npx tsc --noEmit && npx vitest run` → clean, all green.

- [ ] **Step 4: Commit** `git commit -m "feat(gui): mount AttentionStrip in shell (counts wired in Phase 2)"`

---

### Self-review notes
- Reuses existing `AttentionBudgetMeter` + `AttentionBudgetSnapshot` (audit finding G) — no duplicate parser/type.
- Counts stub to 0 with an explicit forward reference to Phase 2 Task 6 (documented, not a hidden placeholder).
- `App.tsx` access via `orchQuery.data` (react-query), not a phantom `useState`/listener (audit fix).
- jsdom pragma on every component test (audit fix).
