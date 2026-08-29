---
title: "Chat Harness Unification — One Dispatch, Visible Orchestration, Measured Models"
description: "Collapse the GUI's two chat dispatch paths into one contract, make model selection per-request and honestly explained, close the skill-activation loop, build the missing backends delegation and planning depend on, and replace a constant labelled quality with measured Efficiency, Responsiveness, and Intelligence."
category: "architecture"
status: "design"
date: 2026-08-28
---

# Chat Harness Unification — One Dispatch, Visible Orchestration, Measured Models

> **Provenance.** Every claim in §1 and §2 was fact-checked against the code by
> an independent audit track. Claims that survived are marked with their
> evidence; claims from the first draft of this document that did **not**
> survive are recorded in §3 rather than deleted, because two of them were
> load-bearing and their failure changes the shape of the work.

## 1. What is actually broken

### 1.1 The composer's controls do not reach the backend

The composer presents eight controls. Seven live in `Loquela.tsx`; the model
picker is rendered by `App.tsx` and injected through Loquela's `trailingSlot`,
which is why its value has to be spliced in at the call site rather than
flowing through `Loquela.send()`'s payload — §4 has to absorb that asymmetry.

The Quick-chat/Background-task switch forks into two backends via an early
`return` spanning `App.tsx:962-1027`. Everything after that return —
context-file resolution, priority, mode, tier, `model_override`, clutch, risk,
dry-run — is unreachable for a quick chat.

| Composer control | `submit_orchestrator_task` | `chat_send_message` | Verified |
|---|---|---|---|
| `model_override` | forwarded (`App.tsx:1043`) | **dropped** | ✅ |
| tier → `model_hint` | forwarded (`App.tsx:1042`) | **dropped** | ✅ |
| `@`-chips → files | forwarded (`App.tsx:1039`) | **dropped** | ✅ |
| priority, clutch, risk, dry-run | forwarded | **dropped** | ✅ |
| `active_skill`, grounding | forwarded | forwarded | ✅ |
| structured intent | *neither, as a field* | *neither, as a field* | ✅ |

Two corrections to the first draft. **Intent is not dropped** — `Loquela.send()`
folds it into `description` via `composeDescription(text, intent)`, so it
reaches the model on both paths; what is dropped is the *derived* `priority`.
And the tier's destination is not what the first draft assumed: the top-level
`model_hint` the background path sends is **never read** by the daemon's
SUBMIT_TASK handler (`orch_daemon/mod.rs:468-597`). Only the
`enqueue_hints.model_preference` copy has any effect. `model_hint` is dead
today on both paths.

`ChatSendInput` declares four fields (`chat.rs:384-394`). The backend is not the
narrow end: `ChatMessageParams` already accepts `context_files`, `temperature`,
`top_p`, and `skill` (`params.rs:37,79,82,87`).

### 1.2 Model selection is per-process, and its rationale is sometimes absent

`mcp_chat_model_override` is a process-global `Arc<PrRwLock<Option<String>>>`
(`server_state.rs:106`) with exactly one production writer — the
`vox_set_active_model` MCP tool (`dispatch.rs:977`). The GUI never touches it:
`transport.ts:466` shadows `callTool('vox_set_active_model')` onto the
GUI-local `set_active_model` command, which writes a `VOX_MODEL` env var and an
`active_model` user preference instead.

`ChatModelPicker` deliberately calls neither — its own doc comment records
lifting to `model_override` as a resolved decision. It is not a decorative
control; it is inert *only* on the sync path, which is §1.1's defect, not a
separate one.

Selection rationale reaches the GUI as `selection_reason` and renders in a
click-toggled `ModelBadge` popover (not a hover tooltip). It is `None` at
**three** sites, not the two the first draft claimed, and they are not the same
kind of thing:

| Site | What it is | Truthful rationale |
|---|---|---|
| `message.rs:605` | **Not a fallback.** The primary cognitive-profile path; `mcp_infer_completion` sets `selection_rationale: None`. | "cognitive profile `<p>` → complexity `<n>`; rationale not surfaced" |
| `message.rs:633` | A real silent fallback the first draft missed: profile model resolution errored → `call_llm`. | "Fallback: cognitive-profile resolution failed (`<err>`)" |
| `message.rs:687` | `try_run_agent_turn` returned `None` → `call_llm`. | "Fallback: attachment present, or provider shape unmapped" |

