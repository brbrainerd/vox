# Track C — Drive Console UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Replace Loquela's `LQ_MODES` segment + passive risk pill with the Drive Console — one brass-trimmed strip carrying clutch · cost · risk · model · send/stop — plus per-task model badges in the transcript.

**Architecture:** New `DriveConsole.tsx` composed of existing `Segment`/`Glass`/`Pill`/`Popover` primitives, reading the `drive-console.v1.yaml` contract for labels/order. `RiskPopover.tsx` for configuration. `ModelBadge.tsx` rendered per execution block with collapsed-by-default progressive disclosure. Send button shows the Enter glyph and flips to Stop during a run.

**Tech Stack:** React + TypeScript (vox-gui/ui), Tailwind, existing design tokens, vitest, Playwright.

**Scope marker:** `[SEQUENTIAL]` after Track B (needs `model_id` + interrupt command).

---

## File Structure

- Create: `crates/vox-gui/ui/src/components/surfaces/Loquela/DriveConsole.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Loquela/RiskPopover.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/ModelBadge.tsx`
- Create: `crates/vox-gui/ui/src/lib/driveConsole.ts` (typed contract loader + ClutchProfile/RiskPosture types mirroring `drive-console.v1.yaml`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx` — remove `LQ_MODES` (line 20) + risk pill (620-626), mount `DriveConsole`; send button (554-556) shows `↵`, flips to Stop on running state.
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatAgentEventRow.tsx` (or the execution-block header) — render `ModelBadge`.
- Test: vitest `*.test.tsx` beside each component.

---

### Task 1: `driveConsole.ts` — typed contract + payload mapping

**Files:**
- Create: `crates/vox-gui/ui/src/lib/driveConsole.ts`
- Test: `crates/vox-gui/ui/src/lib/driveConsole.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect } from "vitest";
import { CLUTCH_DETENTS, RISK_POSTURES, defaultControl, type ControlState } from "./driveConsole";

describe("driveConsole contract", () => {
  it("exposes four clutch detents in order", () => {
    expect(CLUTCH_DETENTS.map(d => d.id)).toEqual(["free", "efficiency", "balanced", "genius"]);
  });
  it("exposes three risk postures", () => {
    expect(RISK_POSTURES.map(r => r.id)).toEqual(["high", "moderate", "low"]);
  });
  it("defaults to efficiency + moderate", () => {
    const s: ControlState = defaultControl();
    expect(s.clutch).toBe("efficiency");
    expect(s.risk).toBe("moderate");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/driveConsole.test.ts 2>&1 | tail -20`
Expected: FAIL — cannot resolve `./driveConsole`.

- [ ] **Step 3: Write the implementation**

```ts
// Mirror of contracts/gui/drive-console.v1.yaml (kept in sync by the BE parity gate).
export type ClutchId = "free" | "efficiency" | "balanced" | "genius";
export type RiskId = "high" | "moderate" | "low";

export const CLUTCH_DETENTS: { id: ClutchId; label: string; hint: string }[] = [
  { id: "free",       label: "Free",   hint: "Free models only" },
  { id: "efficiency", label: "Effic.", hint: "Most out of the tokens you spend; delegates to free agents on simple tasks" },
  { id: "balanced",   label: "Bal.",   hint: "Balanced cost/quality" },
  { id: "genius",     label: "Genius", hint: "Most intelligent solutions; budget relaxed" },
];

export const RISK_POSTURES: { id: RiskId; label: string; tone: "rose" | "amber" | "emerald" }[] = [
  { id: "high",     label: "High",     tone: "rose" },
  { id: "moderate", label: "Moderate", tone: "amber" },
  { id: "low",      label: "Low",      tone: "emerald" },
];

export interface ControlState {
  clutch: ClutchId;
  risk: RiskId;
  safetyTokenBudget?: number; // optional override surfaced in the risk popover
}

export function defaultControl(): ControlState {
  return { clutch: "efficiency", risk: "moderate" };
}
```

- [ ] **Step 4: Run test → PASS**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/driveConsole.test.ts 2>&1 | tail -20`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/driveConsole.ts crates/vox-gui/ui/src/lib/driveConsole.test.ts
git commit -m "feat(gui): driveConsole contract types + default control state"
```

---

### Task 2: `RiskPopover.tsx`

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Loquela/RiskPopover.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Loquela/RiskPopover.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent, screen } from "@testing-library/react";
import { RiskPopover } from "./RiskPopover";

