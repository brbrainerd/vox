# Activity-Log Surface — Implementation Plan (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `crates/vox-skills/skills/superpowers/test-driven-development.skill.md`. Steps use `- [ ]` checkboxes.

> **🤖 EXECUTION TARGET — READ FIRST.** Gemini 3.5 Flash inside Google Antigravity (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context). Plan engineered accordingly. Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md). Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** A crash between tasks leaves a compiling, tested tree.
2. **Verify-before-use.** `rg`/read before referencing any symbol/path; if reality differs, STOP and report.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Two failures → STOP + handoff note.
5. **Parallel dispatch.** Honor `[PARALLEL-SAFE]`/`[SEQUENTIAL]`; never two subagents on one file.
6. **Vox house rules.** Never `cargo fmt --all` (`cargo fmt -p <crate>`); `.vox` automation only; `docs/src/` `.md` needs frontmatter.
7. **Verification ritual** before commit: `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`, paste output. (`vox-gui`: clippy `--lib` only.)
8. **Rollback on broken tree:** `git reset --hard HEAD` to last green, re-attempt the one task.
9. **Rust constraints:** no `.unwrap()` in lib code; inject params in tests; deterministic; `cargo run -p vox-arch-check` passes.

**Goal:** A persisted, filterable agent-activity timeline (distinct from chat): a vox-db `activity_log` table, a sink that writes the high-signal subset of `EventBus` events to it, and a dedicated GUI Activity surface.

**Architecture:** A dedicated `EventBus` subscriber (`ActivitySink`) projects a curated allowlist of `AgentEventKind` into `activity_log` rows; a Tauri `activity_query` reads them paged/filtered; a React Activity surface renders the timeline. The sink is just one more (lossy-tolerant) subscriber, preserving the bus's liveness contract.

**Tech Stack:** Rust (`vox-orchestrator`, `vox-db`); Tauri (`vox-gui`); React/TS + vitest.