Worse than a missing rationale: **an unknown `model_override` is silently
ignored.** `resolve_mcp_chat_model_sync_inner:333-352` falls through to
auto-selection when `registry.get(id)` is `None` — no error, no signal — and
`try_run_agent_turn` further swallows resolution errors with
`Err(_) => return None` (`message.rs:115-117`). A badge that classifies from
the *request* would read "Your pick" over a model the user did not get.

The rich surfaces are not where the first draft said. `get_routing_intentions`
and `nudge_routing_intention` are on **Matrix**; `get_model_scoreboard` is on
**Runs**; `explain_model_selection` has **no UI caller at all**. The
conclusion — never at the point of decision — holds.

### 1.3 `/plan` has no backend

`/plan` is a member of `INTERNAL_MODE_SLASHES`, so it flips a composer-local
mode string that is never rendered and never read. `ChatMessageParams` has no
`mode` field, and nothing in `message.rs` branches on one. Adding the field
alone changes nothing.

The real mechanism is a different tool: **`vox_plan`**, with
`PlanParams { goal, scope_files, write_to_disk, max_tasks, session_id,
plan_depth, loop_mode }` (`params.rs:193-253`) dispatched at `dispatch.rs:1027`
→ `chat_tools::plan_goal`. It has no `prompt` field — the composer text is
`goal` — and its published schema is `additionalProperties: false`, so an
unexpected key is a hard client-side reject.

`insert_plan_node` already exists and is registered (`main.rs:235`,
`PlanPanel.tsx:101`). The first draft's claim that `update_plan_node` is the
only write-back was false.

### 1.4 Delegation is unreachable, unstored, and unreadable

Three independent failures, any one of which is fatal to correlation:

1. **The tools are never offered.** `select_tools_for_turn` ends in
   `.take(40)` over registry order, which is alphabetical
   (`tool_selection.rs:38,154`; `vox-mcp-registry/build.rs:78-88`). After the
   `ai`/`app` lane filter there are 188 candidates and the cut lands at
   `vox_compiler::ast_inspect`. `vox_spawn_agent` is index **166**,
   `vox_submit_task` **168**, `vox_task_status` **170**. The module's own doc
   concedes the truncation is "deliberately a plain truncation, not a relevance
   ranking." A pinned skill removes them a second time, via
   `check_skill_tool_permission`.
2. **Lineage has nowhere durable to live.** `parent_agent_id` /
   `delegation_reason` are on `SpawnAgentParams`, persisted only as an
   in-memory `AgentDelegationBinding` in `Orchestrator::agent_delegations`
   (`spawn.rs:88-96`). There is no DB row; the correlation dies with the daemon.
3. **The read path has no server.** `orch.subagent_tree` is declared in
   `protocol.rs:111` and in the RPC contract, and called by
   `mission_control.rs:53` — but **no handler exists**. The daemon falls through
   to `unknown method`, so `list_subagent_tree` always yields an empty tree and
   `SubAgentTree`/`SubAgentGraph` render nothing. The source data does exist
   (`agent_topology_snapshot` builds `delegation_edges` from
   `agent_delegations`, `accessors.rs:345-394`); only the arm is missing.

The composer's Interrupt/Resume consequently still guesses: exactly one paused
agent fleet-wide, ambiguous with two, as `App.tsx:1405-1424` documents about
itself.

### 1.5 There is no streaming on the sync path, and no second channel to add

`run_sync` is one JSON-RPC request/response; `try_run_agent_turn` dispatches
through `llm_chat`, which is non-streaming. `TokenStreamed` is emitted from
exactly one place — `runtime.rs:359`, the background-task phase runner — and
its own doc says nowhere else in `vox-orchestrator` emits it.

### 1.6 The GUI displays a constant labelled "quality"

`model_scoreboard.quality_score` is `success ? 1.0 : 0.0` per call
(`chat.rs:313`), or `COALESCE(AVG(rating/5.0), 1.0)` from `llm_feedback`
(`ops_scientia.rs:177`) — a table with **zero rows**, whose only inserter
(`FeedbackCollector`) is constructed nowhere outside its own doctest. So the
Runs surface renders ≈1.0 for every model, permanently.

Three more measurement defects that would corrupt anything built on top:

- `p50_latency_ms` is `AVG(latency_ms)` and `p99_latency_ms` is `MAX(...)`
  (`ops_scientia.rs:174-175`). They are not percentiles.
