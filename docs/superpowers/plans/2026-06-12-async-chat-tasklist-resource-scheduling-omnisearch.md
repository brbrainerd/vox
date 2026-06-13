# Async Chat, Task-List GUI, Resource-Aware Scaling & Omni-Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make queued orchestrator work fully visible and user-controllable in the GUI (list/add/edit/remove/reprioritize with icon controls), keep chat always-responsive with **multi-tab sessions** riding the existing `session_id` spine, detect near-duplicate incoming tasks with user-mediated merge/skip (never silent dedup), make the orchestrator scale agents up/down using local CPU/RAM and mesh resource broadcasts (and surface the already-built A2A remote delegation), add a user-configurable OpenRouter/LLM concurrency throttle, fix the audited settings/chat/search bugs, and turn the Cmd+K palette into a true omni-search (commands + agents + skills + settings + surfaces/windows + docs + backend corpora) with a sidebar filter — all settings flowing through the existing SSOT (`Vox.toml` `[orchestrator]`, `~/.vox/config.toml`, GUI DB prefs).

**Revision 2 (2026-06-12):** hand-verified against the codebase by four read-only audit passes — 9 plan errors corrected inline (serde casings, `NodeRecord.id`, `DispatchRequest.model_id`, sysinfo feature gate, `VoxConfig::load()` signature, `resolve_secret(SecretId)`, missing icons, YAML field names, lock patterns); Tracks I (multi-tab sessions), J (intake dedup), K (A2A visibility + composite relief) added. See the Verification addendum.

**Architecture:** The GUI task list is backed by the **live per-agent priority queues** (`Orchestrator::all_tasks()` + existing `orch.cancel_task`/`orch.reorder_task` daemon RPCs), NOT the unwired `hopper/` module — we add only two missing RPCs (`orch.list_tasks`, `orch.edit_task`) and Tauri wrappers. Scaling extends the existing `ScalingService` with a local-resource probe (sysinfo) and the existing `remote_populi_routing_hints` mesh signal; a new populi `GET /v1/populi/resources/summary` endpoint aggregates node broadcasts. LLM concurrency is a new AIMD throttle in `vox-actor-runtime::llm` (the mandated egress facade), configured via a new `[llm]` section in the config SSOT. Omni-search federates client-side sources (settings index, surface registry, docs frontmatter index) with the existing `vox_search_query` backend.

**Tech Stack:** Rust (tokio, axum for populi transport, Tauri 2, sysinfo), TypeScript/React 18 (vitest for unit tests), TOML config via `vox-config`/`VoxManifest`.

**Worktree:** Execute in this worktree (`.claude/worktrees/eager-haibt-b729ad`, branch `claude/eager-haibt-b729ad`).

---

## Audit Findings (verified 2026-06-12)

These drive the bug-fix tasks; each is fixed by the referenced task.

| # | Finding | Where | Fixed by |
|---|---------|-------|----------|
| 1 | Chat is already async (input never disabled, concurrent submits correlate via `agentToTask`) — **no fix needed**, but queued work is invisible | `Loquela.tsx`, `chatCorrelation.ts` | Track B (visibility) |
| 2 | `hopper/` module (Hp-T1) is never wired into the daemon — live state is the agent queues | `crates/vox-orchestrator/src/hopper/`, `orch_daemon/mod.rs` | Track A note (build on queues) |
| 3 | Loquela collects `mode` + `tier` but `handleLoquelaSubmit` drops both | `App.tsx:407-438`, `control_plane.rs:31-61` | C1 |
| 4 | `listModels()` fetched once on mount; tier selector goes stale | `Loquela.tsx:168` | C2 |
| 5 | Settings orchestrator sliders never hydrate from `Vox.toml` (hardcoded defaults) | `SettingsView.tsx:447` | F1 |
| 6 | Theme radio persists to DB but is never applied to the document | `SettingsView.tsx` theme section | F2 |
| 7 | No `[llm]` config section; no LLM concurrency control anywhere; 429 only classified in telemetry | `vox-actor-runtime/src/llm/chat.rs` | E1, E2 |
| 8 | `select_best_node` ranks by CPU% only; ignores `memory_free_bytes`, GPU allocatable, model locality | `vox-populi/src/transport/handlers/dispatch.rs:159-205` | D2 |
| 9 | `ScalingService` ignores local CPU/RAM; `scaling_enabled` defaults false with no GUI | `services/scaling.rs`, `config/orchestrator_fields.rs` | D3, F3 |
| 10 | No aggregated mesh resource API (clients must enumerate nodes) | `vox-populi/src/transport/` | D1 |
| 11 | CommandPalette: client items (agents/skills) excluded from keyboard selection; no wrap-around | `CommandPalette.tsx:84-97` | G1 |
| 12 | SearchView: arrow keys don't wrap; facet chips lack focus rings | `SearchView.tsx:348-357,42-77` | G4 |
| 13 | Sidebar: no filter/search, click-only | `Sidebar.tsx` | G3 |
| 14 | Settings not searchable; no settings/surfaces/docs in palette | `CommandPalette.tsx`, `SettingsView.tsx` | F4, G1, G2 |
| 15 | No duplicate-task detection anywhere — `AgentQueue::enqueue_dedup` exists (`queue/priority.rs:250`) but is **never called** in the live submit path; identical chat submissions enqueue twice | `task_submit.rs:52-138` | Track J |
| 16 | Sessions are half-built: `session_id` flows through `AgentTask` (`tasks.rs:408`), all task events (`events.rs:147-175`), and the A2A envelope — but the GUI hardcodes `session_id: 'gui-loquela'` (`App.tsx:421`) and has no tab/session UI | `App.tsx`, `chatCorrelation.ts` | Track I |
| 17 | A2A remote task distribution **already exists** (`a2a/envelope.rs` RemoteTaskEnvelope with idempotency/lease/session, `a2a/dispatch/{mesh,remote_poller,remote_worker}.rs`, hints fed by `mesh_federation_poll.rs`) but is invisible in the GUI and the scaling relief uses only a GPU count | `a2a/`, `runtime.rs:734-741` | Track K |

**Track independence:** A→B is the only hard chain (and I3/J/K touch Track A's `orch.list_tasks` output — execute A first). C, D, E, F, G are independently shippable; each track ends in a green-gate commit. If splitting work across sessions, treat each track as its own mini-plan.

## Verification addendum (hand-verified against code, 2026-06-12)

Every task below was audited by four read-only verification passes. Corrections are already folded into the task bodies, but these **global facts** apply across tasks — do not "fix" code that follows them:

1. **`TaskPriority` serializes Capitalized** (`"Urgent"|"Normal"|"Background"` — no `rename_all`, `tasks.rs:43-51`); its `Display` impl is lowercase. `task_lifecycle_status_label` returns `Option<String>` with labels `"Completed"|"InProgress"|"Blocked"|"Queued"`. The Tauri DTO layer (A3) normalizes both to lowercase/snake_case; the daemon JSON stays raw.
2. **`NodeRecord`'s id field is `id`, not `node_id`** (`node_record.rs:11`); `loaded_llm_models` is `Option<Vec<String>>`; no `Default` impl.
3. **vox-populi's `DispatchRequest`** (`transport/mod.rs:290-323`) already has a `model_id: Option<String>` field — use it for model-locality scoring; do NOT add a `preferred_model` field.
4. **sysinfo in vox-orchestrator is an optional dep** behind the `system-metrics` feature (`Cargo.toml`), and `orchestrator/scaling.rs:184-185` already uses it (`refresh_cpu_all()` + `refresh_memory()`); mirror that usage and feature gate.
5. **`VoxConfig::load()` returns `Self`, not `Result`** (`impl_ops.rs:17`). Section structs are named `<Name>TomlSection`. `save_merged_global_config(path: &Path, cfg: &VoxConfig) -> std::io::Result<()>` (`persist.rs:18`) merges `[vox] [train] [db] [web] [build]`.
6. **`vox_secrets::resolve_secret` takes a `SecretId` enum** (not `&str`) and returns `ResolvedSecret` (`vox-secrets/src/lib.rs:219`).
7. **Icons.tsx has no `edit`/`trash`/`list` keys** — available: `plus`, `refresh`, `search`, `command`, `link`, `x`, `clock`, `file`, `chevronUp`, `chevronDown`. Task B3 Step 0 adds the three missing icons.
8. **Surface-registry YAML field names** are `view_key`, `cli_group`, `representation_tier`, `nav_label`, `nav_icon`, `nav_group` (snake_case; generated TS camelCases them).
9. **`Orchestrator.agents`** is `Arc<RwLock<HashMap<AgentId, Arc<RwLock<AgentQueue>>>>>`; `AgentQueue::all_tasks_mut()` **includes the in-progress task** (chains `in_progress` + queued, `priority.rs:137+`), so mutation guards check `task.status`, not container membership.
10. **`TaskEnqueueHints` merge happens in `AgentTask::apply_hints`** (`tasks.rs:623-696`; `model_preference` merged at 630-631). `AgentTask` has no `Default`; adding a field breaks ~4 struct-literal sites (`complete/harness.rs:118`, `planning/test_decision.rs:100`, `queue/mod.rs:62`) plus `AgentTask::new` (`tasks.rs:475`).
11. **`handleLoquelaSubmit` wraps args as `{ input: { … } }`** (`App.tsx:407-438`) and Loquela's `send()` payload already carries `mode` and `tier` (`Loquela.tsx:282-292`); App currently drops them and hardcodes `session_id: 'gui-loquela'` (line 421).
12. **No theme CSS hooks exist** — `tailwind.config.js` has static colors only (`brass: '#d4af37'` at line 8); F2 introduces the CSS-variable hook.

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vox-foundation/src/protocol.rs` | Modify | Add `LIST_TASKS`, `EDIT_TASK` method constants |
| `crates/vox-orchestrator/src/orchestrator/accessors.rs` | Modify | `edit_task_description()` |
| `crates/vox-orchestrator/src/orch_daemon/mod.rs` | Modify | Dispatch arms for the two new methods |
| `crates/vox-gui/src/commands/control_plane.rs` | Modify | 4 new Tauri commands (list/edit/cancel/reorder) |
| `crates/vox-gui/src/main.rs` | Modify | Register new commands |
| `contracts/gui/surface-registry.v1.yaml` | Modify | `tasks` surface entry |
| `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts` | Create | Task DTO types + grouping/sorting pure functions |
| `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.test.ts` | Create | vitest for grouping/priority cycling |
| `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` | Create | Task list panel with icon CRUD controls |
| `crates/vox-gui/ui/src/App.tsx` | Modify | Route `tasks` view; tier/mode passthrough; theme boot; palette actions |
| `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx` | Modify | Model-list refresh; queue-depth chip |
| `crates/vox-populi/src/transport/handlers/nodes.rs` | Modify | `resources_summary` handler |
| `crates/vox-populi/src/transport/mod.rs` | Modify | Route registration |
| `crates/vox-populi/src/transport/handlers/dispatch.rs` | Modify | Multi-dimensional `select_best_node` scoring |
| `crates/vox-orchestrator/src/services/local_resources.rs` | Create | Cached sysinfo CPU/RAM snapshot |
| `crates/vox-orchestrator/src/services/mod.rs` | Modify | Export new module |
| `crates/vox-orchestrator/src/services/scaling.rs` | Modify | Local-resource guard in `decide_scaling` |
| `crates/vox-orchestrator/src/runtime.rs` | Modify | Feed local snapshot into scaling tick |
| `crates/vox-orchestrator/src/config/orchestrator_fields.rs` (+ defaults/env/load) | Modify | `scale_cpu_ceiling_pct`, `scale_mem_floor_mb` |
| `crates/vox-config/src/config/vox_config.rs` (+ toml_schema/impl_ops/persist) | Modify | `[llm]` section: concurrency/retry settings |
| `crates/vox-actor-runtime/src/llm/throttle.rs` | Create | Per-provider AIMD concurrency throttle |
| `crates/vox-actor-runtime/src/llm/chat.rs` | Modify | Acquire throttle around HTTP call; 429 header handling |
| `crates/vox-actor-runtime/src/llm/mod.rs` | Modify | Export throttle |
| `crates/vox-gui/src/commands/orchestrator.rs` | Modify | `get_orchestrator_config`; extended `set_orchestrator_config` keys |
| `crates/vox-gui/src/commands/llm_settings.rs` | Create | `get_llm_config`/`set_llm_config`/`openrouter_key_status` |
| `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts` | Create | `SETTINGS_INDEX` — searchable settings manifest (SSOT for search + deep links) |
| `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` | Modify | Hydration, theme fix, Scaling + LLM sections, filter box, deep-link seed |
| `crates/vox-gui/src/commands/docs_index.rs` | Create | `vox_docs_index` Tauri command (frontmatter walk) |
| `crates/vox-gui/ui/src/components/layout/paletteSources.ts` | Create | Pure federation/filter logic for omni-palette |
| `crates/vox-gui/ui/src/components/layout/paletteSources.test.ts` | Create | vitest |
| `crates/vox-gui/ui/src/components/layout/CommandPalette.tsx` | Modify | Omni-search: unified selection, new sections |
| `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` | Modify | Filter input |
| `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx` | Modify | Arrow wrap, facet focus rings |
| `crates/vox-gui/ui/src/components/ui/Icons.tsx` | Modify | Add `edit`, `trash`, `list` icons |
| `crates/vox-gui/ui/src/lib/sessions.ts` (+ `.test.ts`) | Create | Chat-session model: create/close/rename pure helpers |
| `crates/vox-gui/ui/src/components/layout/SessionTabs.tsx` | Create | Tab strip — one chat session per tab |
| `crates/vox-gui/ui/src/lib/chatCorrelation.ts` | Modify | `sessionId` on ChatMessage; per-session transcript filtering |
| `crates/vox-orchestrator/src/services/similarity.rs` | Create | Token-Jaccard near-duplicate detection (pure) |
| `crates/vox-orchestrator/src/orch_daemon/mod.rs` (J2) | Modify | `allow_duplicate` + `duplicate_of` on SUBMIT_TASK |
| `crates/vox-gui/src/commands/mesh_resources.rs` | Create | `get_mesh_resource_summary` Tauri command (calls D1 endpoint) |
| `docs/src/architecture/where-things-live.md` | Modify | New rows (LLM throttle, tasks surface, resources summary, sessions, similarity) |

---

## Conventions for every task

- Run Rust tests with `cargo test -p <crate> <filter>` from the repo root.
- Run GUI tests with `pnpm vitest run <file>` from `crates/vox-gui/ui/` (pnpm, **never npm**).
- Format only touched crates: `cargo fmt -p <crate>` (NEVER `cargo fmt --all` — Windows arg-limit). TS is formatted by the repo's existing tooling; match surrounding style.
- Commit after each task with the message given in the task. All commits on the current branch.
- After any change to `contracts/gui/surface-registry.v1.yaml`, run `vox ci gui-surface-registry --write` to regenerate `surfaceRegistry.generated.ts` — never hand-edit generated files.

---

# Track A — Task control plane (daemon + Tauri)

The daemon already has `orch.submit_task`, `orch.cancel_task`, `orch.reorder_task`. We add `orch.list_tasks` and `orch.edit_task`, then expose all four to the GUI. **Do not wire the `hopper/` module** — it is intentionally left as the future intake-classifier layer; the GUI task list reflects what will actually execute.

### Task A1: `orch.list_tasks` daemon method

**Files:**
- Modify: `crates/vox-foundation/src/protocol.rs` (inside `pub mod orch_daemon_method`, after the `REORDER_TASK` constant at ~line 27)
- Modify: `crates/vox-orchestrator/src/orch_daemon/mod.rs` (dispatch match, after the `REORDER_TASK` arm ending ~line 368)
- Test: same file, `mod isolation_dispatch_tests` sibling — add a new `mod task_dispatch_tests`

- [ ] **Step 1: Add the protocol constant**

In `crates/vox-foundation/src/protocol.rs`, after the `REORDER_TASK` const:

```rust
    /// List every queued or in-progress task across all agents, with assignment + lifecycle label.
    pub const LIST_TASKS: &str = "orch.list_tasks";
```

- [ ] **Step 2: Write the failing dispatch test**

In `crates/vox-orchestrator/src/orch_daemon/mod.rs`, add below `isolation_dispatch_tests`:

```rust
#[cfg(test)]
mod task_dispatch_tests {
    use super::*;
    use crate::config::OrchestratorConfig;

    fn req(method: &str, params: serde_json::Value) -> DispatchRequest {
        DispatchRequest {
            id: "1".to_string(),
            method: method.to_string(),
            params,
        }
    }

    fn result_value(resp: &DispatchResponse) -> &serde_json::Value {
        match &resp.payload {
            DispatchPayload::Result { value } => value,
            other => panic!("expected Result payload, got {other:?}"),
        }
    }

    async fn orch_with_one_task() -> (Arc<Orchestrator>, u64) {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
        orch.spawn_agent("a1").unwrap();
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({ "description": "first task", "priority": "normal" }),
            ),
        )
        .await;
        let task_id = result_value(&resp)["task_id"].as_u64().unwrap();
        (orch, task_id)
    }

    #[tokio::test]
    async fn list_tasks_returns_submitted_task_with_fields() {
        let (orch, task_id) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            orch,
            &req(orch_daemon_method::LIST_TASKS, serde_json::json!({})),
        )
        .await;
        let v = result_value(&resp);
        let tasks = v["tasks"].as_array().expect("tasks array");
        assert_eq!(tasks.len(), 1);
        let t = &tasks[0];
        assert_eq!(t["id"].as_u64(), Some(task_id));
        assert_eq!(t["description"].as_str(), Some("first task"));
        assert!(t["priority"].is_string() || t["priority"].is_object());
        assert!(t["lifecycle"].is_string());
        // agent_id present (assigned or null is acceptable for a fresh queue)
        assert!(t.get("agent_id").is_some());
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-orchestrator task_dispatch_tests -- --nocapture`
Expected: FAIL — response is the `unknown method: orch.list_tasks` error payload, so `result_value` panics.

- [ ] **Step 4: Implement the dispatch arm**

In `crates/vox-orchestrator/src/orch_daemon/mod.rs`, after the `REORDER_TASK` arm:

```rust
        orch_daemon_method::LIST_TASKS => {
            let assignments = orch.task_assignments_copy();
            let tasks: Vec<serde_json::Value> = orch
                .all_tasks()
                .into_iter()
                .map(|t| {
                    let agent_id = assignments.get(&t.id).map(|a| a.0);
                    let lifecycle = orch
                        .task_lifecycle_status_label(t.id)
                        .unwrap_or_else(|| "unknown".to_string());
                    let write_files: Vec<String> = t
                        .file_manifest
                        .iter()
                        .filter(|f| matches!(f.access, crate::types::AccessKind::Write))
                        .map(|f| f.path.to_string_lossy().to_string())
                        .collect();
                    serde_json::json!({
                        "id": t.id.0,
                        "description": t.description,
                        "priority": t.priority,            // raw: "Urgent"|"Normal"|"Background"
                        "status": t.status,
                        "lifecycle": lifecycle,            // raw: "Completed"|"InProgress"|"Blocked"|"Queued"
                        "agent_id": agent_id,
                        "session_id": t.session_id,
                        "estimated_complexity": t.estimated_complexity,
                        "depends_on": t.depends_on.iter().map(|d| d.0).collect::<Vec<u64>>(),
                        "write_files": write_files,
                    })
                })
                .collect();
            response_result(&req.id, serde_json::json!({ "tasks": tasks }))
        }
```

Verified: `task_lifecycle_status_label(&self, task_id: TaskId) -> Option<String>` at `accessors.rs:211`; labels are **Capitalized** (`"InProgress"`, not `"in_progress"`) and `TaskPriority` serializes Capitalized (`"Normal"`). The daemon emits raw values; the Tauri DTO in A3 normalizes to lowercase/snake_case. `FileAffinity { path: PathBuf, access: AccessKind }` is at `tasks.rs:169-174` — confirm the `AccessKind::Write` variant name (grep `enum AccessKind`) and the import path; adjust the `matches!` accordingly.

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test -p vox-orchestrator task_dispatch_tests`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-foundation/src/protocol.rs crates/vox-orchestrator/src/orch_daemon/mod.rs
git commit -m "feat(orchestrator): orch.list_tasks daemon RPC with assignment + lifecycle"
```

### Task A2: `Orchestrator::edit_task_description` + `orch.edit_task`

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/accessors.rs` (after `all_tasks()` ~line 195)
- Modify: `crates/vox-foundation/src/protocol.rs`
- Modify: `crates/vox-orchestrator/src/orch_daemon/mod.rs`
- Test: `task_dispatch_tests` module from A1

- [ ] **Step 1: Write the failing tests**

