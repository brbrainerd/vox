---
title: Chat / Flow / Docking Redesign
status: approved
date: 2026-07-20
---

# Chat / Flow / Docking Redesign — Design

## Problem

Live testing this session surfaced a cluster of related UX problems in the Axis GUI's chat surface:

1. **Event spam in the chat transcript.** `ChatTranscript.tsx` renders `ChatAgentEventRow` for every raw orchestrator event (`CHECKPOINT`, `TASK`, `PHASE`, `COST`, `TOKEN`) inline with actual chat messages. A single "say hello" exchange produces 20-50+ rows before the reply appears, each with a "View in Flow" link — the signal (the conversation) is buried in noise (execution mechanics) that belongs in a different surface.
2. **No unified visibility.** The Flow view (execution graph/event detail) is a separate tab. Users must leave chat to see what an agent is actually doing, and there is no way to keep an eye on progress without either the event-spam above or a full context switch.
3. **Fixed, non-dockable surfaces.** Chat sessions list (left), execution rail (right), and Flow (separate tab) are each hard-coded to one position with no way to move, resize into arbitrary edges, tab together, or hide/show like a real IDE workspace. `dockview` (a VS Code-style docking library) is already an installed dependency with a finished Vox-branded theme (`dockview-vox.css`) but is not wired into the app anywhere.
4. **All-or-nothing validation gates.** Chat replies briefly hit the same code-validation gate cascade (behavioral tests, research/approval/trust/harness gates) as agentic coding tasks — already fixed to skip entirely this session, but that leaves no way to opt back into a lightweight grounding/hallucination check for a chat conversation where factual accuracy matters.
5. **No visible, editable plan/task list.** Users expect something like Cursor's or Claude Code's live todo list — a running plan that grows as the agent works, that the user can also edit directly, with edits taking effect for not-yet-executed steps. The orchestrator already has a versioned plan-DAG subsystem (`crates/vox-orchestrator/src/planning/`) that is not surfaced in the GUI at all.
6. **Minor surface polish**: inline start/stop/resume controls (Claude-Code-style) don't exist in the composer; already fixed this session: sidebar icon fallbacks, execution-rail title truncation, a tab-title mismatch.

## Goals

