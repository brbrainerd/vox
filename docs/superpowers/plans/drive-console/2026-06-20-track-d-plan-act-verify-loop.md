# Track D — Plan/Act/Verify Automated Loop + Intervention Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Turn Plan/Act/Verify from a never-read label into a real orchestrator loop driven by the clutch/risk, with phase state surfaced in the transcript and user intervention at phase boundaries.

**Architecture:** Bind the (now-live) `ResolvedClutch`/`ResolvedRisk` (Track A) and `PlanModeTrigger` into a phase state machine: `Planning → Acting → Verifying → Done`. Auto-chain plan synthesis → `ai.plan.execute` → `OperatingMode::Verification` instead of separate user RPCs. Emit a `PhaseChanged` event per task; add `approve_plan` / `skip_verify` / `force_verify` daemon methods + Tauri commands for boundary intervention. Verification is forced when `RiskPosture::Low` (`socrates_enforce`/`grounding_enforce`), skippable when `High`.

**Tech Stack:** Rust (vox-orchestrator), existing EventBus, daemon RPC + Tauri commands, vitest for the FE phase chip.

**Scope marker:** `[SEQUENTIAL]` after Track C (consumes console control + needs the phase chip in the execution block). Depends on Track A types.
**Execution target:** Sonnet 4.6.

---

## Audit Corrections — verified against code 2026-06-20 (read FIRST; overrides stale claims below)

- **NAME COLLISION — rename our enum to `PavPhase`.** `TaskPhase` ALREADY EXISTS (`types/tasks.rs:101`) as `{Inspect,Localize,Hypothesize,Act,Verify,Decide}` (the OODA-style debug phases), and `AgentTask.current_phase: Option<TaskPhase>` uses it. Our four-state `{Planning,Acting,Verifying,Done}` must be a NEW type — call it **`PavPhase`** — everywhere in Task 1 (the type, `PhaseLoop::phase()->PavPhase`, the FE `Phase` union, serde tags). Do not redefine the existing `TaskPhase`.
- **`PlanModeTrigger::default()` DOES NOT EXIST** — it has `new(config: PlanModeTriggerConfig)` (`plan_mode_trigger.rs:81`). Task 2's test uses `PlanModeTrigger::default()`. **Add `impl Default for PlanModeTrigger { fn default() -> Self { Self::new(PlanModeTriggerConfig::default()) } }`** (confirm `PlanModeTriggerConfig: Default`) as the first step of Task 2, or construct via `new(Default::default())` in the test.
- **CONFIRMED & EASIER THAN PLANNED:** the `ai.plan.execute` enqueue is already a callable function — `crate::planning::schedule::enqueue_runnable_plan_nodes(orch, plan_session_id, plan_version, session_id, tenant_id)` (`schedule.rs:40-87`), invoked by `handle_plan_execute` (`dei_dispatch.rs:172-213`). Auto-chaining (Task 3) just calls it after synthesis — no extraction needed. `OperatingMode::Verification` (`context_envelope.rs:28`) + the `doubt_task` re-enqueue pattern (`doubt.rs:79`) are the SSOT to mimic. `PlanModeSignal`/`PlanModeDecision` fields/variants match.
- **EVENT REALITY — don't invent `PhaseChanged` or a `vox://` channel.** The `EventBus` is a `tokio::broadcast::Sender<AgentEvent>` (`events.rs:718`) — **no string channel paths**; "`vox://agent-events`" is wrong (delete that phrasing). A phase event already exists: `AgentEventKind::TaskPhaseChanged{task_id,agent_id,phase: TaskPhase}` (`events.rs:162`) — but it carries the *existing* `TaskPhase`. **Add a new variant `AgentEventKind::PavPhaseChanged{task_id, phase: PavPhase}`** and have the GUI subscribe through the same agent-events stream the Chat surface already consumes (find that subscription in the FE; it's the existing broadcast bridge, not a new channel).
- **`PhaseLoop` HAS NO STORAGE — add it.** `AgentTask` has `current_phase: Option<TaskPhase>` but no slot for our loop. **Add `pav_loop: Option<PavLoopState>` to `AgentTask`** (serializable: `{phase: PavPhase, verify_required: bool, verify_skipped: bool}`) rather than a side-map (simpler lifecycle; dies with the task). `PhaseLoop` becomes the logic over that state.
- **Intervention handlers don't exist yet (expected)** — `DOUBT_TASK`/`OVERRULE_TASK` constants (`vox-foundation/src/protocol.rs`) + GUI cmds (`control_plane.rs:138,159`) are the pattern to copy for `APPROVE_PLAN`/`SKIP_VERIFY`/`FORCE_VERIFY`. Daemon dispatch arms go in `orch_daemon/mod.rs` (alongside `:527 DOUBT_TASK`).
- **Test harness CONFIRMED:** mimic `test_orchestrator()` + `orch.submit_task(...)` + `orch.status()` (`orchestrator/tests/mod.rs:~237-252`).
- **HARD DEP:** Track A's `ClutchProfile`/`RiskPosture`/`ResolvedRisk` must be landed first (Task 1 reads `risk.resolve().socrates_enforce`).

