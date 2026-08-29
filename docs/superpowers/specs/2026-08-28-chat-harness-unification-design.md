---
title: "Chat Harness Unification — One Dispatch, Visible Orchestration"
description: "Collapse the GUI's two chat dispatch paths into one contract, make model selection per-request and explainable, close the skill-activation loop, and correlate chat turns with the agents they spawn."
category: "architecture"
status: "design"
date: 2026-08-28
---

# Chat Harness Unification — One Dispatch, Visible Orchestration

## 1. Problem

The Axis chat composer (`Loquela.tsx`) presents eight controls: skill pin, model
picker, runtime tier, structured intent, `@`-chips, grounding toggle, slash menu,
and a Quick-chat/Background-task mode switch. That last switch forks into two
different backends that accept different payloads, and the fork is implemented as
an early `return` in the middle of the frontend submit handler
([`App.tsx:965`](../../../crates/vox-gui/ui/src/App.tsx)).

Everything between that early return and the end of the function — context-file
resolution, priority, mode, tier, `model_override`, clutch, risk, dry-run,
task-category — is therefore unreachable for a quick chat.

| Composer control | `submit_orchestrator_task` | `chat_send_message` |
|---|---|---|
| `model_override` (ChatModelPicker) | forwarded | **dropped** |
| `model_hint` / tier ("Run on") | forwarded | **dropped** |
| `@`-chips → `context_files` | forwarded | **dropped** |
| Intent, priority, clutch, risk, dry-run | forwarded | **dropped** |
| `active_skill` | forwarded | forwarded |
| grounding check | forwarded | forwarded |

The dropped fields are not missing from the backend. `ChatMessageParams`
(`chat_tools/params.rs`) already accepts `context_files`, `temperature`, `top_p`,
and `skill`. The GUI simply never sends them on this path, because
[`ChatSendInput`](../../../crates/vox-gui/src/commands/chat.rs) declares four
fields.

Four further defects compound it.

**Three uncoordinated notions of "the model."** `set_active_model` writes the
`VOX_MODEL` env var plus an `active_model` user preference; the daemon holds a
process-global `mcp_chat_model_override: RwLock<Option<String>>`; and the
auto-selector (`vox-orchestrator::models::{registry,select,scoring,routing_table}`)
picks per-request. `ChatModelPicker.apply()` calls neither of the first two — it
sets React state only. The picker's label changes and nothing else does.

**Selection rationale is one line, and sometimes absent.** `choice.rationale`
reaches the GUI as `selection_reason` and renders as a `ModelBadge` tooltip. Two
fallback paths in `chat/message.rs` set it to `None`, so the badge cannot
distinguish "auto-routed deliberately" from "silently fell back." The rich
surfaces — `explain_model_selection`, `get_model_scoreboard`,
`get_routing_intentions`, `nudge_routing_intention` — exist in
`commands/models.rs` and are reachable only from the Models surface, never from
the point of decision.

**`/plan` does not plan.** It is a member of `INTERNAL_MODE_SLASHES`, so it flips
a composer-local mode string. It never calls `chat_tools/plan.rs`. Plans appear
in `PlanPanel` only when the background pipeline synthesised one; the only GUI
write-back is `update_plan_node`, which edits a single node's description.

**Delegation is invisible from the turn that caused it.** The primitives all
exist and are reachable from a chat turn: `vox_spawn_agent`, `vox_submit_task`,
and `vox_task_status` are in `ORCHESTRATOR_TOOLS` and pass
`select_tools_for_turn`. `list_subagent_tree` and the `vox://agent-events` stream
already feed `SubAgentTree`, `SubAgentGraph`, and `AgentFlow`. What is missing is
the join: a synchronous chat reply carries no `task_id`, `SpawnAgentParams`
takes a `parent_agent_id` but no chat session, and `ChatAgentEventRow` renders
exactly two kinds (`doubted`, `token_group`). A chat turn can spawn a fleet and
the transcript shows a paragraph of prose. The composer's Interrupt/Resume then
guesses which agent it means, by looking for exactly one paused agent
fleet-wide — a limitation the code itself documents at `App.tsx:1405-1424`.

## 2. Goals

1. One dispatch contract. Every composer control reaches the backend regardless
   of execution mode; adding a control is a one-place change.
2. Per-request model override, replacing the process-global `RwLock` for
   GUI-originated turns, with the auto-selector still the default.
3. Selection explainability at the point of decision, including an explicit
   `fallback` classification instead of `None`.
4. `/plan` produces a real plan, reviewable and approvable before dispatch.
5. Autonomous skill activation is visible in the transcript.
6. Chat turns are correlated with the agents and tasks they cause, in both
   directions, replacing the single-paused-agent guess.
7. Streaming on the synchronous path.

## 3. Non-goals

