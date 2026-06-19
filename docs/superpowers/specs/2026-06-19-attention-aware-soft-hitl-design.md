# Attention-Aware Soft Human-in-the-Loop — Design

**Date:** 2026-06-19 (rev 2 — hardened against a full codebase audit + design critique)
**Status:** Approved design, pre-implementation
**Scope:** Surface attention metrics + a soft-HITL "Needs You" feedback inbox
(clarifications + doubts), with agent-declared **task gating** so a pending
question parks specific tasks (by `TaskId`) while the rest of the task list keeps
running — shown as a GUI overlay, not a hopper mutation.

> **Rev-2 note.** Rev 1 was audited against the live codebase by 8 agents (96
> findings, 35 blockers). The structural corrections are folded in below; the
> "Decisions" and "What changed in rev 2" sections record why. Read those before
> the plans.

---

## 1. Problem & destination

Vox has a deep attention-budget + interruption-policy backend, a Socrates
clarification system, and an LA-Noire-inspired doubt loop. Gaps:

- The attention budget **is** rendered (`AttentionBudgetMeter` on the Dashboard),
  but only there — it is not in a persistent top strip, and it shows no
  "waiting / blocked" counts.
- Socrates clarifications (`question_events`, `WithheldQuestion`) have **no GUI
  surface**.
- The doubt loop's GUI is hover buttons on the Dashboard `StreamCard`,
  disconnected from the other feedback channels.
- There is **no way to mark a task "waiting on an answer"** while others proceed.

**Destination:** a soft human-in-the-loop where the system surfaces exactly the
feedback worth the user's attention (governed by the *existing* interruption
policy), makes it visually obvious which tasks are parked on answers, and lets
unaffected tasks continue. This is wiring + surfacing of existing backends, plus
one new concept — the **gating edge** (`TaskId` → `FeedbackRequest`).

## 2. Locked decisions

1. **Needs You** unifies **clarifications + doubts** (soft channel). **Approvals
   stay on their own surface** (`ApprovalsView`/`InlineApprovals`) — blocking,
   300 s timeout, different lifecycle.
2. **Gating edges are agent-declared and keyed by `TaskId(u64)`** — the id agents
   and the doubt loop actually hold. NOT `HopperItemId` (a UUID string; the
   hopper→task map `TaskId(stable_hash(item_id.0))` is one-way).
3. **Blocked is a GUI overlay, not a hopper state.** A task row renders "blocked"
   when its `TaskId` is in the union of open `FeedbackRequest.gates`. We do **not**
   add `ItemState::Blocked`, mutate the hopper, or gate the dispatcher in this
   iteration (see §"What changed", item B). Enforcement = the agent that raised
   the gate self-parks the work it cannot proceed on (that is *why* it asked).
4. **Doubts are non-gating actionable cards.** Today's `doubt_task` is
   user-initiated and self-resolving (re-enqueues the task as a Verifier; the
   agent keeps working). So a doubt does NOT park a task. Its card offers
   **Overrule** (dispatches the real `OVERRULE_TASK`) and **Let-verify** (no-op;
   the Verifier pass continues). It carries `doubted_task_id: Option<TaskId>`.
5. **One `FeedbackStore`**, owned by the `Orchestrator`, exposed to the MCP
   `ServerState` by `Arc` clone. Clarifications register from the MCP tool;
   doubts register from an **EventBus projector sink** that listens for
   `TaskDoubted` (because `doubt_task` is synchronous and must not block on an
   async store write).
6. **Per-type response buttons** via a typed `FeedbackAction` enum (no magic
   strings): Clarification → `Answer{option|text}` / `Skip`; Doubt → `Overrule` /
   `LetVerify`.
7. **Click-to-expand is per-kind.** Doubt card → navigate chat to the task thread
   (`navigateTo('chat')` + a new `focusedFeedbackId`). Clarification card →
   expand inline (options + free-text editor) — a fresh clarification has no chat
   thread to scroll to.
8. **Reactivity rides `vox://agent-events`** (filtering the new event variants).
   The `vox://activity-appended` signal has no Rust emitter and is dead.
9. **GUI↔backend via the existing `invoke_mcp_tool`** command (as `ApprovalsView`
   does) — no new Tauri commands, no daemon-state plumbing.
10. **Delivery phased**, attention strip first.

## 3. Data model