---

## File Structure

- Modify: `crates/vox-orchestrator/src/planning/plan_mode_trigger.rs` — feed clutch/risk into the decision.
- Create: `crates/vox-orchestrator/src/planning/phase_loop.rs` — the `TaskPhase` state machine + transition rules.
- Modify: the dispatch path that calls `ai.plan.execute` (`orch_daemon/dei_dispatch.rs:172-213`) — auto-chain.
- Modify: `crates/vox-gui/src/commands/control_plane.rs` — `approve_plan` / `skip_verify` / `force_verify` commands.
- Modify: `ChatAgentEventRow.tsx` / execution-block header — render the live phase chip + boundary buttons.

---

### Task 1: `TaskPhase` state machine (pure)

**Files:**
- Create: `crates/vox-orchestrator/src/planning/phase_loop.rs`
- Modify: `crates/vox-orchestrator/src/planning/mod.rs` (add `pub mod phase_loop;`)

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-orchestrator/src/planning/phase_loop.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::{ClutchProfile, RiskPosture};

    #[test]
    fn genius_plans_then_acts_then_verifies() {
        let mut p = PhaseLoop::start(ClutchProfile::Genius, RiskPosture::Moderate);
        assert_eq!(p.phase(), TaskPhase::Planning);
        p.advance(); assert_eq!(p.phase(), TaskPhase::Acting);
        p.advance(); assert_eq!(p.phase(), TaskPhase::Verifying);
        p.advance(); assert_eq!(p.phase(), TaskPhase::Done);
    }

    #[test]
    fn efficiency_reacts_skips_planning() {
        let p = PhaseLoop::start(ClutchProfile::Efficiency, RiskPosture::High);
        assert_eq!(p.phase(), TaskPhase::Acting); // React: no upfront plan
    }

    #[test]
    fn low_risk_forces_verify_even_when_high_clutch_would_skip() {
        let mut p = PhaseLoop::start(ClutchProfile::Efficiency, RiskPosture::Low);
        // React → Acting, then Low risk forces Verifying (not Done)
        assert_eq!(p.phase(), TaskPhase::Acting);
        p.advance(); assert_eq!(p.phase(), TaskPhase::Verifying);
    }

    #[test]
    fn high_risk_allows_skip_verify() {
        let mut p = PhaseLoop::start(ClutchProfile::Genius, RiskPosture::High);
        p.advance(); // Planning -> Acting
        p.skip_verify();
        p.advance(); assert_eq!(p.phase(), TaskPhase::Done);
    }
}
```

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p vox-orchestrator phase_loop 2>cargo-phase.log; tail -30 cargo-phase.log`
Expected: FAIL — module/type missing.

