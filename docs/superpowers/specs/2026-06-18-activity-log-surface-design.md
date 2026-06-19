# Activity-Log Surface — Design Spec

**Date:** 2026-06-18
**Status:** Design (approved for planning)
**Author:** Audit + brainstorming session (Claude, Opus 4.8)
**Sibling specs:** [task-list-cascade-spine](2026-06-18-task-list-cascade-spine-design.md) · [gamification-surfacing-and-minimap](2026-06-18-gamification-surfacing-and-minimap-design.md) · [dashboard-topbar-unification](2026-06-18-dashboard-topbar-unification-design.md)
**Execution target:** Gemini 3.5 Flash inside Antigravity — see [limitations doc](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md).

---

## Amendment 2026-06-18 (unification) — see [unified-task-message-envelope spec](2026-06-18-unified-task-message-envelope-registers-budget-ssot-design.md)

The §5 cost roll-up should source its totals from the budget SSOT — the *existing*
`BudgetManager` (extended with `snapshot()`), via `budget_get` + `vox://cost-changed` —
rather than summing `CostIncurred` rows independently —
so the activity timeline's spend agrees byte-for-byte with the TopHud tile, the budget widget,
and the gamified treasury. The per-row `CostIncurred` entries still appear in the timeline; only
the *aggregate* number comes from the ledger. Implemented in the unified plan's Tasks 5–6.

---

## 1. Problem

There are two streams of "what happened", and they are mirror-image broken:

