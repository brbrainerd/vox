# Attention-Aware Soft Human-in-the-Loop — Design

**Date:** 2026-06-19
**Status:** Approved design, pre-implementation
**Scope:** Surface the attention budget in the GUI; turn the existing interruption
policy + Socrates clarifications + doubt loop into a single "Needs You" feedback
inbox; introduce agent-declared gating so a pending question can block specific
tasks while the rest of the task list keeps running.

---

## 1. Problem & destination

Vox already has a sophisticated attention-budget and interruption-policy backend,
a Socrates clarification system, and an LA-Noire-inspired doubt loop. But:

- The `attention_budget` snapshot reaches the GUI on `ORCH_STATUS_EVENT` and is
  **dropped** — no component renders it (dark data).
- Socrates clarifications (`question_events`, `WithheldQuestion`) have **no GUI
  surface**.
- The doubt loop's GUI is ad-hoc hover buttons (❓/⚖️) on the Dashboard
  `StreamCard`, disconnected from the other feedback channels.
- The hopper is a **flat queue**: `ItemState` has no blocked variant, the GUI's
  `TaskRow.depends_on` is a dead stub, and `TaskStatus::Blocked` exists but is
  unused. There is no way to hold one task pending an answer while others proceed.

**Destination:** a soft human-in-the-loop where the system periodically surfaces
exactly the feedback worth the user's attention (governed by the existing
attention metrics), makes it visually obvious which tasks are waiting on answers,
and lets unaffected tasks continue. This is mostly **wiring and surfacing** of
existing backends plus one genuinely new concept: the **gating edge**.

---

## 2. Decisions (locked)

1. **Needs You** unifies **clarifications + doubts** (soft, non-blocking).
   **Approvals stay on their own surface** (`InlineApprovals`/`ApprovalsView`) —
   they hold a tool call hostage with a 300s timeout, a different lifecycle.
2. **Per-type response buttons**: Clarification → choices / ✎ answer / skip;
   Doubt → ⚖️ overrule / ✎ answer the conflict / let-it-verify.
3. **Click-to-expand-in-chat**: a Needs-You card is a summary; clicking it scrolls
   the chat surface to the full thread for context.
4. **Gating edges are agent-declared** (option A). The agent that is stuck names
   the task(s) it cannot proceed on, in the same call that raises the question.
   This reuses the agent-initiated pattern the doubt system already proves out.
   Orchestrator-inferred gating (file-affinity overlap) is a possible later assist.
5. **Storage:** one unified `FeedbackRequest` type with the gating edge stored on
   the hopper item (`ItemState::Blocked { gated_by }`), rather than reusing
   `question_events` + a separate `gating_edges` table. Fewer split-brains.
6. **Delivery is phased**, attention strip first (lowest risk, pure surfacing).

---

## 3. Data model

```rust
// New unified feedback request. Clarifications and doubts both project into this.
struct FeedbackRequest {
    id: FeedbackId,
    kind: FeedbackKind,            // Clarification | Doubt
    prompt: String,
    options: Vec<String>,          // empty => free-text answer expected
    gates: Vec<HopperItemId>,      // tasks blocked until resolved (may be empty)
    info_gain_bits: f64,           // from evaluate_interruption()
    surface: Surface,              // NeedsYou | Withheld (policy decides)
    session_id: Option<String>,
    agent_id: Option<AgentId>,
    created_at_ms: u64,
    resolution: Option<FeedbackResolution>,
}

enum FeedbackKind { Clarification, Doubt }
enum Surface { NeedsYou, Withheld }

struct FeedbackResolution {
    chosen_option: Option<usize>,  // index into options, if a button was used
    free_text: Option<String>,
    decided_at_ms: u64,
    decided_by: String,            // "gui" | "cli" | agent id for auto-resolve
}
```

**Hopper change** — extend the existing `ItemState`
(`crates/vox-orchestrator/src/hopper/types.rs`):

```rust
enum ItemState {
    Inbox,
    Assigned { agent_id: String },
    Blocked { gated_by: Vec<FeedbackId> },   // NEW
    Done,
    Overridden,
    Cancelled,
}
```

A blocked item is not dispatched. When every `FeedbackId` in `gated_by` resolves,
the item returns to `Inbox` and is re-admitted.