- [ ] **Step 3: Implement**

```rust
//! Pure Plan/Act/Verify phase machine. Planning is entered only when the clutch
//! warrants plan-first; Verifying is forced by Low risk and skippable under High.
use crate::mode::{ClutchProfile, RiskPosture};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPhase { Planning, Acting, Verifying, Done }

#[derive(Debug, Clone)]
pub struct PhaseLoop {
    phase: TaskPhase,
    verify_required: bool,
    verify_skipped: bool,
}

impl PhaseLoop {
    #[must_use]
    pub fn start(clutch: ClutchProfile, risk: RiskPosture) -> Self {
        // Plan-first for Balanced/Genius; React (act-first) for Free/Efficiency.
        let plan_first = matches!(clutch, ClutchProfile::Balanced | ClutchProfile::Genius);
        // Low risk forces verification; Genius also verifies; High may skip.
        let verify_required = risk.resolve().socrates_enforce
            || matches!(clutch, ClutchProfile::Genius);
        Self {
            phase: if plan_first { TaskPhase::Planning } else { TaskPhase::Acting },
            verify_required,
            verify_skipped: false,
        }
    }

    #[must_use] pub fn phase(&self) -> TaskPhase { self.phase }
    pub fn skip_verify(&mut self) { self.verify_skipped = true; }
    pub fn force_verify(&mut self) { self.verify_required = true; self.verify_skipped = false; }

    /// Advance to the next phase given the current rules.
    pub fn advance(&mut self) {
        self.phase = match self.phase {
            TaskPhase::Planning => TaskPhase::Acting,
            TaskPhase::Acting => {
                if self.verify_required && !self.verify_skipped { TaskPhase::Verifying }
                else { TaskPhase::Done }
            }
            TaskPhase::Verifying => TaskPhase::Done,
            TaskPhase::Done => TaskPhase::Done,
        };
    }
}
```

- [ ] **Step 4: Run → PASS, commit**

Run: `cargo test -p vox-orchestrator phase_loop 2>cargo-phase.log; tail -20 cargo-phase.log` → PASS (4).

```bash
git add crates/vox-orchestrator/src/planning/phase_loop.rs crates/vox-orchestrator/src/planning/mod.rs
git commit -m "feat(orchestrator): pure Plan/Act/Verify phase machine driven by clutch+risk"
```

---

### Task 2: Feed clutch/risk into `PlanModeTrigger`

**Files:**
- Modify: `crates/vox-orchestrator/src/planning/plan_mode_trigger.rs`

- [ ] **Step 1: Read** `plan_mode_trigger.rs` (`decide()` at ~line 90). It currently decides React vs
PlanAndExecute purely from task signals (complexity/dependencies). Add the clutch as an explicit override.

