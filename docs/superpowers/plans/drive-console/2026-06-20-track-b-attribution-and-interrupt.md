# Track B — Model Attribution Capture + `interrupt_task` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Record which model actually completed each task (plus token/cost/latency and an optional I/O digest), surface it on the chat message DTO, and add an interrupt for in-progress local tasks so Stop works.

**Architecture:** Extend `CompletionAttestation` with attribution fields (additive, serde-default → backward compatible), thread `model_id` onto `ChatMessageDto`, and add a `cancel_orchestrator_task`-shaped `interrupt_orchestrator_task` Tauri command backed by a new daemon method.

**Tech Stack:** Rust (vox-orchestrator, vox-gui Tauri commands), serde, existing daemon RPC pattern.

**Scope marker:** `[SEQUENTIAL]` after Track A.
**Execution target:** Sonnet 4.6.

---

## Audit Corrections — verified against code 2026-06-20 (read FIRST; overrides stale claims below)

- **CONFIRMED:** `CompletionAttestation` derives + `PathBuf` import (`types/tasks.rs:292`) — additive serde-default fields are safe (Task 1 OK). `ChatMessageDto` shape (`chat.rs:16-23`) — adding `model_id` OK (Task 3). `CANCEL_TASK = "orch.cancel_task"` is in **`vox-foundation/src/protocol.rs:25`** (NOT a local `orch_daemon_method` module — fix Task 4 Step 1 path); daemon dispatch handles it at `orch_daemon/mod.rs:366-373`; the Tauri handler is registered in **`vox-gui/src/main.rs:138`** inside `generate_handler!` (fix Task 4 Step 4).

- **TASK 2 IS NOT FEASIBLE AS WRITTEN — `ModelSelectionDecision` is NOT in scope at completion.** The primary completion handler is `orchestrator/task_dispatch/complete/success/mod.rs:43-477`; it only has the task's `model_override`/`model_preference` (`:371-374`, fed to `record_bandit_task_outcome` at `:456`). Model *selection* happens at dispatch/inference time, not completion. **Revised Task 2:** capture attribution where the decision actually exists (at dispatch / inference) and stash it on the task so completion can read it. Concretely: add a `selected_model_record: Option<SelectedModelRecord>` field to `AgentTask` (new small struct: `model`, `provider`, `reason`, `request_tokens`, `response_tokens`, `latency_ms`), populate it at the dispatch/inference site that already computes the route, then in the completion handler copy it onto the attestation. This makes Task 2 a 2-step thread (dispatch-write → completion-copy), not a single edit.

- **TASK 2 CODE WON'T COMPILE.** `ModelRouteBackend` (alias `ChatRouteBackend`, `vox-orchestrator-types/src/lib.rs:43-57`) has **no `Display`**, and `ScoreBreakdown` has **no `reason_str()`** — `SelectionReason` (`models/select.rs:528-545`) is an enum `{PremiumAlias{task,alias_model_id}, Scored, LocalOnly, EnvOverride{env_var}}`. Replace the offending lines with:
  ```rust
  att.provider = decision.as_ref().map(|d| {
      let (provider, _route) = vox_orchestrator_types::backend_telemetry_labels(d.provider_route);
      provider.to_string()
  });
  att.selection_reason = decision.as_ref().map(|d| format!("{:?}", d.score_breakdown.reason));
  ```
  (Confirm `backend_telemetry_labels` is the real helper name; if not, match `ModelRouteBackend` variants explicitly.)

- **TASK 3 retrieval:** prefer persisting `model_id` into the message **payload JSON at append time** (`chat_append_message`, `chat.rs:114`) over a per-message daemon round-trip in `chat_get_messages` (`:72-102`). The DTO field is fine; the source should be the payload, populated from the attestation's `completing_model` when the assistant message is written.

- **TASK 4 (interrupt) NEEDS A NEW CANCELLATION PATH — none exists.** `cancel_task` (`lifecycle_ops.rs:110-156`) only handles queued tasks and in-progress **Populi remote** tasks (`populi_remote_delegate.is_some()`); the agent runtime's `AgentCommand::CancelTask` (`runtime.rs:594-599`) only cancels *queued* work; `TaskProcessor::process(&self, agent_id, task)` (`runtime.rs:49-55`) takes **no cancellation token** and runs to completion. So Task 4 is bigger than a daemon method: (a) add a per-task cancellation flag/token store on the Orchestrator (e.g. `Arc<DashMap<TaskId, CancellationToken>>` using `tokio_util::sync::CancellationToken`), (b) thread the token into `TaskProcessor::process` (signature change — update all impls), (c) have the local inference loop poll/select on it and abort, emitting `TaskCancelled{path:"local_interrupt"}`, (d) `interrupt_task` fires the token. Keep the Tauri command + daemon constant exactly as the plan shows (those parts are correct); expand the orchestrator-side step into (a)–(d). This is the largest single piece of work in the program — consider splitting into its own task list.

---

## File Structure