describe("RiskPopover", () => {
  it("emits the chosen posture", () => {
    const onChange = vi.fn();
    render(<RiskPopover risk="moderate" onChange={onChange} open onClose={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /low/i }));
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ risk: "low" }));
  });
});
```

- [ ] **Step 2: Run → FAIL**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Loquela/RiskPopover.test.tsx 2>&1 | tail -20`
Expected: FAIL — cannot resolve `./RiskPopover`.

- [ ] **Step 3: Implement** (keyboard-navigable; describes what each posture means)

```tsx
import React from "react";
import { RISK_POSTURES, type RiskId, type ControlState } from "../../../lib/driveConsole";

const COPY: Record<RiskId, string> = {
  high: "Break things — auto-approve more, gates shadow-only, fewer safety tokens.",
  moderate: "Confirm + enforce grounding. Balanced safety.",
  low: "Enforce verification + grounding, raise approval, spend safety tokens, lean model up.",
};

export function RiskPopover(props: {
  risk: RiskId;
  open: boolean;
  onChange: (next: Partial<ControlState>) => void;
  onClose: () => void;
}) {
  if (!props.open) return null;
  return (
    <div role="dialog" aria-label="Configure acceptable risk"
         className="absolute z-50 w-72 rounded-lg border border-white/10 bg-[#0b0b0e] p-3 text-[11px] shadow-xl">
      <div className="mb-2 text-[10px] uppercase tracking-widest text-zinc-500">Acceptable risk</div>
      {RISK_POSTURES.map(p => (
        <button key={p.id} type="button" onClick={() => { props.onChange({ risk: p.id }); }}
          aria-pressed={props.risk === p.id}
          className={`mb-1 flex w-full flex-col rounded-md border px-2 py-1.5 text-left ${
            props.risk === p.id ? "border-brass/40 bg-brass/10" : "border-white/8 hover:border-white/20"}`}>
          <span className="font-medium capitalize">{p.label} risk</span>
          <span className="text-zinc-400">{COPY[p.id]}</span>
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Run → PASS, then commit**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Loquela/RiskPopover.test.tsx 2>&1 | tail -20` → PASS.

```bash
git add crates/vox-gui/ui/src/components/surfaces/Loquela/RiskPopover.{tsx,test.tsx}
git commit -m "feat(gui): RiskPopover — configurable acceptable-risk control"
```

---

### Task 3: `DriveConsole.tsx`

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Loquela/DriveConsole.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Loquela/DriveConsole.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent, screen } from "@testing-library/react";
import { DriveConsole } from "./DriveConsole";
import { defaultControl } from "../../../lib/driveConsole";

describe("DriveConsole", () => {
  const base = {
    control: defaultControl(),
    onControlChange: vi.fn(),
    spentUsd: 0.42, budgetUsd: 1.0, burnPerMin: 0.08,
    model: "flash", auto: true,
  };
  it("renders all four clutch detents, cost, risk, model", () => {
    render(<DriveConsole {...base} />);
    ["Free", "Effic.", "Bal.", "Genius"].forEach(l =>
      expect(screen.getByRole("button", { name: new RegExp(l, "i") })).toBeTruthy());
    expect(screen.getByText(/0\.42/)).toBeTruthy();
    expect(screen.getByText(/Moderate/i)).toBeTruthy();
    expect(screen.getByText(/flash/i)).toBeTruthy();
  });
  it("emits clutch change", () => {
    const onControlChange = vi.fn();
    render(<DriveConsole {...base} onControlChange={onControlChange} />);
    fireEvent.click(screen.getByRole("button", { name: /Genius/i }));
    expect(onControlChange).toHaveBeenCalledWith(expect.objectContaining({ clutch: "genius" }));
  });
});
```

- [ ] **Step 2: Run → FAIL**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Loquela/DriveConsole.test.tsx 2>&1 | tail -20`
Expected: FAIL — cannot resolve `./DriveConsole`.

