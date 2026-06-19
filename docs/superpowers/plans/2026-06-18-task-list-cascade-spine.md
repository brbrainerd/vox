# Task-List Cascade Spine — Implementation Plan (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: use `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` to execute task-by-task and `crates/vox-skills/skills/superpowers/test-driven-development.skill.md` for each task. Steps use checkbox (`- [ ]`) syntax.

> **🤖 EXECUTION TARGET — READ FIRST.** Run end-to-end by **Gemini 3.5 Flash inside Google Antigravity**. Antigravity is unreliable on long tasks (~48% real-world completion; mid-task termination leaves no checkpoint; quota is a hard cutoff) and Gemini 3.5 Flash hallucinates APIs and has weak long-context recall. This plan is engineered against those failure modes. Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** Finish a task only when its tests pass AND you commit. A crash between tasks must leave a compiling, tested tree. Never split a compile-breaking change across two commits.
2. **Verify-before-use (anti-hallucination).** Before any code step that references a symbol/type/path, run the `rg`/read step in that task and confirm it exists with the stated signature. If reality differs, STOP and report — do not invent.
3. **Self-contained.** Everything you need is in the task. Do not rely on remembering earlier tasks.
4. **Two-strike circuit breaker.** If a step's verification fails twice, STOP, write a one-paragraph handoff note (what failed, last good commit), hand back. Do not loop.
5. **Parallel dispatch.** Tasks are tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`. Only dispatch parallel subagents for `[PARALLEL-SAFE]` tasks whose **Files** sets are disjoint. Never two subagents on one file.
6. **Vox house rules.** Never `cargo fmt --all` (use `cargo fmt -p <crate>`). Automation is `.vox`, not `.ps1/.sh/.py`. `.md` under `docs/src/` needs YAML frontmatter.
7. **Verification ritual** before each commit (`crates/vox-skills/skills/superpowers/verification-before-completion.skill.md`): `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`, pasting real output.
8. **Rollback on broken tree.** If a task aborts mid-edit leaving a non-compiling tree, `git reset --hard HEAD` to the last green commit, then re-attempt that single task.
9. **Rust constraints:** no `.unwrap()` in library code; inject params (no global state) in tests; deterministic output; `cargo run -p vox-arch-check` must pass.

**Goal:** Make the hopper the single canonical task spine: editable from the GUI, dispatched into agent queues by the orchestrator, cascading priority/cancel to running agents, replicated over the mesh, and persisted across restarts.

**Architecture:** Wire `Arc<dyn HopperIntake>` into `Orchestrator`; a dispatcher loop subscribes to `HopperItemAdmitted`/`HopperItemOverridden` bus events and drives per-agent `AgentQueue`s; add `cancel()` + `ItemState::Cancelled`; add a `SqliteHopper` (Hp-T5) behind the existing trait; extend `mesh_adapter` for override/transition ops; expose Tauri commands; extend the GUI `TasksView`.

**Tech Stack:** Rust (`vox-orchestrator`, `vox-db`), `async-trait`, `tokio::broadcast`; Tauri (`vox-gui`); React/TS + vitest (`vox-gui/ui`).

**Design:** [`../specs/2026-06-18-task-list-cascade-spine-design.md`](../specs/2026-06-18-task-list-cascade-spine-design.md).

---

## Flash Execution Addendum (2026-06-18 — second hardening pass)

These override task granularity where they conflict. Source: Flash-executability critique.

**Global gates (apply to every task):**
1. Each Step-1 `rg`/read is a **BLOCKING gate** — run it and paste the output *before* writing any code step; if reality differs from the plan, STOP and report (don't code against memory).
2. **Split-on-overrun:** if an Implement step would touch >1 file OR add >1 new function/struct, commit each sub-bullet as its **own atomic green commit** in the listed order. A Flash cutoff must never straddle two files.
3. Tauri commands register in `crates/vox-gui/src/main.rs`'s `tauri::generate_handler![…]` (not a `commands/mod.rs` macro).
4. For any "for each `match`/site" step, first run the `rg` and paste the **full list of sites**, then edit exactly those.

**Mandatory task splits (execute as separate atomic commits):**
- **Task 1 → 1a / 1b.** 1a: add `ItemState::Cancelled` and fix **every** `ItemState` match/`matches!` site (run `rg -n "ItemState::" crates/vox-orchestrator/src/` first; `Cancelled` is terminal — group with `Done | Overridden`, incl. `history()`); compile + `cargo test -p vox-orchestrator hopper` green; commit. 1b: add `HopperIntake::cancel()` + `InMemoryHopper` impl + the Task-1 test; commit.
- **Task 2:** `AgentTask::new` is **4-arg** — `AgentTask::new(item.item_id.clone(), item.intent.clone(), item.classified_priority, vec![] /* file_manifest */)`. Inline this exact call; do not guess arity.
- **Task 3:** spawn the dispatcher only. **Do NOT add the rehydrate-on-boot call here** — that moves to a new **Task 5-rehydrate** (after `SqliteHopper` exists) so a crash between 3 and 5 never leaves a call to a not-yet-persistent path. Show the `enqueue` closure inline (read `rg -n "pub fn len|fn id" crates/vox-orchestrator/src/queue/mod.rs` first; least-loaded = `agents.values().min_by_key(|q| q.read().len())`).
- **Task 4:** inline BOTH closures: `on_reprioritize` → `queue.reorder(task_id, prio)` and `on_cancel` → `queue.cancel(task_id)`; if the task isn't currently queued, **log and no-op (never `unwrap`/panic)**.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-orchestrator/src/hopper/types.rs` | `ItemState` enum | Modify (Task 1) |
| `crates/vox-orchestrator/src/hopper/store.rs` | `HopperIntake` trait + `InMemoryHopper` | Modify (Tasks 1, 6) |
| `crates/vox-orchestrator/src/orchestrator/dispatch.rs` | `IntakeItem`→task mapper + dispatcher loop | Create (Tasks 2–4) |
| `crates/vox-orchestrator/src/orchestrator.rs` (+ `orchestrator/types.rs`) | hold `hopper`, spawn dispatcher | Modify (Task 3) |
| `crates/vox-orchestrator/src/hopper/sqlite_store.rs` | persistent `HopperIntake` | Create (Task 5) |
| `crates/vox-db/src/schema/domains/*` | `hopper_inbox` table | Modify/Create (Task 5) |
| `crates/vox-orchestrator/src/hopper/mesh_adapter.rs` | outbound + apply override/transition | Modify (Task 7) |
| `crates/vox-gui/src/commands/orchestrator.rs` | `hopper_*` Tauri commands | Modify (Task 8) |
| `crates/vox-gui/ui/src/components/surfaces/**/Tasks*.tsx` | editable list | Modify (Task 9) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "pub enum ItemState" crates/vox-orchestrator/src/hopper/types.rs` — note variants.
- `rg -n "pub trait HopperIntake|async fn (submit|assign|complete|reprioritize)" crates/vox-orchestrator/src/hopper/store.rs` — confirm trait shape (matches design).
- `rg -n "pub struct Orchestrator" crates/vox-orchestrator/src/orchestrator.rs crates/vox-orchestrator/src/orchestrator/types.rs` — find the struct definition file.
- `rg -n "pub fn enqueue|pub struct AgentQueue|fn reorder|fn dequeue" crates/vox-orchestrator/src/queue/mod.rs` — confirm queue API + the task type it stores.
- `rg -n "HopperItemAdmitted|HopperItemOverridden" crates/vox-orchestrator/src/events.rs` — confirm event field names.
- `rg -n "pub fn subscribe|broadcast::Receiver|pub fn emit" crates/vox-orchestrator/src/events.rs` — confirm `EventBus::subscribe()` returns a `broadcast::Receiver<AgentEvent>`.
- `cargo run -p vox-arch-check` — baseline must pass.

---

## Task 1 `[SEQUENTIAL]`: Add `ItemState::Cancelled` + `HopperIntake::cancel()`

Adds a terminal `Cancelled` state and a `cancel()` method to the in-memory hopper.

**Files:**
- Modify: `crates/vox-orchestrator/src/hopper/types.rs` (the `ItemState` enum)
- Modify: `crates/vox-orchestrator/src/hopper/store.rs` (trait + `InMemoryHopper` impl)

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub enum ItemState" -A 12 crates/vox-orchestrator/src/hopper/types.rs`. Confirm variants include `Inbox`, `Assigned`, `Done`, `Overridden`. Run `rg -n "async fn complete" -A 6 crates/vox-orchestrator/src/hopper/store.rs` and read the `complete()` impl to copy its terminal-transition pattern. If `cancel`/`Cancelled` already exist, STOP and report.