- `infer_with_retry` records `latency_ms: 0` on its success path
  (`chat.rs:397-408`), dragging every average toward zero.
- `record_bandit_task_outcome` credits `model_override.or(model_preference)` —
  the **requested** model, not the one that served
  (`success/mod.rs:468-471`). Cascade fallbacks are miscredited. It is also
  in-memory only and flag-gated off by default.

And both existing "quality" heuristics fail on the specific hazard this design
must avoid, in opposite directions:

- `assess_reply_confidence` measures hedging-vocabulary density. An empty or
  purely procedural reply scores `confidence: 1.0, flagged: false`
  (`grounding.rs:163-170`). A terse, confident, incomplete answer is perfect.
- `vox_eval::quality_proxy_score` is literally response-length bands
  (`<10→0.2 … else 1.0`, `vox-eval/src/lib.rs:52-65`). It rewards verbosity.

`ModelMetric` (`llm/types.rs:258-296`) is dead code whose doc references a
table that does not exist.

## 2. What already exists and must not be rebuilt

| Asset | Location | State |
|---|---|---|
| `approve_all_blocked_plan_nodes` (`'blocked_on_approval' → 'pending'`) | `ops_planning.rs:349-368` | **Zero callers**; nothing writes the status. The approval gate, already built. |
| `insert_plan_node` | `main.rs:235`, `PlanPanel.tsx:101` | Shipped. Do not add `add_plan_node`. |
| `harness_eval_task_result ⋈ model_selection_event` on `(run_id, task_id)` | `schema/domains/harness_eval.rs:28-57` | The correct per-model three-axis join. Offline-only; needs a live writer. |
| `scoreboard_feedback_boost` | `scoring.rs:60-108` | A real outcome→selection loop, weighted from `model-routing.v1.yaml`, refreshed on a 5-minute ticker. Its `quality_score` input is the dishonest part, not the loop. |
| `trust_observations` / `trust_rollups` | `agents.rs:368-401` | Model-aware, dimension-keyed, EWMA-ready. Entirely unwritten. |
| `source_task_id` on `SpawnAgentParams` | `params.rs:883` | Lineage field that already exists. |
| `TokenStreamed` + `vox://agent-events` + `taskToSession` buffering | `events.rs:401`, `sessionChatStore.ts:126-148` | Correlation, 30 s replay buffer, and token-group rendering, already solved and tested. |
| `unwrapLlmErrorPrefix`, `BUDGET_EXCEEDED_PATTERN` | `backendGuard.ts:72-93` | The only strings that match production errors. |
| `exclude_name_prefixes` escape hatch on `TurnContext` | `tool_selection.rs:75-80` | The precedent for an include-list. |

## 3. Claims from the first draft that did not survive

Recorded rather than deleted, because two changed the shape of the work.

| First-draft claim | Verdict |
|---|---|
| "The delegation primitives are reachable from a chat turn; only the join is missing." | **False.** Capped out at 40 (§1.4). This was the load-bearing error — Phase D would have stamped lineage onto calls that can never occur. |
| "`list_subagent_tree` and `vox://agent-events` already feed SubAgentTree/Graph — good observability." | **False for the tree.** No handler (§1.4). |
| "`mode: 'plan'` routes to `chat_tools/plan.rs`." | **False.** `vox_plan` is a separate tool (§1.3). |
| "The only GUI plan write-back is `update_plan_node`." | **False.** `insert_plan_node` ships. |
| "Intent is forwarded on the background path and dropped on chat." | **False.** Folded into `description` on both. |
| "Three uncoordinated notions of the model." | **Two plus a deliberate design decision** (§1.2). |
| "`ChatAgentEventRow` handles only `doubted` and `token_group`." | **False.** It renders any agent event, carrying three live HITL controls — `approve_orchestrator_task_plan`, `skip_orchestrator_verify`, `force_orchestrator_verify`. It has no caller, so this is working code with no render site. |
| "The prefix markers stay because other Rust callers depend on them." | **False.** Zero Rust consumers outside `vox-actor-runtime`. |
| "A field-parity test makes dropping a control a CI failure." | **False.** The proposed test compared a hand-maintained constant against itself. |

## 4. Goals

1. One dispatch contract; every composer control reaches the backend on both
   execution modes, with an anti-drift guard that actually reflects both
   structs.
2. Per-request model override, and a selection story that is honest about what
   was **resolved**, not what was requested.