**Design:** [`../specs/2026-06-18-activity-log-surface-design.md`](../specs/2026-06-18-activity-log-surface-design.md).

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-db/src/schema/domains/*` | `activity_log` table | Modify/Create (Task 1) |
| `crates/vox-orchestrator/src/activity/mod.rs` | `is_loggable` allowlist | Create (Task 2) |
| `crates/vox-orchestrator/src/activity/project.rs` | kind → `(summary, detail_json)` | Create (Task 3) |
| `crates/vox-orchestrator/src/activity/sink.rs` | subscribe bus → insert rows | Create (Task 4) |
| `crates/vox-gui/src/commands/activity.rs` | `activity_query` + `vox://activity-appended` | Create (Task 5) |
| `crates/vox-gui/ui/src/components/surfaces/Activity/*` | timeline + filter bar | Create (Task 6) |
| `contracts/gui/surface-registry.v1.yaml` | register `activity` surface | Modify (Task 6) |

**Pre-flight (run once, paste output):**
- `rg -n "pub enum AgentEventKind" -A 5 crates/vox-orchestrator/src/events.rs` — confirm the enum + a few variant shapes (`AgentSpawned`, `TaskCompleted`, `CostIncurred`, `AgentHeartbeat`, `ThroughputTick`).
- `rg -n "pub struct AgentEvent|timestamp_ms|pub fn subscribe" crates/vox-orchestrator/src/events.rs` — confirm `AgentEvent { id, timestamp_ms, kind }` + `EventBus::subscribe()`.
- `rg -n "CREATE TABLE|fn migrations|register" crates/vox-db/src/schema/domains/execution.rs` — copy the table-registration pattern.
- DB handle ctor for tests is **`vox_db::VoxDb::connect(vox_db::DbConfig::Memory)`** (VERIFIED — there is no `open_in_memory`; handle type alias is `Codex`; tests need the `local` feature). Confirm with `rg -n "VoxDb::connect\(DbConfig::Memory\)" crates/vox-db/src/local_tests.rs`.
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task 1 `[SEQUENTIAL]`: `activity_log` table

**Files:**
- Modify/Create: a `vox-db` schema domain file registering `activity_log`

- [ ] **Step 1 (verify-before-use):** Read `crates/vox-db/src/schema/domains/execution.rs` (from Pre-flight). Copy how an existing table's `CREATE TABLE` + indexes are declared and registered into the schema. Note the exact registration call.

- [ ] **Step 2: Write the failing test.** Add a test (in the domain file's test module) that opens an in-memory DB, runs migrations, and asserts inserting + selecting a row from `activity_log` works:

```rust
#[tokio::test]
async fn activity_log_round_trip() {
    // VERIFIED ctor (vox-db has NO open_in_memory): needs the `local` feature.
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db");
    db.execute(
        "INSERT INTO activity_log (ts_ms, agent_id, session_id, kind, summary, detail_json)
         VALUES (1000, 'A1', 's1', 'TaskCompleted', 'done', '{}')", ()
    ).await.expect("insert");
    let rows = db.query("SELECT kind FROM activity_log WHERE agent_id='A1'", ()).await.expect("q");
    assert_eq!(rows.len(), 1);
}
```

(Replace `execute`/`query` with the real vox-db API confirmed in Step 1.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-db activity_log_round_trip` → FAIL (no such table).

- [ ] **Step 4: Implement.** Register the table using the Step-1 pattern:

```sql
CREATE TABLE IF NOT EXISTS activity_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts_ms       INTEGER NOT NULL,
    agent_id    TEXT,
    session_id  TEXT,
    kind        TEXT NOT NULL,
    summary     TEXT NOT NULL,
    detail_json TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_activity_ts   ON activity_log(ts_ms);
CREATE INDEX IF NOT EXISTS idx_activity_agent ON activity_log(agent_id);
CREATE INDEX IF NOT EXISTS idx_activity_kind ON activity_log(kind);
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-db activity_log_round_trip` → PASS.

- [ ] **Step 6: Verify + commit.**

```bash
cargo clippy -p vox-db -- -D warnings && cargo fmt -p vox-db
git add crates/vox-db/src/schema/
git commit -m "feat(db): activity_log table + indexes"
```

---

## Task 2 `[PARALLEL-SAFE]` (new file): `is_loggable` allowlist

**Files:**
- Create: `crates/vox-orchestrator/src/activity/mod.rs`
- Modify: orchestrator module-decl file (`pub mod activity;`)

- [ ] **Step 1 (verify-before-use):** From Pre-flight, confirm the exact variant names for: `AgentSpawned`, `TaskCompleted`, `TaskFailed`, `TaskPhaseChanged`, `CostIncurred`, `AgentHeartbeat`, `ThroughputTick`, `CostTick`, `FileDiagChanged`. If any differ, use the real names in Step 4.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-orchestrator/src/activity/mod.rs`:

```rust
//! Activity-log allowlist: which AgentEventKind variants persist to activity_log.

use crate::events::AgentEventKind;

/// SSOT: true for high-signal lifecycle/resource/build events; false for
/// high-frequency telemetry (heartbeats, throughput/cost ticks, file-diag churn).
pub fn is_loggable(kind: &AgentEventKind) -> bool {
    // Filled in Step 4.
    let _ = kind;
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::AgentEventKind;

    #[test]
    fn lifecycle_is_logged_telemetry_is_not() {
        assert!(is_loggable(&AgentEventKind::AgentSpawned { agent_id: Default::default(), name: "x".into() }));
        assert!(!is_loggable(&AgentEventKind::AgentHeartbeat { agent_id: Default::default(), activity: "x".into(), active_skill: None }));
    }
}
```

(Adjust the two event literals to the real field sets from Step 1.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator lifecycle_is_logged_telemetry_is_not` → FAIL.

- [ ] **Step 4: Implement.** Replace the body with an explicit allowlist `match`:

```rust
pub fn is_loggable(kind: &AgentEventKind) -> bool {
    use AgentEventKind::*;
    matches!(kind,
        AgentSpawned { .. } | AgentRetired { .. }
        | TaskSubmitted { .. } | TaskStarted { .. } | TaskPhaseChanged { .. }
        | TaskCompleted { .. } | TaskFailed { .. } | TaskReprioritized { .. }
        | TaskDelegated { .. } | PlanHandoff { .. }
        | CostIncurred { .. } | BudgetAlert { .. } | AttentionBudgetAlert { .. }
        | LockAcquired { .. } | LockReleased { .. } | ConflictDetected { .. }
        | BuildStage { .. } | MeshTopologyChanged { .. }
        | WorkflowStarted { .. } | WorkflowCompleted { .. } | WorkflowFailed { .. }
    )
    // High-frequency telemetry deliberately excluded:
    // AgentHeartbeat, ThroughputTick, CostTick, FileDiagChanged → false (fall-through).
}
```

Remove any arm whose variant name didn't verify in Step 1; do not invent variants.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator lifecycle_is_logged_telemetry_is_not` → PASS.

- [ ] **Step 6: Declare module + commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/activity/mod.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(activity): is_loggable allowlist SSOT"
```

---

## Task 3 `[PARALLEL-SAFE]` (new file): event → row projection

**Files:**
- Create: `crates/vox-orchestrator/src/activity/project.rs`
- Modify: `crates/vox-orchestrator/src/activity/mod.rs` (`pub mod project;`)

- [ ] **Step 1 (verify-before-use):** Confirm field names for `TaskCompleted` and `CostIncurred` (`agent_id`, `task_id`, `cost_usd`, `model`, …) via `rg -n "TaskCompleted|CostIncurred" -A 4 crates/vox-orchestrator/src/events.rs`.

- [ ] **Step 2: Write the failing test.** Create `project.rs`:

```rust
//! Project an AgentEventKind into an activity_log row (summary + detail json).
use crate::events::AgentEventKind;

pub struct ActivityRow {
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub kind: String,
    pub summary: String,
    pub detail_json: String,
}

pub fn project(kind: &AgentEventKind) -> ActivityRow { let _ = kind; unimplemented!() }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::AgentEventKind;
    #[test]
    fn task_completed_projects_summary() {
        let row = project(&AgentEventKind::TaskCompleted {
            task_id: Default::default(), agent_id: Default::default(),
            session_id: None, audit_report: None,
        });
        assert_eq!(row.kind, "TaskCompleted");
        assert!(row.summary.to_lowercase().contains("completed"));
    }

    #[test]
    fn cost_incurred_projects_with_real_fields() {
        // VERIFIED field set — CostIncurred is NOT just agent_id/cost_usd:
        let row = project(&AgentEventKind::CostIncurred {
            agent_id: Default::default(),
            provider: "anthropic".into(),
            model: "claude-opus".into(),
            input_tokens: 1000,
            output_tokens: 500,
            cost_usd: 0.01,
            temporal_context: None,
        });
        assert_eq!(row.kind, "CostIncurred");
    }
}
```

(Match the real `TaskCompleted`/`CostIncurred` field sets confirmed in Step 1; the `CostIncurred` set above is verified.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator task_completed_projects_summary` → FAIL.

- [ ] **Step 4: Implement.** Write `project()` as a `match` mapping each loggable variant to `{kind: "<VariantName>", summary: "<human one-liner>", detail_json: serde_json::to_string(...)}` and a catch-all `_ => ActivityRow { kind: "Other".into(), summary: format!("{kind:?}"), .. }`. Extract `agent_id`/`session_id` where the variant has them.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator task_completed_projects_summary` → PASS.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/activity/
git commit -m "feat(activity): AgentEventKind -> ActivityRow projection"
```

---

## Task 4 `[SEQUENTIAL]` (depends on Tasks 1–3): `ActivitySink`

**Files:**
- Create: `crates/vox-orchestrator/src/activity/sink.rs`
- Modify: orchestrator constructor (spawn the sink) + `activity/mod.rs`

- [ ] **Step 1 (verify-before-use):** Confirm `EventBus::subscribe()` returns `broadcast::Receiver<AgentEvent>` and how the orchestrator already spawns background tasks (`rg -n "tokio::spawn" crates/vox-orchestrator/src/orchestrator.rs`). Confirm the vox-db insert API.

- [ ] **Step 2: Write the failing test.** Create `sink.rs`:

```rust
//! Drains the EventBus and persists loggable events to activity_log.
use crate::activity::{is_loggable, project::project};
use crate::events::AgentEvent;

/// Run the sink. `insert` persists one row; `max_events` bounds the loop for tests.
pub async fn run_sink(
    mut rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    insert: impl Fn(crate::activity::project::ActivityRow, u64) + Send + 'static,
    max_events: Option<usize>,
) {
    let mut seen = 0usize;
    while let Ok(ev) = rx.recv().await {
        if is_loggable(&ev.kind) {
            insert(project(&ev.kind), ev.timestamp_ms);
            seen += 1;
            if Some(seen) == max_events { break; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use crate::events::{AgentEventKind, EventBus};

    #[tokio::test]
    async fn sink_persists_only_loggable() {
        let bus = Arc::new(EventBus::new(16));
        let rx = bus.subscribe();
        let rows = Arc::new(Mutex::new(Vec::new()));
        let sink = rows.clone();
        let h = tokio::spawn(run_sink(rx, move |r, _ts| sink.lock().unwrap().push(r.kind), Some(1)));
        bus.emit(AgentEventKind::AgentHeartbeat { agent_id: Default::default(), activity: "x".into(), active_skill: None }); // dropped
        bus.emit(AgentEventKind::AgentSpawned { agent_id: Default::default(), name: "n".into() });                          // logged
        h.await.unwrap();
        assert_eq!(rows.lock().unwrap().as_slice(), &["AgentSpawned".to_string()]);
    }
}
```

(Adjust event literals to real field sets.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator sink_persists_only_loggable` → FAIL.

- [ ] **Step 4: Implement.** Confirm `run_sink` compiles against real types. **VERIFIED CONSTRAINT:** `Orchestrator::new()` is **synchronous** and the DB field is `db: Arc<RwLock<Option<...>>>` set to `None` at construction (DB only becomes available after `init_db()`). Therefore do **NOT** spawn the sink in the constructor — it cannot `tokio::spawn` (sync) and has no DB yet. Instead spawn it in `init_db()` **after** `self.db` is set to `Some(handle)`: `tokio::spawn(run_sink(self.event_bus.subscribe(), insert_closure, None))`, where `insert_closure` captures the now-available DB handle and writes each row into `activity_log` (write failures: log + drop, never panic). First run `rg -n "pub.*fn init_db|self.db" crates/vox-orchestrator/src/` to confirm the method name + where the handle is set.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator sink_persists_only_loggable` → PASS; full suite PASS.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/activity/sink.rs crates/vox-orchestrator/src/activity/mod.rs crates/vox-orchestrator/src/orchestrator.rs
git commit -m "feat(activity): ActivitySink subscribes bus and persists activity_log"
```

---

## Task 5 `[PARALLEL-SAFE]` (Tauri, disjoint from Task 6): `activity_query`

**Files:**
- Create: `crates/vox-gui/src/commands/activity.rs`
- Modify: command registration (`invoke_handler!`) + `commands/mod.rs`

- [ ] **Step 1 (verify-before-use):** **VERIFIED:** Tauri commands are registered in `crates/vox-gui/src/main.rs` via `tauri::generate_handler![...]` (the command module does not need pre-declaration there). Find the list with `rg -n "generate_handler!" crates/vox-gui/src/main.rs`, and add `pub mod activity;` to `crates/vox-gui/src/commands/mod.rs`. Confirm the DB access pattern used by an existing read command.

- [ ] **Step 2: Write the failing test.** Create `activity.rs` with a pure filter→SQL builder + test:

```rust
pub struct ActivityFilter { pub agent_id: Option<String>, pub kind: Option<String>, pub limit: u32, pub before_id: Option<i64> }

/// Build a parameterized WHERE clause (returns SQL fragment + bind order).
pub fn build_where(f: &ActivityFilter) -> String {
    let _ = f; String::new() // Step 4
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filter_by_agent_and_kind() {
        let sql = build_where(&ActivityFilter { agent_id: Some("A1".into()), kind: Some("TaskCompleted".into()), limit: 50, before_id: None });
        assert!(sql.contains("agent_id = ?"));
        assert!(sql.contains("kind = ?"));
    }
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-gui filter_by_agent_and_kind` → FAIL.

- [ ] **Step 4: Implement.** Implement `build_where` (compose `agent_id = ?`, `kind = ?`, `id < ?` clauses joined by `AND`, empty if none). Add `#[tauri::command] async fn activity_query(filter) -> Vec<ActivityRowDto>` running `SELECT ... FROM activity_log {where} ORDER BY id DESC LIMIT ?`. Add a `pub const ACTIVITY_APPENDED_EVENT: &str = "vox://activity-appended";` and emit it from the sink path (or leave a `emit_activity_appended` helper for the orchestrator bridge). Register the command in `main.rs`'s `generate_handler!` list.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-gui filter_by_agent_and_kind` → PASS; `cargo check -p vox-gui`.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-gui --lib -- -D warnings && cargo fmt -p vox-gui
git add crates/vox-gui/src/commands/
git commit -m "feat(gui): activity_query Tauri command + vox://activity-appended"
```

---

## Task 6 `[PARALLEL-SAFE]` (frontend, disjoint from Task 5): Activity surface

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Activity/ActivitySurface.tsx` (+ `.test.tsx`)
- Modify: `contracts/gui/surface-registry.v1.yaml`; the view registration (`App.tsx` `View` union + `surfaceComponents.tsx`)

- [ ] **Step 1 (verify-before-use):** `rg -n "view_key:|surfaces:" contracts/gui/surface-registry.v1.yaml | head` to copy a surface entry shape (e.g. the `approvals` entry). **VERIFIED registration points:** (1) the `View` string-literal union in `App.tsx` (add `'activity'`); (2) the **`childRenderer`** switch in `surfaceComponents.tsx` (add `case 'activity': return <ActivitySurface .../>;`) — note the function is `childRenderer`, not `renderSurfaceView`; there is also a decorator-registry override path you can ignore for a simple surface. Confirm with `rg -n "type View|childRenderer|case '" crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`.

- [ ] **Step 2: Write the failing test.** Create `ActivitySurface.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { ActivityTimeline } from './ActivitySurface';

describe('ActivityTimeline', () => {
  it('renders rows newest-first with kind + summary', () => {
    render(<ActivityTimeline rows={[
      { id: 2, ts_ms: 2000, agent_id: 'A1', kind: 'TaskCompleted', summary: 'done' },
      { id: 1, ts_ms: 1000, agent_id: 'A1', kind: 'AgentSpawned', summary: 'spawned' },
    ]} />);
    const items = screen.getAllByTestId('activity-row');
    expect(items[0]).toHaveTextContent('TaskCompleted');
    expect(items[1]).toHaveTextContent('AgentSpawned');
  });
});
```

- [ ] **Step 3: Run → FAIL.** `npm test -- ActivitySurface` → FAIL.

- [ ] **Step 4: Implement.** `ActivityTimeline` (renders rows, each `data-testid="activity-row"`, status-toned by kind) + `ActivitySurface` (filter bar: agent/kind selects; loads via `invoke('activity_query', {filter})`; subscribes to `vox://activity-appended` to prepend; `EmptyState` when zero). Register `activity` in `surface-registry.v1.yaml`, the `View` union (`App.tsx`), and the `childRenderer` switch (`surfaceComponents.tsx`).

- [ ] **Step 5: Run → PASS.** `npm test -- ActivitySurface` → PASS; `npm run build` clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Activity/ crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx contracts/gui/surface-registry.v1.yaml
git commit -m "feat(gui): Activity timeline surface (filterable, reactive)"
```

---

## Task 7 `[PARALLEL-SAFE]` (frontend): cost-row client-side fold

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Activity/ActivitySurface.tsx` (+ test)

- [ ] **Step 1 (verify-before-use):** Read your own `ActivityTimeline` from Task 6.

- [ ] **Step 2: Write the failing test.** Add: given 3 consecutive `CostIncurred` rows for agent A1, the timeline renders **one** folded row labeled "spent … (3 calls)".

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement.** A pure `foldCostRuns(rows)` helper that collapses consecutive same-agent `CostIncurred` rows into one synthetic row with a `count`; render it expandable.

- [ ] **Step 5: Run → PASS.** `npm test -- ActivitySurface` → PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Activity/
git commit -m "feat(gui): fold consecutive cost rows in activity timeline"
```

---

## Parallel waves

- **Wave 1:** Task 1 `[SEQUENTIAL]` (db).
- **Wave 2 (parallel):** Task 2 + Task 3 (disjoint new files under `activity/`).
- **Wave 3:** Task 4 `[SEQUENTIAL]` (needs 1–3; touches orchestrator.rs).
- **Wave 4 (parallel):** Task 5 (Tauri/Rust) + Task 6 (React). Then Task 7 after 6.

## Self-review checklist

- [ ] Spec §4 components covered: table (1), allowlist (2), projection (3), sink (4), query+event (5), surface (6), cost-fold (7). ✔
- [ ] Noisy kinds (`AgentHeartbeat`/`ThroughputTick`/`CostTick`/`FileDiagChanged`) excluded by `is_loggable` and asserted in Task 2/4 tests. ✔
- [ ] Symbol consistency: `is_loggable`, `project`/`ActivityRow`, `run_sink`, `activity_query`, `ACTIVITY_APPENDED_EVENT`, `ActivityTimeline`. ✔
- [ ] No placeholders; every code step shows code or exact verify command. ✔