- [ ] **Step 2: Write the failing test** (in that file's test module)

```rust
#[test]
fn genius_clutch_forces_plan_and_execute_even_on_simple_task() {
    let trigger = PlanModeTrigger::default();
    let simple = PlanModeSignal { complexity: 0, dependency_count: 0, tool_hint_count: 0, prior_adequacy_score: 1.0 };
    assert_eq!(trigger.decide_with_clutch(&simple, Some(ClutchProfile::Genius)), PlanModeDecision::PlanAndExecute);
    assert_eq!(trigger.decide_with_clutch(&simple, Some(ClutchProfile::Efficiency)), PlanModeDecision::React);
}
```

- [ ] **Step 3: Implement `decide_with_clutch`** (keep `decide` as the signal-only path; new method composes)

```rust
pub fn decide_with_clutch(&self, signal: &PlanModeSignal, clutch: Option<crate::mode::ClutchProfile>) -> PlanModeDecision {
    use crate::mode::ClutchProfile;
    if matches!(clutch, Some(ClutchProfile::Balanced | ClutchProfile::Genius)) {
        return PlanModeDecision::PlanAndExecute;
    }
    if matches!(clutch, Some(ClutchProfile::Free | ClutchProfile::Efficiency)) {
        return PlanModeDecision::React;
    }
    self.decide(signal)
}
```

- [ ] **Step 4: Run → PASS, commit**

Run: `cargo test -p vox-orchestrator decide_with_clutch 2>cargo-trig.log; tail -20 cargo-trig.log` → PASS.

```bash
git add crates/vox-orchestrator/src/planning/plan_mode_trigger.rs
git commit -m "feat(orchestrator): clutch overrides plan-vs-react decision"
```

---

### Task 3: Auto-chain plan → act → verify + emit `PhaseChanged`

**Files:**
- Modify: dispatch path (`orch_daemon/dei_dispatch.rs:172-213`) and the task-complete path.

- [ ] **Step 1: Read** `dei_dispatch.rs` `ai.plan.execute` handler + the completion path. Today plan synthesis
and `ai.plan.execute` are separate RPCs; completion does not advance any phase.

- [ ] **Step 2: Write an integration test** asserting that submitting a task with `clutch=genius` results in
(a) a plan being synthesized, (b) execution nodes auto-enqueued without a separate `ai.plan.execute` call, and
(c) a `PhaseChanged` event sequence `Planning,Acting,Verifying,Done` on the EventBus. Use the orchestrator's
existing in-process test harness (grep an existing `submit_*` integration test for the pattern).

- [ ] **Step 3: Implement** — when a task carries a clutch that yields `PlanAndExecute`, after plan synthesis
call the same internal routine `ai.plan.execute` invokes (extract it to a function if it's inline in the RPC),
instead of waiting for the client. Construct a `PhaseLoop` per task (store it on the task or a side-map keyed
by `TaskId`); on each phase transition, publish `AgentEvent`/`PhaseChanged { task_id, phase }` via the EventBus
(reuse the event the audit calls `vox://agent-events`). On task completion, `advance()` the loop; if it lands
on `Verifying`, set `OperatingMode::Verification` (the rider from `context_envelope.rs:47`) and re-enqueue
instead of marking Done.

- [ ] **Step 4: Run the integration test → PASS, commit**

Run: `cargo test -p vox-orchestrator plan_act_verify 2>cargo-chain.log; tail -30 cargo-chain.log` → PASS.

```bash
git add -A && git commit -m "feat(orchestrator): auto-chain plan->act->verify + PhaseChanged events"
```

---

### Task 4: Intervention commands (approve plan / skip verify / force verify)

**Files:**
- Modify: `crates/vox-gui/src/commands/control_plane.rs` + daemon method constants/handler.

- [ ] **Step 1: Add daemon methods** `APPROVE_PLAN`, `SKIP_VERIFY`, `FORCE_VERIFY` (next to `CANCEL_TASK`).

- [ ] **Step 2: Add handlers** routing each to the task's `PhaseLoop`: `approve_plan` advances Planning→Acting
(and unblocks the held plan); `skip_verify` calls `loop.skip_verify()`; `force_verify` calls
`loop.force_verify()`. Each publishes `PhaseChanged`.

- [ ] **Step 3: Add the three Tauri commands** mirroring `cancel_orchestrator_task` (same `ControlPlaneResult`
shape + `emit_tasks_changed`). Register them in `generate_handler!`.

- [ ] **Step 4: Test a handler transition** (orchestrator unit): a task in `Verifying` with `skip_verify`
issued before completion lands on `Done`; a task in `Acting` under High risk with `force_verify` lands on
`Verifying`. Run: `cargo test -p vox-orchestrator intervention 2>cargo-iv.log; tail -20 cargo-iv.log` → PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: phase-boundary intervention commands (approve_plan/skip_verify/force_verify)"
```

---

### Task 5: FE phase chip + boundary buttons

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/PhaseChip.tsx` (+ test)
- Modify: the execution-block header to render it + subscribe to `PhaseChanged`.

- [ ] **Step 1: Write the failing test**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { PhaseChip } from "./PhaseChip";

describe("PhaseChip", () => {
  it("shows the live phase", () => {
    render(<PhaseChip phase="verifying" onApprovePlan={()=>{}} onSkipVerify={()=>{}} onForceVerify={()=>{}} />);
    expect(screen.getByText(/Verifying/i)).toBeTruthy();
  });
  it("offers skip-verify during verifying", () => {
    const onSkip = vi.fn();
    render(<PhaseChip phase="verifying" onApprovePlan={()=>{}} onSkipVerify={onSkip} onForceVerify={()=>{}} />);
    fireEvent.click(screen.getByRole("button", { name: /skip verify/i }));
    expect(onSkip).toHaveBeenCalled();
  });
  it("offers approve-plan during planning", () => {
    const onApprove = vi.fn();
    render(<PhaseChip phase="planning" onApprovePlan={onApprove} onSkipVerify={()=>{}} onForceVerify={()=>{}} />);
    fireEvent.click(screen.getByRole("button", { name: /approve plan/i }));
    expect(onApprove).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run → FAIL**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Chat/PhaseChip.test.tsx 2>&1 | tail -20` → FAIL.

- [ ] **Step 3: Implement**

```tsx
import React from "react";
type Phase = "planning" | "acting" | "verifying" | "done";
const LABEL: Record<Phase,string> = { planning:"Planning…", acting:"Acting…", verifying:"Verifying…", done:"Done" };

export function PhaseChip(props: {
  phase: Phase;
  onApprovePlan: () => void; onSkipVerify: () => void; onForceVerify: () => void;
}) {
  return (
    <span className="inline-flex items-center gap-2 text-[10px]">
      <span className="rounded border border-brass/30 px-1.5 py-0.5 text-brass">{LABEL[props.phase]}</span>
      {props.phase === "planning" && (
        <button className="text-zinc-400 hover:text-zinc-200" onClick={props.onApprovePlan}>approve plan</button>
      )}
      {props.phase === "acting" && (
        <button className="text-zinc-400 hover:text-zinc-200" onClick={props.onForceVerify}>force verify</button>
      )}
      {props.phase === "verifying" && (
        <button className="text-zinc-400 hover:text-zinc-200" onClick={props.onSkipVerify}>skip verify</button>
      )}
    </span>
  );
}
```

- [ ] **Step 4: Wire** — in the execution block, render `<PhaseChip phase={livePhase} ...>` where `livePhase`
comes from a `PhaseChanged` subscription (reuse the existing agent-events subscription in the Chat surface),
and the callbacks invoke the Track D Tauri commands with the task id.

- [ ] **Step 5: Run → PASS, commit**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Chat/PhaseChip.test.tsx 2>&1 | tail -20` → PASS (3).

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/PhaseChip.{tsx,test.tsx} crates/vox-gui/ui/src/components/surfaces/Chat/ChatAgentEventRow.tsx
git commit -m "feat(gui): live Plan/Act/Verify phase chip with boundary intervention"
```

---

## Self-Review

**Spec coverage:** §4 automated loop → Tasks 1–3; intervention points → Tasks 4–5; clutch/risk drive the loop
→ Tasks 1–2. **Type consistency:** `TaskPhase`/`PhaseLoop` (Task 1) consumed by Tasks 3–4; `PhaseChanged` event
name reused in Tasks 3 & 5; `socrates_enforce` from Track A `ResolvedRisk` drives `verify_required`.
**Placeholder scan:** Tasks 2–4 include read-then-edit discovery against named files (the inline
`ai.plan.execute` routine and integration-test harness must be read in-repo); all new types/functions shown in
full. **Risk×phase invariant:** Low risk → `socrates_enforce` true → `verify_required` true (Task 1 test 3);
High risk → skip allowed (test 4).
