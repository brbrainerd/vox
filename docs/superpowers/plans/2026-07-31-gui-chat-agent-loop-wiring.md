# GUI Chat → Real Agent Loop Wiring + Gap-Analysis Follow-ups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Revision note (adversarial-review pass, same day):** This plan was independently fact-checked task-by-task against the live codebase by 4 parallel review agents after its first draft. Several claims in the original draft were **wrong** — a nonexistent frontend dispatch function, a nonexistent reducer action, two nonexistent Rust fixture functions, a `pub(crate)` visibility blocker the draft didn't notice, a missing struct field that would have failed to compile, and a real UX regression (no loading indicator) the draft didn't design for. All are corrected below. Where this revision still could not fully pin down an implementation detail (e.g., an internal reducer's exact current shape), the task says so explicitly and adds an investigation step — it does not paper over the gap with invented code.

**Goal:** Give Vox's own Tauri GUI chat window a synchronous, model-authored reply path (it currently has none — every composer send either silently persists with no reply, or dispatches a full background agent task), and dispose of every other finding from the 2026-07-31 gap-analysis audit (fix the cheap ones, add regression coverage, explicitly defer the large ones with rationale).

**Architecture:** `vox-orchestrator-mcp`'s `vox_chat_message` tool (backed by the real, tested `run_agent_turn` agent loop built earlier this session) is already reachable from `vox-gui` over the existing daemon TCP RPC via the generic `orch.tool_call` method — no new daemon RPC method, no embedded MCP client, and no new IPC transport are needed. **Correction from the first draft:** the new backend command does *not* route through the generic `invoke_mcp_tool` Tauri command (that command wraps its response in an extra `{"tool","is_error","result"}` envelope that would need unwrapping); instead it duplicates the same three-line daemon-call pattern `secretary_confirm_task` already uses in the same file, calling `client.call(TOOL_CALL, ...)` directly. The real gap is: (1) a small new Rust command that calls it, persists the reply the same way `chat_append_message` already persists user turns, and returns a real (not blank) timestamp; (2) frontend wiring — including a genuinely new, additive reducer action (none of the required actions exist today) and a pending/loading bubble — so a plain chat message uses this synchronous path instead of `submit_orchestrator_task`'s full 6-phase agentic pipeline; (3) for the golden-task regression coverage, two of the three proposed tasks need small new `pub` exports added to `vox-orchestrator-mcp` (not reuse of existing test fixtures, which the first draft incorrectly assumed already existed in reusable form).

**Tech Stack:** Rust (Tauri commands, `vox-orchestrator-mcp`, `vox-db`), TypeScript/React (`vox-gui/ui`, Vitest), existing `OrchDaemonClient` daemon-RPC bridge.

---

## Disposition of every 2026-07-31 gap-analysis finding

This plan **fixes**: the GUI-chat-has-no-reply gap (headline finding), the hardcoded `active_skill: null`, and adds 3 new `vox harness eval` golden tasks as real regression coverage (a 4th, originally proposed, recommended-tool-cap task is folded in as the same task — see Task 4).