3. `/plan` produces a real plan, gated on approval before dispatch, without
   changing semantics for non-GUI callers.
4. Autonomous skill activation is visible and correctable, reported from tool
   **results**.
5. Delegation becomes possible (tool reachability), durable (stored lineage),
   and readable (an RPC handler) — in that order.
6. Streaming on the sync path, over the existing event channel.
7. Per-model Efficiency, Responsiveness, and Intelligence that cannot be won by
   answering tersely and incompletely.
8. Every claim verified in the running GUI before it is called done.

## 5. Non-goals

- No change to the scorer's weights, the registry, or OpenRouter egress.
- No new orchestration engine.
- No redesign of Loquela's layout.
- Not removing the display/context persistence split (§9 addresses only the
  user-visible confusion it causes).
- No LLM-judge on the per-turn hot path.

## 6. Architecture — one dispatch, two lifecycles

The correction that matters most from review: **collapse the dispatch, not the
lifecycle.**

```
Loquela.send()  ──► buildChatTurn()  ──► invoke('chat_turn')   [ONE payload, ONE command]
                                              │
                    ┌─────────────────────────┴─────────────────────────┐
        execution: 'sync'                                   execution: 'background'
        chatPending → chatReplySettled                       submit → submitResolved → agentEvent*
        (terminal request/response)                          (long-lived correlated stream)
```

The two store lifecycles are not cosmetic variants. `submitResolved` is the
**only** writer of `taskToSession` (`sessionChatStore.ts:126-148`), and that map
is what routes every `task_*` and `token_streamed` frame to a bubble and what
replays the 30-second pending buffer. Unifying it would leave background turns
with an unroutable event stream and a bubble settled *done and empty*, beyond
the watchdog's reach because it only sweeps `pending`.

So the frontend keeps its branch for **dispatch bookkeeping** while sharing one
payload and one command. Specifically retained on the background branch:
`executeIpcWithRun` (the one place GUI run-ids meet orchestrator task-ids — the
join key `runs.rs` uses for cost and token telemetry), `extractTaskId`, and
`recordGamifyGuiEvent('task_submitted')`.

`run_background` must **not** persist an assistant row. Today's background path
writes no assistant row at all; a persisted "Dispatched as task #N" receipt
would hydrate on reload in place of the live task bubble and would manufacture
exactly the display/context asymmetry §9 exists to explain. Duplicate refusal is
likewise a control-plane outcome, not a message: it returns
`ChatTurnError::Duplicate { of }` with no persistence.

### 6.1 `ChatTurnInput`

Both `ChatTurnInput` and `SubmitTaskInput` derive `Serialize` so the parity
guard can reflect over **both**. `execution` is set explicitly from the
composer's `executionMode`, never derived from the absence of the retiring
`task_category: 'chat'` sentinel.

```rust
pub struct ChatTurnInput {
    pub session_id: String,
    pub content: String,
    #[serde(default)] pub execution: Execution,   // Sync | Background, default Sync
    #[serde(default)] pub model_override: Option<String>,
    #[serde(default)] pub tier: Option<String>,   // local|mesh|cloud|auto
    #[serde(default)] pub clutch: Option<String>,
    #[serde(default)] pub risk: Option<String>,
    #[serde(default)] pub context_files: Vec<String>,
    #[serde(default)] pub active_skill: Option<String>,
    #[serde(default)] pub skill_exclusions: Vec<String>,
    #[serde(default)] pub grounding_check_enabled: Option<bool>,
    #[serde(default)] pub priority: Option<String>,
    #[serde(default)] pub dry_run: Option<bool>,
    #[serde(default)] pub allow_duplicate: Option<bool>,
}
```

`intent` is **not** a field — Loquela folds it into `description`. Adding one
would create a permanently-null key and, if Loquale later emitted it without
removing `composeDescription`, would double-count it.

`Execution::Sync` stays the default: an omitting client (the deprecated
`chat_send_message` shim, the VS Code extension) wants the historical
synchronous reply, and a wrong-way `Background` default fails *silently* with
no reply, where a wrong-way `Sync` merely costs a visible round-trip.

### 6.2 The tier goes to `McpChatModelResolution`, not `cognitive_profile`