- **Chat log** (the user's words) is **durable + visible**: persisted in `vox-db`
  (`conversations`, `conversation_messages`, `conversation_tool_calls`), rendered in
  `ChatSurface` / `Loquela`.
- **Agent activity** (what the AI is *doing*) is **ephemeral + invisible**: the
  `EventBus` emits **70+ `AgentEventKind` variants** (spawn/retire/heartbeat, task phase
  `Inspect→Act→Verify`, delegation/handoff, `CostIncurred`, `LockAcquired`, `BuildStage`,
  `ThroughputTick`, `MeshTopologyChanged`, …) over `vox://agent-events`, but:
  - it is **in-memory broadcast only** — a slow consumer silently **drops** events;
  - **nothing persists** it (no replay, no history, no audit);
  - there is **no dedicated activity-log view** — the dashboard `stream` widget shows a
    derived feed, but you cannot scroll back, filter by agent/kind, or see what an agent
    did five minutes ago.

The user's ask: *"visualize the activity log, what the AI itself is actually doing …
a separate view for that … what should be surfaced there?"*

## 2. Goal

A first-class, **persisted, filterable agent-activity timeline**, distinct from chat:
- a vox-db `activity_log` table that durably captures the high-signal subset of
  `AgentEventKind`;
- a sink that writes the live `EventBus` to that table (lossless w.r.t. the selected
  subset, decoupled from the lossy broadcast);
- a dedicated GUI **Activity** surface: reverse-chronological timeline, filter by agent /
  event kind / session, click-through to the related task or run.

Non-goals (YAGNI): persisting *every* event kind (we curate a high-signal allowlist),
full-text search across activity (phase 2), and merging activity into the chat transcript
(they stay separate views; they correlate via `session_id`).

## 3. What to surface (the curated allowlist)

Not all 70+ kinds belong in a human timeline. Tier them:

| Tier | Event kinds | In activity log? |
|---|---|---|
| **Lifecycle** | `AgentSpawned`, `AgentRetired`, `TaskSubmitted`, `TaskStarted`, `TaskPhaseChanged`, `TaskCompleted`, `TaskFailed`, `TaskReprioritized`, `TaskDelegated`, `PlanHandoff` | **Yes** (the spine of the narrative) |
| **Resource/cost** | `CostIncurred`, `BudgetAlert`, `AttentionBudgetAlert`, `LockAcquired`/`LockReleased`, `ConflictDetected` | **Yes** (but cost rolled up; see §5) |
| **Build/mesh** | `BuildStage`, `MeshTopologyChanged`, `WorkflowStarted`/`Completed`/`Failed` | **Yes** |
| **High-frequency telemetry** | `AgentHeartbeat`, `ThroughputTick`, `CostTick`, `FileDiagChanged` | **No** — too noisy; these feed live dashboard widgets, not the durable log |

The allowlist is a single `fn is_loggable(&AgentEventKind) -> bool` (SSOT), so adding/removing a kind is one edit.

## 4. Architecture

```
 EventBus (broadcast, lossy)
     │  subscribe (dedicated, never the bottleneck)
     ▼
 ActivitySink ── is_loggable? ──► vox-db activity_log (durable)
     │                                  ▲
     │ vox://activity-appended          │ query (paged, filtered)
     ▼                                  │
 GUI Activity surface ◄── Tauri cmd: activity_query(filter, page)
```

### 4.1 Components

| Unit | File(s) | Responsibility |
|---|---|---|
| `activity_log` table | `vox-db` schema domain (**new**) | `id, ts_ms, agent_id, session_id, kind, summary, detail_json` + indexes on `(ts_ms)`, `(agent_id)`, `(kind)`. |
| `is_loggable` allowlist | `vox-orchestrator/src/activity/mod.rs` (**new**) | SSOT for which kinds persist. |
| `AgentEventKind → ActivityRow` projection | `vox-orchestrator/src/activity/project.rs` (**new**) | Pure function: kind → `{summary, detail_json}`. Unit-testable. |
| `ActivitySink` | `vox-orchestrator/src/activity/sink.rs` (**new**) | Subscribe `EventBus`; for loggable events, insert row; tolerate broadcast lag (it's its own consumer, lag only drops *log* rows, never blocks agents). |
| Tauri `activity_query` | `vox-gui/src/commands/activity.rs` (**new**) | Paged, filtered read. |
| `vox://activity-appended` | `vox-gui/src/commands/activity.rs` | Optional reactive nudge so the open timeline auto-appends. |
| GUI **Activity** surface | `vox-gui/ui/src/components/surfaces/Activity/` (**new**) | Timeline list + filter bar; registered in `SURFACE_REGISTRY`. |

### 4.2 Why a sink, not "persist at emit"

The `EventBus` is deliberately lossy for liveness (agents must never block on a slow
subscriber). The sink is *one more subscriber*; if it falls behind, only log rows are
dropped — never agent progress. This preserves the existing liveness contract while
adding durability where it matters. (If zero-loss audit is later required, bound the sink
with a backpressure queue — out of scope here.)

## 5. Cost roll-up

`CostIncurred` can fire frequently. Store each as a row **but** the Activity surface
groups consecutive same-agent cost rows into a single expandable "spent $X over N calls"
entry (client-side fold) so the timeline stays readable. No new aggregation table.

## 6. Activity vs chat — the separation

- **Chat surface** answers "what did I (the user) say and what did the assistant reply."
- **Activity surface** answers "what did the agents *do*" — tool phases, spawns, costs, conflicts, builds.
- They **correlate** via `session_id` (already on both `conversation_messages` and most events). A future "jump to activity for this message" link is cheap because the key already exists — but is out of scope for this plan.

## 7. Error handling

- DB insert failure in the sink → log + drop that row; never crash the sink task; emit a counter.
- `activity_query` with an unknown filter → empty page, not an error.
- Surface with zero rows → `EmptyState` ("No activity yet").

## 8. Testing strategy

- **Unit:** `is_loggable` (allowlist correctness incl. the noisy-kind exclusions); projection (`kind → summary/detail_json`) for a representative event of each tier.
- **Integration:** emit N events on a test `EventBus`; assert exactly the loggable ones land in `activity_log` with correct columns; assert query paging + filter-by-agent/kind.
- Deterministic timestamps injected (no wall-clock in assertions).

## 9. Decomposition into plan tasks (preview)

1. `activity_log` table + migration + row struct.
2. `is_loggable` allowlist + tests.
3. `AgentEventKind → ActivityRow` projection + tests.
4. `ActivitySink` (subscribe + insert) + integration test; spawn from orchestrator boot.
5. Tauri `activity_query` (paged/filtered) + `vox://activity-appended`.
6. GUI Activity surface (timeline + filter bar) + `SURFACE_REGISTRY` entry.
7. Cost-row client-side fold in the surface.

Backend (1–5) and frontend (6–7) split by file disjointness for `[PARALLEL-SAFE]` tagging.