**Doubt projection:** `TaskStatus::Doubted(reason)` projects into a
`FeedbackRequest { kind: Doubt, gates: [doubted_task], options: [] }`. The doubt
loop's existing semantics (re-enqueue as Verifier, `OperatingMode::Verification`)
are unchanged; we are only adding the unified surface + buttons.

---

## 4. Backend flow

1. **Raise.** Agent calls a new MCP tool
   `vox_ask_clarification(prompt, options, gates)`. (Doubts continue to use
   `vox_doubt_task`; both now create a `FeedbackRequest`.)
2. **Police.** The request runs through the **existing** `evaluate_interruption()`.
   Its verdict (`InterruptNow` / `DeferUntilCheckpoint` / `BatchWithExistingPrompt`
   / `ProceedAutonomously` / `RequireHumanBeforeContinue`) sets
   `surface = NeedsYou | Withheld`. **No new policy logic** — this only surfaces
   what the policy already computes, and is the SSOT for cadence/periodicity.
3. **Gate.** Each `gates` hopper item flips to `Blocked { gated_by }`; all other
   items keep dispatching. Emit `HopperItemBlocked`.
4. **Resolve.** User answers via button or inline chat → `FeedbackResolution`
   recorded → gated items return to `Inbox` and re-admit → emit
   `HopperItemUnblocked` + `FeedbackResolved`. Attention cost is debited through
   the existing `BudgetManager::record_attention()`.
5. **Withheld opt-in.** `Withheld` requests are not pushed; they are listed in a
   collapsible section the user can pull from (reusing the existing
   `WithheldQuestion` exposure).

---

## 5. GUI — three additions, all on existing infrastructure

All match the established visual language (dark zinc, `Glass` panels,
emerald/amber/rose accents, tiny uppercase labels) seen in `ActivitySurface`.

1. **Attention strip.** Renders the dark `attention_budget` snapshot already on
   `ORCH_STATUS_EVENT`: focus depth pill, spent/budget gauge, and counts of
   waiting questions + blocked tasks. Lives in the top status bar.
   *(Phase 0 — independent, ships first.)*
2. **Needs You surface.** New entry in `surfaceRegistry`. Unified
   clarifications + doubts, left-edge color-coded by kind, per-type buttons, a
   Withheld section, and card-click → scroll chat to the thread. Subscribes to
   `FeedbackRequested/Resolved` the way `ActivitySurface` subscribes to
   `activity-appended`. The Dashboard `StreamCard` ❓/⚖️ buttons are retired in
   favor of this surface.
3. **Tasks surface.** Gated items render dimmed with a "⛔ blocked on Q-NN"
   caption and a live filter to show/hide blocked tasks. Maps the new
   `ItemState::Blocked` through the existing `HopperTaskDto`.

---

## 6. Events & transport

New `AgentEventKind` variants on the existing EventBus → Tauri bridge:
`FeedbackRequested`, `FeedbackResolved`, `HopperItemBlocked`, `HopperItemUnblocked`.
The Needs-You surface and the Tasks surface refresh reactively off these, exactly
like the activity log. No new transport, no new scheduler — surfacing cadence is
owned by `evaluate_interruption()`.

---

## 7. Phasing (delivery order)

- **Phase 0 — Attention strip.** Pure surfacing of existing dark data. No backend
  change. Independent; ship first.
- **Phase 1 — Feedback model + gating backend.** `FeedbackRequest`,
  `ItemState::Blocked`, `vox_ask_clarification`, doubt projection, the four new
  events, block/unblock + re-admit logic, attention debit on resolve.
- **Phase 2 — Needs You surface + Tasks blocked states.** The unified inbox,
  click-to-chat, per-type buttons, dimmed gated tasks; retire the Dashboard
  doubt buttons.

Each phase is independently testable and leaves the system in a working state.

---

## 8. Explicit non-goals (YAGNI)

- No orchestrator-inferred gating (file-affinity) in this iteration — agent-declared only.
- No change to approval lifecycle or its surfaces.
- No new attention/interruption policy logic — we surface the existing policy.
- No drag-to-gate manual GUI wiring.
- No new scheduler or polling daemon for "periodic" surfacing.
```