`cognitive_profile` accepts `fast|reasoning|creative`; the composer's tier is
`local|mesh|cloud|auto`. Zero overlap — and setting `cognitive_profile`
**switches the turn off the agent loop** onto `mcp_infer_completion`
(`message.rs:587`), losing tool calls and `selection_reason`. Wiring a tier
picker to it would silently disable delegation.

The tier belongs in `McpChatModelResolution`
(`enforce_free_tier_only`, `allow_cheapest_fallback`, `clutch`, `risk`),
threaded through `try_run_agent_turn`'s `resolution_template`.

### 6.3 Published schemas need hand-editing

`ChatMessageParams` has no `deny_unknown_fields` and its published schema is a
hand-written literal ending `additionalProperties: true`
(`input_schemas.rs:626-628`). Adding a struct field therefore neither breaks
old clients nor documents the new one — the literal must be edited, and no CI
gate catches the omission. `SubmitTaskParams` is the opposite: `schemars`
`deny_unknown_fields` publishes `additionalProperties: false`, so a
schema-validating client rejects a new key until the derive is updated.

## 7. Model selection

`ChatMessageParams` gains `model_override`. Precedence:
request → process-global → auto-selector, with blanks treated as absent.

```rust
pub struct SelectionDto {
    pub model_id: String,          // the RESOLVED id
    pub requested: Option<String>,
    pub source: SelectionSource,   // UserOverride | Global | AutoRouted | Fallback
    pub rationale: Option<String>,
    pub context_fill_ratio: Option<f32>,
    pub is_free: bool,
}
```

**`source` is classified from the resolved model, never the request.**
`UserOverride` only when `resolved.id == requested`; a requested model that the
resolver ignored yields `Fallback` with the rationale
"requested `X` is not in the registry". Without this, a stale pinned model
produces a badge reading "Your pick" over a model the user did not get — a
worse honesty failure than the missing rationale it replaces.

Supporting changes: `resolve_mcp_chat_model_sync_inner` sets `rationale_out`
on the unknown-`pref` path instead of falling through mutely, and
`try_run_agent_turn` stops swallowing that error when the pref was
user-supplied. All three `None` sites get truthful strings per §1.2. The
persisted pick is validated against the live registry on mount and cleared if
absent.

`candidates` (ranked alternatives) is **deferred**: `McpModelChoice` carries
`{model, is_free, rationale}` and does not retain the rejected set, so exposing
it means changing the scorer's return shape. `source` + rationale answers "why
this model" without it.

Storage: `chatModelOverride` moves to `useLocalStorage` with a key registered in
`SHELL_PREFERENCE_KEYS` and `contracts/gui/shell-persistence.v1.yaml`, lifted to
`App.tsx` — one store, not the picker-local second copy the first draft created.

## 8. Planning

`/plan` issues a **`vox_plan`** tool call with `{goal, session_id}` — not a
`vox_chat_message` with a `mode` key. `ChatTurnDto` returns
`plan_session_id`/`version` so `ChatSurface` can point `PlanPanel` at it
directly, rather than via the `latest_plan_session_for_chat` badge click.

The approval gate reuses what exists. `plan.rs:516` writes
`"blocked_on_approval"` instead of `"pending"`, and `approve_plan` calls
`approve_all_blocked_plan_nodes` — one SQL statement, zero new status
vocabulary, and `enqueue_runnable_plan_nodes`'s existing
`status != "pending" → skip` is the gate.

**The gate is opt-in.** `vox_plan` gains `require_approval: bool`, set only by
the GUI's `/plan`. Without this, every non-GUI caller breaks: `ai.plan.execute`
becomes a **silent no-op returning `ok: true`**, `goal.rs:719-731` hard-errors
with "planning produced no initial runnable nodes", and successor-node
scheduling stalls mid-plan (`persistence.rs:129`). `insert_plan_node` routes
through the same gate so the GUI cannot create a runnable node inside an
unapproved plan. `row_to_plan_node` gets an explicit arm for the new status
rather than letting it alias to `Pending`.

Rollback for this phase is a data fix-up, not a code revert:
`UPDATE plan_nodes SET status='pending' WHERE status='blocked_on_approval'`.

## 9. Skills

Activation becomes a transcript event **emitted from the tool result, not the
call arguments**. Emitting from args would render "skill activated · ponytail"
for a call that was denied, unknown, or errored — letting a prompt-injecting
file assert in system-styled UI that a trusted skill ran. Skill ids are
resolved against the registry; unknown ids render as unknown. Model-authored
strings (`delegation_reason`, task descriptions) are truncated and rendered as
quoted model text, never as system statements.