```rust
// crate: vox-orchestrator, new module `feedback`.
use vox_orchestrator_types::agent_types::ids::TaskId; // TaskId(pub u64)
use crate::types::AgentId;                              // AgentId(pub u64)

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeedbackId(pub String);                      // "F-000001"

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind { Clarification, Doubt }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface { NeedsYou, Withheld }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FeedbackAction {                                // replaces magic strings
    Answer { option: Option<usize>, text: Option<String> },
    Skip,
    Overrule,
    LetVerify,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackResolution {
    pub action: FeedbackAction,
    pub decided_at_ms: u64,
    pub decided_by: String,                              // "gui" | "cli" | "system"
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackRequest {
    pub id: FeedbackId,
    pub kind: FeedbackKind,
    pub prompt: String,
    pub options: Vec<String>,                            // empty => free-text
    pub gates: Vec<TaskId>,                               // clarification: tasks parked; doubt: empty
    pub doubted_task_id: Option<TaskId>,                 // doubt only — target of Overrule
    pub info_gain_bits: f64,
    pub scaled_cost_ms: u64,                             // persisted at register; debited on resolve
    pub surface: Surface,
    pub session_id: Option<String>,
    pub agent_id: Option<AgentId>,
    pub created_at_ms: u64,
    pub resolution: Option<FeedbackResolution>,
}
```

**No hopper change.** `ItemState` is untouched. The "blocked" overlay is computed
GUI-side: `HopperTaskDto` gains a `task_id: u64` field (= `stable_hash(item_id.0)`,
the same hash the dispatcher uses), and a row is rendered blocked when
`task_id ∈ ⋃ open FeedbackRequest.gates`.

## 4. Backend flow

1. **Raise (clarification).** Agent calls `vox_ask_clarification(prompt, options,
   gates: Vec<u64>)` — `gates` are `TaskId`s the agent cannot proceed on.
2. **Police.** Handler builds `InterruptionSignals` (inline, mirroring
   `chat_socrates_meta.rs:388-404`, channel `ChatClarification`) and calls
   `attention_policy::evaluate_with_state(state, &signals, &att_snap)` (the
   calibrated wrapper — NOT `evaluate_interruption` raw). `surface_for(&decision)`
   → `NeedsYou | Withheld`. `scaled_cost_ms` is taken from the decision.
3. **Record (same file).** The handler MUST call `state.record_attention_event(..)`
   in the same source file (the `attention_ledger_parity` CI gate fails any file
   that evaluates an interruption without recording one).
4. **Register.** `state.orchestrator.feedback().register(..)` returns a
   `FeedbackId`. Emit `FeedbackRequested { feedback_id, kind, gates, surface }`.
   No hopper mutation; the gate is implied by the open request.
5. **Doubt projection.** `doubt_task` (sync) emits `TaskDoubted` as today. A new
   async **projector sink** (mirrors `activity/sink.rs`) listens for `TaskDoubted`
   and registers a `FeedbackRequest { kind: Doubt, gates: vec![], doubted_task_id:
   Some(task_id), surface: NeedsYou, .. }` on the shared store.
6. **Resolve.** `vox_resolve_feedback(feedback_id, action)` records the
   resolution, debits attention (`scaled_cost_ms`), emits `FeedbackResolved`. For
   `action == Overrule` on a doubt, it dispatches the existing `OVERRULE_TASK`
   path for `doubted_task_id`. Resolving a clarification clears its gates → the
   GUI overlay drops; the agent re-attempts on its next poll. On resolve, the
   store re-runs `surface_for` over withheld items (cheap promotion; no scheduler).
7. **Withheld** items are listed in a collapsible section, opt-in. No automatic
   flush beyond the on-resolve re-evaluation (§"What changed", item D).

## 5. GUI — three additions

Match the design system (`Glass`, `Pill` — has `Doubted`/`Verifying` tones,
`Icon`, `EmptyState`, `DataTable`), as `ActivitySurface` does. Every interactive
control gets an `aria-label`; the gauge uses `role="meter"` + `aria-valuemin/max/now`
(copy `AttentionBudgetMeter.tsx:30`).

1. **Attention strip (Phase 0).** Reuse the existing `AttentionBudgetMeter` +
   `AttentionBudgetSnapshot` type (`types/tauri.ts`). Add waiting-questions +
   blocked-tasks counts and place it in the top status bar. No new parser/type.