- Modify: `crates/vox-orchestrator/src/types/tasks.rs:292-321` — add attribution fields to `CompletionAttestation`.
- Modify: `crates/vox-gui/src/commands/chat.rs:16-23` — add `model_id` to `ChatMessageDto`.
- Modify: `crates/vox-gui/src/commands/control_plane.rs` — add `interrupt_orchestrator_task` (mirror `cancel_orchestrator_task:290-308`).
- Modify: daemon method constants + handler (locate via `orch_daemon_method::CANCEL_TASK`).

---

### Task 1: Attribution fields on `CompletionAttestation`

**Files:**
- Modify: `crates/vox-orchestrator/src/types/tasks.rs`

- [ ] **Step 1: Write the failing test** (add to the tests module that covers `tasks.rs`; if none, create `#[cfg(test)] mod attribution_tests` at file end)

```rust
#[cfg(test)]
mod attribution_tests {
    use super::*;

    #[test]
    fn attestation_roundtrips_attribution() {
        let a = CompletionAttestation {
            completing_model: Some("anthropic/claude-opus".into()),
            provider: Some("anthropic".into()),
            selection_reason: Some("scored".into()),
            request_tokens: Some(4200),
            response_tokens: Some(1100),
            latency_ms: Some(820),
            ..Default::default()
        };
        let json = serde_json::to_string(&a).unwrap();
        let back: CompletionAttestation = serde_json::from_str(&json).unwrap();
        assert_eq!(back.completing_model.as_deref(), Some("anthropic/claude-opus"));
        assert_eq!(back.request_tokens, Some(4200));
    }

    #[test]
    fn old_attestation_without_attribution_still_parses() {
        // Backward compatibility: a payload predating these fields must deserialize.
        let old = r#"{"declared_non_placeholder":true}"#;
        let a: CompletionAttestation = serde_json::from_str(old).unwrap();
        assert!(a.completing_model.is_none());
        assert!(a.declared_non_placeholder);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator attestation_roundtrips_attribution 2>cargo-attr.log; tail -30 cargo-attr.log`
Expected: FAIL — `struct CompletionAttestation has no field named completing_model`.

- [ ] **Step 3: Add the fields** (insert before the closing `}` of `CompletionAttestation`, after `observation_summary` at `tasks.rs:320`)

```rust
    /// Model that actually completed this task (e.g. "anthropic/claude-opus").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completing_model: Option<String>,
    /// Provider route for the completing model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Why this model was selected (`SelectionReason` rendered as a short string).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<String>,
    /// Input tokens sent for this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_tokens: Option<u64>,
    /// Output tokens received for this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_tokens: Option<u64>,
    /// End-to-end latency in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    /// Optional pointer to a captured request/response digest (privacy-gated).
    /// Only populated when the user enables I/O capture; never stores raw payload inline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_digest_ref: Option<String>,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-orchestrator attribution_tests 2>cargo-attr.log; tail -20 cargo-attr.log`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/types/tasks.rs