`skill_exclusions` is honoured **in the skill-selection path** — the
load-bearing half the first draft omitted — and "not this one" re-runs the turn
immediately rather than silently affecting the next message. Session-scoped in
React state; `sessionStorage` if persistence is later wanted.

Turn events render in a **new** `ChatTurnEventRow`, with a render site added in
`ChatTranscript.tsx` in the same task. `ChatAgentEventRow` is left alone: it is
a different data source and carries three live HITL controls the first draft
would have deleted.

## 10. Delegation — three prerequisites, in order

None of the correlation work is reachable until all three land.

1. **Reachability.** `TurnContext` gains an include-list mirroring the existing
   `exclude_name_prefixes` hatch, pinning `vox_spawn_agent`, `vox_submit_task`,
   `vox_task_status` ahead of the `.take(40)` cut.
   `is_skill_infrastructure_tool` whitelists them so a pinned skill cannot
   remove them. (Also resolve: `dispatch.rs:140-142` checks the process-global
   `active_skill_id`, not the per-turn `params.skill`.)
2. **Durability.** `chat_session_id` / `origin_turn_id` on
   `AgentDelegationBinding` and `DelegationEdge`, populated in
   `spawn_dynamic_agent_with_parent`, and persisted — the current binding is a
   HashMap that dies with the daemon.
3. **Readability.** Write the `SUBAGENT_TREE` arm in `orch_daemon/mod.rs`
   returning `{"tree": …}` from `agent_topology_snapshot`'s existing
   `delegation_edges`.

Only then: the DTO fields, the delegation event rows, and
`pausedAgentForSession`. Until (3) exists, replacing the single-paused-agent
heuristic makes Interrupt/Resume **vanish** rather than occasionally mis-target
— strictly worse.

`chat_session_id` is injected by the loop at `agent_loop.rs:264-272`
(`arguments` is a `Value` passed by value, already cloned), overwriting any
model-supplied value. Guard `Value::Null`; use a key distinct from
`session_id`, which `dispatch.rs:63` reads for telemetry; leave the assistant
message's recorded `calls` un-injected so the model's own transcript is not
rewritten. The field is lineage metadata only — never a dispatch target.

Delegation from a chat turn is HITL-gated through the existing
`PendingApprovals` registry and rendered inline via `InlineApprovals`.

`/spawn` becomes a background `chat_turn` carrying the user's actual composer
text and chips — today it discards whatever was typed and submits a hardcoded
string.

## 11. Streaming

No second channel. `delta_event_name` would have created an uncorrelated
channel with no turn id, racing a settled bubble and leaking listeners.

Instead the sync chat path emits `AgentEventKind::TokenStreamed` carrying
`session_id`, and `resolveSessionForEvent` learns to route a frame that carries
one directly. One channel, one reducer, correlation and buffering already
solved. This requires the sync path to actually stream — `llm_stream` rather
than `llm_chat` — which is the phase's real work.

## 12. Errors

`chat_turn` returns a typed error. The strings it matches must come from the
**real emitters**, not from prose:

- Every error is wrapped as `format!("LLM error: {e}")` before reaching the GUI
  — hence `unwrapLlmErrorPrefix`. Match after unwrapping, with `contains`, not
  `starts_with`.
- The budget string is `"{scope:?} budget of ${cap:.2} exceeded (spent …)"`,
  matched by `BUDGET_EXCEEDED_PATTERN = /^(Daily|Session) budget of \$/`. A
  `contains("budget exceeded")` check does **not** match it.

Every test fixture is taken from an emitter's `format!` call or `Display` impl.
The first draft's invented fixtures passed green while deleting two working
toasts and replacing them with detectors that could never fire.

Variants: `BudgetExceeded`, `RateLimited`, `ContextExceeded`, `ModelUnavailable`,
`Duplicate { of }`, `Backend`. Note that all 175 existing `vox-gui` commands
return `Result<_, String>`; this is a deliberate break, and every frontend
helper that assumes a string must be updated in the same change.

## 13. Model measurement — Efficiency, Responsiveness, Intelligence

### 13.1 The hazard, stated precisely

There are two opposite length biases and only one applies here.

- **Verbosity bias** (judges prefer longer answers) applies to free-text
  preference comparison with no ground truth.
