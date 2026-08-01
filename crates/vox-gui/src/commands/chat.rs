//! GUI chat session persistence via `conversations` / `conversation_messages`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use vox_db::VoxDb;

use crate::commands::daemon::PersistentDaemon;
use crate::commands::gui_db_pool::{GuiDbPool, map_db_err};

fn pool_db(pool: &GuiDbPool) -> Result<Arc<VoxDb>, String> {
    pool.handle()
}

#[derive(Debug, Serialize)]
pub struct ChatSessionDto {
    pub session_id: String,
    pub title: String,
    pub updated_at: String,
    pub message_count: i64,
    pub conversation_id: i64,
}

#[derive(Debug, Serialize)]
pub struct ChatMessageDto {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub task_id: Option<String>,
    /// Model that produced this message, if recorded at append time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[tauri::command]
pub async fn chat_create_session(
    pool: State<'_, GuiDbPool>,
    title: Option<String>,
) -> Result<ChatSessionDto, String> {
    let db = pool_db(&pool)?;
    let session_id = uuid::Uuid::new_v4().to_string();
    let title = title.unwrap_or_else(|| "New chat".to_string());
    let conv_id = db
        .chat_ensure_gui_session(&session_id, &title)
        .await
        .map_err(map_db_err)?;
    Ok(ChatSessionDto {
        session_id,
        title,
        updated_at: String::new(),
        message_count: 0,
        conversation_id: conv_id,
    })
}

#[tauri::command]
pub async fn chat_list_sessions(
    pool: State<'_, GuiDbPool>,
    limit: Option<usize>,
) -> Result<Vec<ChatSessionDto>, String> {
    let db = pool_db(&pool)?;
    let lim = limit.unwrap_or(40);
    let rows = db.chat_list_gui_sessions(lim).await.map_err(map_db_err)?;
    Ok(rows
        .into_iter()
        .map(
            |(conversation_id, title, session_id, updated_at, message_count)| ChatSessionDto {
                session_id,
                title,
                updated_at,
                message_count,
                conversation_id,
            },
        )
        .collect())
}

#[tauri::command]
pub async fn chat_get_messages(
    pool: State<'_, GuiDbPool>,
    session_id: String,
    limit: Option<usize>,
) -> Result<Vec<ChatMessageDto>, String> {
    let db = pool_db(&pool)?;
    let lim = limit.unwrap_or(500);
    let rows = db
        .chat_get_gui_messages(&session_id, lim)
        .await
        .map_err(map_db_err)?;
    Ok(rows
        .into_iter()
        .map(|(id, role, content, created_at, payload)| {
            let (task_id, model_id) = payload
                .and_then(|p| serde_json::from_str::<serde_json::Value>(&p).ok())
                .map(|v| {
                    let task_id = v
                        .get("task_id")
                        .and_then(|t| t.as_str())
                        .map(str::to_string);
                    let model_id = v
                        .get("model_id")
                        .and_then(|m| m.as_str())
                        .map(str::to_string);
                    (task_id, model_id)
                })
                .unwrap_or((None, None));
            ChatMessageDto {
                id,
                role,
                content,
                created_at,
                task_id,
                model_id,
            }
        })
        .collect())
}

#[derive(Debug, Deserialize)]
pub struct ChatAppendInput {
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub task_id: Option<String>,
    /// Optional model id to record in the message payload (e.g. for assistant messages).
    #[serde(default)]
    pub model_id: Option<String>,
    /// True when the composer already dispatched this message as a task
    /// (`submit_orchestrator_task`). The secretary must not submit it again
    /// (C2: every actionable composer message used to be SUBMIT_TASK'd twice).
    #[serde(default)]
    pub already_submitted: bool,
}

#[tauri::command]
pub async fn chat_append_message<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    input: ChatAppendInput,
    pool: State<'_, GuiDbPool>,
    // No longer used to dispatch a task directly (Task 0.2: propose-only —
    // see `secretary_confirm_task` for the daemon call). Kept in the
    // signature so the Tauri command's argument shape (and existing test
    // call sites) don't need to change.
    _daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<i64, String> {
    if input.session_id.trim().is_empty() {
        return Err("session_id must not be empty".to_string());
    }
    if input.role.trim().is_empty() {
        return Err("role must not be empty".to_string());
    }
    let db = pool_db(&pool)?;
    let conv_id = db
        .chat_ensure_gui_session(&input.session_id, "Chat")
        .await
        .map_err(map_db_err)?;
    let payload = match (&input.task_id, &input.model_id) {
        (None, None) => None,
        _ => {
            let mut obj = serde_json::Map::new();
            if let Some(ref t) = input.task_id {
                obj.insert("task_id".to_string(), serde_json::Value::String(t.clone()));
            }
            if let Some(ref m) = input.model_id {
                obj.insert("model_id".to_string(), serde_json::Value::String(m.clone()));
            }
            Some(serde_json::Value::Object(obj).to_string())
        }
    };
    let msg_id = db
        .chat_append_message(conv_id, &input.role, &input.content, payload.as_deref())
        .await
        .map_err(map_db_err)?;

    // Secretary: detect actionable intent in user messages and *propose* it
    // to the user — it must NOT auto-submit to the orchestrator daemon task
    // graph (SUBMIT_TASK). Task 0.2 (harness parity plan): the secretary used
    // to fire SUBMIT_TASK itself here, silently turning any ≥10-word message
    // containing an action verb into a live task. That is now client-side
    // gated: this only emits the proposal toast; the actual SUBMIT_TASK call
    // happens in `secretary_confirm_task`, invoked when the user explicitly
    // clicks "confirm" on the toast (see `SecretaryToast.tsx`).
    if let Some(classified) =
        secretary_candidate(&input.role, &input.content, input.already_submitted)
    {
        // `item_id` here is a client-side proposal id (not a hopper/task id —
        // no task exists yet). `secretary_confirm_task` is keyed on it so the
        // eventual daemon submission uses the exact same session_id/intent
        // the user was shown in the toast.
        let proposal_id = uuid::Uuid::new_v4().to_string();
        crate::commands::orchestrator::emit_secretary_proposed(
            &app_handle,
            crate::commands::orchestrator::SecretaryProposedPayload {
                item_id: proposal_id,
                intent: classified.intent,
                confidence_pct: classified.confidence_pct,
                session_id: input.session_id.clone(),
            },
        );
    }

    Ok(msg_id)
}

/// Submit a secretary-proposed task to the orchestrator daemon (SUBMIT_TASK),
/// only ever called from the frontend when the user explicitly clicks
/// "confirm" on the `SecretaryToast` — this is the sole path by which a
/// secretary classification becomes a live task (Task 0.2: auto-dispatch →
/// propose-only).
#[tauri::command]
pub async fn secretary_confirm_task<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    session_id: String,
    intent: String,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<Option<String>, String> {
    use vox_foundation::protocol::orch_daemon_method;
    use vox_orchestrator::orch_daemon::OrchDaemonClient;

    let params = serde_json::json!({
        "description": intent,
        "file_manifest": [],
        "priority": null,
        "session_id": session_id,
        "allow_duplicate": false,
        "model_hint": null,
        "dry_run": null,
        "active_skill": null,
    });
    let addr = daemon.ensure().await.map_err(|e| e.to_string())?;
    let client = match daemon.token().await {
        Some(token) => OrchDaemonClient::with_token(addr, token),
        None => OrchDaemonClient::new(addr),
    };
    let raw = client
        .call(orch_daemon_method::SUBMIT_TASK, params)
        .await
        .map_err(|e| e.to_string())?;

    let task_id = submitted_task_id(&raw);
    if task_id.is_some() {
        // Only ping the tasks list when something new was actually enqueued —
        // a deduped submission (task_id null, duplicate_of set) changed nothing.
        crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    }
    Ok(task_id)
}

#[tauri::command]
pub async fn chat_rename_session(
    pool: State<'_, GuiDbPool>,
    session_id: String,
    title: String,
) -> Result<(), String> {
    let db = pool_db(&pool)?;
    let conv_id = db
        .chat_find_gui_conversation_id(&session_id)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| "session not found".to_string())?;
    db.chat_rename_conversation(conv_id, &title)
        .await
        .map_err(map_db_err)
}

