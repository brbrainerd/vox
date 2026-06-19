# Task-List Cascade Spine — Design Spec

**Date:** 2026-06-18
**Status:** Design (approved for planning)
**Author:** Audit + brainstorming session (Claude, Opus 4.8)
**Sibling specs:** [activity-log-surface](2026-06-18-activity-log-surface-design.md) · [gamification-surfacing-and-minimap](2026-06-18-gamification-surfacing-and-minimap-design.md) · [dashboard-topbar-unification](2026-06-18-dashboard-topbar-unification-design.md)
**Execution target:** plan derived from this spec is written for Gemini 3.5 Flash inside Antigravity — see [gemini-3-5-flash-antigravity-limitations](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md).

---

## 1. Problem

Vox has **two task systems that do not talk to each other**:

1. **The hopper** (`crates/vox-orchestrator/src/hopper/`) — a clean intake model
   (`IntakeItem`, priority authority chain `LearningPolicy < Orchestrator < Developer`,
   capability-gated `reprioritize`, full `override_history` audit). It is an **island**:
   - the `Orchestrator` struct holds **no reference** to it;
   - nothing consumes its `HopperItemAdmitted` / `HopperItemOverridden` bus events;
   - `assign()` mutates item state but never enqueues to an agent;
   - mesh replication **logic** exists (`mesh_adapter::apply_op_fragment`) but has **no transport caller**, and only `ItemAdmitted` is applied (`ItemOverridden`/`ItemTransitioned` return `UnsupportedOpVariant`);
   - storage is **in-memory only** (`InMemoryHopper`, Hp-T5 persistence pending);
   - there is **no direct user-input path** (the GUI can only reach it indirectly through the chat "secretary").

2. **The per-agent `AgentQueue`** (`crates/vox-orchestrator/src/queue/`) — the thing that
   *actually drives agents* (enqueue / dequeue / reorder by urgency). The GUI's `TasksView`
   reads this via the `vox://tasks-changed` signal.

**Consequences for the user:**
- You cannot type a task into the GUI and have it run (no editable inbox bound to the hopper).
- Editing a task does **not** cascade — running sub-agents and mesh peers never hear about it.
- Nothing survives a daemon restart.

**The user's explicit requirement:** *"fully edit and visualize the task list … when it is changed, these changes cascade throughout all agents in the orchestrator … to all running sub-agents, agents, and the mesh, smoothly and cleanly."*

## 2. Goal

Make the **hopper the single canonical task spine**: an editable list that the user
drives from the GUI, that the orchestrator dispatches into agent queues, that cascades
priority/cancel changes to running agents, that replicates over the mesh, and that
persists across restarts — all over one reactive event topic.

Non-goals (YAGNI): a new kanban DnD board (the existing `TasksView` is reused/extended),
multi-tenant ACLs beyond the existing `DeveloperOverride` capability, and a new mesh
transport protocol (we wire into the existing federation envelope/transport seam).

## 3. Architecture

```
            ┌────────────── GUI (vox-gui) ──────────────┐
 user types │  TasksView  ──(Tauri cmd)──►  hopper_submit │
 / edits    │     ▲                          hopper_reprioritize
            │     │ vox://tasks-changed       hopper_cancel │
            └─────┼──────────────────────────────────────┘
                  │ (reactive refresh)
        ┌─────────┴───────────────────────────────────────────┐
        │  Orchestrator (vox-orchestrator)                      │
        │   holds Arc<dyn HopperIntake>                         │
        │   ┌────────────┐  HopperItemAdmitted   ┌───────────┐  │
        │   │  Hopper    │ ───────────────────►  │ Dispatcher│  │
        │   │ (intake)   │  HopperItemOverridden │   loop    │  │
        │   └────┬───────┘ ◄───cancel/reprio──── └────┬──────┘  │
        │        │ persist (Hp-T5)                     │ enqueue │
        │        ▼                                     ▼         │
        │  vox-db hopper_inbox            per-agent AgentQueue   │
        └────────┼─────────────────────────────────────────────┘
                 │ outbound ops (ItemAdmitted/Overridden/Transitioned)
                 ▼
       Mesh federation transport ──► peer daemons (apply_op_fragment)
```

### 3.1 Components & responsibilities