- **Terseness bias** applies whenever length sits in a denominator or in a
  latency/cost term. A coding platform ranking on speed and $/request is
  structurally set up to reward a short wrong answer.

Rule: *if a metric divides by or subtracts length, it is terseness-biased; if
it is a judge's preference over prose, it is verbosity-biased.* Never combine
them into one scalar without checking the sign.

Consequence: **completeness is a hard gate, not a weight.** Efficiency
multiplies on the success gate; it never adds alongside it. The RL literature
is explicit that hard truncation imposes no penalty on short-and-wrong outputs
— that is exactly the hole a weighted sum leaves open.

### 13.2 Definitions

**Efficiency** = `Σ(billed cost over ALL attempts, successes and failures) /
count(successes)`. Failed spend stays in the numerator, so cheap-and-incomplete
becomes the *most* expensive option. Undefined — displayed as "insufficient
data", not zero — below 10 successes. Use billed cost
(`cost_source = "provider_reported"`, already reconciled at `infer.rs:601-604`),
never a token estimate.

**Responsiveness** = p95 TTFT, p95 TPOT, and goodput (fraction of turns meeting
an SLO). **End-to-end latency is excluded** — it is the single largest terseness
incentive available, since stopping early wins it. This axis needs new
instrumentation: there is no TTFT or throughput measurement anywhere today.

**Intelligence** = pass^k (all-k-succeed) with the completeness gate applied,
stratified by task class. Report pass@1 and pass^3 side by side; the spread is
the reliability number. pass@k alone flatters an agent that works one time in
three.

**Completeness gate** — cheap signals only, no per-turn LLM judge:

```
completeness_ok = NOT (finish_reason == "length")       // proof, zero false positives
              AND NOT (unbalanced code fence / brace)
              AND NOT (user re-asked or rephrased within 2 turns)

success = tests_passed AND diff_retained AND completeness_ok
```

Signals 5–7 from the research (sub-request coverage, mid-thought truncation,
tool-call abandonment) are **logged features that explain a score**, not terms
in it. `diff_retained` follows GitHub's Copilot precedent — they abandoned
acceptance rate for accepted-**and-retained** characters for precisely this
bias, and we have git.

### 13.3 What to repair, connect, and build

**Repair** (each of these silently corrupts anything layered on it):

| Defect | Fix |
|---|---|
| `p50` is `AVG`, `p99` is `MAX` | real percentiles, or rename the columns honestly |
| `infer_with_retry` records `latency_ms: 0` | record the real elapsed time |
| `quality_score` is `success ? 1.0 : 0.0` / defaults to 1.0 | replace with the §13.2 gate; stop rendering it until it means something |
| bandit credits the requested model | credit the **served** model |
| `list_model_cards` never calls `inject_scoreboard` | inject, as `vox model explain` already does |
| `vox_eval::quality_proxy_score` = length bands | delete or replace |
| `model-catalog-ssot-2026.md:106` says `quality_weights` is ignored | stale — `scoring.rs:93-107` consumes it |

**Connect:** `harness_eval_task_result ⋈ model_selection_event` is the correct
three-axis join and already exists. It needs a **live-orchestrator writer** so
real turns, not just offline harness runs, populate it. That writer is the
single highest-value item in this section — every other metric is decoration
without a solved/not-solved denominator.

**Build:** TTFT/TPOT capture at the egress boundary; the completeness gate;
`entity_type = 'model'` rows in `reliability_scores` (or `trust_observations`,
which is already model-aware and dimension-keyed).

`scoreboard_feedback_boost` is left alone — the loop is fine; only its
`quality_score` input was dishonest.

### 13.4 Ranking

Three axes stay separate. Publish the Pareto-nondominated set; order within it
by a **budget-constrained argmax** (`argmax Intelligence s.t. cost_per_success
≤ B and p95_TTFT ≤ L`), not a weighted sum of incommensurable units. Report a
credible interval on every axis and suppress ranks below a minimum sample count.

Two failure modes to design against explicitly:

- **Rich-get-richer.** Thompson sampling alone does not fix it, because the
  favoured model also receives the easy queries. Needs a ~5% uniform
  forced-exploration stream (the only unbiased comparison data), contextual
  rather than global posteriors, and recency-decayed statistics — models are
  silently updated behind stable slugs.
- **Stale ranks.** Half-life of 2–4 weeks on the posteriors.