- No change to the auto-selector's scoring model, registry, or OpenRouter egress.
  Those work; this design makes their output legible and controllable.
- No change to the deliberate dual persistence (GUI display conversation +
  workspace context conversation). Its rationale is documented in
  `persist_assistant_reply`; §9 addresses only the resulting user-visible
  confusion, not the storage split.
- No new orchestration engine. Delegation uses existing tools.
- Not a redesign of Loquela's visual layout.

## 4. Architecture

### 4.1 One command, one payload

Replace the frontend fork with a single Tauri command that owns the routing
decision in Rust.

```
Loquela.send()
  └─ buildChatTurn()            ← single payload builder (TS)
       └─ invoke('chat_turn')   ← single IPC command (Rust)
            ├─ execution: 'sync'       → daemon TOOL_CALL vox_chat_message
            └─ execution: 'background' → daemon SUBMIT_TASK
```

`chat_turn` supersedes `chat_send_message` and the composer's use of
`submit_orchestrator_task`. `submit_orchestrator_task` itself remains — the Tasks
surface, hopper, and `/spawn` still call it directly — but the composer no longer
chooses between two IPC commands.

The routing decision moves to Rust for one reason: it is the only place where
both branches can be held to the same input struct by the compiler. A frontend
fork can silently drop a field; a Rust `match` over one struct cannot.

### 4.2 `ChatTurnInput`

```rust
// crates/vox-gui/src/commands/chat_turn.rs
#[derive(Debug, Deserialize)]
pub struct ChatTurnInput {
    pub session_id: String,
    pub content: String,
    /// 'sync' → synchronous reply; 'background' → autonomous task.
    #[serde(default)]
    pub execution: Execution,
    // --- routing ---
    pub model_override: Option<String>,
    pub model_hint: Option<String>,      // "Run on" tier
    pub clutch: Option<String>,
    pub risk: Option<String>,
    // --- context ---
    #[serde(default)]
    pub context_files: Vec<String>,      // resolved @-chips
    pub intent: Option<serde_json::Value>,
    // --- behaviour ---
    pub active_skill: Option<String>,
    pub mode: Option<String>,            // plan | act | verify
    pub grounding_check_enabled: Option<bool>,
    pub priority: Option<String>,
    pub dry_run: Option<bool>,
    pub allow_duplicate: Option<bool>,
}
```

Field parity with `SubmitTaskInput` is enforced by a test (§11), so a field added
to one and not the other fails CI rather than silently disappearing on one path.

### 4.3 `ChatTurnDto`

```rust
pub struct ChatTurnDto {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    /// Present on BOTH paths now. Sync turns that spawned work carry the
    /// spawned task id; background turns carry the enqueued task id.
    pub task_id: Option<String>,
    pub model_id: Option<String>,
    pub latency_ms: Option<u64>,
    pub selection: Option<SelectionDto>,   // §5
    pub grounding_flagged: Option<bool>,
    pub events: Vec<TurnEventDto>,         // §7, §8
    pub duplicate_of: Option<String>,
}
```

Returning `task_id` on the synchronous path is what makes §8's correlation
possible; it is currently unset by deliberate omission, documented in `App.tsx`
as "no real task_id for a synchronous chat reply."

## 5. Model selection

### 5.1 Per-request override

Add `model_override: Option<String>` to `ChatMessageParams`. In
`try_run_agent_turn`, the request-scoped value takes precedence over the
process-global lock:

```rust
let pref = params.model_override.clone()
    .or_else(|| poison_rw_read(state.mcp_chat_model_override.read(), …).ok().flatten());
```

The global stays for non-GUI MCP clients (editor extensions) that have no
per-request channel. `set_active_model`'s env-var write is left alone but is no
longer the mechanism the chat picker appears to use — because the picker now
sends `model_override` on the turn instead of pretending to set a global.

`ChatModelPicker` gains a persisted default (`vox_chat_model.v1` in
localStorage) so a pick survives a reload, matching what users already expect
from the label.

### 5.2 Selection is a struct, not a string

```rust
pub struct SelectionDto {
    pub model_id: String,
    pub source: SelectionSource,   // UserOverride | AutoRouted | Fallback | Global
    pub rationale: Option<String>,
    pub candidates: Vec<CandidateDto>,   // id, score, cost_tier, provider, rejected_because
    pub context_fill_ratio: Option<f32>,
}
```

`SelectionSource::Fallback` is the fix for the silent-`None` paths at
`message.rs:604` and `:686`: those return `Fallback` with a rationale naming what
failed, so the badge can render "fell back from X" rather than nothing.

`candidates` is populated from the same `decide()` call the selector already
makes — it is discarded today. Capped at the top 5 by score to bound payload
size.

### 5.3 Explain-in-place