Append to `task_dispatch_tests`:

```rust
    #[tokio::test]
    async fn edit_task_rewrites_description_of_queued_task() {
        let (orch, task_id) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::EDIT_TASK,
                serde_json::json!({ "task_id": task_id, "description": "rewritten" }),
            ),
        )
        .await;
        assert_eq!(result_value(&resp)["ok"], true);

        let list = dispatch_request(
            "rid",
            orch,
            &req(orch_daemon_method::LIST_TASKS, serde_json::json!({})),
        )
        .await;
        let tasks = result_value(&list)["tasks"].as_array().unwrap();
        assert_eq!(tasks[0]["description"].as_str(), Some("rewritten"));
    }

    #[tokio::test]
    async fn edit_task_unknown_id_is_error() {
        let (orch, _) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::EDIT_TASK,
                serde_json::json!({ "task_id": 999_999, "description": "x" }),
            ),
        )
        .await;
        assert!(matches!(resp.payload, DispatchPayload::Error { .. }));
    }

    #[tokio::test]
    async fn edit_task_empty_description_is_error() {
        let (orch, task_id) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::EDIT_TASK,
                serde_json::json!({ "task_id": task_id, "description": "  " }),
            ),
        )
        .await;
        assert!(matches!(resp.payload, DispatchPayload::Error { .. }));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-orchestrator task_dispatch_tests`
Expected: FAIL — `EDIT_TASK` constant does not exist (compile error).

- [ ] **Step 3: Add constant + orchestrator method + dispatch arm**

Protocol constant (after `LIST_TASKS`):

```rust
    /// Rewrite the description of a queued (not in-progress) task.
    pub const EDIT_TASK: &str = "orch.edit_task";
```

In `accessors.rs`, after `all_tasks()`:

```rust
    /// Rewrite the description of a queued task. Returns an error if the task
    /// is unknown or not in `Queued` status (the running prompt must not change
    /// underneath an agent).
    pub fn edit_task_description(
        &self,
        task_id: TaskId,
        new_description: String,
    ) -> Result<(), String> {
        let trimmed = new_description.trim();
        if trimmed.is_empty() {
            return Err("description must be non-empty".to_string());
        }
        let agents = crate::sync_lock::rw_read(&self.agents);
        for queue_lock in agents.values() {
            let mut queue = crate::sync_lock::rw_write(queue_lock);
            // all_tasks_mut() chains the in-progress task with the queued ones
            // (queue/priority.rs), so guard by status, not container membership.
            for task in queue.all_tasks_mut() {
                if task.id == task_id {
                    if !matches!(task.status, crate::types::TaskStatus::Queued) {
                        return Err(format!(
                            "task {} is {:?} and cannot be edited",
                            task_id.0, task.status
                        ));
                    }
                    task.description = trimmed.to_string();
                    return Ok(());
                }
            }
        }
        Err(format!("task {} not found in any queue", task_id.0))
    }
```

Verified shapes: `self.agents` is `Arc<RwLock<HashMap<AgentId, Arc<RwLock<AgentQueue>>>>>` (`orchestrator.rs:62-64`); `all_tasks_mut(&mut self) -> impl Iterator<Item = &mut AgentTask>` at `queue/priority.rs:137+` **includes** the in-progress task; `TaskStatus::Queued` is the dequeue-eligible status used by `queue/drain.rs:6-25`. Confirm the `TaskStatus` import path used inside `accessors.rs` (likely `crate::types::TaskStatus` or already in scope) before compiling.

Dispatch arm in `orch_daemon/mod.rs` (after the `LIST_TASKS` arm):

```rust
        orch_daemon_method::EDIT_TASK => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            let Some(description) = req.params.get("description").and_then(|x| x.as_str()) else {
                return response_err(&req.id, "params.description (string) required");
            };
            match orch.edit_task_description(TaskId(task_id), description.to_string()) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, e),
            }
        }
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p vox-orchestrator task_dispatch_tests`
Expected: PASS (all 5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-foundation/src/protocol.rs crates/vox-orchestrator/src/orchestrator/accessors.rs crates/vox-orchestrator/src/orch_daemon/mod.rs
git commit -m "feat(orchestrator): orch.edit_task RPC — rewrite queued task descriptions"
```

### Task A3: Tauri control-plane wrappers

**Files:**
- Modify: `crates/vox-gui/src/commands/control_plane.rs` (append)
- Modify: `crates/vox-gui/src/main.rs` (command registration — find the existing `tauri::generate_handler![` list containing `submit_orchestrator_task` and append the four new names)

- [ ] **Step 1: Append the commands**

```rust
#[derive(Debug, Serialize)]
pub struct TaskRowDto {
    pub id: u64,
    pub description: String,
    pub priority: String,  // normalized lowercase: urgent|normal|background
    pub lifecycle: String, // normalized snake: queued|in_progress|blocked|completed|unknown
    pub agent_id: Option<u64>,
    pub session_id: Option<String>,
    pub estimated_complexity: u8,
    pub depends_on: Vec<u64>,
    pub write_files: Vec<String>,
}

/// Daemon emits TaskPriority Capitalized ("Normal") and lifecycle labels
/// CamelCase ("InProgress") — normalize once here so the frontend speaks one
/// dialect (and `reorder_orchestrator_task` can echo priorities back verbatim,
/// since REORDER_TASK parses lowercase).
fn normalize_lifecycle(raw: &str) -> String {
    match raw {
        "InProgress" => "in_progress".to_string(),
        "Queued" => "queued".to_string(),
        "Blocked" => "blocked".to_string(),
        "Completed" => "completed".to_string(),
        other => other.to_lowercase(),
    }
}

#[tauri::command]
pub async fn list_orchestrator_tasks() -> Result<Vec<TaskRowDto>, String> {
    let response =
        call_orchestrator_daemon(orch_daemon_method::LIST_TASKS, serde_json::json!({})).await?;
    let tasks = response
        .get("tasks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(tasks
        .into_iter()
        .map(|t| TaskRowDto {
            id: t.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
            description: t
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            priority: t
                .get("priority")
                .and_then(|v| v.as_str())
                .unwrap_or("Normal")
                .to_lowercase(),
            lifecycle: normalize_lifecycle(
                t.get("lifecycle").and_then(|v| v.as_str()).unwrap_or("unknown"),
            ),
            agent_id: t.get("agent_id").and_then(|v| v.as_u64()),
            session_id: t
                .get("session_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            estimated_complexity: t
                .get("estimated_complexity")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u8,
            depends_on: t
                .get("depends_on")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_u64()).collect())
                .unwrap_or_default(),
            write_files: t
                .get("write_files")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(ToString::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect())
}

#[tauri::command]
pub async fn edit_orchestrator_task(
    task_id: u64,
    description: String,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        orch_daemon_method::EDIT_TASK,
        serde_json::json!({ "task_id": task_id, "description": description }),
    )
    .await?;
    Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} updated"),
        task_id: Some(task_id.to_string()),
    })
}

#[tauri::command]
pub async fn cancel_orchestrator_task(task_id: u64) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        orch_daemon_method::CANCEL_TASK,
        serde_json::json!({ "task_id": task_id }),
    )
    .await?;
    Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} cancelled"),
        task_id: Some(task_id.to_string()),
    })
}

#[tauri::command]
pub async fn reorder_orchestrator_task(
    task_id: u64,
    priority: String,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        orch_daemon_method::REORDER_TASK,
        serde_json::json!({ "task_id": task_id, "priority": priority }),
    )
    .await?;
    Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} → {priority}"),
        task_id: Some(task_id.to_string()),
    })
}
```

- [ ] **Step 2: Register in `main.rs`**

Add to the `generate_handler!` list (same block that registers `submit_orchestrator_task`):

```rust
            crate::commands::control_plane::list_orchestrator_tasks,
            crate::commands::control_plane::edit_orchestrator_task,
            crate::commands::control_plane::cancel_orchestrator_task,
            crate::commands::control_plane::reorder_orchestrator_task,
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p vox-gui`
Expected: clean (warnings ok, no errors).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/src/commands/control_plane.rs crates/vox-gui/src/main.rs
git commit -m "feat(gui): Tauri task control-plane — list/edit/cancel/reorder orchestrator tasks"
```

---

# Track B — Tasks GUI surface

### Task B1: Register the `tasks` surface

**Files:**
- Modify: `contracts/gui/surface-registry.v1.yaml`
- Regenerate: `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`

- [ ] **Step 1: Add the registry entry**

Open `contracts/gui/surface-registry.v1.yaml` and add a sibling of the `runs` entry. The verified field names (snake_case; the generator camelCases them) and the verbatim `runs` entry for reference:

```yaml
# existing, for shape reference:
- view_key: runs
  cli_group: null
  representation_tier: live_backend
  nav_label: Runs
  nav_icon: clock
  nav_group: operate

# add:
- view_key: tasks
  cli_group: null
  representation_tier: live_backend
  nav_label: Tasks
  nav_icon: list
  nav_group: operate
```

`nav_icon: list` does not exist yet in `Icons.tsx` — Task B3 Step 0 adds it (execute B3 Step 0 before regenerating if the registry check validates icon keys; if the generator doesn't validate icons, order doesn't matter).

- [ ] **Step 2: Regenerate + verify gate**

Run: `vox ci gui-surface-registry --write` then `vox ci gui-surface-registry`
Expected: regenerated `surfaceRegistry.generated.ts` contains a `tasks` entry; the check passes.

- [ ] **Step 3: Commit**

```bash
git add contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
git commit -m "feat(gui): register tasks surface in operate nav group"
```

### Task B2: Task helpers (pure logic) with vitest

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts`
- Create: `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.test.ts`

- [ ] **Step 1: Write the failing tests**

`tasksHelpers.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { groupTasks, cyclePriority, TaskRow } from './tasksHelpers';

const row = (over: Partial<TaskRow>): TaskRow => ({
  id: 1,
  description: 'd',
  priority: 'normal',
  lifecycle: 'queued',
  agent_id: null,
  session_id: null,
  estimated_complexity: 1,
  depends_on: [],
  write_files: [],
  ...over,
});

describe('groupTasks', () => {
  it('splits in-progress from queued and orders queued urgent>normal>background', () => {
    const rows = [
      row({ id: 1, lifecycle: 'in_progress' }),
      row({ id: 2, priority: 'background' }),
      row({ id: 3, priority: 'urgent' }),
      row({ id: 4, priority: 'normal' }),
    ];
    const g = groupTasks(rows);
    expect(g.inProgress.map(t => t.id)).toEqual([1]);
    expect(g.queued.map(t => t.id)).toEqual([3, 4, 2]);
  });

  it('treats unknown lifecycle labels as queued', () => {
    const g = groupTasks([row({ id: 9, lifecycle: 'weird' })]);
    expect(g.queued).toHaveLength(1);
  });
});