- Chat transcript shows only the conversation, plus one live-updating status line while a task is in flight.
- Full execution detail (all raw events) remains available, in a real dockable panel, not a separate tab you have to navigate away to.
- Every panel around the central chat pane (sessions list, execution rail, Flow, new plan panel) is independently dockable/resizable/hideable via `dockview`, persisted across restarts.
- A live, editable plan/task-list panel: shows the orchestrator's real plan-DAG state, lets the user edit not-yet-run step descriptions (which the scheduler picks up automatically, since it already re-reads current DB state before dispatching each node) and insert new intermediate steps, and grows automatically as the agent discovers it needs more steps (new agent-side capability, modeled on Claude Code's `TodoWrite`).
- Chat gate policy becomes a per-session opt-in toggle for a lightweight grounding check, rather than either "always run the full code-validation cascade" (the pre-existing bug) or "never validate anything" (this session's interim fix).
- Inline start/stop/resume controls live in the composer row.

## Non-goals

- Rebuilding Flow's existing event-graph visualization — it stays as-is, just becomes a dockable panel instead of a fixed tab.
- Any change to the orchestrator's actual task dispatch/queue mechanics beyond what's needed for plan-node editing and dynamic node insertion (Section 5). No new task categories, no new gate types beyond the one described in Section 4.
- A rich WYSIWYG editor for the plan panel — plain checklist-style markdown text editing is sufficient (Section 5).
- Full mobile/touch layout — this is a desktop Tauri app; dockview's desktop drag-and-drop behavior is the target, not touch gestures.

## Section 1: Event routing & verbosity

**Today:** `buildTranscriptTimeline` (`lib/chatTranscriptTimeline.ts`) merges chat messages and raw `AgentEventKind` events (`TaskStarted`, `PhaseChanged`, `CheckpointCaptured`, `CostIncurred`, `TokenStreamed`, ...) into one array; `ChatTranscript.tsx` renders every row, chat and event alike, via `ChatAgentEventRow`.

**Target:**
- `buildTranscriptTimeline` splits into two outputs: a chat-only row list (user/assistant messages) and a full event list (unchanged shape, still consumed by Flow).
- The chat-only list additionally carries one **synthetic live-status row** per in-flight task: `{ phase: string, elapsedMs: number }`, updated in place as `PhaseChanged`/`TaskStarted`/`TaskCompleted` events arrive for that task, removed the moment the task completes and its reply lands.
- Rendered as a single line, e.g. `Verify · 12s`, using the existing `PhaseChip` component's tone mapping (no new visual language needed).
- A global 3-level verbosity setting (**Quiet / Normal / Verbose**, persisted the same way `EXECUTION_RAIL_COLLAPSED_KEY` already persists rail state) controls what, if anything, layers on top of that baseline in the chat feed:
  - **Quiet**: only the live status line, replaced by the reply on completion. No other detail.
  - **Normal** (default): adds one summary line per completed turn — `Done in 12s · $0.003` — using data already emitted via `CostIncurred`.
  - **Verbose**: adds an inline, collapsed-by-default expandable breadcrumb per phase (reusing `ChatAgentEventRow`'s existing `collapsed` row style) without ever leaving the chat tab.
- Full, unfiltered event detail is available at all times in the Flow panel (Section 2), independent of the verbosity setting.

## Section 2: Docking architecture (dockview)

Adopt `dockview` (already a dependency, already themed via `dockview-vox.css`) as the layout engine for the chat workspace shell.

- **Panels**: Chat Sessions (existing left list), Execution Rail (existing `ChatExecutionRail`, today fixed-right), Flow (existing graph view, today a separate top-level tab), Plan (new, Section 5). Central chat transcript is dockview's non-closable main content panel.
- **Behavior**: real drag-and-drop to any edge, tab-grouping (e.g. drag Flow onto Plan to tab them together), per-panel resize, per-panel hide/show, matching `dockview`'s native capabilities — no custom drag/drop code needed, `dockview` provides this.
- **Persistence**: `dockview` supports layout serialization; store the serialized layout in `localStorage` under a new `gui.chat.dockview_layout.v1` key (same persistence pattern as `EXECUTION_RAIL_COLLAPSED_KEY`), restored on mount, reset-to-default available from a menu action.
- **Migration**: `ChatExecutionRail`'s current hand-rolled `collapsed` state/localStorage key is superseded by dockview's own panel-visibility state; the component's content (task list, resource strip, intent map) stays, only the collapse/expand chrome around it changes.
- **Implementation care** (per explicit request): dockview panels wrap existing React components with minimal adapter code — no rewriting `ChatExecutionRail`, `AgentFlow`, or the sessions list internals, only how they're mounted/positioned. A visual pass (screenshots across default layout, a dragged/rearranged layout, and light/dark theme) happens before this is considered done, not just unit tests.

## Section 3: Chat gate policy

- New per-session setting (persisted with the session, alongside existing session-scoped state): **grounding check: off (default) / on**.
- When on, `ChatTaskProcessor` (already the fast single-call path — see `crates/vox-orchestrator/src/chat_processor.rs`) runs a lightweight Socrates/CRAG-style grounding check on its own reply *after* generating it, non-blocking to the reply itself (the reply still streams immediately) — a background pass that, if it flags low grounding confidence, surfaces a small inline warning badge on that message rather than gating/retrying.
- This deliberately does **not** reuse the full behavioral/research/approval/trust/harness gate cascade (`orchestrator/task_dispatch/complete/success/`) that was just fixed to skip for chat entirely — that cascade validates code artifacts (tests, file writes) that don't apply to a conversational reply. This is a new, narrow, chat-specific check.
- Toggle lives in the session's settings (accessible from the chat surface), off by default so normal chat stays at today's (now-fixed) fast latency.

## Section 4: Composer controls

- Start/stop/resume controls move into the composer row itself, replacing/augmenting the send-button area: a stop button while a task is in flight (wired to the existing `interrupt_task`/cancel-flag mechanism already used by `abort_interrupted_task`), a resume button when paused.
- No change to the underlying interrupt/cancel machinery — this is a UI relocation, not new backend behavior.

## Section 5: Editable, auto-growing plan panel

**Existing backend foundation** (confirmed by code reading, not assumed):
- `crates/vox-orchestrator/src/planning/types.rs`: `PlanNode { node_id, description, depends_on, status, execution_policy, workflow_invocation }`, `PlanStatus` (Pending/Queued/InProgress/Completed/Failed/Cancelled/Superseded), `PlanSessionRecord` (versioned via `PlanVersionRecord`).
- `crates/vox-db/src/store/ops_planning.rs`: `upsert_plan_node` (insert-or-update, general-purpose, already used at 3 call sites — all at upfront plan synthesis, none mid-execution), `load_plan_nodes_with_status`, `set_plan_node_status`.
- `crates/vox-orchestrator/src/planning/schedule.rs::enqueue_runnable_plan_nodes`: re-reads current node rows from the DB immediately before dispatching each runnable node (not a stale in-memory snapshot) — this is the mechanism that makes "edit a not-yet-run step and have it take effect" already correct without new plumbing.

**New work required:**
1. **GUI panel**: renders the active `PlanSessionRecord`'s nodes as a checklist (status → visual state, not a manual dispatch checkbox — dispatch stays automatic via the scheduler as today). Editing a node's description text writes through a new Tauri command to `upsert_plan_node` (same DB primitive already used server-side, no new DB schema). Inserting a new node via the panel calls the same primitive with a fresh `node_id` and appropriate `depends_on`.
2. **Dynamic mid-execution growth**: currently the only dynamic-node-creation path is `replan.rs::synthesize_recovery_nodes`, narrowly scoped to failure recovery. New: teach the phase loop (wherever the agent reasons about its current step, alongside the existing 6-phase Inspect/Localize/Hypothesize/Act/Verify/Decide loop) to call `upsert_plan_node` when it determines a new step is warranted — the `TodoWrite`-equivalent capability. This is new agent-decision logic on top of existing, unchanged storage/versioning.
3. **Clarification path**: if the agent encounters an edited or user-inserted node it finds ambiguous, it asks in chat before proceeding — ordinary chat-turn behavior using the existing reply mechanism, not new plan-panel machinery.
4. **Dashboard integration**: flagged by the user as worth considering — out of scope for this design's first cut; the plan panel's data model (real `PlanSessionRecord`/`PlanNode` reads) is dashboard-compatible from day one if a dashboard widget is added later, since it reads the same source of truth.

## Testing

- Frontend: vitest coverage for the timeline split (chat-only vs. full event list), the verbosity-level rendering differences, dockview panel mount/persistence (serialize → reload → same layout), plan-panel edit → API call, composer stop/resume wiring.
- Backend: TDD for the new Tauri command wrapping `upsert_plan_node` from the GUI edit path; TDD for the phase-loop's new dynamic-node-insertion decision logic (using `StubTaskProcessor`/no real LLM calls, consistent with this codebase's hard "no paid LLM calls in tests" constraint); TDD for the chat-session grounding-check toggle's non-blocking badge behavior.
- No paid LLM calls in any test, matching the established pattern for this whole session's orchestrator work.