#[tauri::command]
pub async fn chat_archive_session(
    pool: State<'_, GuiDbPool>,
    session_id: String,
) -> Result<(), String> {
    let db = pool_db(&pool)?;
    let conv_id = db
        .chat_find_gui_conversation_id(&session_id)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| "session not found".to_string())?;
    db.chat_archive_conversation(conv_id)
        .await
        .map_err(map_db_err)
}

/// Secretary gate: never classify a message the composer already submitted
/// as a task — that path caused every actionable composer message to be
/// SUBMIT_TASK'd twice (explicit submit + secretary re-submit).
fn secretary_candidate(
    role: &str,
    content: &str,
    already_submitted: bool,
) -> Option<vox_orchestrator::secretary::ClassifyResult> {
    if already_submitted {
        return None;
    }
    vox_orchestrator::secretary::classify(role, content)
}

/// Task id from a `SUBMIT_TASK` daemon reply. `None` means the daemon
/// deduped the submission (`task_id: null` + `duplicate_of`): nothing new
/// was created, so no "Secretary proposed a task" toast may be emitted.
fn submitted_task_id(raw: &serde_json::Value) -> Option<String> {
    raw.get("task_id")
        .and_then(|v| v.as_u64())
        .map(|v| v.to_string())
}

#[derive(Debug, Deserialize)]
pub struct ChatSendInput {
    pub session_id: String,
    pub content: String,
    #[serde(default)]
    pub active_skill: Option<String>,
}