2. **Needs You surface (Phase 2).** New `needs-you` surface (registered in
   `contracts/gui/surface-registry.v1.yaml` + `App.tsx` `View` union + `childRenderer`
   switch). **Doubts pinned top** as actionable cards; clarifications below,
   sorted by `info_gain_bits` (doubts have gain 0 by construction — never sort
   them by it). Withheld collapsible section. Built from `Pill`/`Glass`/`Icon`/
   `EmptyState`. Retires the Dashboard `StreamCard` doubt buttons.
3. **Tasks surface (Phase 2).** Rows whose `task_id` is in the open-gate set
   render dimmed with a "⛔ waiting on Needs You" caption + a show/hide filter.
   Driven by the computed overlay; `lifecycle` already documents a `'blocked'`
   value in `tasksHelpers.ts`.

## 6. Events & transport

New `AgentEventKind` variants (serde `#[serde(tag="type", rename_all="snake_case")]`
→ wire tags `feedback_requested`, `feedback_resolved`): `FeedbackRequested`,
`FeedbackResolved`. (No `HopperItemBlocked/Unblocked` — there is no hopper
mutation.) Added to `is_loggable`. GUI subscribes to `vox://agent-events` and
refreshes on those two `type`s. GUI reads via `invoke_mcp_tool('vox_feedback_list')`
and `invoke_mcp_tool('vox_resolve_feedback', ...)`. New tools registered in
`contracts/mcp/tool-registry.canonical.yaml` (SSOT) + `http_gateway` allowlist.

## 7. Phasing

- **Phase 0 — Attention strip.** Reuse `AttentionBudgetMeter`; add counts + top
  placement. Counts stub to 0 until Phase 2 wires them. Ships first.
- **Phase 1 — Feedback backend.** `feedback` module (types/store/surface_policy),
  shared `FeedbackStore` on the Orchestrator + `Arc` into `ServerState`,
  `vox_ask_clarification` + `vox_resolve_feedback` (+ `vox_feedback_list`) tools,
  the doubt projector sink, 2 new events, tool-registry/parity wiring.
- **Phase 2 — Needs You + blocked overlay.** Transport via `invoke_mcp_tool`,
  `FeedbackCard`/`NeedsYouSurface` (design-system components), computed blocked
  overlay on Tasks, chat-focus for doubts, retire Dashboard doubt buttons, real
  strip counts.

## 8. What changed in rev 2 (audit corrections)

- **A — id model:** gate by `TaskId(u64)`, not `HopperItemId` (string UUID,
  one-way map). Fixes the impossible `Vec<u64>→HopperItemId` conversion and the
  doubt path (which only has `TaskId`).
- **B — no hopper mutation:** dropped `ItemState::Blocked`, dispatcher gating,
  block/unblock helpers, hopper accessors. The dispatcher is one-shot
  event-driven (`HopperItemAdmitted`) and the GUI/MCP/orchestrator hold *three
  different* hopper instances — mutating state there cannot work end-to-end.
  Blocked is a computed GUI overlay; enforcement is agent self-parking.
- **C — doubt semantics:** doubts are non-gating; they re-enqueue and the agent
  keeps working. Card actions Overrule/Let-verify; Overrule dispatches the real
  `OVERRULE_TASK` (rev 1 silently recorded a string and never overruled).
- **D — withheld:** no "checkpoint flush" (nothing implemented it; `WithheldQuestion`
  is a transient payload field). v1 = collapsible opt-in list + on-resolve
  re-evaluation. No scheduler.
- **E — single store + sync doubt:** one `FeedbackStore` Arc; doubts projected via
  an async EventBus sink, not from sync `doubt_task`.
- **F — transport/reactivity:** `invoke_mcp_tool` + `vox://agent-events` (the
  `activity-appended` reactive path is dead).
- **G — P0 not "dark":** `AttentionBudgetMeter` already renders the snapshot;
  reuse it.
- **H — CI gates:** tool-registry YAML SSOT, `attention_ledger_parity` (record in
  same file as evaluate), `derived_tool_schema!` for params, `gui-surface-registry`
  wiring gate (the `'needs-you'` literal must appear in `App.tsx`).

## 9. Non-goals (YAGNI)

- No orchestrator-enforced task pausing / dispatcher gating this iteration
  (agent self-parks; overlay is visual). A future phase can add real enforcement
  once the hopper-instance SSOT is unified.
- No `ItemState` change, no hopper schema change.
- No new interruption/attention policy logic — surface the existing policy.
- No new scheduler/daemon for "periodic" surfacing; cadence = the policy +
  on-resolve withheld re-evaluation.
- No change to the approval lifecycle or its surfaces.