This plan **verifies**, with a named command/test for each (no vague "re-run the suite and hope"):
- Task 0.1 scorer differentiation → Task 5 Step 4 (`vox model explain --complexity 1` vs `9`, output diffed by eye).
- Task 0.2 secretary propose-only behavior → Task 5 Step 1 explicitly names and re-runs `chat_append_message_does_not_auto_dispatch_to_daemon` and `secretary_skips_messages_the_composer_already_submitted` (`crates/vox-gui/src/commands/chat.rs`'s existing tests) rather than relying on the crate-wide `cargo test` sweep to incidentally cover it.
- Tool-calling wire protocol → Task 5 Step 1 names `crates/vox-llm-egress/tests/wire_mock.rs`.
- Privacy hard filter → Task 4's new `privacy-filter-blocks-live-routing` golden task (this is now a **fix/add**, not just a re-run — it closes the gap-analysis finding that this property was previously only asserted in isolated unit tests, never at the eval-gate level).
- VRAM-fit → Task 5 Step 1 names `crates/vox-orchestrator/src/models/vram.rs`'s test module.
- A11y fixes → Task 5 Step 2 names `VersionMismatchBanner.test.tsx` and `ErrorBoundary.test.tsx` explicitly.
- Toast coalescing → Task 5 Step 2 names `toastQueue.test.ts`.

This plan **explicitly defers**, each with a one-line reason — not silently dropped:
- **Context compaction/budget wiring into `agent_loop.rs`** (`compaction.rs`/`budget/mod.rs` exist but have zero call sites in the loop) — real, valuable work, but it changes the agent loop's hot path and deserves its own TDD plan with its own review cycle, not a rider on a GUI-wiring plan.
- **Prompt caching (`cache_control` wire field)** — the implementation spec itself calls this "the largest single recurring cost in a chat turn after tool definitions" and scoped it as a multi-file, multi-provider-lane effort. Too large to piggyback here; deserves its own plan.
- **`vox model add <endpoint>` CLI verb / `ollama_chat/`-prefix addressing** — real, cheap, but unrelated to chat-reply wiring; a natural next `/superpowers:writing-plans` pass once this lands, not bundled in here.
- **`skill_promotion.rs` gate 6 (`gate_no_known_counterexamples`)** — disclosed, honest pass-through; the promotion gate itself has zero production call sites yet, so hardening one gate inside an unwired pipeline is not the highest-value next step.
- **`Deployment` struct (spec §5.1) / F18 Gemini-URL-sniffing fold-in** — the audit found the prerequisite struct was never built; inventing it now to fold in one low-severity finding is scope creep for this plan.
- **The `agent-loop-terminates` golden task's coverage of `run_agent_turn`'s full production path** — the new eval-gate export (Task 4) proves the iteration-cap property using a purpose-built internal check, not a black-box call through the daemon RPC surface a real GUI/MCP client would use. This is a deliberate, smaller-blast-radius scope reduction from the first draft's (unimplementable) design — see Task 4's own notes.

---

## Phased execution structure (added in this revision)

The first draft numbered 5 tasks as an implicit strict sequence. A structural review found real dependencies between some tasks and none between others — sequencing everything loses parallelism the codebase's own `subagent-driven-development` pattern is built for. Use this phasing instead of the task numbers as an execution order:

- **Phase A — dispatch 2 tasks in parallel, no shared files, no logical dependency:**
  - Task 1 (backend `chat_send_message`)
  - Task 4 (harness-eval golden tasks)
  - **Gate A→B:** both pass their own `cargo test`/`cargo clippy -D warnings` independently before Phase B starts. (Task 1 and Task 4 touch disjoint crates/files — genuinely safe to run as two parallel subagent-driven-development cycles.)

- **Phase B — Task 2 first (real dependency), then Task 3 can run in parallel with Task 2's manual-verification tail:**
  - Task 2 depends on Task 1 (it calls the `chat_send_message` command Task 1 adds) — **do not start Task 2's Step 5 (frontend wiring) until Phase A's gate confirms Task 1 is merged/rebased in.** Task 2 Steps 1-4 (writing/testing the pure `chatSend.ts` helper) have no such dependency and may start immediately in Phase A if a spare subagent is available.
  - Task 3 touches the *same file* as Task 1 (`crates/vox-gui/src/commands/chat.rs`) but a *disjoint function* (`secretary_confirm_task`, not `chat_send_message`) — it has no logical dependency on Task 1's new code, only a mechanical rebase-past-it risk. It is safe to dispatch to a second parallel subagent once Task 1 has landed, running alongside Task 2.
  - **Gate B→C (mandatory, not optional — this was a missing gate in the first draft):** Task 2 is not done, and Task 5 must not start, until ONE of: (a) a real dev-server/browser observation of a rendered assistant reply in the chat transcript, confirmed by a second reviewer (per this session's established spec-compliance-review pattern), or (b) an explicit, named, reviewed reason logged for why live verification was impossible in this environment (e.g. "dev server did not bind in the sandboxed preview browser," the same limitation hit earlier this session). A green `pnpm vitest run` alone does **not** satisfy this gate — none of Task 2's unit tests can prove a message actually renders in the live transcript, only that the pure helper functions behave correctly in isolation.

- **Phase C — sequential, depends on everything above:**
  - Task 5 (verification pass) — must run last; it re-runs the full suite including Task 4's new golden tasks and exercises the Task 2/3 wiring.

---

## File Structure

- **Modify:** `crates/vox-gui/src/commands/chat.rs` — add `chat_send_message`, `parse_chat_message_envelope`, `persist_assistant_reply` (a newly-testable persistence helper, factored out so the daemon round-trip and the DB write are independently unit-testable — see Task 1's TDD note).
- **Modify:** `crates/vox-db/src/codex_chat.rs` — add a small helper to return the real `created_at` of a just-inserted message (closes a real bug the first draft introduced: a hardcoded blank timestamp).
- **Modify:** `crates/vox-gui/src/main.rs` — register the new Tauri command.
- **Modify:** `crates/vox-gui/src/commands/chat.rs` (`secretary_confirm_task`) — thread the real active skill instead of hardcoded `null`.
- **Create:** `crates/vox-gui/ui/src/lib/chatSend.ts` — frontend helper: calls `chat_send_message`, returns a typed result.
- **Create:** `crates/vox-gui/ui/src/lib/chatSend.test.ts` — unit tests for the helper.
- **Modify:** `crates/vox-gui/ui/src/lib/chatCorrelation.ts` (or wherever `ChatAction`/the chat reducer actually lives — confirm exact path in Task 2 Step 0) — add 2 new, additive action variants (`chatPending`, `chatReplySettled`) rather than reusing task-dispatch's `submit`/`submitResolved` (whose `runId`/`taskId` correlation semantics are built for polling a background task, not a synchronous reply — reusing them risks corrupting task-tracking state for a case they weren't designed for).
- **Modify:** `crates/vox-gui/ui/src/App.tsx` (`handleLoquelaSubmit`, confirmed real span: lines 735-833) — for plain chat sends (`task_category === 'chat'`), dispatch a pending bubble, call the new synchronous path, then settle the bubble with the reply or an error — instead of falling through to `submit_orchestrator_task`.
- **Modify:** `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (`confirmSecretaryTask`, confirmed real span: lines 654-665) + `ChatSurfaceProps` — thread a new `activeSkillId` prop down from `App.tsx` via `surfaceComponents.tsx`'s `<ChatSurface ... />` render site (confirmed at `surfaceComponents.tsx:186`).
- **Modify:** `crates/vox-cli/src/commands/harness/eval.rs` — add 3 new golden tasks.
- **Modify:** `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs` — add one small new `pub` eval-gate self-check function (real new code, not reuse of a nonexistent fixture).
- **Modify:** `crates/vox-orchestrator-mcp/src/llm_bridge/local_health.rs` — extract a pure, mode-parameterized core out of `privacy_allows` so an eval-gate check can call it without mutating process env vars (an improvement over the first draft's `std::env::set_var` approach, which this codebase's own test-only override mechanism was deliberately written to avoid).
- **No new files under `docs/src/architecture/`** — this plan's own header + task list is the design record.

---

## Task 1: Backend — `chat_send_message` Tauri command

**Files:**
- Modify: `crates/vox-gui/src/commands/chat.rs`
- Modify: `crates/vox-db/src/codex_chat.rs`
- Test: inline `#[cfg(test)] mod tests` in both files, matching existing convention.

Verified facts (all independently re-checked against the live codebase this revision):
- `crates/vox-orchestrator-mcp/src/dispatch.rs:1001` really does dispatch `"vox_chat_message"` to `chat_tools::chat_message(state, serde_json::from_value(args)?).await`.
- `chat_message`'s success envelope really is `{"success": true, "data": {"message": {..., "content"}, "model_used", "tokens", ...}}` (`message.rs:984-991` builds the `data` object, `ToolResult::ok(result).to_json()` wraps it).
- `db.chat_append_message(&self, conversation_id: i64, role: &str, content_text: &str, payload_json: Option<&str>) -> Result<i64, StoreError>` — exact real signature, `crates/vox-db/src/codex_chat.rs:242-248`. The plan's usage matches this exactly.
- `db.chat_ensure_gui_session(&self, external_session_id: &str, title: &str) -> Result<i64, StoreError>` — exact real signature, `codex_chat.rs:569-573`. Matches.
- `OrchDaemonClient::with_token`/`::new`/`.call(method, params)` and `vox_foundation::protocol::orch_daemon_method::TOOL_CALL` (`= "orch.tool_call"`, `crates/vox-foundation/src/protocol.rs:89`) all exist exactly as used, and `crates/vox-gui/Cargo.toml` already depends on both `vox-orchestrator` and `vox-foundation`.
- `tauri::test::mock_app()` + `app.manage(...)` + `app.state::<...>()` is the file's existing, working test pattern (`chat_append_message_does_not_auto_dispatch_to_daemon`, `chat.rs:360-386`) — the new tests below compile against the same harness with no changes needed.
- **Bug caught in review:** `ChatMessageDto.created_at` cannot be hardcoded to an empty string — `chat_get_gui_messages` (`codex_chat.rs:680-696`) returns the *real* `created_at` column for the same row on reload, so a naive implementation would show a blank/wrong timestamp until the page refreshes. Fixed in Step 5 below via a new `chat_message_created_at` helper.
- **TDD gap acknowledged, partially closed, remainder explicitly scoped to Phase B's Gate:** this task cannot unit-test the actual daemon TCP round-trip without standing up a real daemon (no existing precedent for mocking `OrchDaemonClient` at the TCP layer in this codebase). What Task 1 *can* and does unit-test: input validation, envelope parsing (already covered), and — new in this revision — the DB-persistence half of the flow via an extracted `persist_assistant_reply` helper that takes already-parsed `(content, model_id)` and talks to the real in-memory test DB, with no daemon involved. The live daemon round-trip itself is verified live, not by a unit test, at Phase B's mandatory gate.

- [ ] **Step 1: Write the failing tests**

Add to `crates/vox-gui/src/commands/chat.rs`'s existing `#[cfg(test)] mod tests` block:

```rust
#[tokio::test]
async fn chat_send_message_rejects_empty_session_id() {
    use tauri::Manager;
    let app = tauri::test::mock_app();
    app.manage(Arc::new(PersistentDaemon::default()));
    app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
    let daemon = app.state::<Arc<PersistentDaemon>>();
    let pool = app.state::<GuiDbPool>();
    let result = chat_send_message(
        app.handle().clone(),
        ChatSendInput {
            session_id: String::new(),
            content: "hello".to_string(),
            active_skill: None,
        },
        pool,
        daemon,
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("session_id"));
}

#[test]
fn parse_chat_message_envelope_extracts_content_and_model() {
    let envelope = serde_json::json!({
        "success": true,
        "data": {
            "message": {"id": "m1", "role": "assistant", "content": "Hi there"},
            "model_used": "openrouter/auto",
            "tokens": 42
        }
    });
    let (content, model_id) = parse_chat_message_envelope(&envelope).expect("parse ok");
    assert_eq!(content, "Hi there");
    assert_eq!(model_id.as_deref(), Some("openrouter/auto"));
}

#[test]
fn parse_chat_message_envelope_reports_tool_error() {
    let envelope = serde_json::json!({"success": false, "error": "model unavailable"});
    let err = parse_chat_message_envelope(&envelope).unwrap_err();
    assert_eq!(err, "model unavailable");
}

#[tokio::test]
async fn persist_assistant_reply_writes_row_and_returns_real_created_at() {
    let pool = GuiDbPool::connect_memory().await.expect("memory pool");
    let db = pool.handle().expect("db handle");
    let conv_id = db
        .chat_ensure_gui_session("sess-persist", "Chat")
        .await
        .expect("ensure session");
    let dto = persist_assistant_reply(&db, conv_id, "sess-persist", "Hello!", Some("openrouter/auto"))
        .await
        .expect("persist ok");
    assert_eq!(dto.role, "assistant");
    assert_eq!(dto.content, "Hello!");
    assert_eq!(dto.model_id.as_deref(), Some("openrouter/auto"));
    assert!(!dto.created_at.is_empty(), "created_at must not be blank");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-gui --lib commands::chat -- chat_send_message_rejects_empty_session_id parse_chat_message_envelope_extracts_content_and_model parse_chat_message_envelope_reports_tool_error persist_assistant_reply_writes_row_and_returns_real_created_at`
Expected: compile error — none of `chat_send_message`, `ChatSendInput`, `parse_chat_message_envelope`, `persist_assistant_reply` exist yet.

- [ ] **Step 3: Add the `chat_message_created_at` DB helper**

In `crates/vox-db/src/codex_chat.rs`, add near `chat_append_message` (read that function's exact surrounding style — connection acquisition, error mapping — and mirror it exactly rather than guessing; the sketch below assumes the same `self.conn`/`StoreError` pattern the file's other methods use, confirm before finalizing):

```rust
/// Returns the `created_at` timestamp SQLite assigned to a message row,
/// immediately after insert — used by callers (like `chat_send_message`)
/// that need to return a DTO with a real, not-yet-reloaded timestamp.
pub async fn chat_message_created_at(&self, message_id: i64) -> Result<String, StoreError> {
    // Mirror chat_append_message's connection/query pattern exactly.
    // SELECT created_at FROM conversation_messages WHERE id = ?1
    todo_confirm_against_real_pattern!()
}
```

Do not literally commit a `todo_confirm_against_real_pattern!()` macro — this is a placeholder ONLY inside this plan document to signal "read `chat_append_message`'s real body first." When implementing, replace the function body with a real `SELECT created_at FROM conversation_messages WHERE id = ?1` query using the exact connection/row-mapping idiom `chat_append_message` uses two functions above it in the same file (e.g. if `chat_append_message` uses `self.conn.call(...)` with a closure and `.map_err(StoreError::from)`, use the identical shape here).

- [ ] **Step 4: Implement `parse_chat_message_envelope` and `persist_assistant_reply`**

Add to `crates/vox-gui/src/commands/chat.rs`, above the `#[cfg(test)]` block:

```rust
/// Extracts `(content, model_id)` from a `vox_chat_message` `ToolResult`
/// envelope (`{"success", "data": {"message": {..., "content"}, "model_used"}}`
/// or `{"success": false, "error"}`) as returned directly by
/// `OrchDaemonClient::call(TOOL_CALL, ...)`.
fn parse_chat_message_envelope(envelope: &serde_json::Value) -> Result<(String, Option<String>), String> {
    let success = envelope.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    if !success {
        let err = envelope
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("vox_chat_message failed with no error detail")
            .to_string();
        return Err(err);
    }
    let data = envelope
        .get("data")
        .ok_or_else(|| "vox_chat_message succeeded with no data".to_string())?;
    let content = data
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| "vox_chat_message response missing message.content".to_string())?
        .to_string();
    let model_id = data
        .get("model_used")
        .and_then(|m| m.as_str())
        .map(str::to_string);
    Ok((content, model_id))
}

/// Persists an already-parsed assistant reply and returns a DTO with a real
/// (not blank) `created_at`. Split out from `chat_send_message` so this half
/// of the flow — the part that doesn't need a live daemon — is independently
/// unit-testable against the in-memory test DB.
async fn persist_assistant_reply(
    db: &VoxDb,
    conv_id: i64,
    session_id: &str,
    content: &str,
    model_id: Option<&str>,
) -> Result<ChatMessageDto, String> {
    let payload = model_id.map(|m| serde_json::json!({ "model_id": m }).to_string());
    let msg_id = db
        .chat_append_message(conv_id, "assistant", content, payload.as_deref())
        .await
        .map_err(map_db_err)?;
    let created_at = db
        .chat_message_created_at(msg_id)
        .await
        .map_err(map_db_err)?;
    let _ = session_id; // reserved: kept as a parameter for a future per-session cache invalidation hook, not used yet
    Ok(ChatMessageDto {
        id: msg_id,
        role: "assistant".to_string(),
        content: content.to_string(),
        created_at,
        task_id: None,
        model_id: model_id.map(str::to_string),
    })
}
```

If `session_id` ends up genuinely unused (no near-term caller need), remove the parameter and the `_ = session_id;` line entirely rather than keeping dead-looking code — only keep it if a real near-term caller (e.g. a per-session in-memory cache invalidation, if one exists — check `GuiDbPool`/`pool_db` for any such cache before deciding) needs it.

- [ ] **Step 5: Run these 3 tests, confirm they pass**

Run: `cargo test -p vox-gui --lib commands::chat -- parse_chat_message_envelope_extracts_content_and_model parse_chat_message_envelope_reports_tool_error persist_assistant_reply_writes_row_and_returns_real_created_at`
Expected: 3 passed.

- [ ] **Step 6: Implement `ChatSendInput` and `chat_send_message`**

```rust
#[derive(Debug, Deserialize)]
pub struct ChatSendInput {
    pub session_id: String,
    pub content: String,
    #[serde(default)]
    pub active_skill: Option<String>,
}

/// Synchronous chat reply: calls the real agent loop (`vox_chat_message` via
/// the daemon's `orch.tool_call`) and persists the assistant's reply via
/// `persist_assistant_reply`. Unlike `secretary_confirm_task` (which submits
/// a background work-order task via `SUBMIT_TASK`), this returns a
/// model-authored reply directly for immediate transcript rendering — the
/// synchronous chat path this GUI never had (2026-07-31 gap-analysis
/// finding). Mirrors `secretary_confirm_task`'s daemon-call shape exactly
/// (same file, a few lines below) rather than going through the generic
/// `invoke_mcp_tool` command, whose extra `{"tool","is_error","result"}`
/// wrapper this function does not need.
#[tauri::command]
pub async fn chat_send_message<R: tauri::Runtime>(
    _app_handle: tauri::AppHandle<R>,
    input: ChatSendInput,
    pool: State<'_, GuiDbPool>,
    daemon: State<'_, Arc<PersistentDaemon>>,
) -> Result<ChatMessageDto, String> {
    if input.session_id.trim().is_empty() {
        return Err("session_id must not be empty".to_string());
    }
    if input.content.trim().is_empty() {
        return Err("content must not be empty".to_string());
    }
    let addr = daemon.ensure().await?;
    let client = match daemon.token().await {
        Some(token) => vox_orchestrator::orch_daemon::OrchDaemonClient::with_token(addr, token),
        None => vox_orchestrator::orch_daemon::OrchDaemonClient::new(addr),
    };
    // ChatMessageParams accepts this field as `skill` with a `#[serde(alias = "active_skill")]`
    // — confirm this alias still exists in crates/vox-orchestrator-mcp/src/chat_tools/params.rs
    // before finalizing; sending "active_skill" here relies on it.
    let mut args = serde_json::json!({
        "prompt": input.content,
        "session_id": input.session_id,
    });
    if let Some(skill) = input.active_skill.as_ref() {
        args["active_skill"] = serde_json::Value::String(skill.clone());
    }
    let envelope = client
        .call(
            vox_foundation::protocol::orch_daemon_method::TOOL_CALL,
            serde_json::json!({ "name": "vox_chat_message", "args": args }),
        )
        .await
        .map_err(|e| format!("vox_chat_message failed: {e}"))?;
    let (content, model_id) = parse_chat_message_envelope(&envelope)?;

    let db = pool_db(&pool)?;
    let conv_id = db
        .chat_ensure_gui_session(&input.session_id, "Chat")
        .await
        .map_err(map_db_err)?;
    persist_assistant_reply(&db, conv_id, &input.session_id, &content, model_id.as_deref()).await
}
```

Note the redundant `.map_err(|e| e.to_string())` on `daemon.ensure()` that appeared in the first draft (and that already exists on `secretary_confirm_task` a few lines below) is dropped here since `ensure()` already returns `Result<String, String>` — this is a minor stylistic cleanup, not required, but do not reintroduce the no-op `.map_err` when writing this.

- [ ] **Step 7: Run the full test module**

Run: `cargo test -p vox-gui --lib commands::chat`
Expected: all tests pass, including the 4 new ones.

- [ ] **Step 8: Register the command**

In `crates/vox-gui/src/main.rs`, find the `commands::chat::*` registrations near `commands::mcp::invoke_mcp_tool,` (~line 217), add:

```rust
            commands::chat::chat_send_message,
```

- [ ] **Step 9: Build and lint**

Run: `cargo build -p vox-gui && cargo build -p vox-db && cargo clippy -p vox-gui -p vox-db --lib -- -D warnings`
Expected: clean build, clean clippy.

- [ ] **Step 10: Commit**

```bash
git add crates/vox-gui/src/commands/chat.rs crates/vox-gui/src/main.rs crates/vox-db/src/codex_chat.rs
git commit -m "feat(vox-gui): add chat_send_message — synchronous reply via the real agent loop"
```

---

## Task 2: Frontend — wire plain chat sends to the new synchronous path

**Files:**
- Create: `crates/vox-gui/ui/src/lib/chatSend.ts`, `crates/vox-gui/ui/src/lib/chatSend.test.ts`
- Modify: the file containing `ChatAction`/the session-chat reducer (confirm exact path in Step 0 — reviewed as likely `crates/vox-gui/ui/src/lib/chatCorrelation.ts` for the per-session `ChatState` reducer and `crates/vox-gui/ui/src/lib/sessionChatStore.ts` for the `SessionChatAction` wrapper around it, but confirm both file paths and both action-union definitions before writing code, since a wrong guess here would silently fail to compile in a way this plan cannot pre-verify further without executing it)
- Modify: `crates/vox-gui/ui/src/App.tsx` (`handleLoquelaSubmit`, confirmed real span: **lines 735-833**, not 735-923 as the first draft guessed — 923 is where a different function, `handleLoquelaSlash`, ends)

**Depends on Task 1 having landed** (calls the `chat_send_message` command it adds) — see the Phase A→B gate above.

- [ ] **Step 0: Investigate the real reducer shape before writing any reducer code**

Read `crates/vox-gui/ui/src/lib/chatCorrelation.ts` in full. Confirm:
- The exact `ChatAction` union (reviewed as `submit | submitResolved | failRun | agentEvent | pendingTimeout` — confirm these names and each variant's field shape).
- The exact `ChatMessage` interface (reviewed as `{id, role, text, status, runId, taskId?, error?, sessionId?, modelId?, groundingFlagged?, createdAtMs?}` — confirm).
- What the `'submit'` case's reducer logic actually does (reviewed: creates both a user bubble and a `status: 'pending'` empty assistant bubble, keyed by a `runId`) — this confirms the codebase already has a "show a pending bubble immediately" pattern; Task 2 will add a **new, separate** pair of actions rather than overload `submit`/`submitResolved`, because those are built around `runId`↔`taskId` correlation semantics for polling a background task's async completion — forcing a synchronous chat reply through that machinery risks corrupting real task-tracking state for a case it wasn't designed for. Confirm this reasoning still holds once you've read the real code; if `submit`/`submitResolved` turn out to be more generic than described, reusing them may be cleaner than adding new actions — use judgment, but document the choice either way in the Step 3 commit.

Also confirm `crates/vox-gui/ui/src/lib/sessionChatStore.ts`'s `SessionChatAction` wrapper (the outer dispatch surface `App.tsx` actually calls, per the first draft's correctly-identified `dispatchSessionChat` — see Step 4) and how it forwards into the per-session `chatCorrelation` reducer.

- [ ] **Step 1: Write the failing test for the frontend helper**

Create `crates/vox-gui/ui/src/lib/chatSend.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { parseSendReply } from './chatSend';

describe('parseSendReply', () => {
  it('extracts content and role from a successful ChatMessageDto', () => {
    const dto = { id: 7, role: 'assistant', content: 'Hello!', created_at: '2026-07-31T00:00:00Z', task_id: null, model_id: 'openrouter/auto' };
    const parsed = parseSendReply(dto);
    expect(parsed).toEqual({ id: '7', role: 'assistant', text: 'Hello!', modelId: 'openrouter/auto', createdAt: '2026-07-31T00:00:00Z' });
  });

  it('returns undefined modelId when absent', () => {
    const dto = { id: 8, role: 'assistant', content: 'Hi', created_at: '2026-07-31T00:00:01Z', task_id: null };
    const parsed = parseSendReply(dto);
    expect(parsed.modelId).toBeUndefined();
  });
});
```

- [ ] **Step 2: Run the test, verify it fails**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/lib/chatSend.test.ts`
Expected: FAIL — `./chatSend` module not found.

- [ ] **Step 3: Implement the helper**

Create `crates/vox-gui/ui/src/lib/chatSend.ts`:

```typescript
import { invoke } from '@tauri-apps/api/core';

export interface ChatSendInput {
  session_id: string;
  content: string;
  active_skill?: string | null;
}

export interface ChatMessageDto {
  id: number;
  role: string;
  content: string;
  created_at: string;
  task_id: string | null;
  model_id?: string;
}

export interface ParsedChatReply {
  id: string;
  role: 'assistant';
  text: string;
  modelId?: string;
  createdAt: string;
}

export function parseSendReply(dto: ChatMessageDto): ParsedChatReply {
  return {
    id: String(dto.id),
    role: 'assistant',
    text: dto.content,
    modelId: dto.model_id,
    createdAt: dto.created_at,
  };
}

/**
 * Calls the real agent loop for a plain chat message and returns the
 * model's reply, already persisted server-side by `chat_send_message`
 * (with a real, non-blank `created_at`). Throws on failure — callers
 * should catch and settle the pending bubble as failed.
 */
export async function sendChatMessage(input: ChatSendInput): Promise<ParsedChatReply> {
  const dto = await invoke<ChatMessageDto>('chat_send_message', { input });
  return parseSendReply(dto);
}
```

- [ ] **Step 4: Run the test, verify it passes**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/lib/chatSend.test.ts`
Expected: 2 passed.

- [ ] **Step 5: Add the new, additive pending/settle reducer actions**

Based on Step 0's investigation, add two new variants to `ChatAction` (in `chatCorrelation.ts`) — this is genuinely new code the plan is adding, not a reuse of anything that already exists:

```typescript
  | { type: 'chatPending'; sessionId: string; tempId: string; userText: string }
  | { type: 'chatReplySettled'; sessionId: string; tempId: string; result: { ok: true; message: ChatMessage } | { ok: false; error: string } }
```

Add matching reducer cases (insert alongside the existing `switch`/`case` arms in the same reducer function Step 0 located — write the full case bodies, do not leave them as comments):

```typescript
    case 'chatPending': {
      const pendingMessage: ChatMessage = {
        id: action.tempId,
        role: 'assistant',
        text: '',
        status: 'pending',
        runId: action.tempId,
        sessionId: action.sessionId,
      };
      // Append pendingMessage to this session's message list — match the
      // exact array-update idiom the 'submit' case already uses in this
      // same reducer (confirmed in Step 0) rather than reinventing one.
      return { ...state /* , messages: [...state.messages, pendingMessage] — adjust to the real state shape confirmed in Step 0 */ };
    }
    case 'chatReplySettled': {
      // Replace the message with id === action.tempId in place: on ok,
      // with action.result.message (status: 'done'); on failure, with
      // status: 'failed' and action.result.error — match the exact
      // in-place-replace idiom 'submitResolved'/'failRun' already use for
      // settling a pending bubble by runId (confirmed in Step 0).
      return { ...state /* replace-in-place per the real reducer shape */ };
    }
```

The `/* ... */` comments above mark the one place this plan cannot give literal, final code without having read the reducer's real current body (Step 0's job) — they are not a placeholder for logic to invent later; they are an instruction to use the SAME array-update/replace-in-place pattern the reducer's own existing `submit`/`submitResolved`/`failRun` cases already use, just targeting the new action's fields. Do not leave the comments in the committed code — replace them with the real, matching update logic.

Add corresponding cases (or straight pass-through) to `SessionChatAction`/its reducer in `sessionChatStore.ts` if that wrapper needs its own switch arm (per Step 0's findings).

- [ ] **Step 6: Write a reducer unit test before wiring `App.tsx`**

Add to whichever test file already covers this reducer (find it — likely `chatCorrelation.test.ts`; if none exists, create `crates/vox-gui/ui/src/lib/chatCorrelation.test.ts` matching the file's own testing conventions):

```typescript
it('chatPending appends a pending assistant bubble, chatReplySettled replaces it on success', () => {
  let state = initialChatState; // use the real initial-state constructor/fixture this test file already has
  state = chatReducer(state, { type: 'chatPending', sessionId: 's1', tempId: 't1', userText: 'hi' });
  expect(state.messages.some(m => m.id === 't1' && m.status === 'pending')).toBe(true);
  state = chatReducer(state, {
    type: 'chatReplySettled',
    sessionId: 's1',
    tempId: 't1',
    result: { ok: true, message: { id: 'm1', role: 'assistant', text: 'hello back', status: 'done', runId: 't1' } },
  });
  expect(state.messages.some(m => m.id === 'm1' && m.status === 'done')).toBe(true);
  expect(state.messages.some(m => m.id === 't1')).toBe(false);
});
```

Run it, confirm it fails first (functions/cases don't exist), then confirm it passes after Step 5's implementation — this is the TDD gap the first draft's Task 2 had (frontend reducer changes were previously undertested).

- [ ] **Step 7: Wire `handleLoquelaSubmit` for plain chat sends, with a pending bubble**

Read `App.tsx:735-833` in full first (do not disturb the existing `dispatchAttempt`/`allowDuplicate` retry loop for `submit_orchestrator_task`, used for every OTHER `task_category`). Immediately after the existing `invoke('chat_append_message', ...)` call (line 742-744) and before the `dispatchAttempt` block, add:

```typescript
    if (payload.task_category === 'chat') {
      const tempId = `pending-${crypto.randomUUID()}`;
      dispatchSessionChat({ type: 'chatPending', sessionId, tempId, userText: payload.description });
      try {
        const reply = await sendChatMessage({
          session_id: sessionId,
          content: payload.description,
          active_skill: payload.active_skill ?? activeSkill?.id ?? null,
        });
        dispatchSessionChat({
          type: 'chatReplySettled',
          sessionId,
          tempId,
          result: { ok: true, message: { id: reply.id, role: 'assistant', text: reply.text, status: 'done', runId: tempId, modelId: reply.modelId } },
        });
      } catch (err) {
        dispatchSessionChat({
          type: 'chatReplySettled',
          sessionId,
          tempId,
          result: { ok: false, error: sanitizeErrorForToast(err) },
        });
        pushToast({ tone: 'warn', title: 'Chat reply failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      }
      recordGamifyGuiEvent('chat_message_sent', { session_id: sessionId }, { enabled: gamifySettings.enabled });
      return;
    }
```

**Correction from the first draft:** the dispatch function is `dispatchSessionChat`, not a bare `dispatch` — the first draft's code called a nonexistent identifier and would not have compiled. Confirm this is still the real name in scope near `handleLoquelaSubmit` before finalizing (it was independently verified at `App.tsx:296` during this review pass, but re-check given App.tsx may have changed by execution time). Add `import { sendChatMessage } from './lib/chatSend';` near the file's other `lib/` imports.

- [ ] **Step 8 (Phase B→C gate — mandatory, see the Phased execution structure above): manually verify with the dev server**

Use `preview_start` with the `vox-gui`/`vox-gui-limes` launch config. If it binds in this environment: open the Chat surface, send a plain message, confirm (a) a pending/loading bubble appears immediately, (b) it's replaced by a real assistant reply without a page reload, (c) sending while the daemon is unreachable shows a failed-state bubble plus a toast, not a silent hang.

If it does not bind (as happened earlier this session): explicitly log that limitation here, and get a second reviewer (per this session's established two-stage review pattern) to independently confirm the static evidence (passing tests + a careful manual code read of the full path) is an acceptable substitute before proceeding — do not silently treat a green test suite as equivalent to this gate.

- [ ] **Step 9: Run the full frontend test suite for touched files**

Run: `cd crates/vox-gui/ui && pnpm typecheck && pnpm vitest run src/lib/chatSend.test.ts src/lib/chatCorrelation.test.ts src/App.test.tsx`
Expected: typecheck clean, all tests pass. **Note (correction from the first draft):** `App.test.tsx` was grepped during this review and has **zero existing tests** referencing `task_category`, `submit_orchestrator_task`, or `handleLoquelaSubmit` — there is no stale old-behavior test to update, contrary to the first draft's assumption. Instead, ADD a new test here asserting that a `task_category: 'chat'` submit calls `chat_send_message` (mock `invoke`) and does NOT call `submit_orchestrator_task`.

- [ ] **Step 10: Commit**

```bash
git add crates/vox-gui/ui/src/lib/chatSend.ts crates/vox-gui/ui/src/lib/chatSend.test.ts crates/vox-gui/ui/src/lib/chatCorrelation.ts crates/vox-gui/ui/src/lib/chatCorrelation.test.ts crates/vox-gui/ui/src/lib/sessionChatStore.ts crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/App.test.tsx
git commit -m "feat(vox-gui): wire plain chat sends to the synchronous agent-loop reply path, with a pending bubble"
```

---

## Task 3: Thread the real active skill instead of hardcoded `null`

**Files:**
- Modify: `crates/vox-gui/src/commands/chat.rs` (`secretary_confirm_task`, confirmed real span: **lines 213-249**)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (`confirmSecretaryTask`, confirmed real span: **lines 654-665**, not "~657" as the first draft vaguely cited) + `ChatSurfaceProps` (lines 313-352)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/surfaceComponents.tsx` (the real `<ChatSurface ... />` render site, confirmed at line 186)

**Correction from the first draft:** `ChatSurface.tsx` has **zero** existing pinned-skill state (grepped for `activeSkill`/`pinnedSkill`/`skill`, case-insensitive — no matches) and `ChatSurfaceProps` has no skill-related prop. The first draft's Step 5 ("search the file for whatever pinned-skill state it already tracks") would have found nothing — this revision adds the missing prop-threading explicitly instead of punting on it.

This task has no logical dependency on Task 1/2 and may run in a parallel subagent alongside Task 2, per the Phased execution structure above (same file as Task 1, but a disjoint function — only a mechanical rebase risk).

- [ ] **Step 1: Write the failing backend test**

```rust
#[test]
fn secretary_confirm_task_params_include_active_skill_when_provided() {
    let params = build_submit_task_params("sess-1", "do the thing", Some("code-review"));
    assert_eq!(params["active_skill"], serde_json::json!("code-review"));
}

#[test]
fn secretary_confirm_task_params_active_skill_null_when_absent() {
    let params = build_submit_task_params("sess-1", "do the thing", None);
    assert_eq!(params["active_skill"], serde_json::Value::Null);
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p vox-gui --lib commands::chat -- secretary_confirm_task_params`
Expected: compile error, `build_submit_task_params` not found.

- [ ] **Step 3: Extract a testable pure function, thread a new parameter**

In `crates/vox-gui/src/commands/chat.rs:213-249`, extract the `params` construction:

```rust
fn build_submit_task_params(
    session_id: &str,
    intent: &str,
    active_skill: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "description": intent,
        "file_manifest": [],
        "priority": null,
        "session_id": session_id,
        "allow_duplicate": false,
        "model_hint": null,
        "dry_run": null,
        "active_skill": active_skill,
    })
}
```

Update `secretary_confirm_task`'s signature to accept `active_skill: Option<String>` (a new Tauri-command argument), replace its inline `params` literal with:

```rust
    let params = build_submit_task_params(&session_id, &intent, active_skill.as_deref());
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test -p vox-gui --lib commands::chat`
Expected: all pass.

- [ ] **Step 5: Thread `activeSkillId` from `App.tsx` down to `ChatSurface.tsx`**

In `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`:

```typescript
// ChatSurfaceProps (extend the existing interface, ~lines 313-352):
interface ChatSurfaceProps {
  // ...existing props unchanged...
  activeSkillId?: string | null;
}
```

In `confirmSecretaryTask` (lines 654-665), update the `invoke` call:

```typescript
  await invoke('secretary_confirm_task', {
    sessionId: payload.session_id,
    intent: payload.intent,
    activeSkill: activeSkillId ?? null,
  });
```

(Confirm the existing call's exact parameter names — `sessionId`/`intent` above are placeholders for whatever the real current call already passes; only ADD the new `activeSkill` field, do not rename the existing ones without checking `secretary_confirm_task`'s real Tauri argument names first.)

In `crates/vox-gui/ui/src/components/surfaces/Chat/surfaceComponents.tsx:186`, add the new prop to the render site:

```typescript
  <ChatSurface ... activeSkillId={activeSkill?.id ?? null} />
```

This requires `activeSkill` (the same `App.tsx:238` state Task 2 already reads) to be threaded into whatever props `surfaceComponents.tsx` itself receives from `App.tsx` — read `surfaceComponents.tsx`'s own prop chain first; if `activeSkill` isn't already passed down to it, add it as a new prop there too, one level at a time, rather than reaching past intermediate components.

- [ ] **Step 6: Write a frontend test for the new prop threading**

Add to `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx`:

```typescript
it('passes activeSkillId through to secretary_confirm_task', async () => {
  // Render ChatSurface with activeSkillId="code-review", trigger confirmSecretaryTask
  // (via whatever test harness this file's existing secretary-related tests already use),
  // assert the mocked invoke('secretary_confirm_task', ...) call included
  // { activeSkill: 'code-review' } — mirror this file's existing invoke-mock-assertion
  // pattern exactly rather than inventing a new one.
});
```

Write the real assertion body by copying the mock-and-assert idiom from this file's existing `secretary_confirm_task`-related test(s) (there should be at least one, since `confirmSecretaryTask` already has coverage per the earlier gap-analysis audit's Task 4.1/4.2 review pattern) — do not leave this as a stub.

- [ ] **Step 7: Verify and commit**

Run: `cargo test -p vox-gui --lib && cd crates/vox-gui/ui && pnpm typecheck && pnpm vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: all green.

```bash
git add crates/vox-gui/src/commands/chat.rs crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx crates/vox-gui/ui/src/components/surfaces/Chat/surfaceComponents.tsx
git commit -m "fix(vox-gui): thread the pinned active skill into secretary task submission"
```

---

## Task 4: Golden-task regression coverage in `vox harness eval`

**Files:**
- Modify: `crates/vox-cli/src/commands/harness/eval.rs`
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs`
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/local_health.rs`

Has no dependency on Tasks 1-3 — dispatch in Phase A alongside Task 1.

**Corrections from the first draft (all independently verified this review):**
- `GoldenTask` really has a third, mandatory field: `struct GoldenTask { name: &'static str, run: fn() -> Result<()>, skip_if: Option<fn() -> Option<String>> }` (`eval.rs:66-70`). Every registration in this task must include `skip_if: None` or it will not compile.
- `vox-orchestrator-mcp` is already a `vox-cli` dependency (`crates/vox-cli/Cargo.toml:210`) — no Cargo change needed there, and there is no dependency cycle (`vox-orchestrator-mcp` depends on `vox-cli-core`, not `vox-cli`).
- The tool registry constant is `vox_mcp_registry::TOOL_REGISTRY`, not `vox_orchestrator_mcp::TOOL_REGISTRY`; the test-fixture skill-registry constructor is `new_registry_arc()`, not `skill_registry_for_tests`. `TurnContext.exclude_name_prefixes: Vec<&'static str>` is confirmed still present and correctly named.
- **The first draft's Steps 5 and 6 (`agent-loop-terminates`, `privacy-filter-blocks-live-routing`) referenced functions that do not exist anywhere in the codebase** (`run_agent_turn_against_always_tool_calling_model_for_eval_gate`, `env_test_lock()`, `local_only_privacy_mode`, `fixture_cloud_model_spec`/`fixture_local_model_spec`) and targeted `pub(crate)`-only items (`run_agent_turn`, `privacy_allows`, `inference_privacy_mode`) that `vox-cli` cannot call at all without new exports. This revision replaces both with real, minimal new `pub` functions purpose-built for the eval gate — not "reuse," genuinely new code, written out in full below.

- [ ] **Step 1: `tool-cap-never-exceeds-cap` — write the failing test**

```rust
#[test]
fn tool_cap_never_exceeds_cap_task_passes() {
    let result = tool_cap_never_exceeds_cap_task();
    assert!(result.is_ok(), "{result:?}");
}
```

Run: `cargo test -p vox-cli --lib commands::harness::eval -- tool_cap_never_exceeds_cap_task_passes`
Expected: compile error, function not found.

- [ ] **Step 2: Implement it with the corrected real names**

```rust
fn tool_cap_never_exceeds_cap_task() -> anyhow::Result<()> {
    use vox_mcp_registry::TOOL_REGISTRY;
    use vox_orchestrator_mcp::llm_bridge::tool_selection::{
        DEFAULT_MAX_TOOLS, TurnContext, new_registry_arc, select_tools_for_turn,
    };

    let ctx = TurnContext {
        permission_mode: None,
        lanes: vec!["default"],
        active_skill_id: None,
        max_tools: DEFAULT_MAX_TOOLS,
        exclude_name_prefixes: vec!["vox_chat_"],
    };
    let reg = new_registry_arc();
    let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
    if selected.len() > DEFAULT_MAX_TOOLS {
        anyhow::bail!("selected {} tools, cap is {}", selected.len(), DEFAULT_MAX_TOOLS);
    }
    if selected.iter().any(|t| t.name.starts_with("vox_chat_")) {
        anyhow::bail!("a vox_chat_* tool was offered to the model — recursion-guard regression");
    }
    Ok(())
}
```

Before finalizing, confirm `new_registry_arc` and the `vox_mcp_registry::TOOL_REGISTRY` re-export are genuinely `pub` (not `pub(crate)`) from `vox-cli`'s vantage point — grep their definitions' visibility keyword directly; if either is `pub(crate)`, widen it (same pattern as this session's other `pub(crate)`→needed-elsewhere widenings) rather than duplicating the fixture.

- [ ] **Step 3: Register it, run**

```rust
GoldenTask { name: "tool-cap-never-exceeds-cap", run: tool_cap_never_exceeds_cap_task, skip_if: None },
```

Run: `cargo test -p vox-cli --lib commands::harness::eval`
Expected: pass.

- [ ] **Step 4: `agent-loop-terminates` — add a real, new, purpose-built eval-gate check**

First, in `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs`, write the failing test for the new function (add to this file's own `#[cfg(test)] mod tests`):

```rust
#[tokio::test]
async fn eval_gate_agent_loop_terminates_check_reports_ok_when_loop_is_bounded() {
    let result = eval_gate_agent_loop_terminates_check().await;
    assert!(result.is_ok(), "{result:?}");
}
```

Run: `cargo test -p vox-orchestrator-mcp --lib chat_tools::chat::agent_loop -- eval_gate_agent_loop_terminates_check`
Expected: compile error.

Implement, reusing this file's own existing wiremock server setup from `max_iterations_bound_actually_stops_the_loop` (read that test's body first — copy its wiremock-server-returning-a-tool-call-every-time setup verbatim rather than re-deriving it) and calling the real `pub(crate) run_agent_turn` from within the SAME crate (no cross-crate visibility problem here, since this function lives in `vox-orchestrator-mcp` itself):

```rust
/// Purpose-built for `vox harness eval`'s `agent-loop-terminates` golden
/// task: stands up a wiremock model that always returns a tool call, runs
/// `run_agent_turn` against it, and asserts the loop stops at its iteration
/// cap rather than recursing forever. `pub` (not `pub(crate)`) specifically
/// so `vox-cli`'s eval gate can call it — this is new code, not a reuse of
/// `max_iterations_bound_actually_stops_the_loop`'s test body, though it
/// reuses that test's wiremock setup pattern.
pub async fn eval_gate_agent_loop_terminates_check() -> Result<(), String> {
    // Body: copy max_iterations_bound_actually_stops_the_loop's wiremock
    // server construction and run_agent_turn call exactly, but return
    // Result<(), String> instead of using #[tokio::test]'s panic-on-assert
    // convention, so a caller outside this crate's test harness can inspect
    // the outcome programmatically:
    //   let outcome = run_agent_turn(...).await.map_err(|e| e.to_string())?;
    //   if !outcome.hit_iteration_limit {
    //       return Err("expected the loop to hit its iteration cap and stop".to_string());
    //   }
    //   Ok(())
    todo!("copy the real wiremock setup from max_iterations_bound_actually_stops_the_loop, adjust as shown above")
}
```

Do not commit a `todo!()` — replace it with the real body once `max_iterations_bound_actually_stops_the_loop`'s exact wiremock construction has been read and copied. This step's `todo!()` is a plan-document placeholder for "go read the real test first," not implementation code to ship.

Run the new test, confirm it passes. Then in `crates/vox-cli/src/commands/harness/eval.rs`, add the golden task:

```rust
fn agent_loop_terminates_task() -> anyhow::Result<()> {
    // vox harness eval's own run: fn() -> Result<()> signature is synchronous;
    // block_on the async check here the same way this file already bridges
    // any other async work into its synchronous GoldenTask::run — check
    // whether eval.rs already has a tokio runtime handle in scope for this
    // (e.g. for live-model-smoke) and reuse it rather than spinning up a
    // second one.
    let rt = tokio::runtime::Handle::try_current()
        .map_err(|_| anyhow::anyhow!("no tokio runtime available for agent-loop-terminates"))?;
    rt.block_on(vox_orchestrator_mcp::chat_tools::chat::agent_loop::eval_gate_agent_loop_terminates_check())
        .map_err(|e| anyhow::anyhow!(e))
}
```

Confirm whether `golden_tasks()`'s existing `run: fn() -> Result<()>` signature is actually called from an async context anywhere already (check how `live-model-smoke`, the one existing task that would plausibly need async, handles this) — reuse that exact bridging approach instead of the sketch above if it differs.

Register: `GoldenTask { name: "agent-loop-terminates", run: agent_loop_terminates_task, skip_if: None }`.

Run: `cargo test -p vox-cli --lib commands::harness::eval && cargo run -p vox-cli -- harness eval --task agent-loop-terminates`
Expected: pass.

- [ ] **Step 5: `privacy-filter-blocks-live-routing` — extract a pure, mode-parameterized core (no env mutation)**

First, in `crates/vox-orchestrator-mcp/src/llm_bridge/local_health.rs`, write the failing test:

```rust
#[test]
fn privacy_allows_for_mode_blocks_cloud_under_local_only_and_allows_local() {
    let cloud = gate_spec("cloud-model", ProviderType::OpenRouter);
    let local = gate_spec("local-model", ProviderType::Ollama);
    assert!(!privacy_allows_for_mode(&cloud, true), "cloud model must be blocked under local_only");
    assert!(privacy_allows_for_mode(&local, true), "local model must be allowed under local_only");
    assert!(privacy_allows_for_mode(&cloud, false), "cloud model must be allowed when not local_only");
}
```

Run: `cargo test -p vox-orchestrator-mcp --lib llm_bridge::local_health -- privacy_allows_for_mode`
Expected: compile error, `privacy_allows_for_mode` not found (reuses the existing `gate_spec` fixture, confirmed real at `local_health.rs:172`).

Implement by extracting the real logic out of the existing `privacy_allows` into a pure, explicit-mode core, then having `privacy_allows` delegate to it (this is a refactor of existing real logic, not new invented behavior — read `privacy_allows`'s current body first and move its post-env-read logic verbatim into the new function):

```rust
/// The pure decision core `privacy_allows` delegates to after reading
/// `VOX_INFERENCE_PRIVACY` from the environment. Exposed `pub` (read: check
/// this crate's actual visibility needs — `pub(crate)` may suffice if the
/// eval-gate caller ends up living inside this crate per Step 6 below rather
/// than in `vox-cli` directly) specifically so the eval gate can exercise
/// the real decision logic with an explicit mode, without mutating process
/// environment variables — this codebase's own `#[cfg(test)]`-only
/// `TEST_PRIVACY_OVERRIDE` mechanism was deliberately written to avoid env
/// mutation in tests; this eval-gate path preserves that intent instead of
/// reintroducing it via `std::env::set_var`.
pub(crate) fn privacy_allows_for_mode(model: &ModelSpec, local_only: bool) -> bool {
    if !local_only {
        return true;
    }
    is_local_http_provider(model.provider_type)
}

pub(crate) fn privacy_allows(model: &ModelSpec) -> bool {
    let local_only = inference_privacy_mode() == "local_only"; // adjust to the real return type/comparison once read
    privacy_allows_for_mode(model, local_only)
}
```

Read `privacy_allows`'s CURRENT real body before finalizing this step — the sketch above assumes its logic reduces to exactly "local_only and not a local provider ⇒ false, else true" (confirmed by an earlier holistic-review pass this session as `!local_only || is_local_provider`), but confirm the exact current implementation and adjust `privacy_allows_for_mode`'s body to match it precisely, not the paraphrase above.

- [ ] **Step 6: Decide the eval-gate call boundary, add the golden task**

`privacy_allows_for_mode` may need to be `pub` (crate-external) rather than `pub(crate)` depending on whether the golden task's `run` function lives in `vox-cli` (cross-crate, needs `pub`) or whether it's cleaner to add a second small `pub async fn eval_gate_privacy_filter_check() -> Result<(), String>` inside `vox-orchestrator-mcp` itself (mirroring Step 4's `agent-loop-terminates` approach) that `vox-cli` calls, keeping `privacy_allows_for_mode` itself `pub(crate)`. **Prefer the latter** — it keeps the privacy-decision internals encapsulated in the crate that owns them and matches the pattern Step 4 already established, rather than exporting a scoring primitive across the crate boundary. Add:

```rust
/// Purpose-built for `vox harness eval`'s `privacy-filter-blocks-live-routing`
/// golden task.
pub async fn eval_gate_privacy_filter_check() -> Result<(), String> {
    let cloud = gate_spec("cloud-model", ProviderType::OpenRouter);
    let local = gate_spec("local-model", ProviderType::Ollama);
    if !privacy_allows_for_mode(&local, true) {
        return Err("local model was blocked under local_only — over-blocking regression".to_string());
    }
    if privacy_allows_for_mode(&cloud, true) {
        return Err("cloud model was allowed under local_only — privacy-filter regression".to_string());
    }
    Ok(())
}
```

Then in `eval.rs`:

```rust
fn privacy_filter_blocks_live_routing_task() -> anyhow::Result<()> {
    let rt = tokio::runtime::Handle::try_current()
        .map_err(|_| anyhow::anyhow!("no tokio runtime available for privacy-filter-blocks-live-routing"))?;
    rt.block_on(vox_orchestrator_mcp::llm_bridge::local_health::eval_gate_privacy_filter_check())
        .map_err(|e| anyhow::anyhow!(e))
}
```

Register: `GoldenTask { name: "privacy-filter-blocks-live-routing", run: privacy_filter_blocks_live_routing_task, skip_if: None }`.

Run: `cargo test -p vox-orchestrator-mcp --lib llm_bridge::local_health && cargo test -p vox-cli --lib commands::harness::eval && cargo run -p vox-cli -- harness eval --task privacy-filter-blocks-live-routing`
Expected: all pass.

- [ ] **Step 7: Run the full gate and full crate test suite**

```bash
cargo run -p vox-cli -- harness eval --samples 3
cargo test -p vox-cli --lib
cargo test -p vox-orchestrator-mcp --lib
cargo clippy -p vox-cli -p vox-orchestrator-mcp --lib -- -D warnings
```
Expected: exit 0 from the eval run with all 7 tasks (4 original + 3 new) reporting PASS/SKIP as appropriate; all tests pass; clippy clean.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli/src/commands/harness/eval.rs crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs crates/vox-orchestrator-mcp/src/llm_bridge/local_health.rs
git commit -m "test: add agent-loop/tool-cap/privacy-filter golden tasks to vox harness eval"
```

---

## Task 5: Verification pass — re-confirm previously-claimed-fixed findings

**Files:** none modified — this task only runs commands and records results; if any command fails, STOP and open a new task to fix the regression rather than silently proceeding. Runs last (Phase C), after Task 2's mandatory live-verification gate.

- [ ] **Step 1: Re-run named tests for each "verified" finding (not just a blanket crate sweep)**

```bash
cargo test -p vox-gui --lib commands::chat -- chat_append_message_does_not_auto_dispatch_to_daemon secretary_skips_messages_the_composer_already_submitted
cargo test -p vox-llm-egress --test wire_mock
cargo test -p vox-orchestrator --lib models::vram
cargo test -p vox-orchestrator --lib
cargo test -p vox-orchestrator-mcp --lib
cargo test -p vox-db --lib
cargo test -p vox-cli --lib
cargo test -p vox-plugin-host --lib
cargo test -p vox-plugin-types --lib
```
Expected: all pass (0 unexplained failures; the one pre-existing unrelated `merge_group_fanout_guard` failure is expected and may be ignored).

- [ ] **Step 2: Re-run named frontend tests, then the full suite**

```bash
cd crates/vox-gui/ui
pnpm vitest run src/components/layout/VersionMismatchBanner.test.tsx src/components/ui/ErrorBoundary.test.tsx src/lib/toastQueue.test.ts
pnpm typecheck && pnpm vitest run
```
Expected: clean typecheck, all tests pass.

- [ ] **Step 3: Re-run `vox harness eval` at the default sample count**

```bash
cargo run -p vox-cli -- harness eval --samples 3
```
Expected: exit 0, every golden task (4 original + 3 new from Task 4) reports pass^3 or SKIP (`live-model-smoke`).

- [ ] **Step 4: Spot-check the model scorer's differentiation property (Task 0.1's original acceptance test)**

```bash
cargo run -p vox-cli -- model explain --complexity 1
cargo run -p vox-cli -- model explain --complexity 9
```
Expected: visibly different models/rationale — read the output, don't just check exit code.

- [ ] **Step 5: Confirm Task 2's mandatory live-verification gate was actually satisfied**

Re-read Task 2 Step 8's outcome (either a real browser observation, or an explicitly logged, second-reviewed reason it wasn't possible). If neither is present, this task is NOT complete — go back and satisfy Task 2 Step 8 before proceeding.

- [ ] **Step 6: Record results**

If every step above passed with no unexplained failures, this task is complete — no commit needed. If anything failed, create a new task in the task tracker describing the regression precisely (command, expected vs. actual output) before continuing.

---

## Self-Review Notes (for whoever executes this plan)

- This plan went through one full adversarial-review cycle already (4 parallel review agents against the live codebase) — the corrections above are real, verified fixes to real errors in the first draft, not hypothetical hardening. Treat every "confirm before finalizing" instruction remaining in this document as a genuine unresolved unknown, not boilerplate caution — each one is a spot where the reviewers could not fully pin down an internal implementation detail without executing the plan.
- Task 4 Steps 4 and 5 both use a `todo!()`/prose-described function body rather than final Rust — this is the ONE place this plan intentionally defers literal code, because it requires reading another function's exact current body first (`max_iterations_bound_actually_stops_the_loop`'s wiremock setup, `privacy_allows`'s exact current logic) to copy correctly; guessing that code without reading it would very likely have been wrong, exactly as several first-draft guesses were. Read first, then write.
- If, during execution, Task 2 Step 0's investigation reveals the reducer is structured very differently from what's described here (e.g. a single combined action already generically supports "settle a pending bubble with arbitrary content"), prefer reusing it over adding `chatPending`/`chatReplySettled` — the guidance to add new actions is a considered default given what's known now, not a mandate to avoid a cleaner fit if one turns out to exist.