describe('cyclePriority', () => {
  it('cycles background→normal→urgent→background', () => {
    expect(cyclePriority('background')).toBe('normal');
    expect(cyclePriority('normal')).toBe('urgent');
    expect(cyclePriority('urgent')).toBe('background');
    expect(cyclePriority('garbage')).toBe('normal');
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run (from `crates/vox-gui/ui/`): `pnpm vitest run src/components/surfaces/Tasks/tasksHelpers.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

`tasksHelpers.ts`:

```ts
export interface TaskRow {
  id: number;
  description: string;
  priority: string; // 'urgent' | 'normal' | 'background' (normalized by the Tauri DTO)
  lifecycle: string; // 'queued' | 'in_progress' | 'blocked' | 'completed' | 'unknown'
  agent_id: number | null;
  session_id: string | null;
  estimated_complexity: number;
  depends_on: number[];
  write_files: string[];
}

export interface GroupedTasks {
  inProgress: TaskRow[];
  queued: TaskRow[];
}

const PRIORITY_ORDER: Record<string, number> = { urgent: 0, normal: 1, background: 2 };

export function groupTasks(rows: TaskRow[]): GroupedTasks {
  const inProgress = rows.filter(t => t.lifecycle === 'in_progress');
  const queued = rows
    .filter(t => t.lifecycle !== 'in_progress')
    .sort(
      (a, b) =>
        (PRIORITY_ORDER[a.priority] ?? 1) - (PRIORITY_ORDER[b.priority] ?? 1) || a.id - b.id,
    );
  return { inProgress, queued };
}

export function cyclePriority(p: string): string {
  if (p === 'background') return 'normal';
  if (p === 'normal') return 'urgent';
  if (p === 'urgent') return 'background';
  return 'normal';
}
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm vitest run src/components/surfaces/Tasks/tasksHelpers.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Tasks/
git commit -m "feat(gui): task grouping + priority-cycle helpers with vitest"
```

### Task B3: TasksView component

**Files:**
- Modify: `crates/vox-gui/ui/src/components/ui/Icons.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx`

Before writing, read `crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.tsx` and copy its container/header CSS classes so Tasks matches the design system.

- [ ] **Step 0: Add the three missing icons**

Verified available keys: `plus` (line 172), `refresh` (156), `search` (47), `command` (106), `link` (177), `x` (101), `clock` (161), `file` (111), `chevronUp` (188), `chevronDown` (183). **`edit`, `trash`, and `list` do not exist.** Open `Icons.tsx`, copy the exact component shape of the `x` icon (same svg attrs, stroke props, className passthrough), and add three keys with these path data:

- `edit`: `<path d="M17 3l4 4L8 21H4v-4L17 3z" />`
- `trash`: `<path d="M3 6h18" /><path d="M8 6V4h8v2" /><path d="M6 6l1 14h10l1-14" />`
- `list`: `<path d="M8 6h13M8 12h13M8 18h13" /><path d="M3.5 6h.01M3.5 12h.01M3.5 18h.01" />`

Register them on the `Icon` export object exactly like the existing keys. Run `pnpm typecheck` to confirm.

- [ ] **Step 1: Implement the component**

```tsx
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Icon } from '../../ui/Icons';
import { TaskRow, groupTasks, cyclePriority } from './tasksHelpers';

const POLL_MS = 4000;

const PRIORITY_STYLE: Record<string, string> = {
  urgent: 'text-red-300 border-red-400/30 bg-red-400/10',
  normal: 'text-zinc-300 border-white/10 bg-white/[0.03]',
  background: 'text-zinc-500 border-white/5 bg-transparent',
};

function PriorityChip({ value, onCycle }: { value: string; onCycle: () => void }) {
  return (
    <button
      onClick={onCycle}
      title="Click to cycle priority (urgent → background → normal)"
      className={`shrink-0 rounded border px-1.5 py-px font-mono text-[9px] uppercase tracking-widest transition focus:outline-none focus:ring-1 focus:ring-brass/40 ${PRIORITY_STYLE[value] ?? PRIORITY_STYLE.normal}`}
    >
      {value}
    </button>
  );
}

export function TasksView() {
  const [rows, setRows] = useState<TaskRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [draft, setDraft] = useState('');
  const [newTask, setNewTask] = useState('');
  const [busy, setBusy] = useState(false);
  const mounted = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const data = await invoke<TaskRow[]>('list_orchestrator_tasks');
      if (mounted.current) {
        setRows(data);
        setError(null);
      }
    } catch (e) {
      if (mounted.current) setError(String(e));
    } finally {
      if (mounted.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    refresh();
    const t = setInterval(refresh, POLL_MS);
    return () => {
      mounted.current = false;
      clearInterval(t);
    };
  }, [refresh]);

  const act = useCallback(
    async (fn: () => Promise<unknown>) => {
      setBusy(true);
      try {
        await fn();
        await refresh();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const addTask = () => {
    const description = newTask.trim();
    if (!description) return;
    setNewTask('');
    act(() => invoke('submit_orchestrator_task', { input: { description, files: [], priority: 'normal', session_id: null } }));
  };

  const saveEdit = (id: number) => {
    const description = draft.trim();
    setEditingId(null);
    if (!description) return;
    act(() => invoke('edit_orchestrator_task', { taskId: id, description }));
  };

  const remove = (id: number) =>
    act(() => invoke('cancel_orchestrator_task', { taskId: id }));

  const reprioritize = (t: TaskRow) =>
    act(() => invoke('reorder_orchestrator_task', { taskId: t.id, priority: cyclePriority(t.priority) }));

  const { inProgress, queued } = groupTasks(rows);

  const renderRow = (t: TaskRow, editable: boolean) => (
    <div
      key={t.id}
      className="group flex items-center gap-2 rounded-lg border border-white/5 bg-white/[0.02] px-3 py-2"
    >
      <PriorityChip value={t.priority} onCycle={() => editable && reprioritize(t)} />
      <div className="min-w-0 flex-1">
        {editingId === t.id ? (
          <input
            autoFocus
            value={draft}
            onChange={e => setDraft(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter') saveEdit(t.id);
              if (e.key === 'Escape') setEditingId(null);
            }}
            onBlur={() => saveEdit(t.id)}
            className="w-full bg-transparent text-[13px] text-zinc-100 outline-none border-b border-brass/40"
          />
        ) : (
          <span className="block truncate text-[13px] text-zinc-200" title={t.description}>
            {t.description}
          </span>
        )}
        <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-600">
          #{t.id}
          {t.agent_id != null ? ` · agent ${t.agent_id}` : ''}
          {' · '}
          {t.lifecycle}
        </span>
      </div>
      {editable && (
        <div className="flex shrink-0 items-center gap-1 opacity-0 transition group-hover:opacity-100 focus-within:opacity-100">
          <button
            title="Edit task text"
            onClick={() => {
              setEditingId(t.id);
              setDraft(t.description);
            }}
            className="rounded p-1 text-zinc-400 hover:bg-white/[0.06] hover:text-zinc-100 focus:outline-none focus:ring-1 focus:ring-brass/40"
          >
            <Icon.edit className="size-3.5" />
          </button>
          <button
            title="Remove task"
            onClick={() => remove(t.id)}
            className="rounded p-1 text-zinc-400 hover:bg-red-400/10 hover:text-red-300 focus:outline-none focus:ring-1 focus:ring-red-400/40"
          >
            <Icon.trash className="size-3.5" />
          </button>
        </div>
      )}
    </div>
  );

  return (
    <div className="flex h-full flex-col gap-4 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-[15px] font-medium text-zinc-100">Tasks</h1>
          <p className="text-[11px] text-zinc-500">
            Everything queued or running across the agent fleet. Chat submissions land here.
          </p>
        </div>
        <button
          onClick={refresh}
          title="Refresh"
          className="rounded-lg border border-white/10 p-1.5 text-zinc-400 hover:bg-white/[0.05] focus:outline-none focus:ring-1 focus:ring-brass/40"
        >
          <Icon.refresh className="size-4" />
        </button>
      </div>

      {/* Add task */}
      <div className="flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.02] px-3 py-2">
        <Icon.plus className="size-4 text-brass" />
        <input
          value={newTask}
          onChange={e => setNewTask(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && addTask()}
          placeholder="Add a task…"
          className="flex-1 bg-transparent text-[13px] text-zinc-100 placeholder:text-zinc-600 outline-none"
        />
        <button
          onClick={addTask}
          disabled={busy || !newTask.trim()}
          className="rounded-lg border border-brass/30 bg-brass/10 px-2.5 py-1 text-[11px] text-brass disabled:opacity-40 focus:outline-none focus:ring-1 focus:ring-brass/40"
        >
          Add
        </button>
      </div>

      {error && (
        <div className="rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2 text-[11px] text-red-300">
          {error}
        </div>
      )}

      <div className="flex-1 space-y-5 overflow-auto custom-scrollbar">
        <section>
          <h2 className="mb-2 px-1 text-[10px] uppercase tracking-widest text-zinc-500">
            In progress ({inProgress.length})
          </h2>
          <div className="space-y-1.5">
            {inProgress.map(t => renderRow(t, false))}
            {inProgress.length === 0 && !loading && (
              <p className="px-1 text-[11px] text-zinc-600">Nothing running.</p>
            )}
          </div>
        </section>
        <section>
          <h2 className="mb-2 px-1 text-[10px] uppercase tracking-widest text-zinc-500">
            Queued ({queued.length})
          </h2>
          <div className="space-y-1.5">
            {queued.map(t => renderRow(t, true))}
            {queued.length === 0 && !loading && (
              <p className="px-1 text-[11px] text-zinc-600">Queue is empty — the agent is all yours.</p>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
```

`Icon.edit` / `Icon.trash` exist after Step 0; `Icon.plus` / `Icon.refresh` are pre-existing (verified).

- [ ] **Step 2: Route it**

In `App.tsx` (or `decoratorRegistry.ts` if other `live_backend` surfaces are mapped there — follow wherever `RunsView` is wired): map view key `'tasks'` → `<TasksView />`. Match the existing pattern exactly.

- [ ] **Step 3: Verify build + manual smoke**

Run (from `crates/vox-gui/ui/`): `pnpm vitest run` then `pnpm build` (or the repo's `tsc`/`vite build` equivalent in `package.json`).
Expected: typecheck/build clean. Then `cargo check -p vox-gui`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts
git commit -m "feat(gui): Tasks surface — live queue with add/edit/remove/reprioritize icon controls"
```

### Task B4: Queue-depth chip in Loquela

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx`
- Modify: `crates/vox-gui/ui/src/App.tsx`

- [ ] **Step 1: Pass queue depth + navigation into Loquela**

Verified anchors: the KPI is `status.total_queued`, surfaced at `App.tsx:209-212` as `kpis.queueDepth.value`; the view setter is `setActiveView` (declared at `App.tsx:159` via `useLocalStorage<View>('vox_active_view', 'dashboard')`). Find where `<Loquela` is rendered (it is imported at `App.tsx:11`; the render site is in the main layout below line 600) and pass: `queueDepth={kpis.queueDepth.value}` and `onOpenTasks={() => setActiveView('tasks')}` (cast `'tasks'` to the `View` type if the union doesn't yet include it — extend the `View` type where it is declared).

- [ ] **Step 2: Render the chip**

In `Loquela.tsx`, extend the `LoquelaProps` interface (lines 101-110: `chips`, `setChips`, `onSubmit`, `activeSkill`, `setActiveSkill`, `skills`, `toast?`, `agents?`) with `queueDepth?: number; onOpenTasks?: () => void;` and render next to the existing mode/tier selectors row:

```tsx
{typeof queueDepth === 'number' && queueDepth > 0 && (
  <button
    onClick={onOpenTasks}
    title="Open task list"
    className="flex items-center gap-1 rounded-full border border-brass/25 bg-brass/10 px-2 py-0.5 font-mono text-[10px] text-brass hover:bg-brass/20 focus:outline-none focus:ring-1 focus:ring-brass/40"
  >
    {queueDepth} queued
  </button>
)}
```

- [ ] **Step 3: Verify + commit**

Run: `pnpm build` (ui dir) — clean.

```bash
git add crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui): queue-depth chip in chat composer linking to Tasks"
```

---

# Track C — Chat fixes

### Task C1: Stop dropping mode/tier on chat submit

**Files:**
- Modify: `crates/vox-orchestrator/src/types/tasks.rs` (AgentTask ~line 341, TaskEnqueueHints ~line 211)
- Modify: `crates/vox-gui/src/commands/control_plane.rs`
- Modify: `crates/vox-gui/ui/src/App.tsx` (`handleLoquelaSubmit`, ~line 407)

`TaskEnqueueHints.model_preference` is documented as "Non-binding preference string (e.g. tier hint)" — exactly what Loquela's tier is. Mode gets a new optional hint field.

- [ ] **Step 1: Write the failing Rust test**

In `task_dispatch_tests` (orch_daemon/mod.rs):

```rust
    #[tokio::test]
    async fn submit_with_enqueue_hints_carries_tier_and_mode() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
        orch.spawn_agent("a1").unwrap();
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({
                    "description": "tiered task",
                    "enqueue_hints": { "model_preference": "mesh", "mode": "plan" }
                }),
            ),
        )
        .await;
        let task_id = result_value(&resp)["task_id"].as_u64().unwrap();
        let task = orch
            .all_tasks()
            .into_iter()
            .find(|t| t.id.0 == task_id)
            .unwrap();
        assert_eq!(task.model_preference.as_deref(), Some("mesh"));
        assert_eq!(task.mode.as_deref(), Some("plan"));
    }
```

Run: `cargo test -p vox-orchestrator submit_with_enqueue_hints_carries_tier_and_mode`
Expected: FAIL — `AgentTask` has no field `mode`.

- [ ] **Step 2: Add the `mode` field**

In `types/tasks.rs`, add to `AgentTask` (next to `model_preference`):

```rust
    /// Interaction mode requested at submit time (`plan` | `act` | `verify`).
    /// Advisory: routing/verification policies may consult it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
```

Add the identical field to `TaskEnqueueHints`. The merge site is **`AgentTask::apply_hints` at `tasks.rs:623-696`** — `model_preference` is merged at lines 630-631:

```rust
    if let Some(ref p) = h.model_preference {
        self.model_preference = Some(p.clone());
    }
```

Add the symmetric block directly after it:

```rust
    if let Some(ref m) = h.mode {
        self.mode = Some(m.clone());
    }
```

Construction sites that will need `mode: None` (verified — `AgentTask` has **no** `Default` impl): `AgentTask::new` at `tasks.rs:475`, `orchestrator/task_dispatch/complete/harness.rs:118`, `planning/test_decision.rs:100`, `queue/mod.rs:62`. Let the compiler find any others; `TaskEnqueueHints` construction sites use `..Default::default()` style or field lists — same treatment.

- [ ] **Step 3: Run to verify pass**

Run: `cargo test -p vox-orchestrator task_dispatch_tests`
Expected: PASS.

- [ ] **Step 4: Thread through Tauri + frontend**

`control_plane.rs` — extend the input struct and forward as enqueue_hints:

```rust
#[derive(Debug, Deserialize)]
pub struct SubmitTaskInput {
    pub description: String,
    #[serde(default)]
    pub files: Vec<String>,
    pub priority: Option<String>,
    pub session_id: Option<String>,
    pub mode: Option<String>,
    pub tier: Option<String>,
}
```

In `submit_orchestrator_task`, build hints and add to the JSON params:

```rust
    let mut enqueue_hints = serde_json::Map::new();
    if let Some(tier) = input.tier.as_deref().filter(|t| !t.trim().is_empty()) {
        enqueue_hints.insert("model_preference".into(), serde_json::json!(tier));
    }
    if let Some(mode) = input.mode.as_deref().filter(|m| !m.trim().is_empty()) {
        enqueue_hints.insert("mode".into(), serde_json::json!(mode));
    }
```

and in the `json!` params object:

```rust
            "enqueue_hints": if enqueue_hints.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::Object(enqueue_hints)
            },
```

(Daemon side: `SUBMIT_TASK` already parses `enqueue_hints` when non-null — `orch_daemon/mod.rs:269`. Confirm the daemon treats `null` as absent; if `serde_json::from_value::<TaskEnqueueHints>(Value::Null)` errors, omit the key entirely instead by building the params map imperatively.)

`App.tsx` `handleLoquelaSubmit` (lines 407-438): verified, the IPC args are wrapped as `{ input: { … } }` and currently send `description`, `files`, `priority`, `session_id` (with `session_id: payload.session_id ?? 'gui-loquela'`). Loquela's `send()` payload (Loquela.tsx:282-292) carries `mode` and `tier` at the top level. Extend the `input` object:

```ts
      {
        input: {
          description: payload.description,
          files: contextFiles,
          priority: payload.priority ?? null,
          session_id: payload.session_id ?? 'gui-loquela',
          mode: payload.mode ?? null,
          tier: payload.tier ?? null,
        }
      },
```

(Track I later replaces the hardcoded `'gui-loquela'` with the active session tab — don't change session handling here.)

- [ ] **Step 5: Verify + commit**

Run: `cargo check -p vox-gui && cargo test -p vox-orchestrator task_dispatch_tests` and `pnpm build` in ui dir.

```bash
git add crates/vox-orchestrator/src/types/tasks.rs crates/vox-orchestrator/src/orch_daemon/mod.rs crates/vox-gui/src/commands/control_plane.rs crates/vox-gui/ui/src/App.tsx
git commit -m "fix(gui+orchestrator): carry chat mode/tier through submit as enqueue hints"
```

### Task C2: Refresh model/tier list instead of fetch-once

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx` (~line 168)

- [ ] **Step 1: Re-fetch on focus + interval**

Verified: the one-shot effect is at `Loquela.tsx:152-174`; it calls `voxTransport.listModels(24)` (→ Tauri `list_model_cards`), maps the first 4 models into tier entries, and sets `setRuntimeTiers([...])`; tier state is `tier`/`setTier` (lines 114-115). Refactor by extracting the existing effect body into a `loadTiers` function **unchanged** (keep the mapping, the `'auto'` sentinel entry, and the stale-tier reset logic exactly as-is), then re-trigger it:

```tsx
  useEffect(() => {
    let cancelled = false;
    const loadTiers = () => {
      voxTransport.listModels(24).then((models: any) => {
        if (cancelled) return;
        /* …existing body from lines 153-172 verbatim… */
      }).catch(() => {});
    };
    loadTiers();
    const interval = setInterval(loadTiers, 60_000);
    const onFocus = () => loadTiers();
    window.addEventListener('focus', onFocus);
    return () => {
      cancelled = true;
      clearInterval(interval);
      window.removeEventListener('focus', onFocus);
    };
  }, []);
```

One behavioral guard to add inside the existing body: only call `setTier(dynamic[0]?.id ?? 'auto')` when the current `tier` is missing from the refreshed list **and** is not `'auto'` (this condition already exists at line 170 — keep it; it prevents a background refresh from yanking the user's selection).

- [ ] **Step 2: Verify + commit**

Run: `pnpm build` (ui) — clean.

```bash
git add crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx
git commit -m "fix(gui): refresh chat model/tier list on focus and every 60s"
```

---

# Track D — Resource-aware mesh scheduling

### Task D1: `GET /v1/populi/resources/summary`

**Files:**
- Modify: `crates/vox-populi/src/transport/handlers/nodes.rs` (append handler)
- Modify: `crates/vox-populi/src/transport/mod.rs` (route registration — add next to the existing `GET /v1/populi/nodes` route)
- Test: inline `#[cfg(test)]` in nodes.rs

The pure aggregation is a free function over `&[NodeRecord]` so it unit-tests without HTTP.

- [ ] **Step 1: Write the failing test**

Append to `nodes.rs`:

```rust
#[cfg(test)]
mod resources_summary_tests {
    use super::*;
    use vox_populi_types::NodeRecord;

    // NodeRecord has NO Default impl (verified); every non-id field is
    // Option/serde-default, so deserialize a minimal JSON object instead.
    fn node(cpu: Option<f32>, mem_free: Option<u64>, gpus_alloc: Option<u32>) -> NodeRecord {
        let mut n: NodeRecord =
            serde_json::from_value(serde_json::json!({ "id": "test-node" }))
                .expect("minimal NodeRecord");
        n.cpu_usage_pct = cpu;
        n.memory_free_bytes = mem_free;
        n.gpu_allocatable_count = gpus_alloc;
        n
    }
    // If deserialization fails because some field lacks a serde default,
    // copy the construction pattern from an existing NodeRecord test in
    // vox-populi or vox-populi-types instead of fighting the literal.

    #[test]
    fn aggregates_counts_and_capacity() {
        let nodes = vec![
            node(Some(10.0), Some(8 * 1024 * 1024 * 1024), Some(1)),
            node(Some(90.0), Some(2 * 1024 * 1024 * 1024), Some(0)),
            {
                let mut q = node(Some(5.0), None, Some(4));
                q.quarantined = Some(true);
                q
            },
        ];
        let s = aggregate_resources(&nodes);
        assert_eq!(s.node_count, 3);
        assert_eq!(s.eligible_node_count, 2); // quarantined excluded
        assert_eq!(s.gpu_allocatable_total, 1); // quarantined node's 4 GPUs excluded
        assert_eq!(s.memory_free_bytes_total, 10 * 1024 * 1024 * 1024);
        assert!((s.cpu_usage_pct_avg - 50.0).abs() < 0.01);
    }
}
```

(If `NodeRecord` does not implement `Default`, construct it the way existing tests in `vox-populi` do — search the crate's tests for `NodeRecord {` and copy the minimal constructor.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-populi resources_summary_tests`
Expected: FAIL — `aggregate_resources` not found.

- [ ] **Step 3: Implement aggregation + handler + route**

In `nodes.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct MeshResourceSummary {
    pub node_count: usize,
    /// Nodes accepting new work (not quarantined, not in maintenance drain).
    pub eligible_node_count: usize,
    pub gpu_total: u32,
    pub gpu_allocatable_total: u32,
    pub memory_free_bytes_total: u64,
    /// Mean of reported cpu_usage_pct across eligible nodes (0 when none report).
    pub cpu_usage_pct_avg: f32,
    pub nodes: Vec<MeshResourceNode>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MeshResourceNode {
    pub node_id: String,
    pub eligible: bool,
    pub cpu_usage_pct: Option<f32>,
    pub memory_free_bytes: Option<u64>,
    pub gpu_allocatable_count: Option<u32>,
    pub gpu_total_count: Option<u32>,
    pub loaded_llm_models: Vec<String>,
    pub labels: Vec<String>,
}

pub(crate) fn aggregate_resources(nodes: &[vox_populi_types::NodeRecord]) -> MeshResourceSummary {
    let now = crate::now_ms();
    let mut summary = MeshResourceSummary {
        node_count: nodes.len(),
        eligible_node_count: 0,
        gpu_total: 0,
        gpu_allocatable_total: 0,
        memory_free_bytes_total: 0,
        cpu_usage_pct_avg: 0.0,
        nodes: Vec::with_capacity(nodes.len()),
    };
    let mut cpu_sum = 0.0f64;
    let mut cpu_n = 0usize;
    for n in nodes {
        let eligible =
            n.quarantined != Some(true) && !node_maintenance_blocks_new_work(now, n);
        if eligible {
            summary.eligible_node_count += 1;
            summary.gpu_total += n.gpu_total_count.unwrap_or(0);
            summary.gpu_allocatable_total += n.gpu_allocatable_count.unwrap_or(0);
            summary.memory_free_bytes_total += n.memory_free_bytes.unwrap_or(0);
            if let Some(c) = n.cpu_usage_pct {
                cpu_sum += f64::from(c);
                cpu_n += 1;
            }
        }
        summary.nodes.push(MeshResourceNode {
            node_id: n.id.clone(),
            eligible,
            cpu_usage_pct: n.cpu_usage_pct,
            memory_free_bytes: n.memory_free_bytes,
            gpu_allocatable_count: n.gpu_allocatable_count,
            gpu_total_count: n.gpu_total_count,
            loaded_llm_models: n.loaded_llm_models.clone().unwrap_or_default(),
            labels: n.capabilities.labels.clone(),
        });
    }
    if cpu_n > 0 {
        summary.cpu_usage_pct_avg = (cpu_sum / cpu_n as f64) as f32;
    }
    summary
}
```

Verified anchors used above:
- Import exactly as `dispatch.rs:8` does: `use crate::{NodeRecord, node_maintenance_blocks_new_work};` — signature `node_maintenance_blocks_new_work(now_ms: u64, n: &NodeRecord) -> bool` (defined at `vox-populi-types/src/node_record.rs:181`, re-exported at `vox-populi/src/lib.rs:235`).
- NodeRecord field is **`id`** (not `node_id`); `loaded_llm_models: Option<Vec<String>>` (hence `unwrap_or_default()`); GPU counts `Option<u32>`, `memory_free_bytes: Option<u64>`, `cpu_usage_pct: Option<f32>`.
- `capabilities.labels` — confirm the collection type on `vox_repository::TaskCapabilityHints` (`capabilities.rs:13-68`); if not `Vec<String>`, convert with `.iter().cloned().collect()`.

Then the HTTP handler — model it on the verified `list_nodes` handler (`handlers/nodes.rs:95-121`), which guards with `auth_allows_worker_plane(&ctx)` and reads `st.inner.read().await` (a `PopuliRegistryFile` whose `.nodes` is `Vec<NodeRecord>`):

```rust
pub(crate) async fn resources_summary(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
) -> Result<Json<MeshResourceSummary>, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for resource summary".into(),
        ));
    }
    let registry = st.inner.read().await;
    Ok(Json(aggregate_resources(&registry.nodes)))
}
```

(Use the exact same imports `list_nodes` uses — `State`, `Extension`, `Json`, `StatusCode`, `ResponseErr`, `auth_allows_worker_plane` are all already in scope in `nodes.rs`.)

Route in **`crates/vox-populi/src/transport/router.rs`** (verified — the router lives here, not `mod.rs`), next to line 72's `.route("/v1/populi/nodes", get(list_nodes))`:

```rust
        .route("/v1/populi/resources/summary", get(resources_summary))
```

adding `resources_summary` to the `handlers` import at `router.rs:20`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p vox-populi resources_summary_tests && cargo check -p vox-populi`
Expected: PASS / clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-populi/src/transport/
git commit -m "feat(populi): GET /v1/populi/resources/summary — aggregated mesh CPU/GPU/RAM capacity"
```

### Task D2: Multi-dimensional `select_best_node` scoring

**Files:**
- Modify: `crates/vox-populi/src/transport/handlers/dispatch.rs:159-205`
- Test: inline `#[cfg(test)] mod select_best_node_tests` in same file

Score (higher wins): `score = (100 − cpu%) + 20·log2(1 + free_GiB) + 15·gpu_allocatable + 25·model_locality`. CPU stays dominant for CPU-bound scripts; GPU/memory/model-affinity break ties meaningfully. Unknown metrics score pessimistically (cpu=100, mem=0, gpu=0) — same conservatism as today's `unwrap_or(100.0)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod select_best_node_tests {
    use super::*;

    // NodeRecord has no Default (verified) — deserialize a minimal object.
    fn node(id: &str, cpu: Option<f32>, mem_free_gib: u64, gpus: u32) -> NodeRecord {
        let mut n: NodeRecord =
            serde_json::from_value(serde_json::json!({ "id": id })).expect("minimal NodeRecord");
        n.cpu_usage_pct = cpu;
        n.memory_free_bytes = Some(mem_free_gib * 1024 * 1024 * 1024);
        n.gpu_allocatable_count = Some(gpus);
        n
    }

    // DispatchRequest (transport/mod.rs:290-323) has no Default but every
    // filter field is serde-optional — deserialize the minimal request.
    fn plain_req() -> DispatchRequest {
        serde_json::from_value(serde_json::json!({ "source": "test" }))
            .expect("minimal DispatchRequest")
    }

    #[test]
    fn prefers_lower_cpu_all_else_equal() {
        let nodes = vec![node("busy", Some(80.0), 4, 0), node("idle", Some(10.0), 4, 0)];
        let best = select_best_node(&nodes, &plain_req()).unwrap();
        assert_eq!(best.id, "idle");
    }

    #[test]
    fn gpu_and_memory_break_cpu_ties() {
        let nodes = vec![node("small", Some(50.0), 1, 0), node("beefy", Some(50.0), 32, 2)];
        let best = select_best_node(&nodes, &plain_req()).unwrap();
        assert_eq!(best.id, "beefy");
    }

    #[test]
    fn model_locality_outweighs_small_cpu_difference() {
        let mut warm = node("warm", Some(55.0), 8, 1);
        warm.loaded_llm_models = Some(vec!["qwen3.5-2b".to_string()]);
        let cold = node("cold", Some(45.0), 8, 1);
        let mut req = plain_req();
        req.model_id = Some("qwen3.5-2b".to_string());
        let best = select_best_node(&[warm, cold], &req).unwrap();
        assert_eq!(best.id, "warm");
    }
}
```

**Do NOT add a `preferred_model` field** — verified: `DispatchRequest` (defined at `transport/mod.rs:290-323`, fields: `source`, `node_id`, `timeout_secs`, `is_bundle`, `source_blake3_hex`, `required_labels`, `is_detached`, `priority`, `task_kind`, `model_id`, `min_vram_mb`) **already has `model_id: Option<String>`** — that is the locality hint. If `plain_req()`'s minimal deserialization fails (a non-optional field beyond `source`), add the missing required fields to the `json!` literal rather than touching the struct.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p vox-populi select_best_node_tests`
Expected: FAIL (tie-break and locality tests; the first may pass already).

- [ ] **Step 3: Replace the sort with scoring**

Replace lines 195–204 (`// Load balancing: Sort by CPU usage ascending` … `candidates.first().copied()`) with:

```rust
    // Multi-dimensional score (higher wins): idle CPU dominates; free memory,
    // allocatable GPUs, and model locality break ties. Unreported metrics
    // score pessimistically so silent nodes don't win by omission.
    fn score(n: &NodeRecord, req: &DispatchRequest) -> f64 {
        let cpu = f64::from(n.cpu_usage_pct.unwrap_or(100.0));
        let free_gib = n.memory_free_bytes.unwrap_or(0) as f64 / (1024.0 * 1024.0 * 1024.0);
        let gpus = f64::from(n.gpu_allocatable_count.unwrap_or(0));
        let locality = match (&req.model_id, &n.loaded_llm_models) {
            (Some(m), Some(loaded)) if loaded.iter().any(|l| l == m) => 1.0,
            _ => 0.0,
        };
        (100.0 - cpu) + 20.0 * (1.0 + free_gib).log2() + 15.0 * gpus + 25.0 * locality
    }

    candidates.sort_by(|a, b| {
        score(b, req)
            .partial_cmp(&score(a, req))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    candidates.first().copied()
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p vox-populi select_best_node_tests && cargo test -p vox-populi dispatch`
Expected: PASS, and pre-existing dispatch tests still green.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-populi/src/transport/handlers/dispatch.rs
git commit -m "feat(populi): resource-aware dispatch scoring — CPU + memory + GPU + model locality"
```

### Task D3: Local-resource-aware scaling

**Files:**
- Create: `crates/vox-orchestrator/src/services/local_resources.rs`
- Modify: `crates/vox-orchestrator/src/services/mod.rs`
- Modify: `crates/vox-orchestrator/src/config/orchestrator_fields.rs` (+ `impl_default.rs`, `impl_env.rs`, `impl_load.rs` following the exact pattern of `scaling_threshold`)
- Modify: `crates/vox-orchestrator/src/services/scaling.rs`
- Modify: `crates/vox-orchestrator/src/runtime.rs` (~lines 716–765)

**Verified dependency facts:** sysinfo 0.39 is already a workspace dep AND already wired into vox-orchestrator as `sysinfo = { workspace = true, optional = true }` behind the **`system-metrics`** feature — and `orchestrator/scaling.rs:184-185` already calls `sys.refresh_cpu_all(); sys.refresh_memory();`. Before writing the new module, read `orchestrator/scaling.rs:170-200` — if a reusable local probe already exists there, extract/reuse it instead of duplicating. The new module must be feature-gated the same way, with a no-op fallback so default builds compile.

- [ ] **Step 1: Write the failing scaling tests**

In `services/scaling.rs` tests:

```rust
    #[test]
    fn does_not_scale_up_when_local_cpu_above_ceiling() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.scaling_enabled = true;
        cfg.max_agents = 4;
        cfg.scaling_threshold = 1;
        cfg.scale_cpu_ceiling_pct = 85.0;
        let local = crate::services::local_resources::LocalResourceSnapshot {
            cpu_usage_pct: 95.0,
            memory_free_mb: 16_000,
        };
        let action = ScalingService::decide_scaling(
            &status(5, 3.0),
            &cfg,
            &[],
            0,
            &[],
            &BudgetManager::new(None),
            Some(&local),
        );
        assert!(matches!(action, ScalingAction::NoOp));
    }

    #[test]
    fn does_not_scale_up_when_memory_below_floor() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.scaling_enabled = true;
        cfg.max_agents = 4;
        cfg.scaling_threshold = 1;
        cfg.scale_mem_floor_mb = 2_048;
        let local = crate::services::local_resources::LocalResourceSnapshot {
            cpu_usage_pct: 10.0,
            memory_free_mb: 512,
        };
        let action = ScalingService::decide_scaling(
            &status(5, 3.0),
            &cfg,
            &[],
            0,
            &[],
            &BudgetManager::new(None),
            Some(&local),
        );
        assert!(matches!(action, ScalingAction::NoOp));
    }

    #[test]
    fn none_snapshot_preserves_existing_behavior() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.scaling_enabled = true;
        cfg.max_agents = 4;
        cfg.scaling_threshold = 1;
        let action = ScalingService::decide_scaling(
            &status(5, 3.0),
            &cfg,
            &[],
            0,
            &[],
            &BudgetManager::new(None),
            None,
        );
        assert!(matches!(action, ScalingAction::ScaleUp { .. }));
    }
```

Also update the two existing tests in this file to pass `None` as the new final argument.

Run: `cargo test -p vox-orchestrator scaling` — Expected: FAIL (compile: unknown fields/arity).

- [ ] **Step 2: Add config fields**

`config/orchestrator_fields.rs` (near `scaling_threshold`, line ~143):

```rust
    /// Do not spawn new agents while local CPU usage is at/above this percent (0 disables the guard).
    pub scale_cpu_ceiling_pct: f32,
    /// Do not spawn new agents while local free memory is below this many MiB (0 disables the guard).
    pub scale_mem_floor_mb: u64,
```

`impl_default.rs` (next to `scaling_enabled: default_false()`):

```rust
            scale_cpu_ceiling_pct: 85.0,
            scale_mem_floor_mb: 1024,
```

`impl_env.rs` (copy the `scaling_enabled` parse_or_warn pattern at line ~139):

```rust
        self.scale_cpu_ceiling_pct = parse_or_warn(
            "VOX_ORCHESTRATOR_SCALE_CPU_CEILING_PCT",
            self.scale_cpu_ceiling_pct,
        );
        self.scale_mem_floor_mb = parse_or_warn(
            "VOX_ORCHESTRATOR_SCALE_MEM_FLOOR_MB",
            self.scale_mem_floor_mb,
        );
```

(Match the real `parse_or_warn` signature — it takes the env var name and current value per the existing call.) Then find where `[orchestrator]` Vox.toml keys are merged (`impl_load.rs` / `merge_populi.rs` — wherever `max_agents` is read from the toml table) and add the two keys with the same pattern, names `scale_cpu_ceiling_pct` and `scale_mem_floor_mb`.

- [ ] **Step 3: Local resource probe module**

`services/local_resources.rs`:

```rust
//! Cached local CPU/RAM snapshot for scaling decisions.
//!
//! sysinfo refreshes are not free; the scaling tick runs frequently, so the
//! snapshot is cached for `CACHE_TTL` and refreshed lazily.

#[cfg(feature = "system-metrics")]
use std::sync::Mutex;
#[cfg(feature = "system-metrics")]
use std::time::{Duration, Instant};

#[cfg(feature = "system-metrics")]
const CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalResourceSnapshot {
    /// Global CPU utilization percent (0–100).
    pub cpu_usage_pct: f32,
    /// Free (available) memory in MiB.
    pub memory_free_mb: u64,
}

#[cfg(feature = "system-metrics")]
mod probe {
    use super::*;
    use sysinfo::System;

    struct ProbeState {
        system: System,
        last: Option<(Instant, LocalResourceSnapshot)>,
    }

    static PROBE: Mutex<Option<ProbeState>> = Mutex::new(None);

    /// Best-effort snapshot; `None` only if the probe lock is poisoned.
    pub fn snapshot() -> Option<LocalResourceSnapshot> {
        let mut guard = PROBE.lock().ok()?;
        let state = guard.get_or_insert_with(|| ProbeState {
            system: System::new_all(),
            last: None,
        });
        if let Some((at, snap)) = state.last {
            if at.elapsed() < CACHE_TTL {
                return Some(snap);
            }
        }
        // Verified repo idiom (orchestrator/scaling.rs:184-185 and
        // vox-ml-cli populi_cli.rs:1210-1214):
        state.system.refresh_cpu_all();
        state.system.refresh_memory();
        let snap = LocalResourceSnapshot {
            cpu_usage_pct: state.system.global_cpu_usage(),
            memory_free_mb: state.system.available_memory() / (1024 * 1024),
        };
        state.last = Some((Instant::now(), snap));
        Some(snap)
    }
}

#[cfg(feature = "system-metrics")]
pub use probe::snapshot;

/// Without the `system-metrics` feature there is no probe; scaling falls back
/// to its pre-existing behavior (no local guard).
#[cfg(not(feature = "system-metrics"))]
pub fn snapshot() -> Option<LocalResourceSnapshot> {
    None
}

#[cfg(all(test, feature = "system-metrics"))]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_plausible_values() {
        let s = snapshot().expect("probe");
        assert!(s.cpu_usage_pct >= 0.0);
        assert!(s.memory_free_mb > 0);
    }
}
```

Export from `services/mod.rs`: `pub mod local_resources;`. Note: sysinfo's first CPU refresh after process start can report 0% (it needs two samples) — that is fine here; a 0% reading never *blocks* scale-up, and the 5s cache means subsequent ticks get real numbers. Run the feature-gated test with `cargo test -p vox-orchestrator --features system-metrics local_resources`.

- [ ] **Step 4: Extend `decide_scaling`**

Add parameter `local: Option<&crate::services::local_resources::LocalResourceSnapshot>` after `budgets`, and insert the guard at the top of the scale-up branch (immediately inside `if agent_count < max_agents && !cost_critical {`):

```rust
            // Local resource guard: a saturated host must not take on more agents,
            // regardless of queue pressure. Mesh dispatch still drains the queue.
            if let Some(local) = local {
                let cpu_blocked = config.scale_cpu_ceiling_pct > 0.0
                    && local.cpu_usage_pct >= config.scale_cpu_ceiling_pct;
                let mem_blocked = config.scale_mem_floor_mb > 0
                    && local.memory_free_mb < config.scale_mem_floor_mb;
                if cpu_blocked || mem_blocked {
                    return ScalingAction::NoOp;
                }
            }
```

Note the guard must only suppress **scale-up**: place it inside the scale-up branch so scale-*down* of idle agents still proceeds (a saturated host *wants* scale-down).

- [ ] **Step 5: Feed the snapshot in the runtime tick**

The verified call site (`runtime.rs:753-764`) is:

```rust
        let load_history: Vec<f64> = crate::sync_lock::rw_read(&*self.orchestrator.load_history)
            .iter()
            .copied()
            .collect();
        let action = ScalingService::decide_scaling(
            &status,
            &config,
            &load_history,
            remote_gpu_capacity,
            &idle_dynamic,
            &crate::sync_lock::rw_read(&budget_manager),
        );
```

Add one line so it becomes:

```rust
        let local_snapshot = crate::services::local_resources::snapshot();
        let action = ScalingService::decide_scaling(
            &status,
            &config,
            &load_history,
            remote_gpu_capacity,
            &idle_dynamic,
            &crate::sync_lock::rw_read(&budget_manager),
            local_snapshot.as_ref(),
        );
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p vox-orchestrator scaling && cargo test -p vox-orchestrator local_resources`
Expected: PASS (all new + updated tests).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator/src/services/ crates/vox-orchestrator/src/config/ crates/vox-orchestrator/src/runtime.rs crates/vox-orchestrator/Cargo.toml
git commit -m "feat(orchestrator): scaling honors local CPU ceiling + memory floor (sysinfo probe)"
```

---

# Track E — LLM egress concurrency (OpenRouter-aware)

OpenRouter facts driving the design (researched 2026-06-12, sources: openrouter.ai/docs/api/reference/limits, /docs/faq): paid models have **no platform RPM cap** (upstream-provider-bound); free `:free` models are capped at 20 RPM + 50/day (1000/day with ≥$10 lifetime credits); `GET https://openrouter.ai/api/v1/key` returns `limit_remaining`, `usage`, `is_free_tier`; 429 responses may carry `Retry-After` (seconds) and `X-RateLimit-Reset` (epoch **milliseconds**). Therefore: semaphore-bounded concurrency with AIMD (halve on 429, +1 per 8 successes), header-driven cooldowns, user-configurable per-provider maxima.

### Task E1: `[llm]` config section in the SSOT

**Files:**
- Modify: `crates/vox-config/src/config/vox_config.rs`
- Modify: `crates/vox-config/src/config/toml_schema.rs`
- Modify: `crates/vox-config/src/config/impl_ops.rs` (load merge, `get_key`/`set_key`/`known_keys`)
- Modify: `crates/vox-config/src/config/persist.rs` (include `[llm]` in `save_merged_global_config`)
- Test: existing config test module in vox-config (find `#[cfg(test)]` in `impl_ops.rs` or `config/tests.rs` and extend)

- [ ] **Step 1: Write the failing test**

In the vox-config test module:

```rust
    #[test]
    fn llm_keys_roundtrip_through_get_set() {
        let mut cfg = VoxConfig::default();
        assert_eq!(cfg.llm_max_concurrent_requests, 8);
        // set_key's return type varies — assert via the readback, not the return.
        let _ = cfg.set_key("llm.max_concurrent_requests", "16");
        assert_eq!(cfg.get_key("llm.max_concurrent_requests").as_deref(), Some("16"));
        let _ = cfg.set_key("llm.openrouter_max_concurrent", "4");
        assert_eq!(cfg.llm_openrouter_max_concurrent, Some(4));
        let _ = cfg.set_key("llm.retry_max_attempts", "5");
        assert_eq!(cfg.llm_retry_max_attempts, 5);
        assert!(VoxConfig::known_keys().contains(&"llm.max_concurrent_requests"));
    }
```

Run: `cargo test -p vox-config llm_keys_roundtrip` — Expected: FAIL (no such fields).

- [ ] **Step 2: Add fields + schema + ops**

`vox_config.rs` — add to `VoxConfig` and `Default`:

```rust
    /// Global ceiling on concurrent LLM HTTP requests across all providers.
    pub llm_max_concurrent_requests: usize,        // default 8
    /// Per-provider override for OpenRouter (None = use global).
    pub llm_openrouter_max_concurrent: Option<usize>,
    /// Per-provider override for OpenAI (None = use global).
    pub llm_openai_max_concurrent: Option<usize>,
    /// Max retry attempts on 429 before surfacing the error.
    pub llm_retry_max_attempts: u32,               // default 4
```

`toml_schema.rs` — verified shape: `VoxToml { vox: Option<VoxTomlSection>, train: Option<TrainTomlSection>, db: Option<DbTomlSection>, web: Option<WebTomlSection>, build: Option<BuildTomlSection> }` (lines 9-17); section structs follow the `<Name>TomlSection` convention. Add:

```rust
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct LlmTomlSection {
    pub max_concurrent_requests: Option<usize>,
    pub openrouter_max_concurrent: Option<usize>,
    pub openai_max_concurrent: Option<usize>,
    pub retry_max_attempts: Option<u32>,
}
```

and on `VoxToml`: `pub llm: Option<LlmTomlSection>,` (copy the exact serde attrs of the sibling `web` field).

`impl_ops.rs` — in `load()` where other sections merge (`[vox]`, `[train]`…):

```rust
        if let Some(llm) = toml.llm {
            if let Some(v) = llm.max_concurrent_requests { cfg.llm_max_concurrent_requests = v; }
            cfg.llm_openrouter_max_concurrent = llm.openrouter_max_concurrent.or(cfg.llm_openrouter_max_concurrent);
            cfg.llm_openai_max_concurrent = llm.openai_max_concurrent.or(cfg.llm_openai_max_concurrent);
            if let Some(v) = llm.retry_max_attempts { cfg.llm_retry_max_attempts = v; }
        }
```

`get_key`/`set_key`/`known_keys`: add the four dotted keys (`llm.max_concurrent_requests`, `llm.openrouter_max_concurrent`, `llm.openai_max_concurrent`, `llm.retry_max_attempts`) following the exact match-arm pattern of `web.run_mode`. Env override in the same place other env overrides happen (`VOX_LLM_MAX_CONCURRENCY` → `llm_max_concurrent_requests`).

`persist.rs`: extend `save_merged_global_config(path: &Path, cfg: &VoxConfig) -> std::io::Result<()>` (verified signature, line 18) to also write the `[llm]` table — it currently merges `[vox]`, `[train]`, `[db]`, `[web]`, `[build]`; add `[llm]` with the same merge-don't-clobber behavior.

- [ ] **Step 3: Run to verify pass**

Run: `cargo test -p vox-config`
Expected: PASS including the new test.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-config/src/config/
git commit -m "feat(config): [llm] section — concurrency + retry settings in SSOT"
```

### Task E2: AIMD provider throttle in the egress facade

**Files:**
- Create: `crates/vox-actor-runtime/src/llm/throttle.rs`
- Modify: `crates/vox-actor-runtime/src/llm/mod.rs` (add `pub mod throttle;`)
- Modify: `crates/vox-actor-runtime/src/llm/chat.rs`

- [ ] **Step 1: Write the failing tests** (inside throttle.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn permits_bound_concurrency() {
        let t = ProviderThrottle::new(2);
        let g1 = t.acquire().await;
        let _g2 = t.acquire().await;
        // Third acquire must not resolve while two are held.
        let pending = tokio::time::timeout(Duration::from_millis(50), t.acquire()).await;
        assert!(pending.is_err(), "third permit should block at limit 2");
        drop(g1);
        let g3 = tokio::time::timeout(Duration::from_millis(200), t.acquire()).await;
        assert!(g3.is_ok(), "released permit should admit a waiter");
    }

    #[tokio::test]
    async fn rate_limit_halves_and_successes_recover() {
        let t = ProviderThrottle::new(8);
        t.on_rate_limited(None);
        assert_eq!(t.current_limit(), 4);
        t.on_rate_limited(None);
        assert_eq!(t.current_limit(), 2);
        for _ in 0..8 {
            t.on_success();
        }
        assert_eq!(t.current_limit(), 3);
    }

    #[tokio::test]
    async fn cooldown_blocks_until_deadline() {
        let t = ProviderThrottle::new(4);
        t.on_rate_limited(Some(Duration::from_millis(120)));
        let start = std::time::Instant::now();
        let _g = t.acquire().await;
        assert!(start.elapsed() >= Duration::from_millis(100), "acquire should wait out cooldown");
    }
}
```

Run: `cargo test -p vox-actor-runtime throttle` — Expected: FAIL (module missing).

- [ ] **Step 2: Implement**

```rust
//! Per-provider AIMD concurrency throttle for LLM egress.
//!
//! Design (OpenRouter-informed, 2026-06): paid OpenRouter traffic has no
//! platform RPM cap — concurrency is the real dial — while free models 429
//! readily. We bound in-flight requests per provider with a user-configured
//! ceiling, halve the window on 429 (honoring Retry-After / X-RateLimit-Reset
//! as a cooldown), and additively recover one permit per 8 successes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

pub struct ProviderThrottle {
    max_limit: usize,
    current_limit: AtomicUsize,
    in_flight: AtomicUsize,
    success_streak: AtomicUsize,
    cooldown_until: Mutex<Option<Instant>>,
    notify: Notify,
}

/// RAII permit: releases the slot on drop.
pub struct Permit<'a> {
    throttle: &'a ProviderThrottle,
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.throttle.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.throttle.notify.notify_waiters();
    }
}

impl ProviderThrottle {
    pub fn new(max_limit: usize) -> Self {
        let max = max_limit.max(1);
        Self {
            max_limit: max,
            current_limit: AtomicUsize::new(max),
            in_flight: AtomicUsize::new(0),
            success_streak: AtomicUsize::new(0),
            cooldown_until: Mutex::new(None),
            notify: Notify::new(),
        }
    }

    pub fn current_limit(&self) -> usize {
        self.current_limit.load(Ordering::SeqCst)
    }

    /// Wait for a free slot (and for any active cooldown to elapse).
    pub async fn acquire(&self) -> Permit<'_> {
        loop {
            let wait = {
                let guard = self.cooldown_until.lock().expect("throttle lock");
                guard.and_then(|until| until.checked_duration_since(Instant::now()))
            };
            if let Some(d) = wait {
                tokio::time::sleep(d).await;
                continue;
            }
            let limit = self.current_limit.load(Ordering::SeqCst);
            let cur = self.in_flight.load(Ordering::SeqCst);
            if cur < limit
                && self
                    .in_flight
                    .compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                return Permit { throttle: self };
            }
            self.notify.notified().await;
        }
    }

    /// Multiplicative decrease + optional header-driven cooldown.
    pub fn on_rate_limited(&self, retry_after: Option<Duration>) {
        let limit = self.current_limit.load(Ordering::SeqCst);
        self.current_limit
            .store((limit / 2).max(1), Ordering::SeqCst);
        self.success_streak.store(0, Ordering::SeqCst);
        if let Some(d) = retry_after {
            let mut guard = self.cooldown_until.lock().expect("throttle lock");
            let until = Instant::now() + d;
            *guard = Some(guard.map_or(until, |existing| existing.max(until)));
        }
        self.notify.notify_waiters();
    }

    /// Additive increase: +1 permit per 8 consecutive successes, up to max.
    pub fn on_success(&self) {
        let streak = self.success_streak.fetch_add(1, Ordering::SeqCst) + 1;
        if streak % 8 == 0 {
            let limit = self.current_limit.load(Ordering::SeqCst);
            if limit < self.max_limit {
                self.current_limit.store(limit + 1, Ordering::SeqCst);
            }
            self.notify.notify_waiters();
        }
    }
}

static REGISTRY: OnceLock<Mutex<HashMap<String, &'static ProviderThrottle>>> = OnceLock::new();

/// Throttle for `provider`, created on first use from VoxConfig limits.
/// Leaked intentionally: providers are a small fixed set per process.
pub fn for_provider(provider: &str) -> &'static ProviderThrottle {
    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = registry.lock().expect("throttle registry lock");
    if let Some(t) = map.get(provider) {
        return t;
    }
    // Verified: VoxConfig::load() returns Self (impl_ops.rs:17), not Result.
    let cfg = vox_config::VoxConfig::load();
    let limit = match provider {
        "openrouter" => cfg
            .llm_openrouter_max_concurrent
            .unwrap_or(cfg.llm_max_concurrent_requests),
        "openai" => cfg
            .llm_openai_max_concurrent
            .unwrap_or(cfg.llm_max_concurrent_requests),
        _ => cfg.llm_max_concurrent_requests,
    };
    let throttle: &'static ProviderThrottle = Box::leak(Box::new(ProviderThrottle::new(limit)));
    map.insert(provider.to_string(), throttle);
    throttle
}

/// Parse `Retry-After` (seconds) or `X-RateLimit-Reset` (epoch ms) into a wait.
pub fn retry_after_from_headers(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    if let Some(v) = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
    {
        return Some(Duration::from_secs(v.min(120)));
    }
    if let Some(reset_ms) = headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u128>().ok())
    {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis();
        if reset_ms > now_ms {
            return Some(Duration::from_millis(((reset_ms - now_ms) as u64).min(120_000)));
        }
    }
    None
}
```

(Verified: `vox_config` is a direct dep of vox-actor-runtime (`Cargo.toml:20`), `reqwest` is a direct dep with the `stream` feature (`Cargo.toml:29`), and `vox_http_client::client()` returns `reqwest::Client` (`vox-http-client/src/lib.rs:34`) — the `reqwest::header` types used in `retry_after_from_headers` resolve without new deps.)

- [ ] **Step 3: Integrate into `llm_chat`**

In `chat.rs`, inside the async block:

1. Before `let client = vox_http_client::client();` (line ~55):

```rust
            let throttle = super::throttle::for_provider(&config.provider);
            let _permit = throttle.acquire().await;
```

2. In the error branch (line ~79, `if !res.status().is_success() {`), before consuming the body, capture headers and feed the throttle:

```rust
                let status = res.status();
                let retry_after = if status.as_u16() == 429 {
                    super::throttle::retry_after_from_headers(res.headers())
                } else {
                    None
                };
                if status.as_u16() == 429 {
                    throttle.on_rate_limited(retry_after);
                }
```

(then the existing `res.text()` body read continues; note `let status = res.status();` already exists — don't duplicate it, reorder so headers are read before `.text()` consumes `res`.)

3. In the success path, after the JSON parse succeeds (line ~134): `throttle.on_success();`

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-actor-runtime throttle && cargo check -p vox-actor-runtime`
Expected: PASS / clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-actor-runtime/src/llm/
git commit -m "feat(llm): per-provider AIMD concurrency throttle with Retry-After cooldowns"
```

### Task E3: OpenRouter key-status Tauri command

**Files:**
- Create: `crates/vox-gui/src/commands/llm_settings.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`, `crates/vox-gui/src/main.rs`

- [ ] **Step 1: Implement**

```rust
//! LLM settings bridge: [llm] config SSOT + OpenRouter key status probe.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LlmConfigDto {
    pub max_concurrent_requests: usize,
    pub openrouter_max_concurrent: Option<usize>,
    pub openai_max_concurrent: Option<usize>,
    pub retry_max_attempts: u32,
}

#[tauri::command]
pub async fn get_llm_config() -> Result<LlmConfigDto, String> {
    let cfg = vox_config::VoxConfig::load(); // returns Self (verified)
    Ok(LlmConfigDto {
        max_concurrent_requests: cfg.llm_max_concurrent_requests,
        openrouter_max_concurrent: cfg.llm_openrouter_max_concurrent,
        openai_max_concurrent: cfg.llm_openai_max_concurrent,
        retry_max_attempts: cfg.llm_retry_max_attempts,
    })
}

#[tauri::command]
pub async fn set_llm_config(config: serde_json::Value) -> Result<(), String> {
    let mut cfg = vox_config::VoxConfig::load();
    if let Some(v) = config.get("maxConcurrentRequests").and_then(|v| v.as_u64()) {
        cfg.llm_max_concurrent_requests = (v as usize).clamp(1, 256);
    }
    if let Some(v) = config.get("openrouterMaxConcurrent") {
        cfg.llm_openrouter_max_concurrent =
            v.as_u64().map(|n| (n as usize).clamp(1, 256));
    }
    if let Some(v) = config.get("openaiMaxConcurrent") {
        cfg.llm_openai_max_concurrent = v.as_u64().map(|n| (n as usize).clamp(1, 256));
    }
    if let Some(v) = config.get("retryMaxAttempts").and_then(|v| v.as_u64()) {
        cfg.llm_retry_max_attempts = (v as u32).clamp(0, 10);
    }
    // Persist to ~/.vox/config.toml ([llm] table merged, user additions
    // preserved). Verified free-fn signature (persist.rs:18):
    //   save_merged_global_config(path: &Path, cfg: &VoxConfig) -> io::Result<()>
    // Resolve the global config path the same way the CLI `vox config set`
    // path does (vox-config/src/paths.rs data_dir()).
    let path = vox_config::paths::data_dir().join("config.toml");
    vox_config::config::persist::save_merged_global_config(&path, &cfg)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct OpenRouterKeyStatusDto {
    pub configured: bool,
    pub is_free_tier: Option<bool>,
    pub limit_remaining: Option<f64>,
    pub usage: Option<f64>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn openrouter_key_status() -> Result<OpenRouterKeyStatusDto, String> {
    // VERIFIED GOTCHA: vox_secrets::resolve_secret takes a `SecretId` ENUM
    // (vox-secrets/src/lib.rs:219) and returns a `ResolvedSecret` struct, NOT
    // resolve_secret("OPENROUTER_API_KEY"). Before writing this, open
    // crates/vox-gui/src/commands/ and find the existing secrets bridge
    // (list_secret_status / set_secret are already registered commands) and
    // copy its exact resolution call for the OpenRouter key. The spec id lives
    // in vox-secrets/src/spec/registry/llm.rs (OpenRouterApiKey, canonical env
    // OPENROUTER_API_KEY). The shape is approximately:
    //   let resolved = vox_secrets::resolve_secret(SecretId::OpenRouterApiKey);
    //   let key: String = resolved.value().unwrap_or_default();
    // — mirror whatever accessor the existing GUI secrets code uses.
    let key = resolve_openrouter_key(); // helper you write per the above
    if key.trim().is_empty() {
        return Ok(OpenRouterKeyStatusDto {
            configured: false,
            is_free_tier: None,
            limit_remaining: None,
            usage: None,
            error: None,
        });
    }
    let client = vox_http_client::client();
    let res = client
        .get("https://openrouter.ai/api/v1/key")
        .bearer_auth(key.trim())
        .send()
        .await;
    match res {
        Ok(r) if r.status().is_success() => {
            let v: serde_json::Value = r.json().await.map_err(|e| e.to_string())?;
            let data = v.get("data").cloned().unwrap_or(serde_json::Value::Null);
            Ok(OpenRouterKeyStatusDto {
                configured: true,
                is_free_tier: data.get("is_free_tier").and_then(|x| x.as_bool()),
                limit_remaining: data.get("limit_remaining").and_then(|x| x.as_f64()),
                usage: data.get("usage").and_then(|x| x.as_f64()),
                error: None,
            })
        }
        Ok(r) => Ok(OpenRouterKeyStatusDto {
            configured: true,
            is_free_tier: None,
            limit_remaining: None,
            usage: None,
            error: Some(format!("HTTP {}", r.status())),
        }),
        Err(e) => Ok(OpenRouterKeyStatusDto {
            configured: true,
            is_free_tier: None,
            limit_remaining: None,
            usage: None,
            error: Some(e.to_string()),
        }),
    }
}
```

Adapt: the module paths `vox_config::paths::data_dir` / `vox_config::config::persist::save_merged_global_config` must match the crate's actual re-exports (check `vox-config/src/lib.rs` for what's `pub`; if `persist` isn't public, add the `pub` or call through whatever public save API the CLI `vox config set` uses). Register `pub mod llm_settings;` in `commands/mod.rs` and the three commands in `main.rs`'s `generate_handler!` (verified block at `main.rs:99-119`).

- [ ] **Step 2: Verify + commit**

Run: `cargo check -p vox-gui`

```bash
git add crates/vox-gui/src/commands/
git commit -m "feat(gui): LLM settings bridge — [llm] SSOT read/write + OpenRouter key status"
```

---

# Track F — Settings: fix, extend, make searchable

### Task F1: Hydrate orchestrator settings from Vox.toml

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs` (after `set_orchestrator_config`, line ~321)
- Modify: `crates/vox-gui/src/main.rs` (register)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` (~line 447 state init)

- [ ] **Step 1: Add `get_orchestrator_config`**

```rust
#[tauri::command]
pub async fn get_orchestrator_config() -> Result<serde_json::Value, String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let (manifest, _path) = VoxManifest::discover(&current_dir).map_err(|e| e.to_string())?;
    let t = manifest.orchestrator.unwrap_or_default();
    let get_i = |k: &str| t.get(k).and_then(|v| v.as_integer());
    let get_f = |k: &str| {
        t.get(k)
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
    };
    let get_b = |k: &str| t.get(k).and_then(|v| v.as_bool());
    let get_s = |k: &str| t.get(k).and_then(|v| v.as_str().map(ToString::to_string));
    Ok(serde_json::json!({
        "concurrency": get_i("max_agents"),
        "capUsd": get_i("financial_cost_budget_micros").map(|m| m as f64 / 1_000_000.0),
        "doubtThresh": get_f("trust_auto_approve_min"),
        "isolation": get_s("scope_enforcement").map(|s| match s.as_str() {
            "Wasm" => "wasm", "Container" => "ctr", _ => "native",
        }),
        "autobudget": get_b("exec_time_budget_enabled"),
        "doubt": get_b("socrates_gate_enforce"),
        "scalingEnabled": get_b("scaling_enabled"),
        "minAgents": get_i("min_agents"),
        "scalingThreshold": get_i("scaling_threshold"),
        "scaleCpuCeilingPct": get_f("scale_cpu_ceiling_pct"),
        "scaleMemFloorMb": get_i("scale_mem_floor_mb"),
    }))
}
```

Register in `main.rs`.

- [ ] **Step 2: Hydrate in SettingsView**

At the `useState<SettingsState>` block (~line 447), add a mount effect that overwrites defaults with real values:

```tsx
  useEffect(() => {
    (async () => {
      try {
        const cfg = await invoke<Record<string, unknown>>('get_orchestrator_config');
        setVals(prev => ({
          ...prev,
          ...(cfg.concurrency != null ? { concurrency: Number(cfg.concurrency) } : {}),
          ...(cfg.capUsd != null ? { capUsd: Number(cfg.capUsd) } : {}),
          ...(cfg.doubtThresh != null ? { doubtThresh: Number(cfg.doubtThresh) } : {}),
          ...(cfg.isolation != null ? { isolation: String(cfg.isolation) } : {}),
          ...(cfg.autobudget != null ? { autobudget: Boolean(cfg.autobudget) } : {}),
          ...(cfg.doubt != null ? { doubt: Boolean(cfg.doubt) } : {}),
        }));
      } catch {
        /* daemon-less dev: keep defaults */
      }
    })();
  }, []);
```

Verified — `SettingsState` (lines 31-42) is exactly `{ doubt, autobudget, theme, concurrency, capUsd, doubtThresh, sign, telemetry, isolation, checkpointMins }`, so the spread keys above are correct as written. (`set_orchestrator_config` consumes the same camelCase names — `orchestrator.rs:257-301`.)

- [ ] **Step 3: Verify + commit**

Run: `cargo check -p vox-gui` + `pnpm build` (ui).

```bash
git add crates/vox-gui/src/commands/orchestrator.rs crates/vox-gui/src/main.rs crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx
git commit -m "fix(gui): hydrate orchestrator settings from Vox.toml instead of hardcoded defaults"
```

### Task F2: Make the theme picker actually apply

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` (theme section)

- [ ] **Step 1: Create the CSS hook (verified: none exists)**

Verified: `tailwind.config.js` has only static colors (`brass: '#d4af37'` at line 8) and **no** `[data-theme]` hooks anywhere; the theme radios persist to DB but nothing reads the value. First read the theme section's render code in `SettingsView.tsx` to get the exact persisted value strings (expected `'arcane' | 'void' | 'glacier'` — use whatever the radios actually store).

Then make the accent color variable-driven so themes have a visible effect:

1. In `tailwind.config.js`, change `brass: '#d4af37'` → `brass: 'var(--brass, #d4af37)'` (Tailwind passes `var()` strings through to CSS; opacity modifiers like `brass/40` stop working with plain `var()` — if the build or visuals break on opacity variants, use the channel form instead: define `--brass-rgb: 212 175 55` and `brass: 'rgb(var(--brass-rgb) / <alpha-value>)'`, with per-theme overrides of `--brass-rgb`).
2. In `index.css` (top, after any `@tailwind` directives):

```css
:root[data-theme='void'] { --brass-rgb: 139 92 246; }    /* violet accent */
:root[data-theme='glacier'] { --brass-rgb: 56 189 248; } /* sky accent */
:root[data-theme='void'] body { background-color: #050507; }
:root[data-theme='glacier'] body { background-color: #0a0f14; }
```

(Adjust the body selector to whatever element actually paints the app background — find the top-level container's bg class in App.tsx and target that if `body` is transparent.)

- [ ] **Step 2: Apply on change + on boot**

In SettingsView's theme `update()` handler, after persisting via `set_gui_preference`, add:

```tsx
document.documentElement.setAttribute('data-theme', value);
```

In `App.tsx`, in the existing boot effect that loads preferences (or a new one):

```tsx
  useEffect(() => {
    (async () => {
      try {
        const theme = await invoke<string | null>('get_gui_preference', { key: 'gui.theme' });
        if (theme) document.documentElement.setAttribute('data-theme', theme);
      } catch { /* default theme */ }
    })();
  }, []);
```

- [ ] **Step 3: Verify + commit**

Run: `pnpm build` (ui). Manually verify by toggling in dev if a session is available; otherwise rely on typecheck.

```bash
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx crates/vox-gui/ui/src/index.css
git commit -m "fix(gui): theme selection applies data-theme attribute on boot and change"
```

### Task F3: Scaling + LLM settings sections

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs` (`set_orchestrator_config` — new keys)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`

- [ ] **Step 1: Extend `set_orchestrator_config`**

After the existing key blocks (line ~301), add:

```rust
    if let Some(v) = config.get("scalingEnabled").and_then(|v| v.as_bool()) {
        orch_table.insert("scaling_enabled".to_string(), toml::Value::Boolean(v));
    }
    if let Some(v) = config.get("minAgents").and_then(|v| v.as_u64()) {
        orch_table.insert("min_agents".to_string(), toml::Value::Integer(v as i64));
    }
    if let Some(v) = config.get("scalingThreshold").and_then(|v| v.as_u64()) {
        orch_table.insert("scaling_threshold".to_string(), toml::Value::Integer(v as i64));
    }
    if let Some(v) = config.get("scaleCpuCeilingPct").and_then(|v| v.as_f64()) {
        orch_table.insert("scale_cpu_ceiling_pct".to_string(), toml::Value::Float(v));
    }
    if let Some(v) = config.get("scaleMemFloorMb").and_then(|v| v.as_u64()) {
        orch_table.insert("scale_mem_floor_mb".to_string(), toml::Value::Integer(v as i64));
    }
```

- [ ] **Step 2: Add the two sections to SettingsView**

The section nav is the `SECTIONS` array at `SettingsView.tsx:8-18` (verified shape `{ id, icon, label }`, e.g. `{ id: 'orchestrator', icon: 'cpu', label: 'Orchestrator' }`). Insert after the orchestrator entry:

```ts
  { id: 'scaling', icon: 'cpu',  label: 'Scaling' },
  { id: 'llm',     icon: 'bolt', label: 'LLM & providers' },
```

(`cpu` and `bolt` are icon keys already used by this array.) Then render the two sections after the orchestrator block (`section === 'orchestrator'` starts at line 588), reusing the existing `Row`/`RangeInline` components (verified: `Row({ label, hint, children })` at line 44; `RangeInline({ value, min, max, step = 1, suffix = '', onChange })` at line 64 — `step` is supported):

```tsx
        {section === 'scaling' && (
          <div className="space-y-4">
            <Row label="Auto-scaling" hint="Spawn/retire agents based on queue load and host resources">
              <input
                type="checkbox"
                checked={vals.scalingEnabled}
                onChange={e => update({ scalingEnabled: e.target.checked })}
              />
            </Row>
            <Row label="Min agents" hint="Never retire below this fleet size">
              <RangeInline value={vals.minAgents} min={0} max={8} onChange={v => update({ minAgents: v })} />
            </Row>
            <Row label="Max agents" hint="Hard ceiling on concurrent agents">
              <RangeInline value={vals.concurrency} min={1} max={16} onChange={v => update({ concurrency: v })} />
            </Row>
            <Row label="Queue threshold" hint="Per-agent load that triggers a scale-up">
              <RangeInline value={vals.scalingThreshold} min={1} max={20} onChange={v => update({ scalingThreshold: v })} />
            </Row>
            <Row label="CPU ceiling %" hint="Don't spawn agents while local CPU is above this (0 = off)">
              <RangeInline value={vals.scaleCpuCeilingPct} min={0} max={100} onChange={v => update({ scaleCpuCeilingPct: v })} />
            </Row>
            <Row label="Memory floor (MiB)" hint="Don't spawn agents below this free RAM (0 = off)">
              <RangeInline value={vals.scaleMemFloorMb} min={0} max={16384} step={256} onChange={v => update({ scaleMemFloorMb: v })} />
            </Row>
          </div>
        )}

        {section === 'llm' && <LlmSettingsSection />}
```

Extend `SettingsState` + defaults with `scalingEnabled: false, minAgents: 1, scalingThreshold: 5, scaleCpuCeilingPct: 85, scaleMemFloorMb: 1024`, hydrate them in the F1 effect (the keys are already returned by `get_orchestrator_config`), and route their changes through the same `update()` → `set_orchestrator_config` path the orchestrator section uses (read the existing `update` implementation in the un-excerpted part of SettingsView — it batches the full camelCase state into one `invoke('set_orchestrator_config', { config })` call; the new keys ride along automatically once they're in state and the F3 Step 1 backend keys exist).

`LlmSettingsSection` (new component in the same file, modeled on the secrets section's load/save pattern):

```tsx
function LlmSettingsSection() {
  const [cfg, setCfg] = useState({
    maxConcurrentRequests: 8,
    openrouterMaxConcurrent: null as number | null,
    retryMaxAttempts: 4,
  });
  const [keyStatus, setKeyStatus] = useState<{
    configured: boolean;
    is_free_tier?: boolean | null;
    limit_remaining?: number | null;
    error?: string | null;
  } | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const c = await invoke<Record<string, unknown>>('get_llm_config');
        setCfg({
          maxConcurrentRequests: Number(c.max_concurrent_requests ?? 8),
          openrouterMaxConcurrent:
            c.openrouter_max_concurrent == null ? null : Number(c.openrouter_max_concurrent),
          retryMaxAttempts: Number(c.retry_max_attempts ?? 4),
        });
      } catch { /* defaults */ }
      try {
        setKeyStatus(await invoke('openrouter_key_status'));
      } catch { /* probe is best-effort */ }
    })();
  }, []);

  const save = (next: typeof cfg) => {
    setCfg(next);
    invoke('set_llm_config', {
      config: {
        maxConcurrentRequests: next.maxConcurrentRequests,
        openrouterMaxConcurrent: next.openrouterMaxConcurrent,
        retryMaxAttempts: next.retryMaxAttempts,
      },
    }).catch(() => {});
  };

  return (
    <div className="space-y-4">
      <Row label="Max parallel LLM requests" hint="Global ceiling across providers; OpenRouter paid tier is provider-bound, so this is the real dial">
        <RangeInline
          value={cfg.maxConcurrentRequests}
          min={1}
          max={64}
          onChange={v => save({ ...cfg, maxConcurrentRequests: v })}
        />
      </Row>
      <Row label="OpenRouter override" hint="Provider-specific cap (0 = use global)">
        <RangeInline
          value={cfg.openrouterMaxConcurrent ?? 0}
          min={0}
          max={64}
          onChange={v => save({ ...cfg, openrouterMaxConcurrent: v === 0 ? null : v })}
        />
      </Row>
      <Row label="429 retry attempts" hint="Backoff retries before surfacing a rate-limit error">
        <RangeInline
          value={cfg.retryMaxAttempts}
          min={0}
          max={10}
          onChange={v => save({ ...cfg, retryMaxAttempts: v })}
        />
      </Row>
      {keyStatus && (
        <div className="rounded-lg border border-white/10 bg-white/[0.02] px-3 py-2 text-[11px] text-zinc-400">
          {keyStatus.configured ? (
            keyStatus.error ? (
              <>OpenRouter key probe failed: {keyStatus.error}</>
            ) : (
              <>
                OpenRouter: {keyStatus.is_free_tier ? 'free tier (20 req/min cap)' : 'paid tier (no platform cap)'}
                {keyStatus.limit_remaining != null && <> · credit limit remaining: {keyStatus.limit_remaining}</>}
              </>
            )
          ) : (
            <>No OpenRouter key configured — add one under Keys &amp; Secrets.</>
          )}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Verify + commit**

Run: `cargo check -p vox-gui` + `pnpm build` (ui).

```bash
git add crates/vox-gui/src/commands/orchestrator.rs crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx
git commit -m "feat(gui): Scaling and LLM settings sections wired to config SSOT"
```

### Task F4: Settings index (search SSOT) + in-settings filter + deep links

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts`
- Create: `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.test.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`

- [ ] **Step 1: Failing test**

`settingsIndex.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { SETTINGS_INDEX, searchSettings } from './settingsIndex';

describe('SETTINGS_INDEX', () => {
  it('has unique ids and a section for every entry', () => {
    const ids = SETTINGS_INDEX.map(s => s.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(SETTINGS_INDEX.every(s => s.section.length > 0)).toBe(true);
  });
});

describe('searchSettings', () => {
  it('matches on label, hint, and keywords case-insensitively', () => {
    expect(searchSettings('OPENROUTER').length).toBeGreaterThan(0);
    expect(searchSettings('parallel').some(s => s.section === 'llm')).toBe(true);
    expect(searchSettings('zzzznothing')).toHaveLength(0);
  });
});
```

Run: `pnpm vitest run src/components/surfaces/Settings/settingsIndex.test.ts` — FAIL (module missing).

- [ ] **Step 2: Implement the index**

`settingsIndex.ts` — one entry per Row across **all** sections (orchestrator, scaling, llm, routing, mesh, signing, secrets, telemetry, keybinds, theme, gamify). Full content:

```ts
export interface SettingEntry {
  id: string;        // unique, kebab-case
  section: string;   // section id used by SettingsView
  label: string;     // visible Row label
  hint: string;      // visible Row hint
  keywords: string[];
}

export const SETTINGS_INDEX: SettingEntry[] = [
  { id: 'orch-max-agents', section: 'orchestrator', label: 'Max agents', hint: 'Concurrent agent ceiling', keywords: ['concurrency', 'fleet', 'parallel'] },
  { id: 'orch-budget', section: 'orchestrator', label: 'Budget (USD)', hint: 'Financial cost budget', keywords: ['cost', 'cap', 'spend'] },
  { id: 'orch-doubt-threshold', section: 'orchestrator', label: 'Doubt threshold', hint: 'Trust auto-approve minimum', keywords: ['trust', 'approval', 'socrates'] },
  { id: 'orch-isolation', section: 'orchestrator', label: 'Isolation', hint: 'Scope enforcement: wasm, container, native', keywords: ['sandbox', 'wasm', 'container'] },
  { id: 'scaling-enabled', section: 'scaling', label: 'Auto-scaling', hint: 'Spawn/retire agents based on load and resources', keywords: ['scale', 'autoscale', 'dynamic'] },
  { id: 'scaling-min-agents', section: 'scaling', label: 'Min agents', hint: 'Never retire below this fleet size', keywords: ['floor', 'scale down'] },
  { id: 'scaling-threshold', section: 'scaling', label: 'Queue threshold', hint: 'Per-agent load that triggers scale-up', keywords: ['queue', 'pressure', 'load'] },
  { id: 'scaling-cpu-ceiling', section: 'scaling', label: 'CPU ceiling %', hint: 'Block agent spawn above this local CPU usage', keywords: ['cpu', 'resources', 'host'] },
  { id: 'scaling-mem-floor', section: 'scaling', label: 'Memory floor (MiB)', hint: 'Block agent spawn below this free RAM', keywords: ['ram', 'memory', 'resources'] },
  { id: 'llm-max-concurrency', section: 'llm', label: 'Max parallel LLM requests', hint: 'Global ceiling across providers', keywords: ['openrouter', 'parallel', 'concurrency', 'rate limit', 'throttle'] },
  { id: 'llm-openrouter-cap', section: 'llm', label: 'OpenRouter override', hint: 'Provider-specific concurrency cap', keywords: ['openrouter', 'provider', 'cap'] },
  { id: 'llm-retry', section: 'llm', label: '429 retry attempts', hint: 'Backoff retries on rate limit', keywords: ['retry', 'backoff', '429', 'rate limit'] },
  { id: 'routing-priority', section: 'routing', label: 'Routing priority', hint: 'Six-axis model routing emphasis', keywords: ['model', 'routing', 'efficiency', 'precision', 'latency'] },
  { id: 'routing-chain', section: 'routing', label: 'Priority chain', hint: 'Model selection fallback chain', keywords: ['fallback', 'selection', 'policy'] },
  { id: 'mesh-nodes', section: 'mesh', label: 'Mesh nodes', hint: 'Discover and trust mesh peers', keywords: ['populi', 'peers', 'nodes', 'distributed'] },
  { id: 'signing-keys', section: 'signing', label: 'Signing keys', hint: 'ed25519 key status and rotation', keywords: ['ed25519', 'rotate', 'signature'] },
  { id: 'secrets-keys', section: 'secrets', label: 'Keys & secrets', hint: 'Provider API keys (OpenRouter, Gemini, …)', keywords: ['api key', 'openrouter', 'anthropic', 'token', 'clavis'] },
  { id: 'telemetry-mode', section: 'telemetry', label: 'Telemetry', hint: 'Off, local OTLP, or cloud', keywords: ['otlp', 'tracing', 'privacy'] },
  { id: 'keybinds', section: 'keybinds', label: 'Keybinds', hint: 'Global keyboard shortcuts', keywords: ['shortcuts', 'hotkeys', 'keyboard'] },
  { id: 'theme', section: 'theme', label: 'Theme', hint: 'Arcane, Void, or Glacier', keywords: ['dark', 'appearance', 'color'] },
  { id: 'gamify', section: 'gamify', label: 'Gamification', hint: 'Enable and pick a mode', keywords: ['ludus', 'rewards', 'xp'] },
];

export function searchSettings(query: string): SettingEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  return SETTINGS_INDEX.filter(
    s =>
      s.label.toLowerCase().includes(q) ||
      s.hint.toLowerCase().includes(q) ||
      s.keywords.some(k => k.includes(q)),
  );
}
```

- [ ] **Step 3: Run to verify pass**

Run: `pnpm vitest run src/components/surfaces/Settings/settingsIndex.test.ts` — PASS.

- [ ] **Step 4: Deep-link seed + filter box in SettingsView**

In `SettingsView` (top of the component): consume a deep-link seed (same localStorage pattern as the search seed):

```tsx
  // Deep link from omni-search: { section: string }
  useEffect(() => {
    try {
      const raw = localStorage.getItem('vox_settings_seed');
      if (raw) {
        localStorage.removeItem('vox_settings_seed');
        const seed = JSON.parse(raw) as { section?: string };
        if (seed.section) setSection(seed.section);
      }
    } catch { /* ignore malformed seed */ }
  }, []);
```

Add a filter input above the section nav that uses `searchSettings(q)` and renders matches as clickable rows (`onClick={() => setSection(entry.section)}`), clearing the filter on click.

- [ ] **Step 5: Verify + commit**

Run: `pnpm vitest run` (settings tests) + `pnpm build`.

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/
git commit -m "feat(gui): searchable settings index with deep-link seed (SSOT for omni-search)"
```

---

# Track G — Omni-search + sidebar filter + keyboard polish

### Task G1: Docs index Tauri command

**Files:**
- Create: `crates/vox-gui/src/commands/docs_index.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`, `crates/vox-gui/src/main.rs`

Every authored doc under `docs/src/` is **mandated** to carry YAML frontmatter (`title`/`description`/`category`) — that's the index.

- [ ] **Step 1: Write the failing test** (inline in docs_index.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_title_and_description() {
        let md = "---\ntitle: Mesh SSOT\ndescription: Seven phases of mesh work\ncategory: architecture\n---\n\n# Body\n";
        let fm = parse_frontmatter(md).expect("frontmatter");
        assert_eq!(fm.title, "Mesh SSOT");
        assert_eq!(fm.description, "Seven phases of mesh work");
    }

    #[test]
    fn missing_frontmatter_returns_none() {
        assert!(parse_frontmatter("# Just a heading\n").is_none());
    }
}
```

Run: `cargo test -p vox-gui docs_index` — FAIL (module missing).

- [ ] **Step 2: Implement**

```rust
//! Docs index for omni-search: walks docs/src/**/*.md frontmatter.
//! Authored docs are required to carry title/description frontmatter
//! (see documentation-governance.md), so this is a complete index.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DocEntry {
    pub title: String,
    pub description: String,
    pub path: String, // absolute path for open_locator
}

pub(crate) struct Frontmatter {
    pub title: String,
    pub description: String,
}

pub(crate) fn parse_frontmatter(content: &str) -> Option<Frontmatter> {
    let rest = content.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let block = &rest[..end];
    let mut title = None;
    let mut description = None;
    for line in block.lines() {
        if let Some(v) = line.strip_prefix("title:") {
            title = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(v) = line.strip_prefix("description:") {
            description = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    Some(Frontmatter {
        title: title?,
        description: description.unwrap_or_default(),
    })
}

fn walk_docs(root: &std::path::Path, out: &mut Vec<DocEntry>) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_docs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(fm) = parse_frontmatter(&content) {
                    out.push(DocEntry {
                        title: fm.title,
                        description: fm.description,
                        path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }
}

static DOCS_CACHE: OnceLock<Vec<DocEntry>> = OnceLock::new();

#[tauri::command]
pub async fn vox_docs_index() -> Result<Vec<DocEntry>, String> {
    Ok(DOCS_CACHE
        .get_or_init(|| {
            let hint = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let repo = vox_repository::discover_repository_or_fallback(&hint);
            let mut out = Vec::new();
            walk_docs(&repo.root.join("docs").join("src"), &mut out);
            out.sort_by(|a, b| a.title.cmp(&b.title));
            out
        })
        .clone())
}
```

(`vox_repository::discover_repository_or_fallback` is already used by vox-gui — see `orch_daemon/mod.rs:153` for the call shape; confirm vox-gui depends on `vox-repository`, which `search.rs` already does per the audit.) Register module + command.

- [ ] **Step 3: Run to verify pass**

Run: `cargo test -p vox-gui docs_index && cargo check -p vox-gui` — PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/src/commands/
git commit -m "feat(gui): vox_docs_index — frontmatter-driven docs index for omni-search"
```

### Task G2: Palette federation logic (pure, tested)

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/paletteSources.ts`
- Create: `crates/vox-gui/ui/src/components/layout/paletteSources.test.ts`

- [ ] **Step 1: Failing tests**

```ts
import { describe, it, expect } from 'vitest';
import { buildPaletteItems, PaletteItem } from './paletteSources';

const surfaces = [
  { viewKey: 'tasks', cliGroup: null, tier: 'live_backend', navLabel: 'Tasks', navIcon: 'list', navGroup: 'operate' },
  { viewKey: null, cliGroup: 'mens', tier: 'none', navLabel: null, navIcon: null, navGroup: null },
];
const settings = [
  { id: 'llm-max-concurrency', section: 'llm', label: 'Max parallel LLM requests', hint: 'Global ceiling', keywords: ['openrouter'] },
];
const docs = [{ title: 'Mesh SSOT', description: 'phases', path: 'C:/x/mesh.md' }];

describe('buildPaletteItems', () => {
  it('matches surfaces by navLabel and excludes non-navigable entries', () => {
    const items = buildPaletteItems('task', { surfaces, settings, docs, agents: [], skills: [] });
    const surfaceHits = items.filter(i => i.kind === 'surface');
    expect(surfaceHits).toHaveLength(1);
    expect(surfaceHits[0].label).toBe('Tasks');
  });

  it('matches settings by keyword', () => {
    const items = buildPaletteItems('openrouter', { surfaces, settings, docs, agents: [], skills: [] });
    expect(items.some(i => i.kind === 'setting' && i.targetSection === 'llm')).toBe(true);
  });

  it('matches docs by title and carries the path', () => {
    const items = buildPaletteItems('mesh', { surfaces, settings, docs, agents: [], skills: [] });
    const doc = items.find(i => i.kind === 'doc');
    expect(doc?.path).toBe('C:/x/mesh.md');
  });

  it('empty query returns no federation items', () => {
    expect(buildPaletteItems('', { surfaces, settings, docs, agents: [], skills: [] })).toHaveLength(0);
  });
});
```

Run: `pnpm vitest run src/components/layout/paletteSources.test.ts` — FAIL.

- [ ] **Step 2: Implement**

```ts
import { SettingEntry } from '../surfaces/Settings/settingsIndex';

export interface SurfaceEntryLike {
  viewKey: string | null;
  navLabel: string | null;
  navGroup: string | null;
  navIcon: string | null;
  cliGroup: string | null;
  tier: string;
}

export interface DocEntryLike {
  title: string;
  description: string;
  path: string;
}

export type PaletteItem =
  | { kind: 'surface'; label: string; detail: string; viewKey: string }
  | { kind: 'setting'; label: string; detail: string; targetSection: string }
  | { kind: 'doc'; label: string; detail: string; path: string };

interface Sources {
  surfaces: SurfaceEntryLike[];
  settings: SettingEntry[];
  docs: DocEntryLike[];
  agents: unknown[]; // filtered by caller; present for arity stability
  skills: unknown[];
}

const MAX_PER_KIND = 5;

export function buildPaletteItems(query: string, sources: Sources): PaletteItem[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const items: PaletteItem[] = [];

  for (const s of sources.surfaces) {
    if (!s.viewKey || !s.navLabel) continue;
    if (s.navLabel.toLowerCase().includes(q) || (s.navGroup ?? '').toLowerCase().includes(q)) {
      items.push({ kind: 'surface', label: s.navLabel, detail: s.navGroup ?? '', viewKey: s.viewKey });
    }
  }

  let settingCount = 0;
  for (const s of sources.settings) {
    if (settingCount >= MAX_PER_KIND) break;
    if (
      s.label.toLowerCase().includes(q) ||
      s.hint.toLowerCase().includes(q) ||
      s.keywords.some(k => k.includes(q))
    ) {
      items.push({ kind: 'setting', label: s.label, detail: s.hint, targetSection: s.section });
      settingCount += 1;
    }
  }

  let docCount = 0;
  for (const d of sources.docs) {
    if (docCount >= MAX_PER_KIND) break;
    if (d.title.toLowerCase().includes(q) || d.description.toLowerCase().includes(q)) {
      items.push({ kind: 'doc', label: d.title, detail: d.description, path: d.path });
      docCount += 1;
    }
  }

  return items;
}
```

- [ ] **Step 3: Run to verify pass**

Run: `pnpm vitest run src/components/layout/paletteSources.test.ts` — PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/paletteSources.*
git commit -m "feat(gui): palette federation logic — surfaces, settings, docs sources"
```

### Task G3: Omni-CommandPalette + App actions

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/CommandPalette.tsx`
- Modify: `crates/vox-gui/ui/src/App.tsx` (`handleCommandAction`, ~line 532)

- [ ] **Step 1: Extend the palette**

Changes to `CommandPalette.tsx` (keep the existing structure; this is additive, not a rewrite):

1. Imports + new state:

```tsx
import { SURFACE_REGISTRY } from '../../generated/surfaceRegistry.generated';
import { SETTINGS_INDEX } from '../surfaces/Settings/settingsIndex';
import { buildPaletteItems, PaletteItem, DocEntryLike } from './paletteSources';
```

```tsx
  const [docs, setDocs] = useState<DocEntryLike[]>([]);
  useEffect(() => {
    if (!open || docs.length > 0) return;
    invoke<DocEntryLike[]>('vox_docs_index').then(setDocs).catch(() => setDocs([]));
  }, [open, docs.length]);
```

2. Compute federation items next to the existing client filters (after `filteredSkills`):

```tsx
  const fedItems = buildPaletteItems(q, {
    surfaces: SURFACE_REGISTRY,
    settings: SETTINGS_INDEX,
    docs,
    agents: [],
    skills: [],
  });
```

3. Unified keyboard selection with wrap-around. Replace the `selectedBackendIdx` mechanism with a single flat selection list. Define after the filters:

```tsx
  type SelEntry =
    | { sel: 'agent'; agent: Agent }
    | { sel: 'skill'; skill: CommandCatalogEntry }
    | { sel: 'fed'; item: PaletteItem }
    | { sel: 'hit'; hit: UnifiedHit };

  const selectable: SelEntry[] = [
    ...filteredAgents.map(agent => ({ sel: 'agent' as const, agent })),
    ...filteredSkills.map(skill => ({ sel: 'skill' as const, skill })),
    ...fedItems.map(item => ({ sel: 'fed' as const, item })),
    ...backendHits.map(hit => ({ sel: 'hit' as const, hit })),
  ];
```

Replace the keydown handler:

```tsx
      } else if (e.key === 'ArrowDown' && selectable.length > 0) {
        e.preventDefault();
        setSelectedIdx(i => (i + 1) % selectable.length);
      } else if (e.key === 'ArrowUp' && selectable.length > 0) {
        e.preventDefault();
        setSelectedIdx(i => (i <= 0 ? selectable.length - 1 : i - 1));
      } else if (e.key === 'Enter' && selectedIdx >= 0 && selectedIdx < selectable.length) {
        e.preventDefault();
        activate(selectable[selectedIdx]);
      }
```

with `selectedIdx` replacing `selectedBackendIdx` (reset to `-1` on query change/close) and:

```tsx
  const activate = useCallback((entry: SelEntry) => {
    if (entry.sel === 'agent') { onAction(entry.agent); onClose(); return; }
    if (entry.sel === 'skill') { onAction(entry.skill); onClose(); return; }
    if (entry.sel === 'hit') { openHit(entry.hit); return; }
    const item = entry.item;
    if (item.kind === 'surface') {
      onAction({ id: 'navigate', viewKey: item.viewKey });
    } else if (item.kind === 'setting') {
      try { localStorage.setItem('vox_settings_seed', JSON.stringify({ section: item.targetSection })); } catch { /* ignore */ }
      onAction({ id: 'navigate', viewKey: 'settings' });
    } else {
      invoke('open_locator', { locator: { kind: 'file', path: item.path } }).catch(() => {});
    }
    onClose();
  }, [onAction, onClose, openHit]);
```

(Check `open_locator`'s expected payload shape in `search.rs` `OpenLocatorDto` — mirror the field names exactly.)

4. Render the three new sections between Skills and the backend results, with the selected style applied via the flat index (each rendered button computes its flat index from section offsets and applies the same `bg-brass/[0.08] border border-brass/20` selected classes the backend hits use today). Apply that same selected-style to agents/skills rows too — this fixes audit finding #11.

5. Update placeholder text: `"Search commands, settings, docs, windows, agents…"`.

- [ ] **Step 2: Route navigation in App**

Verified — `handleCommandAction` (App.tsx:532-549) is an if/else-if chain ending in `else if (cmd.id === 'search') { setActiveView('search'); } else { pushToast(...) }`. Insert a branch before the final `else`:

```ts
    } else if (cmd.id === 'navigate' && typeof cmd.viewKey === 'string') {
      setActiveView(cmd.viewKey as View);
    } else {
```

(`View` is the union type used by `useLocalStorage<View>('vox_active_view', …)` at line 159 — extend it if the registry-driven keys aren't all members; if `View` is derived from the surface registry already, the cast is enough.)

- [ ] **Step 3: Verify + commit**

Run: `pnpm vitest run && pnpm build` (ui).

```bash
git add crates/vox-gui/ui/src/components/layout/CommandPalette.tsx crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui): omni-search palette — settings, surfaces, docs + unified keyboard selection"
```

### Task G4: Sidebar filter

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`

- [ ] **Step 1: Add the filter input + filtering**

At the top of the Sidebar component add state `const [filter, setFilter] = useState('');` and render an input above the sections (styled like the palette input, compact):

```tsx
      <div className="px-3 pb-2">
        <input
          value={filter}
          onChange={e => setFilter(e.target.value)}
          placeholder="Filter…"
          aria-label="Filter sidebar"
          className="w-full rounded-lg border border-white/10 bg-white/[0.03] px-2.5 py-1.5 text-[12px] text-zinc-200 placeholder:text-zinc-600 outline-none focus:border-brass/30"
        />
      </div>
```

Verified internals: collapse state is `collapsedSections` (`useLocalStorage<Record<string, boolean>>('vox_nav_sections', …)` at lines 118-120); items come from `itemsForGroup(group)` (lines 87-90); rendering goes through `renderSection(id, label, items)` (lines 161-186) whose open condition is `const open = !collapsedSections[id] || containsActive;`. Apply the filter at the call sites (lines 188-195) and in the open condition:

```tsx
  const visibleItems = (group: string) => {
    const items = itemsForGroup(group);
    const f = filter.trim().toLowerCase();
    if (!f) return items;
    return items.filter(e => (e.navLabel ?? '').toLowerCase().includes(f));
  };
```

- At each `renderSection(id, label, itemsForGroup(group))` call site, pass `visibleItems(group)` instead — `renderSection` already returns `null` for empty item lists (line 162), so filtered-out sections disappear for free.
- In `renderSection`, change the open condition to `const open = !collapsedSections[id] || containsActive || filter.trim().length > 0;` (filter state must be in scope — declare it in the component body above `renderSection`).
- Skip the filter input entirely in rail mode (the `collapsed` boolean prop branch at line 163) — there's no room for it.

- [ ] **Step 2: Verify + commit**

Run: `pnpm build` (ui).

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx
git commit -m "feat(gui): sidebar filter — auto-expands sections, hides empty groups"
```

### Task G5: SearchView keyboard polish

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx`

- [ ] **Step 1: Wrap-around arrows**

At lines ~348–357, change the clamped index math to modular:

```tsx
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelected(i => (hits.length === 0 ? -1 : (i + 1) % hits.length));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelected(i => (hits.length === 0 ? -1 : i <= 0 ? hits.length - 1 : i - 1));
      }
```

(Adapt local names `setSelected`/`hits` to the actual ones at that site.)

- [ ] **Step 2: Facet chip focus rings**

In the `ScopeChip`/facet chip components (lines ~32–77), append to the button className:

```
focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40
```

- [ ] **Step 3: Verify + commit**

Run: `pnpm build` (ui).

```bash
git add crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx
git commit -m "fix(gui): search keyboard wrap-around + facet focus rings"
```

---

# Track I — Multi-tab chat sessions

**Why tabs, and what they are.** The orchestrator already threads `session_id` through everything that matters: `AgentTask.session_id` (`tasks.rs:408`), every task event (`TaskSubmitted/Started/Completed/Failed` all carry `session_id: Option<String>`, `events.rs:147-175`), the A2A `RemoteTaskEnvelope` (`a2a/envelope.rs`), and Socrates context envelopes (`socrates.rs:8-12`, key `context_envelope:{session_id}`). The only thing missing is the front half: the GUI hardcodes `session_id: 'gui-loquela'` (App.tsx:421) and renders one transcript. So "multiple tabs" = **in-app session tabs over the existing session spine** — NOT multiple OS windows (tauri.conf.json defines a single `main` window; multi-window state sharing is a much bigger lift for no user-visible gain).

**Resource model (write this into the Tasks UI copy):** tabs do NOT partition the fleet. All sessions submit into the same agent queues; the orchestrator divides work by file affinity, priority, and load exactly as it does today (`resolve_route` consults `FileAffinityMap` + `FileLockManager`; `rebalance()` work-steals between agents respecting write locks — `orchestrator/scaling.rs:45-120`). Two tabs working on different parts of the codebase get naturally parallel agents because their file manifests don't overlap; two tabs touching the same files get serialized by the lock manager and `SplitChanges` isolation (`isolation.rs:62-74` auto-selects it on predicted overlap). A tab is a *view + attribution scope*, not a resource reservation.

### Task I1: Session model helpers (pure, tested)

**Files:**
- Create: `crates/vox-gui/ui/src/lib/sessions.ts`
- Create: `crates/vox-gui/ui/src/lib/sessions.test.ts`

- [ ] **Step 1: Failing tests**

```ts
import { describe, it, expect } from 'vitest';
import { createSession, closeSession, renameSession, ChatSession } from './sessions';

describe('createSession', () => {
  it('mints unique gui- ids and numbered default titles', () => {
    const a = createSession([]);
    const b = createSession([a]);
    expect(a.id).toMatch(/^gui-/);
    expect(b.id).not.toBe(a.id);
    expect(a.title).toBe('Chat 1');
    expect(b.title).toBe('Chat 2');
  });

  it('attaches optional scope paths', () => {
    const s = createSession([], { scopePaths: ['crates/vox-gui'] });
    expect(s.scopePaths).toEqual(['crates/vox-gui']);
  });
});

describe('closeSession', () => {
  it('removes the session and nominates a neighbor as next active', () => {
    const a = createSession([]);
    const b = createSession([a]);
    const { sessions, nextActiveId } = closeSession([a, b], a.id);
    expect(sessions).toHaveLength(1);
    expect(nextActiveId).toBe(b.id);
  });

  it('never closes the last session — returns it unchanged', () => {
    const a = createSession([]);
    const { sessions, nextActiveId } = closeSession([a], a.id);
    expect(sessions).toHaveLength(1);
    expect(nextActiveId).toBe(a.id);
  });
});

describe('renameSession', () => {
  it('renames by id and ignores unknown ids', () => {
    const a = createSession([]);
    expect(renameSession([a], a.id, 'Mesh work')[0].title).toBe('Mesh work');
    expect(renameSession([a], 'nope', 'x')[0].title).toBe('Chat 1');
  });
});
```

Run (from `crates/vox-gui/ui/`): `pnpm vitest run src/lib/sessions.test.ts` — FAIL (module missing).

- [ ] **Step 2: Implement**

```ts
export interface ChatSession {
  id: string;          // session_id sent to the orchestrator, prefix 'gui-'
  title: string;
  createdAt: number;
  /** Paths auto-attached to every submission's file affinity (working set). */
  scopePaths: string[];
}

let counter = 0;

export function createSession(
  existing: ChatSession[],
  opts?: { scopePaths?: string[] },
): ChatSession {
  counter += 1;
  // Unique without Date.now collisions across fast double-clicks.
  const id = `gui-${Date.now().toString(36)}-${counter.toString(36)}`;
  const n =
    existing.reduce((max, s) => {
      const m = /^Chat (\d+)$/.exec(s.title);
      return m ? Math.max(max, Number(m[1])) : max;
    }, 0) + 1;
  return { id, title: `Chat ${n}`, createdAt: Date.now(), scopePaths: opts?.scopePaths ?? [] };
}

export function closeSession(
  sessions: ChatSession[],
  id: string,
): { sessions: ChatSession[]; nextActiveId: string } {
  if (sessions.length <= 1) {
    return { sessions, nextActiveId: sessions[0]?.id ?? '' };
  }
  const idx = sessions.findIndex(s => s.id === id);
  if (idx === -1) return { sessions, nextActiveId: sessions[0].id };
  const remaining = sessions.filter(s => s.id !== id);
  const neighbor = remaining[Math.max(0, idx - 1)] ?? remaining[0];
  return { sessions: remaining, nextActiveId: neighbor.id };
}

export function renameSession(
  sessions: ChatSession[],
  id: string,
  title: string,
): ChatSession[] {
  const t = title.trim();
  if (!t) return sessions;
  return sessions.map(s => (s.id === id ? { ...s, title: t } : s));
}
```

- [ ] **Step 3: Run to verify pass**

Run: `pnpm vitest run src/lib/sessions.test.ts` — PASS (6 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/lib/sessions.*
git commit -m "feat(gui): chat-session model helpers (create/close/rename) with vitest"
```

### Task I2: Session tabs + per-session transcript

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/SessionTabs.tsx`
- Modify: `crates/vox-gui/ui/src/lib/chatCorrelation.ts` (+ its existing `chatCorrelation.test.ts`)
- Modify: `crates/vox-gui/ui/src/App.tsx`

- [ ] **Step 1: Extend the chat reducer with sessionId (failing test first)**

Read `chatCorrelation.ts` fully before editing — it owns `ChatMessage { id, role, text, status, runId, taskId?, error? }`, the reducer, and the `agentToTask`/`taskToRun` maps; `chatCorrelation.test.ts` shows the action shapes. Add to its existing test file:

```ts
  it('submit carries sessionId onto both bubbles and filtering selects them', () => {
    let state = initialChatState; // use the exported initial-state name from this module
    state = chatReducer(state, { type: 'submit', runId: 'r1', prompt: 'p', sessionId: 'gui-a' });
    state = chatReducer(state, { type: 'submit', runId: 'r2', prompt: 'q', sessionId: 'gui-b' });
    const a = messagesForSession(state, 'gui-a');
    expect(a).toHaveLength(2); // user + pending assistant
    expect(a.every(m => m.sessionId === 'gui-a')).toBe(true);
  });
```

(Adapt `initialChatState`/`chatReducer` to the module's real export names — they exist; the test file imports them today.) Run: `pnpm vitest run src/lib/chatCorrelation.test.ts` — FAIL.

Implement: add `sessionId?: string` to `ChatMessage`; the `'submit'` action gains `sessionId` and stamps it on both created bubbles; `'agentEvent'`/`'submitResolved'` need no change (they find messages by runId/taskId). Export:

```ts
export function messagesForSession(state: ChatState, sessionId: string): ChatMessage[] {
  return state.messages.filter(m => m.sessionId === sessionId || m.sessionId == null);
}
```

(`== null` keeps pre-existing messages visible in whatever tab is active rather than orphaning them.) Run the test — PASS.

- [ ] **Step 2: SessionTabs component**

```tsx
import React, { useState } from 'react';
import { Icon } from '../ui/Icons';
import { ChatSession } from '../../lib/sessions';

interface SessionTabsProps {
  sessions: ChatSession[];
  activeId: string;
  onSelect: (id: string) => void;
  onNew: () => void;
  onClose: (id: string) => void;
  onRename: (id: string, title: string) => void;
  /** queued-task count per session id (from TasksView data), for badges */
  queuedBySession?: Record<string, number>;
}

export function SessionTabs({
  sessions, activeId, onSelect, onNew, onClose, onRename, queuedBySession,
}: SessionTabsProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState('');

  return (
    <div className="flex items-center gap-1 overflow-x-auto custom-scrollbar px-1 pb-1">
      {sessions.map(s => {
        const active = s.id === activeId;
        const queued = queuedBySession?.[s.id] ?? 0;
        return (
          <div
            key={s.id}
            className={`group flex shrink-0 items-center gap-1.5 rounded-t-lg border-x border-t px-2.5 py-1 text-[12px] transition ${
              active
                ? 'border-white/10 bg-white/[0.04] text-zinc-100'
                : 'border-transparent text-zinc-500 hover:bg-white/[0.02] hover:text-zinc-300'
            }`}
          >
            {editingId === s.id ? (
              <input
                autoFocus
                value={draft}
                onChange={e => setDraft(e.target.value)}
                onKeyDown={e => {
                  if (e.key === 'Enter') { onRename(s.id, draft); setEditingId(null); }
                  if (e.key === 'Escape') setEditingId(null);
                }}
                onBlur={() => { onRename(s.id, draft); setEditingId(null); }}
                className="w-24 bg-transparent outline-none border-b border-brass/40"
              />
            ) : (
              <button
                onClick={() => onSelect(s.id)}
                onDoubleClick={() => { setEditingId(s.id); setDraft(s.title); }}
                title={`${s.title} — double-click to rename`}
                className="focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40 rounded"
              >
                {s.title}
              </button>
            )}
            {queued > 0 && (
              <span className="rounded-full bg-brass/15 px-1.5 font-mono text-[9px] text-brass">{queued}</span>
            )}
            {sessions.length > 1 && (
              <button
                onClick={() => onClose(s.id)}
                title="Close tab (queued tasks keep running)"
                className="rounded p-0.5 text-zinc-600 opacity-0 transition group-hover:opacity-100 hover:text-zinc-200 focus:outline-none focus-visible:opacity-100"
              >
                <Icon.x className="size-3" />
              </button>
            )}
          </div>
        );
      })}
      <button
        onClick={onNew}
        title="New chat session"
        className="shrink-0 rounded p-1 text-zinc-500 hover:bg-white/[0.04] hover:text-zinc-200 focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40"
      >
        <Icon.plus className="size-3.5" />
      </button>
    </div>
  );
}
```

- [ ] **Step 3: Wire into App**

In `App.tsx`:

```tsx
  const [sessions, setSessions] = useLocalStorage<ChatSession[]>('vox_chat_sessions', []);
  const [activeSessionId, setActiveSessionId] = useLocalStorage<string>('vox_active_session', '');
  // Migration: ensure at least one session exists (adopt the legacy id so old
  // transcripts/tasks submitted as 'gui-loquela' stay attached to a tab).
  useEffect(() => {
    if (sessions.length === 0) {
      const legacy: ChatSession = { id: 'gui-loquela', title: 'Chat 1', createdAt: Date.now(), scopePaths: [] };
      setSessions([legacy]);
      setActiveSessionId(legacy.id);
    } else if (!sessions.some(s => s.id === activeSessionId)) {
      setActiveSessionId(sessions[0].id);
    }
  }, [sessions, activeSessionId, setSessions, setActiveSessionId]);
```

Render `<SessionTabs … />` directly above wherever the Transcript/Loquela pair renders, with handlers delegating to `createSession`/`closeSession`/`renameSession`. Pass the transcript `messagesForSession(chatState, activeSessionId)` instead of the raw message list, and pass `sessionId: activeSessionId` into the chat reducer's `'submit'` dispatch inside `handleLoquelaSubmit`. **Closing a tab does not cancel its tasks** — they keep running and remain visible in TasksView under that session id (state the same in the close button's title text, already done above).

- [ ] **Step 4: Verify + commit**

Run: `pnpm vitest run && pnpm build` (ui).

```bash
git add crates/vox-gui/ui/src/components/layout/SessionTabs.tsx crates/vox-gui/ui/src/lib/chatCorrelation.* crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui): multi-tab chat sessions over the existing session_id spine"
```

### Task I3: Sessions in submit + task-list session filter

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (`handleLoquelaSubmit`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts` (+ test)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx`

- [ ] **Step 1: Replace the hardcoded session id**

In `handleLoquelaSubmit`, change `session_id: payload.session_id ?? 'gui-loquela'` → `session_id: payload.session_id ?? activeSessionId`. Also attach the active session's working set: `files: [...contextFiles, ...activeSession.scopePaths]` (where `activeSession = sessions.find(s => s.id === activeSessionId)`; dedupe with a `Set`).

- [ ] **Step 2: Session filter helper (failing test first)**

Add to `tasksHelpers.test.ts`:

```ts
describe('filterBySession', () => {
  it('returns all rows for null filter and only matching session otherwise', () => {
    const rows = [row({ id: 1, session_id: 'gui-a' }), row({ id: 2, session_id: 'gui-b' }), row({ id: 3, session_id: null })];
    expect(filterBySession(rows, null)).toHaveLength(3);
    expect(filterBySession(rows, 'gui-a').map(t => t.id)).toEqual([1]);
  });
});
```

Implement in `tasksHelpers.ts`:

```ts
export function filterBySession(rows: TaskRow[], sessionId: string | null): TaskRow[] {
  if (!sessionId) return rows;
  return rows.filter(t => t.session_id === sessionId);
}
```

Run: `pnpm vitest run src/components/surfaces/Tasks/tasksHelpers.test.ts` — PASS.

- [ ] **Step 3: Filter chips in TasksView**

Add a session filter row under the header: an "All" chip plus one chip per distinct `session_id` present in `rows` (label it with the session title when the id matches a `ChatSession` from `vox_chat_sessions` localStorage; otherwise the raw id). State `const [sessionFilter, setSessionFilter] = useState<string | null>(null);`, apply `filterBySession(rows, sessionFilter)` before `groupTasks`. Style the chips exactly like SearchView's `ScopeChip` (active: `border-brass/40 bg-brass/10 text-brass`).

- [ ] **Step 4: Verify + commit**

Run: `pnpm vitest run && pnpm build` (ui).

```bash
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/Tasks/
git commit -m "feat(gui): per-session task attribution — active tab feeds session_id, tasks filterable by session"
```

### Task I4: Per-tab working-set scope (different parts of the codebase)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/SessionTabs.tsx` or Loquela chips area
- Modify: `crates/vox-gui/ui/src/App.tsx`

- [ ] **Step 1: Scope editor**

Loquela already has a context-chips mechanism (`chips`/`setChips` props, file/url chips). Reuse it: when the user pins a chip (add a small pin toggle on file-kind chips), write its path into the active session's `scopePaths` (`setSessions(renameless update)`), and show pinned chips with a filled style. On session switch, hydrate the pinned chips from `scopePaths`. This gives each tab a persistent working set — e.g. tab 1 pinned to `crates/vox-gui`, tab 2 to `crates/vox-populi` — which flows into `file_manifest` on every submit (I3 Step 1), which is exactly what `resolve_route` uses to keep the two streams on different agents and what `choose_strategy` uses to pick VCS isolation on overlap.

Implementation detail: read the chip component in `Loquela.tsx` first; the pin toggle is a small button inside the chip with `Icon.link` (exists) or a `●` glyph; pinned state = membership in `activeSession.scopePaths`.

- [ ] **Step 2: Verify + commit**

Run: `pnpm build` (ui). Manual check: pin a path in tab 1, switch tabs, switch back — chip persists.

```bash
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx crates/vox-gui/ui/src/components/layout/SessionTabs.tsx
git commit -m "feat(gui): pinned working-set chips per chat session feed file affinity"
```

---

# Track J — Intake intelligence: near-duplicate detection + overlap visibility

**Verified baseline:** there is NO duplicate detection in the live submit path — `AgentQueue::enqueue_dedup` (`queue/priority.rs:250`) exists but is never called; `submit_task_with_agent` (`task_submit.rs:52-138`) enqueues unconditionally. Dependencies ARE enforced at dequeue (`drain.rs:6-25` checks `t.is_ready(&self.completed)` against `depends_on`). File contention is already handled at runtime by `FileLockManager` + isolation-strategy selection — so Track J does **detection and user-mediated dedup**, not automatic chaining (the runtime already serializes overlapping writes; silently rewriting user intent is exactly what the user asked to avoid).

### Task J1: Similarity module (pure, tested)

**Files:**
- Create: `crates/vox-orchestrator/src/services/similarity.rs`
- Modify: `crates/vox-orchestrator/src/services/mod.rs` (`pub mod similarity;`)

- [ ] **Step 1: Failing tests** (inline)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_descriptions_score_one() {
        assert!((jaccard("fix the flaky auth test", "fix the flaky auth test") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn disjoint_descriptions_score_zero() {
        assert_eq!(jaccard("refactor mesh dispatch", "write release notes"), 0.0);
    }

    #[test]
    fn near_duplicates_score_high() {
        let a = "Fix the flaky auth test in vox-gui";
        let b = "fix flaky auth test in vox-gui please";
        assert!(jaccard(a, b) > 0.6, "got {}", jaccard(a, b));
    }

    #[test]
    fn case_and_punctuation_are_normalized() {
        assert!((jaccard("Add CI gate!", "add ci gate") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn empty_inputs_score_zero() {
        assert_eq!(jaccard("", "anything"), 0.0);
        assert_eq!(jaccard("", ""), 0.0);
    }
}
```

Run: `cargo test -p vox-orchestrator similarity` — FAIL (module missing).

- [ ] **Step 2: Implement**

```rust
//! Token-set similarity for near-duplicate task detection.
//!
//! Deliberately cheap (no embeddings, no model calls): lowercased alphanumeric
//! token sets + Jaccard. Good enough to catch "the user typed the same ask
//! twice" and "two tabs filed the same bug"; the GUI mediates anything fuzzier.

use std::collections::HashSet;

fn token_set(s: &str) -> HashSet<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// Jaccard similarity of the two descriptions' token sets, in [0, 1].
pub fn jaccard(a: &str, b: &str) -> f64 {
    let sa = token_set(a);
    let sb = token_set(b);
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    inter / union
}

/// Threshold above which two task descriptions are treated as near-duplicates.
pub const NEAR_DUPLICATE_THRESHOLD: f64 = 0.85;
```

- [ ] **Step 3: Run to verify pass**

Run: `cargo test -p vox-orchestrator similarity` — PASS (5 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator/src/services/
git commit -m "feat(orchestrator): token-Jaccard near-duplicate similarity for task intake"
```

### Task J2: Duplicate-aware SUBMIT_TASK + GUI confirm flow

**Files:**
- Modify: `crates/vox-orchestrator/src/orch_daemon/mod.rs` (`SUBMIT_TASK` arm)
- Modify: `crates/vox-gui/src/commands/control_plane.rs`
- Modify: `crates/vox-gui/ui/src/App.tsx` + `TasksView.tsx`
- Test: `task_dispatch_tests`

- [ ] **Step 1: Failing dispatch tests**

```rust
    #[tokio::test]
    async fn near_duplicate_blocked_when_not_allowed() {
        let (orch, first_id) = orch_with_one_task().await; // "first task"
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({
                    "description": "first task",
                    "allow_duplicate": false,
                }),
            ),
        )
        .await;
        let v = result_value(&resp);
        assert_eq!(v["duplicate_of"].as_u64(), Some(first_id));
        assert!(v["task_id"].is_null());
        // Nothing new enqueued:
        assert_eq!(orch.all_tasks().len(), 1);
    }

    #[tokio::test]
    async fn near_duplicate_enqueued_but_flagged_when_allowed() {
        let (orch, first_id) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({ "description": "first task" }), // allow_duplicate defaults true
            ),
        )
        .await;
        let v = result_value(&resp);
        assert!(v["task_id"].as_u64().is_some());
        assert_eq!(v["duplicate_of"].as_u64(), Some(first_id));
        assert_eq!(orch.all_tasks().len(), 2);
    }

    #[tokio::test]
    async fn distinct_task_has_no_duplicate_flag() {
        let (orch, _) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({ "description": "completely unrelated migration work" }),
            ),
        )
        .await;
        assert!(result_value(&resp)["duplicate_of"].is_null());
    }
```

Run: `cargo test -p vox-orchestrator task_dispatch_tests` — FAIL (no `duplicate_of` in response).

- [ ] **Step 2: Implement in the SUBMIT_TASK arm**

In `orch_daemon/mod.rs`, inside the `SUBMIT_TASK` arm after `description` is parsed and **before** the `orch.submit_task_with_agent(...)` call, insert:

```rust
            let allow_duplicate = req
                .params
                .get("allow_duplicate")
                .and_then(|x| x.as_bool())
                .unwrap_or(true);
            // Near-duplicate scan over live (queued + in-progress) tasks.
            let duplicate_of = orch
                .all_tasks()
                .iter()
                .filter(|t| {
                    crate::services::similarity::jaccard(&t.description, description)
                        >= crate::services::similarity::NEAR_DUPLICATE_THRESHOLD
                })
                .map(|t| t.id.0)
                .next();
            if let Some(dup) = duplicate_of {
                if !allow_duplicate {
                    return response_result(
                        &req.id,
                        serde_json::json!({ "task_id": null, "duplicate_of": dup }),
                    );
                }
            }
```

and extend the success response from `json!({ "task_id": task_id.0 })` to:

```rust
                    response_result(
                        &req.id,
                        serde_json::json!({ "task_id": task_id.0, "duplicate_of": duplicate_of }),
                    )
```

- [ ] **Step 3: Run to verify pass**

Run: `cargo test -p vox-orchestrator task_dispatch_tests` — PASS.

- [ ] **Step 4: Thread through Tauri + GUI confirm**

`control_plane.rs`: add `pub allow_duplicate: Option<bool>` to `SubmitTaskInput`, forward `"allow_duplicate": input.allow_duplicate.unwrap_or(true)` in the params, and surface the response field — extend `ControlPlaneResult` with `pub duplicate_of: Option<String>` (set from `response.get("duplicate_of").and_then(|v| v.as_u64()).map(|v| v.to_string())`; existing constructors gain `duplicate_of: None`).

GUI (`App.tsx` `handleLoquelaSubmit` and TasksView `addTask`): submit with `allow_duplicate: false` first; when the result has `duplicate_of` and no `task_id`:

```ts
const dup = res.duplicate_of;
if (dup && !res.task_id) {
  const goAhead = window.confirm(
    `A nearly identical task (#${dup}) is already queued.\n\nOK = add anyway as a separate task\nCancel = skip (open Tasks to edit #${dup} instead)`
  );
  if (!goAhead) {
    pushToast({ tone: 'info', title: 'Skipped duplicate', body: `Existing task #${dup} kept` });
    return;
  }
  // resubmit, explicitly allowing the duplicate
  await executeIpcWithRun('submit_orchestrator_task', { input: { ...inputArgs, allow_duplicate: true } }, 'gui.loquela.submit');
}
```

(In `handleLoquelaSubmit` the chat bubbles are created before the IPC resolves — on the skip path dispatch the existing failure/removal action the reducer has for failed submissions, or set the assistant bubble status to `'failed'` with error text `'skipped: duplicate of #N'`; read the reducer's action set and use what exists.) This is the "incoming work feeding into existing — don't duplicate" control: detection is automatic, the decision is the user's.

- [ ] **Step 5: Verify + commit**

Run: `cargo check -p vox-gui && pnpm build` (ui).

```bash
git add crates/vox-orchestrator/src/orch_daemon/mod.rs crates/vox-gui/src/commands/control_plane.rs crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx
git commit -m "feat: near-duplicate task detection with user-mediated confirm on submit"
```

### Task J3: Overlap + dependency visibility in TasksView

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/tasksHelpers.ts` (+ test)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx`

- [ ] **Step 1: Failing test**

```ts
describe('findWriteOverlaps', () => {
  it('maps each task to the queued task ids sharing a write file', () => {
    const rows = [
      row({ id: 1, write_files: ['a.rs', 'b.rs'] }),
      row({ id: 2, write_files: ['b.rs'] }),
      row({ id: 3, write_files: ['c.rs'] }),
    ];
    const m = findWriteOverlaps(rows);
    expect(m.get(1)).toEqual([2]);
    expect(m.get(2)).toEqual([1]);
    expect(m.get(3)).toBeUndefined();
  });
});
```

- [ ] **Step 2: Implement**

```ts
export function findWriteOverlaps(rows: TaskRow[]): Map<number, number[]> {
  const byFile = new Map<string, number[]>();
  for (const t of rows) {
    for (const f of t.write_files) {
      const list = byFile.get(f) ?? [];
      list.push(t.id);
      byFile.set(f, list);
    }
  }
  const out = new Map<number, number[]>();
  for (const ids of byFile.values()) {
    if (ids.length < 2) continue;
    for (const id of ids) {
      const others = ids.filter(o => o !== id);
      const cur = out.get(id) ?? [];
      out.set(id, [...new Set([...cur, ...others])].sort((a, b) => a - b));
    }
  }
  return out;
}
```

Run: `pnpm vitest run src/components/surfaces/Tasks/tasksHelpers.test.ts` — PASS.

- [ ] **Step 3: Render badges**

In each task row, when `findWriteOverlaps` has an entry, render a small amber chip `⚠ overlaps #2` (title: "These tasks write the same files — the orchestrator serializes them via file locks and may split VCS changes"); when `depends_on` is non-empty render `→ after #N`. Both as chips in the row's metadata line, same chip shell as the priority chip.

- [ ] **Step 4: Verify + commit**

Run: `pnpm vitest run && pnpm build` (ui).

```bash
git add crates/vox-gui/ui/src/components/surfaces/Tasks/
git commit -m "feat(gui): overlap and dependency badges in the task list"
```

---

# Track K — Mesh/A2A visibility + composite remote relief

**Verified baseline:** remote distribution already exists — `RemoteTaskEnvelope` (`a2a/envelope.rs:5-102`, with `idempotency_key`, `exec_lease_id`, `session_id`, `capability_requirements_json`), dispatch machinery in `a2a/dispatch/{mesh,remote_poller,remote_worker}.rs`, `PopuliRemoteDelegate { idempotency_key, lease_id, claimer_node_id }` (`types/tasks.rs:196-207`), and routing hints written by `mesh_federation_poll.rs` via `set_remote_populi_routing_hints` and consumed at `runtime.rs:734-741`. Track K makes the existing machinery *visible* and the scaling relief smarter — it does not build new distribution.

### Task K1: Remote-delegation badge in the task list

**Files:**
- Modify: `crates/vox-orchestrator/src/orch_daemon/mod.rs` (LIST_TASKS arm)
- Modify: `crates/vox-gui/src/commands/control_plane.rs` (TaskRowDto)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/` (TaskRow + render)

- [ ] **Step 1: Find the delegate field**

Grep `PopuliRemoteDelegate` in `types/tasks.rs` to find the field name on `AgentTask` that holds it (the struct is defined at lines 196-207; the field is nearby in the AgentTask definition — likely `populi_remote: Option<PopuliRemoteDelegate>`). Use the real name below.

- [ ] **Step 2: Extend LIST_TASKS + DTO + UI**

In the LIST_TASKS json: `"remote_node": t.<field>.as_ref().and_then(|d| d.claimer_node_id.clone()),`. In `TaskRowDto` + the TS `TaskRow`: `remote_node: Option<String>` / `remote_node: string | null` (extend the test factory default with `remote_node: null`). In the row render, when set: a chip `mesh: {remote_node}` (title: "Executing remotely via A2A lease"). Extend the A1 dispatch test: assert the field is present (null for a local task).

- [ ] **Step 3: Verify + commit**

Run: `cargo test -p vox-orchestrator task_dispatch_tests && pnpm vitest run && pnpm build`.

```bash
git add crates/vox-orchestrator/src/orch_daemon/mod.rs crates/vox-gui/src/commands/control_plane.rs crates/vox-gui/ui/src/components/surfaces/Tasks/
git commit -m "feat: surface A2A remote delegation (claimer node) in the task list"
```

### Task K2: Composite remote-capacity relief in scaling

**Files:**
- Modify: `crates/vox-orchestrator/src/runtime.rs` (~line 734-741, the hint→capacity mapping)
- Test: `services/scaling.rs` (existing `remote_gpu_capacity_reduces_scale_up_pressure` test stays green)

- [ ] **Step 1: Read the hint struct**

Open `runtime.rs:734-741` and the writer (`mesh_federation_poll.rs` → `set_remote_populi_routing_hints` in `orchestrator/agent/registration.rs`) to learn the hint element type and which capacity fields it carries (GPU count is currently extracted; check for memory/CPU fields).

- [ ] **Step 2: Composite capacity**

Today the mapping reduces hints to a GPU count. Replace the reduction with a composite "relief units" integer: `gpu_count + (free_mem_gb / 8).floor()` per remote node, summed (when the hint exposes memory; if it only carries GPU data, extend the hint struct where it's defined and populate it in `mesh_federation_poll.rs` from the same node records that feed GPU counts — they're `NodeRecord`s, which carry `memory_free_bytes`). Keep the variable name `remote_gpu_capacity` → rename to `remote_capacity_units` at the call site and in `decide_scaling`'s parameter docs (the parameter is already generically "capacity relief"; `ScalingService` math is unchanged). Add a unit test in `scaling.rs` mirroring `remote_gpu_capacity_reduces_scale_up_pressure` but with capacity coming from the memory term.

- [ ] **Step 3: Verify + commit**

Run: `cargo test -p vox-orchestrator scaling`.

```bash
git add crates/vox-orchestrator/src/runtime.rs crates/vox-orchestrator/src/services/scaling.rs crates/vox-orchestrator/src/mesh_federation_poll.rs
git commit -m "feat(orchestrator): composite remote capacity relief (GPU + memory) in scaling"
```

### Task K3: Mesh resources card in the GUI

**Files:**
- Create: `crates/vox-gui/src/commands/mesh_resources.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`, `crates/vox-gui/src/main.rs`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` (mesh section) — or the Populi surface if it has a resources area; check both, put it where node trust already lives

- [ ] **Step 1: Tauri command**

```rust
//! Mesh resource summary bridge — calls the populi control plane's
//! /v1/populi/resources/summary endpoint (Track D1).

#[tauri::command]
pub async fn get_mesh_resource_summary() -> Result<serde_json::Value, String> {
    // Resolve the control-plane base URL the same way the mesh join path does:
    // VOX_ORCHESTRATOR_MESH_CONTROL_URL > VOX_MESH_CONTROL_ADDR > Vox.toml [mesh].control_url.
    // Find the existing resolution helper in vox-populi (http_lifecycle.rs uses it)
    // or in whatever vox-gui code backs the existing mesh settings section
    // (it already lists nodes via invoke_mcp_tool('vox_mesh_nodes') — if that
    // MCP path is the only working transport, mirror it instead of raw HTTP).
    let base = std::env::var("VOX_ORCHESTRATOR_MESH_CONTROL_URL")
        .or_else(|_| std::env::var("VOX_MESH_CONTROL_ADDR"))
        .map_err(|_| "mesh control URL not configured".to_string())?;
    let url = format!("{}/v1/populi/resources/summary", base.trim_end_matches('/'));
    let client = vox_http_client::client();
    let mut req = client.get(&url);
    if let Ok(token) = std::env::var("VOX_MESH_TOKEN") {
        req = req.bearer_auth(token);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("mesh control plane returned {}", res.status()));
    }
    res.json().await.map_err(|e| e.to_string())
}
```

(Vox.toml `[mesh].control_url` fallback: parse via `vox_repository`'s populi_toml reader — `crates/vox-repository/src/populi_toml.rs:29-49` — if the env vars are unset; add it if straightforward, else env-only is acceptable for this slice and the error string tells the user what to set.)

- [ ] **Step 2: Card in the mesh settings section**

In the mesh section of SettingsView (the one listing nodes with trust toggles), add above the node list:

```tsx
{summary && (
  <div className="grid grid-cols-4 gap-2 rounded-lg border border-white/10 bg-white/[0.02] p-3 text-center">
    <div><div className="text-[18px] text-zinc-100">{summary.eligible_node_count}/{summary.node_count}</div><div className="text-[9px] uppercase tracking-widest text-zinc-500">nodes ready</div></div>
    <div><div className="text-[18px] text-zinc-100">{summary.gpu_allocatable_total}</div><div className="text-[9px] uppercase tracking-widest text-zinc-500">GPUs free</div></div>
    <div><div className="text-[18px] text-zinc-100">{(summary.memory_free_bytes_total / 2 ** 30).toFixed(0)} GiB</div><div className="text-[9px] uppercase tracking-widest text-zinc-500">RAM free</div></div>
    <div><div className="text-[18px] text-zinc-100">{summary.cpu_usage_pct_avg.toFixed(0)}%</div><div className="text-[9px] uppercase tracking-widest text-zinc-500">avg CPU</div></div>
  </div>
)}
```

loaded via `invoke('get_mesh_resource_summary')` in the section's existing load effect, with failures non-fatal (no mesh configured → card hidden).

- [ ] **Step 3: Verify + commit**

Run: `cargo check -p vox-gui && pnpm build` (ui).

```bash
git add crates/vox-gui/src/commands/ crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx
git commit -m "feat(gui): mesh resource summary card — nodes/GPUs/RAM/CPU at a glance"
```

---

# Track H — Documentation + final gates

### Task H1: Where-things-live + docs

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Add rows** (match the table's existing column format exactly):

| Concept | Lives in |
|---|---|
| GUI task list (queue CRUD) | `vox-gui/ui/src/components/surfaces/Tasks/` + `vox-orchestrator orch.list_tasks/orch.edit_task` |
| LLM concurrency throttle (AIMD, 429) | `vox-actor-runtime::llm::throttle` |
| Mesh resource summary endpoint | `vox-populi /v1/populi/resources/summary` |
| Local CPU/RAM probe for scaling | `vox-orchestrator::services::local_resources` |
| Settings search index | `vox-gui/ui/.../Settings/settingsIndex.ts` |
| Chat session tabs (multi-tab) | `vox-gui/ui/src/lib/sessions.ts` + `components/layout/SessionTabs.tsx` |
| Near-duplicate task detection | `vox-orchestrator::services::similarity` (consumed in `orch_daemon` SUBMIT_TASK) |

- [ ] **Step 2: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs: where-things-live rows for tasks surface, llm throttle, mesh resources"
```

### Task H2: Full verification sweep

- [ ] **Step 1: Rust gates**

```bash
cargo test -p vox-orchestrator
cargo test -p vox-orchestrator --features system-metrics local_resources
cargo test -p vox-populi
cargo test -p vox-config
cargo test -p vox-actor-runtime
cargo check -p vox-gui
cargo run -p vox-arch-check
```

Expected: all green. If arch-check flags the new `services/local_resources.rs` or LoC budgets, address per `docs/src/architecture/layers.toml` (vox-actor-runtime and vox-orchestrator additions here are small; budgets should hold).

- [ ] **Step 2: Frontend gates** (from `crates/vox-gui/ui/`)

```bash
pnpm vitest run
pnpm build
```

Expected: all vitest suites green (existing 11 + new), build clean.

- [ ] **Step 3: SSOT gates**

```bash
vox ci gui-surface-registry
vox ci ssot-drift
```

Expected: pass (surface registry regenerated in B1; no CLI command names changed so catalog sync is unaffected).

- [ ] **Step 4: Format touched crates**

```bash
cargo fmt -p vox-orchestrator -p vox-populi -p vox-config -p vox-actor-runtime -p vox-gui -p vox-foundation
```

(Per-crate fmt only; **never** `cargo fmt --all`.)

- [ ] **Step 5: Final commit if formatting changed anything**

```bash
git add -A
git commit -m "chore: fmt sweep for async-chat/tasklist/scaling/omnisearch tracks"
```

---

## Deliberately out of scope (do not build)

- **Hopper wiring / persistence (Hp-T5)** — the GUI task list rides the live agent queues; the hopper remains the future intake-classifier seam. Wiring it now would create two sources of truth.
- **Mesh gossip of resources** — heartbeat + control-plane pull stays; D1's summary endpoint is the aggregation layer. Gossip belongs to the mesh SSOT phases (P3+).
- **Auto-spawning remote nodes** — scaling adjusts the local fleet and *relieves* pressure using remote capacity (existing hints, K2 makes them composite); the A2A remote-execution path already exists (`a2a/dispatch/`) and Track K only *surfaces* it.
- **Automatic task chaining / silent dedup** — file contention is already serialized at runtime by `FileLockManager` + isolation-strategy selection; near-duplicates are *detected* (J2) but the decision is always the user's. Rewriting `depends_on` behind the user's back contradicts the manual-control requirement.
- **Embedding/LLM-based similarity** — token Jaccard (J1) is the intake bar; anything fuzzier goes through the GUI confirm. An embedding upgrade can swap into `services::similarity` later without touching the daemon protocol.
- **Multiple OS windows** — tabs are in-app over the session_id spine (Track I); Tauri multi-window adds per-window state plumbing with no additional capability.
- **Per-session/tenant resource reservations** — tabs share the fleet; `tenant_id` budget gating exists for billing-style quotas and is not extended here.
- **Backend docs corpus in vox-search/tantivy** — the frontmatter index (G1) covers GUI search; a tantivy docs corpus can supersede it later without UI changes.
- **`vox config` CLI surface for `[llm]`** — `known_keys` registration in E1 makes `vox config set llm.max_concurrent_requests 16` work for free; no new CLI commands.

## Execution order

A → B → (C, D, E in any order) → F → G → I → J → K → H. The A-track RPCs are the substrate for B/I3/J2/K1; F4's `SETTINGS_INDEX` precedes G2/G3; everything else parallelizes. Each task is one commit; each track ends with its crate-local gates green.

## Self-review notes

- Spec coverage: async chat (already true; gap was visibility → B), task list with full manual icon-driven control (A+B: add/edit/remove/reprioritize, per-session filtering I3), resource awareness incl. mesh nodes + CPU/GPU broadcast (D1/D2/K2/K3), dynamic scale up/down (D3 + existing ScalingService), user-configurable in settings extending the SSOT (E1/F1/F3), OpenRouter limits researched + parallelism configurable (E1–E3), omni-search across commands/settings/docs/windows (G1–G3), sidebar search (G4), visual/keyboard bugs (C2, G3 selected-states, G5), settings hydration + theme bugs (F1/F2), multiple tabs over the session spine (I1–I4), intelligent division of incoming work with dedup under manual control (J1–J3 + existing affinity routing/rebalance documented in Track I preamble), A2A distribution surfaced (K1) with composite capacity relief (K2).
- Type consistency: `TaskRow`/`TaskRowDto` field names match (`agent_id`, `session_id`, `write_files`, `remote_node` added in K1, `lifecycle` normalized in the Tauri layer); `cyclePriority` values match daemon `REORDER_TASK` lowercase strings; `vox_settings_seed` shared between F4 and G3; `SettingEntry` shared between F4 and G2; `ChatSession` shared between I1/I2/I4; `duplicate_of` shape shared between J2's daemon arm, `ControlPlaneResult`, and both GUI submit paths.
- Verification pass (2026-06-12): four read-only audit agents checked every signature/field anchor; all corrections are folded inline and the cross-cutting ones are listed in the Verification addendum. Remaining read-and-mirror points are deliberate (un-excerpted code regions: SettingsView `update()` internals, theme value strings, `OpenLocatorDto` exact fields, `AgentTask`'s populi-delegate field name, hint struct in K2, secrets accessor in E3) — each names the exact file/lines to read first.