`ModelBadge` becomes a popover, not a tooltip: source, rationale, ranked
candidates with scores and cost tiers, and an "override for this session"
control that writes the same `model_override` the picker does. This is the
existing `explain_model_selection` data rendered where the decision is visible,
rather than in the Models surface.

## 6. Planning

`/plan` moves out of `INTERNAL_MODE_SLASHES` and becomes a real dispatch:
`chat_turn` with `mode: 'plan'`, routed to the existing `chat_tools/plan.rs`
tool. The reply is a plan DAG, not prose.

`PlanPanel` gains write-back beyond the current description edit:

| Action | Backend |
|---|---|
| Edit node description | `update_plan_node` (exists) |
| Add node | `upsert_plan_node` (exists, new command wrapper) |
| Delete node | new `delete_plan_node` |
| Reorder / re-parent | `upsert_plan_node` with edited `dependencies_json` |
| **Approve plan → dispatch** | new `approve_plan`, enqueues runnable nodes |
| Reject plan | new `discard_plan`, marks version abandoned |

The approval gate is the substantive addition: today a synthesised plan is
dispatched by the scheduler with no user checkpoint, and the only reason node
edits work at all is that `enqueue_runnable_plan_nodes` re-reads DB state before
each dispatch — a race the user is expected to win by typing fast. `approve_plan`
replaces that race with a gate: nodes in an unapproved plan version are not
runnable.

Edits are rejected for any node not in `status = 'pending'`, returning a
structured error the panel renders inline rather than a toast.

## 7. Skills

The backend's three-tier disclosure is correct and unchanged. Two additions
close the loop:

1. **Activation is an event.** When the agent loop dispatches `vox_skill_use`,
   emit a `TurnEventDto { kind: 'skill_activated', skill_id, reason }`. The
   transcript renders it as a chip above the reply. The user can currently pin a
   skill but cannot see which one the model chose on its own.
2. **Correction is one click.** The chip offers "pin this" (make it explicit for
   the session) and "not this one" (re-run the turn with the skill excluded via
   a `skill_exclusions` field on `ChatMessageParams`). The exclusion is
   session-scoped and not persisted.

`ChatAgentEventRow` grows from two kinds to a discriminated union over
`TurnEventDto['kind']`, with an explicit exhaustiveness check so a new event kind
is a type error rather than a silently unrendered row.

## 8. Delegation and correlation

### 8.1 Session lineage

Add `chat_session_id: Option<String>` to `SpawnAgentParams` and to
`vox_submit_task`'s params. The agent loop injects the current turn's session id
automatically when the model calls either tool, so the model cannot forget it and
cannot forge a different session's id.

`SubagentTreeNode` gains `chat_session_id` and `origin_turn_id`. This is the join
that lets both directions work:

- **Turn → agents.** The transcript renders a `delegation` event row per spawn,
  with live status from `vox://agent-events` filtered to that turn.
- **Agents → turn.** `SubAgentTree` nodes deep-link back to the originating chat
  turn.

### 8.2 Real per-session agent identity

With `chat_session_id` on the topology, the composer's Interrupt/Resume stops
guessing. `currentPausedAgent` is derived by filtering the subagent tree to the
active session, which is correct with any number of paused agents fleet-wide and
removes the documented wrong-agent hazard at `App.tsx:1405-1424`.

### 8.3 Delegation is approvable

A spawn from inside a chat turn is a HITL-gated action, routed through the
existing `PendingApprovals` registry that `list_mc_approvals` already reads. It
renders inline in the transcript via the existing `InlineApprovals` component
rather than only in the Approvals surface, so the user approves a fan-out where
they can see what asked for it. Gating is governed by permission mode: `Plan`
and `Ask` gate; a permissive mode auto-approves, consistent with the existing
dispatch HITL gate.

`/spawn` stops being a client-side shim that submits a hardcoded description. It
becomes `chat_turn { execution: 'background', mode: 'act' }` carrying the user's
actual composer text and chips, with the session lineage set — which also fixes
the current behaviour where `/spawn` discards whatever the user typed.

## 9. Streaming and transcript honesty