/// Parsed reply extracted from a `vox_chat_message` `ToolResult` envelope.
#[derive(Debug)]
struct ParsedChatReply {
    content: String,
    model_id: Option<String>,
}

/// Extracts a [`ParsedChatReply`] from a `vox_chat_message` `ToolResult`
/// envelope (`{"success", "data": {"message": {..., "content"}, "model_used"}}`
/// or `{"success": false, "error"}`) as returned directly by
/// `OrchDaemonClient::call(TOOL_CALL, ...)`.
fn parse_chat_message_envelope(envelope: &serde_json::Value) -> Result<ParsedChatReply, String> {
    let success = envelope
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
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
    Ok(ParsedChatReply { content, model_id })
}

/// Persists an already-parsed assistant reply and returns a DTO with a real
/// (not blank) `created_at`. Split out from `chat_send_message` so this half
/// of the flow — the part that doesn't need a live daemon — is independently
/// unit-testable against the in-memory test DB.
async fn persist_assistant_reply(
    db: &VoxDb,
    conv_id: i64,
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
    Ok(ChatMessageDto {
        id: msg_id,
        role: "assistant".to_string(),
        content: content.to_string(),
        created_at,
        task_id: None,
        model_id: model_id.map(str::to_string),
    })
}

/// Synchronous chat reply: calls the real agent loop (`vox_chat_message` via
/// the daemon's `orch.tool_call`) and persists the assistant's reply via
/// `persist_assistant_reply`. Unlike `secretary_confirm_task` (which submits
/// a background work-order task via `SUBMIT_TASK`), this returns a
/// model-authored reply directly for immediate transcript rendering — the
/// synchronous chat path this GUI never had. Mirrors `secretary_confirm_task`'s
/// daemon-call shape exactly (same file, a few lines above) rather than going
/// through the generic `invoke_mcp_tool` command, whose extra
/// `{"tool","is_error","result"}` wrapper this function does not need.
#[tauri::command]
pub async fn chat_send_message<R: tauri::Runtime>(
    _app_handle: tauri::AppHandle<R>,
    input: ChatSendInput,
    pool: State<'_, GuiDbPool>,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
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
        .map_err(|e| e.to_string())?;
    let reply = parse_chat_message_envelope(&envelope)?;

    let db = pool_db(&pool)?;
    let conv_id = db
        .chat_ensure_gui_session(&input.session_id, "Chat")
        .await
        .map_err(map_db_err)?;
    persist_assistant_reply(&db, conv_id, &reply.content, reply.model_id.as_deref()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_db::{DbConfig, VoxDb};

    #[tokio::test]
    async fn chat_append_rejects_empty_session_id() {
        use tauri::Manager;
        let app = tauri::test::mock_app();
        app.manage(Arc::new(PersistentDaemon::default()));
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let input = ChatAppendInput {
            session_id: "   ".to_string(),
            role: "user".to_string(),
            content: "hi".to_string(),
            task_id: None,
            model_id: None,
            already_submitted: false,
        };
        let daemon = app.state::<Arc<PersistentDaemon>>();
        let pool = app.state::<GuiDbPool>();
        let err = chat_append_message(app.handle().clone(), input, pool, daemon)
            .await
            .expect_err("empty session");
        assert!(err.contains("session_id"));
    }

    #[tokio::test]
    async fn chat_session_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let session_id = "test-session-1";
        let conv = db
            .chat_ensure_gui_session(session_id, "Test")
            .await
            .expect("ensure");
        let msg_id = db
            .chat_append_message(conv, "user", "hello", None)
            .await
            .expect("append");
        assert!(msg_id > 0);
        let msgs = db.chat_get_gui_messages(session_id, 10).await.expect("get");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].2, "hello");
    }

    /// Task 0.2 regression: a classified message must NOT trigger a
    /// `SUBMIT_TASK` daemon round-trip from `chat_append_message` — it only
    /// emits the proposal event. Before this fix, an actionable message
    /// spawned a task that called `daemon.ensure()` (which spawns/connects to
    /// the real orchestrator daemon binary and can take seconds). Bounding
    /// the call in a short timeout proves no such daemon interaction happens
    /// on this path anymore.
    #[tokio::test]
    async fn chat_append_message_does_not_auto_dispatch_to_daemon() {
        use tauri::Manager;
        let app = tauri::test::mock_app();
        app.manage(Arc::new(PersistentDaemon::default()));
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let input = ChatAppendInput {
            session_id: "propose-only-session".to_string(),
            role: "user".to_string(),
            // Actionable per the classifier (>=10 words, whole-word verb).
            content: "please fix the broken authentication flow in the login \
                      page it keeps failing"
                .to_string(),
            task_id: None,
            model_id: None,
            already_submitted: false,
        };
        let daemon = app.state::<Arc<PersistentDaemon>>();
        let pool = app.state::<GuiDbPool>();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            chat_append_message(app.handle().clone(), input, pool, daemon),
        )
        .await
        .expect("chat_append_message must not block on the orchestrator daemon")
        .expect("append should succeed");
        assert!(result > 0);
    }

    #[test]
    fn secretary_classify_short_user_message_returns_none() {
        // Verify that short messages don't trigger the secretary
        let result = vox_orchestrator::secretary::classify("user", "fix it");
        assert!(result.is_none(), "short message should return None");
    }

    #[test]
    fn secretary_classify_long_action_message_returns_some() {
        let result = vox_orchestrator::secretary::classify(
            "user",
            "fix the broken authentication flow in the login page it keeps redirecting users",
        );
        assert!(result.is_some());
    }

    #[test]
    fn chat_message_dto_model_id_roundtrip() {
        let dto = ChatMessageDto {
            id: 1,
            role: "assistant".to_string(),
            content: "hi".to_string(),
            created_at: "now".to_string(),
            task_id: Some("7".to_string()),
            model_id: Some("anthropic/claude-opus-4-5".to_string()),
        };
        let j = serde_json::to_string(&dto).unwrap();
        assert!(
            j.contains("\"model_id\":\"anthropic/claude-opus-4-5\""),
            "model_id present: {j}"
        );
        assert!(j.contains("\"task_id\":\"7\""), "task_id present: {j}");
    }

    #[test]
    fn chat_message_dto_model_id_absent_when_none() {
        let dto = ChatMessageDto {
            id: 2,
            role: "user".to_string(),
            content: "hello".to_string(),
            created_at: "now".to_string(),
            task_id: None,
            model_id: None,
        };
        let j = serde_json::to_string(&dto).unwrap();
        assert!(!j.contains("model_id"), "model_id absent when None: {j}");
    }

    #[test]
    fn secretary_skips_messages_the_composer_already_submitted() {
        // Precondition: this message IS actionable for the classifier.
        let msg = "fix the broken authentication flow in the login page it keeps redirecting users";
        assert!(
            vox_orchestrator::secretary::classify("user", msg).is_some(),
            "precondition: classifier finds this actionable"
        );
        // The composer already dispatched it -> secretary must stand down.
        assert!(secretary_candidate("user", msg, true).is_none());
        // Same message NOT pre-submitted -> secretary still classifies it.
        assert!(secretary_candidate("user", msg, false).is_some());
    }

    #[test]
    fn submitted_task_id_is_none_when_daemon_dedupes() {
        // Dedupe reply: null task_id + duplicate_of. No toast may be built from this.
        assert_eq!(
            submitted_task_id(&serde_json::json!({"task_id": null, "duplicate_of": 7})),
            None
        );
        assert_eq!(submitted_task_id(&serde_json::json!({})), None);
        assert_eq!(
            submitted_task_id(&serde_json::json!({"task_id": 42})),
            Some("42".to_string())
        );
    }

    #[test]
    fn chat_append_input_already_submitted_defaults_to_false() {
        let input: ChatAppendInput = serde_json::from_str(
            r#"{"session_id":"s","role":"user","content":"hi","task_id":null}"#,
        )
        .expect("older frontends omit the field");
        assert!(!input.already_submitted);
    }

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
        let reply = parse_chat_message_envelope(&envelope).expect("parse ok");
        assert_eq!(reply.content, "Hi there");
        assert_eq!(reply.model_id.as_deref(), Some("openrouter/auto"));
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
        let dto = persist_assistant_reply(&db, conv_id, "Hello!", Some("openrouter/auto"))
            .await
            .expect("persist ok");
        assert_eq!(dto.role, "assistant");
        assert_eq!(dto.content, "Hello!");
        assert_eq!(dto.model_id.as_deref(), Some("openrouter/auto"));
        assert!(!dto.created_at.is_empty(), "created_at must not be blank");
    }
}