- [ ] **Step 2: Write the failing test.** Append to `crates/vox-orchestrator/src/hopper/store.rs` inside its `#[cfg(test)] mod tests` (create the module at end of file if none exists):

```rust
#[cfg(test)]
mod cancel_tests {
    use super::*;
    use crate::hopper::types::{IntakeSource, PriorityHint};

    #[tokio::test]
    async fn cancel_moves_item_to_cancelled_terminal() {
        let hopper = InMemoryHopper::headless();
        let item = hopper
            .submit("do thing".into(), vec![], PriorityHint::Normal, IntakeSource::Developer, None)
            .await;
        let cancelled = hopper.cancel(&item.item_id).await.expect("cancel ok");
        assert!(matches!(cancelled.state, ItemState::Cancelled));
        // second cancel on a terminal item is an error
        let err = hopper.cancel(&item.item_id).await;
        assert!(err.is_err(), "cancelling a terminal item must error");
    }
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator cancel_moves_item_to_cancelled_terminal` → FAIL (`Cancelled`/`cancel` missing).

- [ ] **Step 4: Implement.** (a) In `types.rs`, add to `ItemState`:

```rust
    /// Terminal: the developer cancelled the item before completion.
    Cancelled,
```

(b) In `store.rs` `HopperIntake` trait, add the method signature after `complete`:

```rust
    /// Cancel an item (terminal). Errors if already terminal.
    async fn cancel(&self, item_id: &HopperItemId) -> Result<IntakeItem, HopperError>;
```

(c) In `InMemoryHopper`'s `impl HopperIntake`, mirror `complete()` but set `ItemState::Cancelled` and reject items already in `Done | Overridden | Cancelled` with `HopperError::Terminal`. Use the exact locking/iteration pattern you read in Step 1. `cancel()` does **not** emit a reward/bus event (cancellation is not an achievement); a `HopperItemTransitioned`-style emit is added later in Task 6 for mesh, not here. (d) **`Cancelled` is terminal and appears in `history()`.** Update every `matches!(... , ItemState:: ...)` site the new variant touched — in `store.rs` the `history()` filter currently matches `ItemState::Done | ItemState::Overridden`; change it to `ItemState::Done | ItemState::Overridden | ItemState::Cancelled`, and add a `Cancelled` arm to any other non-exhaustive `match`/`matches!` on `ItemState` (grep first: `rg -n "ItemState::" crates/vox-orchestrator/src/`).

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator cancel_moves_item_to_cancelled_terminal` → PASS. Then `cargo test -p vox-orchestrator hopper` → all hopper tests PASS (fix any non-exhaustive `match ItemState` the new variant broke — add a `Cancelled` arm).

- [ ] **Step 6: Verify + commit.** `cargo clippy -p vox-orchestrator -- -D warnings`; `cargo fmt -p vox-orchestrator`; then:

```bash
git add crates/vox-orchestrator/src/hopper/types.rs crates/vox-orchestrator/src/hopper/store.rs
git commit -m "feat(hopper): add Cancelled terminal state and cancel()"
```

---

## Task 2 `[SEQUENTIAL]` (new file): `IntakeItem → AgentTask` mapper

A pure function converting a hopper item into whatever the agent queue enqueues.

**Files:**
- Create: `crates/vox-orchestrator/src/orchestrator/dispatch.rs`
- Modify: the orchestrator module-decl file (add `pub mod dispatch;`)

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub fn enqueue" -A 8 crates/vox-orchestrator/src/queue/mod.rs` and read the exact **task type** `enqueue` accepts (e.g. `AgentTask { task_id, description, priority, .. }`). Run `rg -n "pub struct IntakeItem" -A 25 crates/vox-orchestrator/src/hopper/types.rs` to confirm `item_id`, `intent`, `affinity_hints`, `classified_priority` field names/types. Run `rg -n "mod dispatch|pub mod " crates/vox-orchestrator/src/orchestrator.rs crates/vox-orchestrator/src/orchestrator/mod.rs` to find where to declare the new module. Inline the real `AgentTask` constructor into Step 4 — do NOT assume field names.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-orchestrator/src/orchestrator/dispatch.rs`:

```rust
//! Hopper → agent-queue dispatch: pure mapping + the dispatcher loop.