The synchronous path becomes streaming: `chat_turn` emits
`vox://chat-turn/{session_id}` deltas via `llm_stream` (already in the facade,
already used by the task path's token groups) and resolves with the final
`ChatTurnDto`. Token-group rows are reused unchanged.

Separately, the transcript gains a small explicit marker for the display/context
split: messages that exist in the GUI display conversation but not in the
workspace context conversation are rendered muted with a "not in model context"
affordance. The dual write stays as designed; the user simply stops assuming the
visible transcript is the model's context.

## 10. Error handling

`chat_turn` returns structured errors, not prefixed strings:

```rust
pub enum ChatTurnError {
    BudgetExceeded { spent: f64, cap: f64 },
    RateLimited { provider: String, retry_after_s: Option<u64> },
    ContextExceeded { model: String, tokens: u64, limit: u64 },
    ModelUnavailable { model: String, reason: String },
    Duplicate { of: String },
    Backend(String),
}
```

The existing `CONTEXT_EXCEEDED_PREFIX` / `RATE_LIMITED_PREFIX` string markers
stay in `vox-actor-runtime` (other callers depend on them); `chat_turn` parses
them once at its boundary and returns the typed variant. The frontend's
`dispatchErrorToast` string-sniffing (`isBudgetExceededError`,
`isRateLimitedError`) is replaced by a match, so a message-wording change
upstream can no longer silently degrade a specific toast to the generic one.

`ModelUnavailable` is new and is what makes §5's picker honest: picking a model
whose provider is down currently produces a generic dispatch failure.

## 11. Testing

Rust:
- `chat_turn` field-parity test: `ChatTurnInput` and `SubmitTaskInput` routing
  fields must match; adding one without the other fails.
- Per-request `model_override` beats the process-global lock; absent override
  falls through to it; absent both, auto-selector runs.
- `SelectionSource::Fallback` is produced on both former `None` paths.
- `approve_plan` gates dispatch: unapproved nodes are not runnable.
- Plan-node edit on a dispatched node returns a structured rejection.
- `chat_session_id` propagates spawn → topology → `list_subagent_tree`.
- Each `ChatTurnError` variant round-trips from its upstream string marker.

TypeScript (vitest):
- `buildChatTurn` includes every composer control (snapshot over the key set —
  the direct regression test for the dropped-field table).
- `ChatAgentEventRow` exhaustiveness over `TurnEventDto['kind']`.
- `ModelBadge` popover renders source, rationale, candidates; renders a
  fallback-specific message for `Fallback`.
- `currentPausedAgent` resolves correctly with 2+ paused agents fleet-wide.

Playwright (`ui/e2e/chat-harness.spec.ts`):
- Pick a model → send a quick chat → badge reports that model as `UserOverride`.
  This test fails on `main` today and is the acceptance criterion for §4–5.
- `/plan` → plan renders → edit a node → approve → nodes dispatch.
- A delegating turn renders delegation rows; Interrupt targets the right agent.

## 12. Phasing

Each phase is independently shippable and independently valuable.

| Phase | Content | Unblocks |
|---|---|---|
| A | `chat_turn` command, `buildChatTurn`, field parity test, `model_override` on `ChatMessageParams` | The dropped-field table; the picker starts working |
| B | `SelectionDto`, `SelectionSource`, `ModelBadge` popover, `ModelUnavailable` | Selection explainability |
| C | `ChatTurnError`, typed frontend match | Removes string-sniffing |
| D | `chat_session_id` lineage, delegation event rows, real `currentPausedAgent`, `/spawn` rewrite | Correlation, correct Interrupt/Resume |
| E | Skill-activation events, pin/exclude controls | Skill loop closure |
| F | `/plan` real dispatch, PlanPanel write-back, `approve_plan` gate | Planning |
| G | Streaming sync path, context-membership marker | Parity with task path |

Phase A is the load-bearing one: D, E, and F all ride on the single payload and
the returned `task_id`.

## 13. Risks

**Tool-schema drift.** `ChatMessageParams` and `SpawnAgentParams` gain fields.
Their MCP tool schemas are derived from the structs (`derived_tool_schema!` in
`input_schemas.rs`), so the schemas regenerate rather than needing hand-editing —
but any generated artifact that snapshots them is SSOT-gated by
`vox ci ssot-drift` and must be regenerated in the same change. No new daemon RPC
method is introduced (`contracts/orchestration/orch-daemon-rpc-methods.schema.json`
enumerates method names only, and Phase F's plan commands are Tauri commands, not
daemon methods). All new fields are `#[serde(default)]`, so older clients (the
VS Code extension) are unaffected. `orch.subagent_tree`'s response gains two
optional fields, which existing consumers ignore.

**Recursion.** `agent_loop` already excludes `vox_chat_*` from a turn's tool set
to bound re-entrancy. `vox_spawn_agent` and `vox_submit_task` create *new*
top-level work rather than re-entering `run_agent_turn`, so they do not extend
that cycle — but §8's auto-injected session id must not be interpreted by the
spawned agent as permission to dispatch back into the same session. The
`chat_session_id` field is lineage metadata only, never a dispatch target.

**Tool-cap pressure.** `DEFAULT_MAX_TOOLS` is 40 and the delegation tools are
already in the registry, so §8 adds no new cap pressure. §7's `skill_exclusions`
is a params field, not a tool.

**Payload size.** `candidates` capped at 5; `events` is per-turn, not
cumulative.