- [ ] **Step 3: Implement** (one continuous strip; risk = 3px state bar; single brass accent; ≥24px targets)

```tsx
import React, { useState } from "react";
import { CLUTCH_DETENTS, RISK_POSTURES, type ControlState } from "../../../lib/driveConsole";
import { RiskPopover } from "./RiskPopover";

const TONE_BG: Record<string, string> = { rose: "bg-rose-400", amber: "bg-amber-400", emerald: "bg-emerald-400" };

export function DriveConsole(props: {
  control: ControlState;
  onControlChange: (next: Partial<ControlState>) => void;
  spentUsd: number; budgetUsd: number; burnPerMin?: number;
  model: string; auto: boolean;
}) {
  const [riskOpen, setRiskOpen] = useState(false);
  const risk = RISK_POSTURES.find(r => r.id === props.control.risk)!;
  const pct = props.budgetUsd > 0 ? Math.min(100, (props.spentUsd / props.budgetUsd) * 100) : 0;
  return (
    <div className="relative flex items-stretch overflow-hidden rounded-lg border border-white/10 text-[11px]">
      {/* ① Clutch */}
      <div className="flex items-center gap-1 border-r border-white/[0.07] px-2.5 py-1.5">
        <span className="text-zinc-500" aria-hidden>⚙</span>
        <div role="radiogroup" aria-label="Clutch — how much to spend" className="flex gap-0.5">
          {CLUTCH_DETENTS.map(d => (
            <button key={d.id} type="button" title={d.hint} aria-pressed={props.control.clutch === d.id}
              onClick={() => props.onControlChange({ clutch: d.id })}
              className={`min-h-[24px] rounded px-1.5 font-medium ${
                props.control.clutch === d.id ? "bg-brass/[0.16] text-brass" : "text-zinc-400 hover:text-zinc-200"}`}>
              {d.label}
            </button>
          ))}
        </div>
      </div>
      {/* ② Cost */}
      <div className="flex items-center gap-2 border-r border-white/[0.07] px-2.5 py-1.5" title="Live spend">
        <span className="font-mono text-brass">${props.spentUsd.toFixed(2)}</span>
        <span className="font-mono text-zinc-500">/{props.budgetUsd.toFixed(2)}</span>
        <span className="relative h-[3px] w-12 rounded bg-white/[0.08]">
          <span className="absolute inset-y-0 left-0 rounded bg-gradient-to-r from-emerald-400 to-brass"
                style={{ width: `${pct}%` }} />
        </span>
        {props.burnPerMin != null && <span className="text-zinc-500">↑${props.burnPerMin.toFixed(2)}/m</span>}
      </div>
      {/* ③ Risk */}
      <button type="button" aria-label={`Risk: ${risk.label} — click to configure`} aria-expanded={riskOpen}
        onClick={() => setRiskOpen(o => !o)}
        className="flex items-center gap-1.5 border-r border-white/[0.07] px-2.5 py-1.5 hover:bg-white/[0.03]">
        <span className={`h-3.5 w-[3px] rounded ${TONE_BG[risk.tone]}`} aria-hidden />
        <span>{risk.label}</span><span className="text-zinc-600">▾</span>
      </button>
      {/* ④ Model read-out */}
      <div className="flex items-center gap-1 px-2.5 py-1.5" title="Active model (Auto shows live pick)">
        {props.auto && <span className="text-zinc-500">Auto·</span>}
        <span className="text-brass">{props.model}</span>
        <span className="text-zinc-600" aria-hidden>ⓘ</span>
      </div>
      <RiskPopover open={riskOpen} risk={props.control.risk}
        onChange={(n) => { props.onControlChange(n); setRiskOpen(false); }} onClose={() => setRiskOpen(false)} />
    </div>
  );
}
```