git commit -m "feat(orchestrator): record completing-model attribution on CompletionAttestation"
```

---

### Task 2: Populate attribution at completion

The fields exist but nothing fills them. Populate from the `ModelSelectionDecision` available at completion.

**Files:**
- Modify: the MCP/orchestrator completion handler that builds `CompletionAttestation` (locate it).

- [ ] **Step 1: Locate the completion handler**

Run: `grep -rn "CompletionAttestation {" crates/ --include=*.rs | grep -v test`
Expected: the construction site(s) where a task is marked complete (likely under `task_dispatch/complete/`).
Read that function to see what `ModelSelectionDecision` / usage data is in scope.

- [ ] **Step 2: Write the failing test**

In the handler's test module, add a test asserting that after completing a task whose dispatch carried a
`ModelSelectionDecision { selected_model: "x/y", provider_route, .. }` and usage `(req=10, resp=5)`, the
stored attestation has `completing_model == Some("x/y")` and `request_tokens == Some(10)`. Use the handler's
existing test fixtures for a completed task; if the fixture lacks a decision, extend it minimally.

```rust
#[test]
fn completion_records_model_attribution() {
    // Arrange: a completed task with a known selection decision + usage.
    // (use the module's existing complete-task fixture; inject decision + usage)
    let att = build_attestation_for_completed(/* fixture */);
    assert_eq!(att.completing_model.as_deref(), Some("x/y"));
    assert_eq!(att.request_tokens, Some(10));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator completion_records_model_attribution 2>cargo-pop.log; tail -30 cargo-pop.log`
Expected: FAIL — attribution is `None`.

- [ ] **Step 4: Wire the population** — at the attestation construction site, set the new fields from the
decision and usage already in scope:

```rust
att.completing_model = decision.as_ref().map(|d| d.selected_model.clone());
att.provider = decision.as_ref().map(|d| d.provider_route.to_string());
att.selection_reason = decision.as_ref().map(|d| d.score_breakdown.reason_str());
att.request_tokens = usage.map(|u| u.input as u64);
att.response_tokens = usage.map(|u| u.output as u64);
att.latency_ms = latency_ms; // if measured here; else leave None
```

If `provider_route` has no `Display`, map it explicitly (read `ModelRouteBackend`). If `score_breakdown`
has no `reason_str()`, use the existing field (audit names `reason: SelectionReason`) via `format!("{:?}", ...)`.

- [ ] **Step 5: Run test to verify it passes + commit**

Run: `cargo test -p vox-orchestrator completion_records_model_attribution 2>cargo-pop.log; tail -20 cargo-pop.log`
Expected: PASS.

```bash
git add -A && git commit -m "feat(orchestrator): populate model attribution from selection decision at completion"
```

---

### Task 3: Thread `model_id` onto `ChatMessageDto`

**Files:**
- Modify: `crates/vox-gui/src/commands/chat.rs:16-23` and the message-load query (`chat_get_messages`).

- [ ] **Step 1: Add the field**

```rust
#[derive(Debug, Serialize)]
pub struct ChatMessageDto {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}
```

- [ ] **Step 2: Populate it** — in `chat_get_messages`, when a message has a `task_id`, look up the task's
attestation `completing_model` (via the orchestrator daemon status call already used elsewhere) and set
`model_id`. Where the assistant message is persisted (`chat_append_message`, `chat.rs:114`), also persist the
model id into the message payload JSON so it survives reloads without a daemon round-trip.

- [ ] **Step 3: Build + a serialization test**

```rust
#[test]
fn chat_message_dto_serializes_model_id() {
    let dto = ChatMessageDto { id:1, role:"assistant".into(), content:"hi".into(),
        created_at:"now".into(), task_id:Some("7".into()), model_id:Some("x/y".into()) };
    let j = serde_json::to_string(&dto).unwrap();
    assert!(j.contains("\"model_id\":\"x/y\""));
}
```

Run: `cargo test -p vox-gui chat_message_dto_serializes_model_id 2>cargo-dto.log; tail -20 cargo-dto.log` → PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/src/commands/chat.rs && git commit -m "feat(gui): expose completing model_id on ChatMessageDto"
```

---

### Task 4: `interrupt_orchestrator_task` (Stop for in-progress local work)

Mirror `cancel_orchestrator_task` (`control_plane.rs:290-308`) with a new daemon method `INTERRUPT_TASK`.

**Files:**
- Modify: `crates/vox-gui/src/commands/control_plane.rs`
- Modify: daemon method constants + handler (where `CANCEL_TASK` is defined/handled).

- [ ] **Step 1: Add the daemon method constant** — find `orch_daemon_method` (grep `CANCEL_TASK`) and add:

```rust
pub const INTERRUPT_TASK: &str = "orch.interrupt_task";
```

- [ ] **Step 2: Add the orchestrator handler** — in the daemon dispatch match (alongside `CANCEL_TASK`),
route `INTERRUPT_TASK` to a new `Orchestrator::interrupt_task(task_id)` that signals the in-progress local
runtime to abort (set a cancellation flag the local inference loop checks), then emits `TaskCancelled` with
path `"local_interrupt"`. Read `lifecycle_ops.rs:111-150` (`cancel_task`) for the lock/affinity-release
pattern and reuse it; the new piece is signalling the running local task (the existing cancel only handles
queued + remote Populi).

- [ ] **Step 3: Add the Tauri command** (after `cancel_orchestrator_task`)

```rust
#[tauri::command]
pub async fn interrupt_orchestrator_task(
    app_handle: tauri::AppHandle,
    task_id: u64,
) -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(
        orch_daemon_method::INTERRUPT_TASK,
        serde_json::json!({ "task_id": task_id }),
    )
    .await?;
    let result = Ok(ControlPlaneResult {
        ok: true,
        message: format!("task {task_id} interrupted"),
        task_id: Some(task_id.to_string()),
        duplicate_of: None,
    });
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    result
}
```

- [ ] **Step 4: Register the command** — add `interrupt_orchestrator_task` to the `tauri::generate_handler!`
list (grep `cancel_orchestrator_task` in the builder).

- [ ] **Step 5: Test the handler transition + commit**

Add an orchestrator unit test: an in-progress local task, after `interrupt_task`, transitions out of running
(status no longer `InProgress`) and releases its locks. Run:
`cargo test -p vox-orchestrator interrupt 2>cargo-int2.log; tail -20 cargo-int2.log` → PASS.

```bash
git add -A && git commit -m "feat: orch.interrupt_task + interrupt_orchestrator_task command (Stop for local work)"
```

---

## Self-Review

**Spec coverage:** §3.4 attribution fields → Tasks 1–3. §3.5 Stop/interrupt → Task 4. **Type consistency:**
field names (`completing_model`, `provider`, `selection_reason`, `request_tokens`, `response_tokens`,
`latency_ms`, `io_digest_ref`) identical across tasks 1–3; `model_id` on the DTO maps from
`completing_model`. **Placeholder scan:** Tasks 2 & 4 contain locate-then-edit discovery steps (real actions
against named files) because the exact construction/dispatch sites must be read in-repo; all code shown is
concrete. **Backward compatibility:** all new attestation fields are `serde(default)` (Task 1 test 2 proves
old payloads parse).