| Unit | File(s) | Responsibility |
|---|---|---|
| `HopperIntake` trait | `hopper/store.rs` (exists) | CRUD-ish contract. **Add** `cancel()`. |
| `SqliteHopper` (Hp-T5) | `hopper/sqlite_store.rs` (**new**) | Persistent `HopperIntake` impl over a `hopper_inbox` vox-db table. |
| `hopper_inbox` schema | `vox-db` schema domain (**new** table) | Durable item rows. |
| Dispatcher loop | `orchestrator/dispatch.rs` (**new** or extend) | Subscribe to `HopperItemAdmitted`; convert `IntakeItem`→agent task; pick agent by affinity; `enqueue`. Subscribe to `HopperItemOverridden`/cancel; reorder/remove from agent queue. |
| `IntakeItem → AgentTask` mapper | `orchestrator/dispatch.rs` | Pure function, unit-testable. |
| Orchestrator wiring | `orchestrator.rs` / `orchestrator/types.rs` | Add `hopper: Arc<dyn HopperIntake>` field + accessor; spawn dispatcher. |
| Outbound mesh emit | `hopper/mesh_adapter.rs` (extend) | Emit `ItemOverridden`/`ItemTransitioned` ops; support their apply paths. |
| Mesh transport caller | existing federation transport seam | Call `apply_op_fragment` on inbound envelopes. |
| Tauri commands | `vox-gui/src/commands/orchestrator.rs` (extend) | `hopper_submit`, `hopper_reprioritize`, `hopper_cancel`, `hopper_list`. Each calls `emit_tasks_changed`. |
| GUI `TasksView` | `vox-gui/ui/src/components/surfaces/.../Tasks*` | Editable list: text-entry composer, priority control, cancel; subscribe to `vox://tasks-changed`. |

### 3.2 Data flow (the cascade)

1. **Create:** user types in `TasksView` composer → `hopper_submit` Tauri cmd → `hopper.submit(...)` → item lands in Inbox, emits `HopperItemAdmitted` → `emit_tasks_changed`.
2. **Dispatch:** dispatcher loop receives `HopperItemAdmitted` → maps to an agent task → selects agent by `affinity_hints` (fallback: least-loaded) → `agent_queue.enqueue(task)` → `hopper.assign(item, agent_id)` (emits transition).
3. **Reprioritize:** user changes priority → `hopper_reprioritize` (mints `DeveloperOverride`) → `hopper.reprioritize(...)` emits `HopperItemOverridden` → dispatcher reorders the live agent queue entry → `emit_tasks_changed`.
4. **Cancel:** user cancels → `hopper_cancel` → new `hopper.cancel(...)` sets `ItemState::Cancelled` (terminal) → dispatcher removes from agent queue if still queued → `emit_tasks_changed`.
5. **Mesh:** every admit/override/transition emits a `HopperOpSync` op → wrapped in signed `OpFragmentEnvelope` → transport publishes → peer `apply_op_fragment` converges idempotently.
6. **Restart:** `SqliteHopper` rehydrates Inbox/Assigned from `hopper_inbox`; dispatcher re-enqueues un-terminal items on boot.

### 3.3 The reactive topic

Reuse the **existing** `vox://tasks-changed` Tauri signal (it already exists and the
frontend already subscribes). It stays a signal-only "refresh now" nudge to avoid a
second source of truth in the payload; the frontend re-reads via `hopper_list`. This
keeps the SSOT in the hopper, not in event payloads (consistent with how
`emit_tasks_changed` is documented today).

## 4. Error handling

- `reprioritize`/`assign`/`cancel` on terminal items → `HopperError::Terminal` (exists); surfaced to GUI as a toast, no state change.
- Dispatcher cannot find an eligible agent → item stays in Inbox; dispatcher retries on next admit tick (no spin loop). Logged, not fatal.
- Mesh apply with bad signature / low trust tier → reject (existing `apply_op_fragment` gates), counted in a metric, never panics.
- Persistence write failure → command returns `Err`; GUI shows error; in-memory state not advanced past DB (write-through, DB is source of truth).

## 5. Testing strategy

- **Unit:** `IntakeItem → AgentTask` mapper (affinity, priority mapping); `cancel()` state machine; `SqliteHopper` round-trip (submit→reload).
- **Integration:** admit → dispatcher enqueues to the right agent; override → agent queue reorders; cancel → removed from queue; restart → inbox rehydrates.
- **Mesh:** two in-process hoppers; emit op from A, apply on B, assert convergence + idempotency on re-apply.
- All tests follow Vox rules: inject params (no global state), `Span::new(0,0)` fixtures, deterministic, no `.unwrap()` in lib code.

## 6. Decomposition into plan tasks (preview)

1. Add `ItemState::Cancelled` + `HopperIntake::cancel()` (in-memory impl) — atomic, green.
2. `IntakeItem → AgentTask` pure mapper + tests.
3. Add `hopper` field + accessor to `Orchestrator`; spawn dispatcher (admit→enqueue).
4. Dispatcher: override→reorder, cancel→remove.
5. Hp-T5 `hopper_inbox` table + `SqliteHopper` + rehydrate-on-boot.
6. Outbound mesh ops for override/transition + apply paths.
7. Wire transport caller to `apply_op_fragment`.
8. Tauri `hopper_submit`/`reprioritize`/`cancel`/`list` (+ `emit_tasks_changed`).
9. GUI `TasksView` editable composer + priority/cancel controls + reactive refresh.

Each is a self-contained, atomic-green-committed task; backend tasks (1–7) and GUI
tasks (8–9) split by file disjointness for `[PARALLEL-SAFE]` tagging in the plan.