- [ ] **Step 4: Run → PASS, commit**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Loquela/DriveConsole.test.tsx 2>&1 | tail -20` → PASS (2).

```bash
git add crates/vox-gui/ui/src/components/surfaces/Loquela/DriveConsole.{tsx,test.tsx}
git commit -m "feat(gui): DriveConsole — clutch/cost/risk/model strip"
```

---

### Task 4: Mount in Loquela; Enter-on-button; Send↔Stop flip

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx`

- [ ] **Step 1: Remove the old controls** — delete `LQ_MODES` (line 20-24), the `<Segment value={mode}…>`
mount (line 560), and the risk pill (lines 620-626). Keep the `tier` picker for now (model selection stays
available; the console's model read-out reflects it). Replace `mode` state with `control: ControlState`
(`useState(defaultControl())`), persisted to `gui.chat.control.v1` via the existing `useLocalStorage` helper.

- [ ] **Step 2: Mount DriveConsole** in the bottom control row (where the segment was):

```tsx
<DriveConsole
  control={control}
  onControlChange={(n) => setControl(c => ({ ...c, ...n }))}
  spentUsd={sessionSpentUsd} budgetUsd={sessionBudgetUsd} burnPerMin={burnPerMin}
  model={liveModel ?? (tierObj.label.split(" · ")[0])} auto={tier === "auto"} />
```

`sessionSpentUsd`/`sessionBudgetUsd`/`burnPerMin` come from the budget SSOT (the cost segment that
`ChatExecutionRail` already fetches); `liveModel` from the model read-out source. If a value isn't wired yet,
pass `0`/`undefined` — the console renders gracefully (Track F wires live series).

- [ ] **Step 3: Update the payload** — replace `mode` in the `ChatPayload` (send(), line 412-426) with
`clutch: control.clutch` and `risk: control.risk` (additive; BE reads these via Track D). Keep `tier`.

- [ ] **Step 4: Enter-on-button + Stop flip** — change the send button (554-556) so the Enter glyph is inside
the button and the button is the click target, and flip to Stop when a run is active:

```tsx
{running ? (
  <button type="button" onClick={() => onInterrupt?.()} aria-label="Stop (Enter)"
    className="inline-flex items-center gap-2 rounded-md border border-rose-400/45 bg-rose-400/[0.12] px-3 py-1.5 text-rose-300">
    <Icon.stop className="size-3.5" /> Stop <kbd className="rounded border border-current px-1 text-[9px] opacity-75">↵</kbd>
  </button>
) : (
  <button type="button" onClick={send} disabled={!text.trim()} aria-label="Run (Enter)"
    className={`inline-flex items-center gap-2 rounded-md border px-3 py-1.5 ${text.trim()
      ? "border-brass/40 bg-brass/15 text-brass" : "cursor-not-allowed border-white/10 text-zinc-600"}`}>
    {dryRun ? "Dry-run" : "Run"} <kbd className="rounded border border-current px-1 text-[9px] opacity-75">↵</kbd>
  </button>
)}
```

`onInterrupt` calls the `interrupt_orchestrator_task` command (Track B) with the active task id; `running`
is true while a submitted task is in-progress (derive from the existing task/stream state). Add `Icon.stop`
if absent (reuse an existing square/stop glyph in the Icon set).

- [ ] **Step 5: Build + smoke test + commit**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Loquela 2>&1 | tail -20` → PASS.
Run: `cd crates/vox-gui/ui && npx tsc --noEmit 2>&1 | tail -20` → no errors.

```bash
git add crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx
git commit -m "feat(gui): mount DriveConsole in Loquela; Enter-on-button; Send<->Stop flip"
```

---

### Task 5: `ModelBadge.tsx` — per-task attribution + progressive disclosure

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/ModelBadge.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ModelBadge.test.tsx`
- Modify: the execution-block header (`ChatAgentEventRow.tsx` or `ChatTranscript.tsx`) to render it.

- [ ] **Step 1: Write the failing test**

```tsx
import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ModelBadge } from "./ModelBadge";

describe("ModelBadge", () => {
  const attr = { model: "claude-opus", reqTokens: 4200, respTokens: 1100, costUsd: 0.06,
    selectionReason: "scored", latencyMs: 820 };
  it("shows model + tokens collapsed", () => {
    render(<ModelBadge {...attr} />);
    expect(screen.getByText(/claude-opus/)).toBeTruthy();
    expect(screen.queryByText(/scored/)).toBeNull(); // detail hidden by default
  });
  it("reveals detail on activate (keyboard reachable)", () => {
    render(<ModelBadge {...attr} />);
    fireEvent.click(screen.getByRole("button", { name: /claude-opus/i }));
    expect(screen.getByText(/scored/)).toBeTruthy();
    expect(screen.getByText(/820/)).toBeTruthy();
  });
  it("renders unknown when no model", () => {
    render(<ModelBadge model={undefined} />);
    expect(screen.getByText(/model unknown/i)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run → FAIL**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Chat/ModelBadge.test.tsx 2>&1 | tail -20`
Expected: FAIL — cannot resolve `./ModelBadge`.

- [ ] **Step 3: Implement** (button = focus-reachable; not hover-only, per a11y requirement)

```tsx
import React, { useState } from "react";

export function ModelBadge(props: {
  model?: string; provider?: string; reqTokens?: number; respTokens?: number;
  costUsd?: number; selectionReason?: string; latencyMs?: number;
}) {
  const [open, setOpen] = useState(false);
  if (!props.model) return <span className="text-[10px] text-zinc-600">model unknown</span>;
  return (
    <span className="relative">
      <button type="button" onClick={() => setOpen(o => !o)} aria-expanded={open}
        aria-label={`Completed by ${props.model} — details`}
        className="rounded border border-brass/30 px-1.5 py-0.5 text-[10px] text-brass hover:bg-brass/[0.08]">
        {props.model}
        {props.reqTokens != null && <span className="ml-1 text-zinc-500">{props.reqTokens}↑ {props.respTokens}↓</span>}
        {props.costUsd != null && <span className="ml-1 text-zinc-500">${props.costUsd.toFixed(2)}</span>}
        <span className="ml-1 text-zinc-600" aria-hidden>ⓘ</span>
      </button>
      {open && (
        <div role="region" className="absolute right-0 z-50 mt-1 w-64 rounded-md border border-white/10 bg-[#0b0b0e] p-2 text-[10px] text-zinc-300">
          {props.provider && <div>provider: {props.provider}</div>}
          {props.selectionReason && <div>reason: {props.selectionReason}</div>}
          {props.latencyMs != null && <div>latency: {props.latencyMs} ms</div>}
          <div className="mt-1 text-zinc-500">What was sent / received is available when I/O capture is enabled.</div>
        </div>
      )}
    </span>
  );
}
```

- [ ] **Step 4: Render it in the execution block** — at the top of each execution/agent block header, add
`<ModelBadge model={row.model_id} reqTokens={...} .../>` sourced from the message's `model_id` (Track B) and,
where available, the task attestation. Collapsed by default satisfies "progressive disclosure".

- [ ] **Step 5: Run → PASS, commit**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Chat/ModelBadge.test.tsx 2>&1 | tail -20` → PASS (3).

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ModelBadge.{tsx,test.tsx} crates/vox-gui/ui/src/components/surfaces/Chat/ChatAgentEventRow.tsx
git commit -m "feat(gui): per-task ModelBadge with progressive disclosure"
```

---

## Self-Review

**Spec coverage:** §3 console (clutch/cost/risk/model) → Tasks 1–4; §3.4 per-task attribution badge → Task 5;
§3.5 Enter-on-button + Stop flip → Task 4 Step 4; §7 styling (one strip, 3px risk bar, single brass accent,
≥24px targets, focus-reachable disclosure) → Tasks 3 & 5. **Type consistency:** `ControlState`/`ClutchId`/
`RiskId` from `driveConsole.ts` used by RiskPopover, DriveConsole, Loquela; `model_id` (Track B) feeds
ModelBadge `model`. **Placeholder scan:** payload/budget wiring points name concrete sources (`ChatPayload`
send(), the budget segment in `ChatExecutionRail`) with a graceful-fallback note rather than inventing values;
Loquela edits cite exact line ranges. **A11y:** radiogroup + aria-pressed clutch, aria-expanded risk/badge,
badge detail is button-activated (not hover-only).