`SelectionAxes` (`select.rs:306-313`) already has exactly these three axes as
user-tunable weights. They are configuration, and the GUI currently renders
them as if they were confidence — that presentation is corrected here.

## 14. Verification — the actual gate

Playwright in this repo runs plain Vite with an injected `switch (cmd)` fake
backend (`e2e/lib/tauriMock.ts`); **no Rust executes**, and
`gui-playwright-smoke` is tiered off `main`/`full-ci` so it gates nothing. An
e2e assertion against a mock the author wrote cannot fail when the code is
wrong.

So the acceptance gate is a **manual checklist executed against the running
app**:

1. `vox run scripts/gui-build.vox` — builds the CLI sidecar. Also the
   prerequisite for `cargo test -p vox-gui`, which otherwise dies in
   `tauri-build` on the missing `externalBin`.
2. `cargo tauri dev` with a live orchestrator daemon.
3. One checklist row per control, each naming the observable that must change
   and where to read it.

Playwright specs are demoted to regression-shape checks, and `tauriMock.ts`
gains cases for every new command.

**Every task must state what observable behaviour fails if the feature is
absent.** Review found seven tests in the first draft that would have passed
while the feature did nothing: a component with no render site, mocked e2e
goldens, a parity guard comparing a constant to itself, a Phase-A assertion on
args the daemon discards, error fixtures invented rather than taken from
emitters, `pausedAgentForSession` fed hand-written arrays against an empty
source, and a delta-channel test over a channel nothing writes. A test that
only inspects a payload it constructed is not acceptance.

## 15. Phasing

Ordered by prerequisite depth, not by narrative.

| Phase | Content | Prerequisite |
|---|---|---|
| **0** | Sidecar build; `pnpm install` | — |
| **A** | `chat_turn`, `buildChatTurn`, real parity guard, `model_override` + tier on `ChatMessageParams` + schema literal, lifecycle branch retained | 0 |
| **B** | `SelectionDto` classified from resolved; three truthful rationales; unknown-pref signalling; badge; registered storage key | A |
| **C** | Typed errors from real emitter strings | A |
| **E** | Skill events from results; `skill_exclusions` honoured in selection; `ChatTurnEventRow` + render site | A |
| **F** | `/plan` → `vox_plan`; `require_approval` opt-in; `blocked_on_approval` + `approve_all_blocked_plan_nodes`; `insert_plan_node` gated | A |
| **D0** | Tool reachability (include-list + skill whitelist) | — |
| **D1** | Durable lineage on `AgentDelegationBinding`/`DelegationEdge` | D0 |
| **D2** | `SUBAGENT_TREE` handler | D1 |
| **D3** | Lineage DTO, delegation rows, `pausedAgentForSession`, `/spawn` rewrite | D2 |
| **G** | Sync path streams via `llm_stream`; `TokenStreamed` carries `session_id` | A |
| **M0** | Measurement repairs (§13.3) | — |
| **M1** | Live writer for the harness-eval join | M0 |
| **M2** | Completeness gate; TTFT/TPOT capture | M1 |
| **M3** | Pareto surface + budget knob; stop rendering `quality_score` until M2 | M2 |

Phase A remains load-bearing for the chat work. D0–D2 and M0–M1 are the
backends that did not exist; they are the honest cost of the first draft's two
false premises.

## 16. Risks

**Tool-cap change (D0) alters every chat turn's tool set.** Pinning three tools
displaces three others at the cut. Verify what falls off the end before landing.

**Approval gate (F) is the only phase that cannot be cleanly reverted.** Rows
written as `blocked_on_approval` stay unrunnable after a code revert; the
data fix-up must be written down before the change lands.

**Typed errors (§12) break a 175-command convention.** Every string-assuming
frontend helper must be updated in the same change.

**Schema literals drift silently.** `ChatMessageParams`'s published schema is
hand-written with `additionalProperties: true`; nothing fails when it falls out
of sync with the struct. Editing it is a manual step in every task that adds a
field.

**`gui-surface-coverage.v1.json` drifts on every new Tauri command**, failing
`ssot-drift` in the *fast* pre-push tier. Regenerate with
`vox ci gui-surface-coverage --write` in the same commit.

**Parallel execution is width 2–3, not 19.** `chat_turn.rs`, `App.tsx`, and
`main.rs` are contended hubs; one agent owns each for the duration of its lane,
or the result is merge conflicts plus the parallel-worktree fmt drift that
AGENTS.md lists as a perennial.