use crate::hopper::types::IntakeItem;
// NOTE: replace `AgentTask` + its constructor below with the REAL type confirmed in Step 1.
use crate::queue::AgentTask;

/// Convert an admitted hopper item into the task an `AgentQueue` enqueues.
/// Pure + deterministic so it is unit-testable in isolation.
pub fn intake_to_task(item: &IntakeItem) -> AgentTask {
    // Filled in Step 4 using the real AgentTask constructor.
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hopper::types::{IntakeSource, PriorityHint};
    use crate::hopper::store::{HopperIntake, InMemoryHopper};

    #[tokio::test]
    async fn maps_intent_and_priority() {
        let hopper = InMemoryHopper::headless();
        let item = hopper
            .submit("fix login bug".into(), vec!["crates/auth".into()],
                    PriorityHint::Urgent, IntakeSource::Developer, None)
            .await;
        let task = intake_to_task(&item);
        // Assert the description carries the intent and priority is preserved.
        // Replace `.description` / `.priority` with the REAL AgentTask field names from Step 1.
        assert!(format!("{task:?}").contains("fix login bug"));
    }
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator maps_intent_and_priority` → FAIL (`unimplemented!()` panics).

- [ ] **Step 4: Implement.** Replace `intake_to_task`'s body with the real constructor confirmed in Step 1, e.g.:

```rust
pub fn intake_to_task(item: &IntakeItem) -> AgentTask {
    AgentTask::new(
        item.item_id.clone(),        // or the queue's task-id type
        item.intent.clone(),
        item.classified_priority,
    )
}
```

Adjust to the exact `AgentTask` shape. Refine the Step-2 assertion to check the real description field.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator maps_intent_and_priority` → PASS.

- [ ] **Step 6: Declare the module + commit.** Add `pub mod dispatch;` to the file found in Step 1. Verify + commit:

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/orchestrator/dispatch.rs crates/vox-orchestrator/src/orchestrator.rs
git commit -m "feat(orchestrator): pure IntakeItem->AgentTask mapper"
```

---

## Task 3 `[SEQUENTIAL]`: Hold the hopper + spawn the admit→enqueue dispatcher

Wires the hopper into the orchestrator and routes admitted items to agents.

**Files:**
- Modify: the `Orchestrator` struct file (from Pre-flight) + `crates/vox-orchestrator/src/orchestrator/dispatch.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub struct Orchestrator" -A 20 <struct-file>` (the file from Pre-flight). Confirm fields incl. `agents: Arc<RwLock<HashMap<AgentId, ...>>>` and an `EventBus`/`bus` field. Run `rg -n "fn new\(|pub fn spawn|tokio::spawn" <struct-file>` to find the constructor + how background tasks are spawned. Read how an existing per-agent queue is reached for `enqueue`.

- [ ] **Step 2: Write the failing test.** In `dispatch.rs` add:

```rust
/// Runs the admit→enqueue loop: every HopperItemAdmitted becomes an enqueued task.
/// Returns after `max_events` for test determinism (None = run forever).
pub async fn run_dispatcher(
    mut rx: tokio::sync::broadcast::Receiver<crate::events::AgentEvent>,
    enqueue: impl Fn(AgentTask) + Send + 'static,
    max_events: Option<usize>,
) {
    let mut seen = 0usize;
    while let Ok(ev) = rx.recv().await {
        if let crate::events::AgentEventKind::HopperItemAdmitted { item_id, classified_priority, .. } = ev.kind {
            // Build a minimal task from the event (no item lookup in this loop variant).
            let task = AgentTask::from_admitted(item_id, classified_priority); // real ctor in Step 4
            enqueue(task);
            seen += 1;
            if Some(seen) == max_events { break; }
        }
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use crate::events::EventBus;
    use crate::hopper::store::{HopperIntake, InMemoryHopper};
    use crate::hopper::types::{IntakeSource, PriorityHint};

    #[tokio::test]
    async fn admit_enqueues_one_task() {
        let bus = Arc::new(EventBus::new(16));
        let rx = bus.subscribe();
        let hopper = InMemoryHopper::new(bus.clone());
        let enqueued = Arc::new(Mutex::new(Vec::new()));
        let sink = enqueued.clone();
        let handle = tokio::spawn(run_dispatcher(rx, move |t| sink.lock().unwrap().push(t), Some(1)));
        hopper.submit("t".into(), vec![], PriorityHint::Normal, IntakeSource::Developer, None).await;
        handle.await.unwrap();
        assert_eq!(enqueued.lock().unwrap().len(), 1);
    }
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator admit_enqueues_one_task` → FAIL (no `from_admitted`, possibly `subscribe`/event field mismatches).

- [ ] **Step 4: Implement.** (a) Add the real `AgentTask::from_admitted(...)` (or reuse `intake_to_task` by looking the item up — pick the simpler one matching the queue API confirmed in Task 2). **VERIFIED:** `AgentTask::new(id, description, priority, file_manifest)` exists (`crates/vox-orchestrator/src/types/tasks.rs`) — match its real arity. (b) Fix the `HopperItemAdmitted` destructure to the real fields (VERIFIED: `{ item_id, classified_priority, classified_affinity, confidence, session_id }`) — the `..` rest-pattern in the test already tolerates the extras. (c) In the `Orchestrator` struct: add field `hopper: Arc<dyn crate::hopper::store::HopperIntake>`, construct it with the bus in `new()`, and after construction `tokio::spawn(run_dispatcher(self.event_bus.subscribe(), enqueue_closure, None))` (VERIFIED: the struct's field is `event_bus`, not `bus`) where `enqueue_closure` enqueues onto the least-loaded agent's queue. Add a `pub fn hopper(&self) -> Arc<dyn HopperIntake>` accessor. **ORDERING (avoids a lost-event race):** call `self.event_bus.subscribe()` and spawn the dispatcher **before** the rehydrate step (Task 5) and before any external `submit()` can run, so no admit emitted at boot is missed.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator admit_enqueues_one_task` → PASS; `cargo test -p vox-orchestrator` → all PASS.

- [ ] **Step 6: Verify + commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/orchestrator.rs crates/vox-orchestrator/src/orchestrator/dispatch.rs
git commit -m "feat(orchestrator): wire hopper + spawn admit->enqueue dispatcher"
```

---

## Task 4 `[SEQUENTIAL]`: Cascade reprioritize + cancel to live agent queues

Override/cancel in the hopper now reorders/removes the running agent's queued task.

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/dispatch.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "fn reorder|fn remove|fn cancel|fn dequeue" crates/vox-orchestrator/src/queue/mod.rs`. Confirm a reorder/remove-by-id API exists; if only `reorder` exists, use it; inline its signature into Step 4. Run `rg -n "HopperItemOverridden" -A 6 crates/vox-orchestrator/src/events.rs` to confirm field names (`item_id`, `developer_priority`, …).

- [ ] **Step 2: Write the failing test.** Add to `dispatcher_tests`:

```rust
    #[tokio::test]
    async fn override_event_triggers_reprioritize_callback() {
        use crate::events::{AgentEvent, AgentEventKind};
        let bus = Arc::new(EventBus::new(16));
        let rx = bus.subscribe();
        let reprioritized = Arc::new(Mutex::new(Vec::new()));
        let sink = reprioritized.clone();
        let handle = tokio::spawn(run_cascade(rx, move |id, _p| sink.lock().unwrap().push(id), Some(1)));
        bus.emit(AgentEventKind::HopperItemOverridden {
            item_id: Default::default(),
            original_priority: crate::types::TaskPriority::Normal,
            developer_priority: crate::types::TaskPriority::Urgent,
            delta_seconds_since_admit: 0,
        });
        handle.await.unwrap();
        assert_eq!(reprioritized.lock().unwrap().len(), 1);
    }
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator override_event_triggers_reprioritize_callback` → FAIL (`run_cascade` missing; fix the event literal to the real field set).

- [ ] **Step 4: Implement `run_cascade`.** Mirror `run_dispatcher`, matching `HopperItemOverridden { item_id, developer_priority, .. }` → call an `on_reprioritize(item_id, developer_priority)` closure. **Verified queue API** (confirm in Step 1): the per-agent queue exposes `reorder(task_id: TaskId, new_priority: TaskPriority) -> bool` and `cancel(task_id: TaskId) -> Option<AgentTask>` (there is **no** `remove`). So the orchestrator wires `on_reprioritize` → `queue.reorder(id, prio)` and `on_cancel` → `queue.cancel(id)`. In the `Orchestrator`, spawn `run_cascade` alongside `run_dispatcher` with closures bound to the agent queues.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator override_event_triggers_reprioritize_callback` → PASS; full suite PASS.

- [ ] **Step 6: Verify + commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/orchestrator/dispatch.rs crates/vox-orchestrator/src/orchestrator.rs
git commit -m "feat(orchestrator): cascade hopper override/cancel to agent queues"
```

---

## Task 5 `[SEQUENTIAL]`: Persistent `SqliteHopper` (Hp-T5) + rehydrate on boot

Durable storage behind the existing trait; inbox survives restart.

**Files:**
- Create: `crates/vox-orchestrator/src/hopper/sqlite_store.rs`
- Modify: a `vox-db` schema domain file (add `hopper_inbox` table) + `crates/vox-orchestrator/src/hopper/mod.rs` (declare module)

- [ ] **Step 1 (verify-before-use):** Run `rg -n "CREATE TABLE|fn migrations|pub fn schema" crates/vox-db/src/schema/domains/` to find how a table/migration is registered (copy an existing domain's pattern). Run `rg -n "pub fn open|pub struct .*Db|connection\(\)" crates/vox-db/src/lib.rs` to confirm how a DB handle is obtained. Inline the real connection type into Step 4.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-orchestrator/src/hopper/sqlite_store.rs`:

```rust
//! Persistent HopperIntake backed by vox-db `hopper_inbox` (Hp-T5).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hopper::store::HopperIntake;
    use crate::hopper::types::{IntakeSource, PriorityHint};

    #[tokio::test]
    async fn submit_then_reload_preserves_inbox() {
        // VERIFIED ctor (vox-db has NO open_in_memory): needs the `local` feature.
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db");
        let hopper = SqliteHopper::new(db.clone());
        hopper.submit("persist me".into(), vec![], PriorityHint::Normal, IntakeSource::Developer, None).await;
        // Drop and rebuild over the same DB to simulate a restart.
        let reloaded = SqliteHopper::new(db);
        let inbox = reloaded.inbox().await;
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].intent, "persist me");
    }
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator submit_then_reload_preserves_inbox` → FAIL (`SqliteHopper` missing).

- [ ] **Step 4: Implement.** (a) Add the `hopper_inbox` table (columns: `item_id TEXT PRIMARY KEY, intent TEXT, affinity_json TEXT, priority INTEGER, source TEXT, session_id TEXT, state TEXT, submitted_at INTEGER`) via the migration pattern from Step 1. (b) Implement `SqliteHopper { db }` and `impl HopperIntake` writing-through to the table for every method (`submit`/`assign`/`complete`/`cancel`/`reprioritize` UPDATE rows; `inbox`/`assigned`/`history` SELECT by `state`; `replay_admitted` INSERT-OR-IGNORE on `item_id`). Emit the same bus events as `InMemoryHopper` on `submit`/`reprioritize`. No `.unwrap()` in lib code — propagate `Result`/log.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator submit_then_reload_preserves_inbox` → PASS.

- [ ] **Step 6: Rehydrate-on-boot + commit.** On boot, call `hopper.inbox()` and feed each item through the enqueue path **using the idempotent enqueue** to avoid double-dispatch. **VERIFIED:** the queue exposes `enqueue_dedup(...)` (see `crates/vox-orchestrator/src/queue/priority.rs`) — use it (not plain `enqueue`) for rehydration so a rapid crash-restart-restart never queues the same item twice. Add a test that rehydrates the **same** persisted item twice and asserts the agent queue depth is 1, not 2. Verify + commit:

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator vox-db
git add crates/vox-orchestrator/src/hopper/sqlite_store.rs crates/vox-orchestrator/src/hopper/mod.rs crates/vox-db/src/schema/
git commit -m "feat(hopper): persistent SqliteHopper (Hp-T5) + rehydrate on boot"
```

---

## Task 6 `[SEQUENTIAL]`: Mesh ops for override + transition

Replicate priority changes and state transitions, not just admissions.

**Files:**
- Modify: `crates/vox-orchestrator/src/hopper/mesh_adapter.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "enum HopperOpSync|ItemOverridden|ItemTransitioned|UnsupportedOpVariant" -A 4 crates/vox-orchestrator/src/hopper/mesh_adapter.rs`. Confirm the op variants exist and that `apply_op_fragment` currently returns `UnsupportedOpVariant` for them. Read `replay_admitted` apply path to mirror it.

- [ ] **Step 2: Write the failing test.** Add to `mesh_adapter.rs` tests: emit an `ItemOverridden` op into `apply_op_fragment` (with a valid signed envelope per the existing test helper) against a hopper holding the item, and assert the item's `classified_priority` updated and **no** error returned.

```rust
    #[tokio::test]
    async fn apply_item_overridden_updates_priority() {
        // build hopper + admit an item (reuse the existing test helper that mints a valid envelope)
        // emit ItemOverridden op; apply; assert priority == Urgent and Ok(())
    }
```

(Fill the body using the existing `apply_op_fragment` test in this file as the template — copy its envelope/signature setup verbatim.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator apply_item_overridden_updates_priority` → FAIL (`UnsupportedOpVariant`).

- [ ] **Step 4: Implement.** In `apply_op_fragment`, add match arms for `ItemOverridden` → apply priority to the local item (add a `replay_overridden` method on `HopperIntake` if needed, mirroring `replay_admitted`: idempotent, no local re-emit) and `ItemTransitioned` → apply state. Add the corresponding **outbound** emit: where `submit`/`reprioritize`/`cancel` already emit bus events, also publish the matching `HopperOpSync` op (guard behind the existing trust/transport gate so non-federated runs are unaffected).

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator apply_item_overridden_updates_priority` → PASS; full suite PASS.

- [ ] **Step 6: Verify + commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/hopper/mesh_adapter.rs crates/vox-orchestrator/src/hopper/store.rs
git commit -m "feat(hopper): mesh replication for override + transition ops"
```

---

## Task 7 `[SEQUENTIAL]`: Wire the federation transport caller

The apply seam exists but nothing calls it on inbound envelopes. Wire it.

**Files:**
- Modify: the federation transport file (found below)

- [ ] **Step 1 (verify-before-use):** Run `rg -n "OpFragmentEnvelope|apply_op_fragment|federation|inbound" crates/vox-orchestrator/src/ crates/*/src/ -l`. Identify the transport that receives `OpFragmentEnvelope`s off the wire. If **no** inbound transport exists yet (design notes P6-T9 pending), STOP and report: this task depends on the federation transport; mark it blocked and proceed to Task 8 (the local cascade is fully functional without mesh). Do NOT scaffold a transport from scratch here.

- [ ] **Step 2–6:** Only if a transport exists: write a test that feeds an inbound envelope and asserts `apply_op_fragment` was invoked (item appears in the peer hopper), implement the call, verify, commit:

```bash
git commit -m "feat(mesh): invoke hopper apply_op_fragment on inbound federation envelopes"
```

If blocked, record the block in the handoff note and continue.

---

## Task 8 `[PARALLEL-SAFE]` (Tauri layer, disjoint from Task 9): `hopper_*` commands

Expose submit/reprioritize/cancel/list to the GUI.

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs` (+ command registration in the Tauri builder file)

- [ ] **Step 1 (verify-before-use):** Run `rg -n "emit_tasks_changed|#\[tauri::command\]|generate_handler" crates/vox-gui/src/commands/orchestrator.rs crates/vox-gui/src/main.rs`. Confirm `emit_tasks_changed` exists (it does) and find the **`tauri::generate_handler![...]` list in `main.rs`** (commands register there, not an `invoke_handler!` in `commands/mod.rs`). Run `rg -n "DeveloperOverrideMint|fn mint" crates/vox-orchestrator/src/hopper/capability.rs` to confirm how to mint a `DeveloperOverride` for `reprioritize`.

- [ ] **Step 2: Write the failing test.** Add a Rust unit test (in `commands/orchestrator.rs` test module) for a pure DTO mapping function `hopper_item_to_dto(&IntakeItem) -> HopperTaskDto` (fields: `item_id, intent, priority, state`). Assert it maps a sample item correctly.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-gui hopper_item_to_dto` → FAIL.

- [ ] **Step 4: Implement.** Add `HopperTaskDto` + `hopper_item_to_dto`. Add `#[tauri::command]` fns: `hopper_list() -> Vec<HopperTaskDto>` (inbox+assigned), `hopper_submit(intent, affinity) -> HopperTaskDto`, `hopper_reprioritize(item_id, priority)` (mints `DeveloperOverride`), `hopper_cancel(item_id)`. Each mutating command calls `emit_tasks_changed(&app_handle)` after success. Register all four in the `tauri::generate_handler!` list in `main.rs`.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-gui hopper_item_to_dto` → PASS; `cargo check -p vox-gui` compiles.

- [ ] **Step 6: Verify + commit.**

```bash
cargo clippy -p vox-gui --lib -- -D warnings && cargo fmt -p vox-gui
git add crates/vox-gui/src/
git commit -m "feat(gui): hopper submit/reprioritize/cancel/list Tauri commands"
```

> **Note:** `vox-gui` breaks `clippy --all-targets` via its Tauri build script — use `--lib` (see project feedback). Exclude `vox-gui` from any workspace clippy sweep.

---

## Task 9 `[PARALLEL-SAFE]` (frontend, disjoint from Task 8): editable `TasksView`

User can type a task, set priority, cancel; list refreshes reactively.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/**/Tasks*.tsx` (the Tasks surface found below)
- Test: a sibling `*.test.tsx`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "tasks-changed|TasksView|hopper_list|invoke\(" crates/vox-gui/ui/src/ -l` to find the Tasks surface component and how it currently lists tasks + subscribes to `vox://tasks-changed`. Read it to match patterns (the `invoke()` wrapper, the event-subscribe hook).

- [ ] **Step 2: Write the failing test.** Create `Tasks.composer.test.tsx`:

```tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { TaskComposer } from './TaskComposer';

describe('TaskComposer', () => {
  it('submits typed intent via onSubmit', () => {
    const onSubmit = vi.fn();
    render(<TaskComposer onSubmit={onSubmit} />);
    fireEvent.change(screen.getByPlaceholderText(/add a task/i), { target: { value: 'ship it' } });
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    expect(onSubmit).toHaveBeenCalledWith('ship it');
  });
});
```

- [ ] **Step 3: Run → FAIL.** `npm test -- Tasks.composer` (from `crates/vox-gui/ui`) → FAIL (`TaskComposer` missing).

- [ ] **Step 4: Implement.** Create `TaskComposer.tsx` (controlled textarea + Add button calling `onSubmit(text)` then clearing). In the Tasks surface: wire `onSubmit` → `invoke('hopper_submit', { intent, affinity: [] })`; add per-row priority `<select>` → `invoke('hopper_reprioritize', ...)` and a Cancel button → `invoke('hopper_cancel', ...)`; load rows via `invoke('hopper_list')`; subscribe to `vox://tasks-changed` to re-call `hopper_list` (reuse the existing subscribe hook). Honor the design-system components (`Button`, `Glass`, status tones).

- [ ] **Step 5: Run → PASS.** `npm test -- Tasks.composer` → PASS; `npm run build` (tsc) clean.

- [ ] **Step 6: Verify + commit.**

```bash
git add crates/vox-gui/ui/src/components/surfaces/
git commit -m "feat(gui): editable TasksView composer + priority/cancel + reactive refresh"
```

---

## Parallel waves

- **Wave 1 (sequential, backend spine):** Tasks 1 → 2 → 3 → 4 → 5 → 6 → 7 (each modifies overlapping orchestrator files; run in order on one agent).
- **Wave 2 (parallel):** Task 8 (Tauri/Rust) and Task 9 (React/TS) touch disjoint files — dispatch together after Wave 1.

## Self-review checklist (run after execution, before final handoff)

- [ ] Spec §3 components all have a task (cancel, mapper, dispatcher, cascade, sqlite, mesh ops, transport, Tauri, GUI). ✔ Tasks 1–9.
- [ ] No placeholder steps; every code step shows code or an exact `rg`/verify command. ✔
- [ ] Symbol names consistent across tasks (`HopperIntake`, `intake_to_task`, `run_dispatcher`/`run_cascade`, `SqliteHopper`, `hopper_*` commands, `emit_tasks_changed`). ✔
- [ ] Task 7 has an explicit blocked-path if no transport exists (avoids scaffolding from scratch). ✔
