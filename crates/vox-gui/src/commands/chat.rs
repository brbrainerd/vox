//! GUI chat session persistence via `conversations` / `conversation_messages`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vox_db::DbConnectSurface;
use vox_db::connect_workspace_journey_optional;

use crate::commands::daemon::PersistentDaemon;

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

async fn gui_db() -> Result<vox_db::VoxDb, String> {
    connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "workspace database unavailable".to_string())
}

#[tauri::command]
pub async fn chat_create_session(title: Option<String>) -> Result<ChatSessionDto, String> {
    let db = gui_db().await?;
    let session_id = uuid::Uuid::new_v4().to_string();
    let title = title.unwrap_or_else(|| "New chat".to_string());
    let conv_id = db
        .chat_ensure_gui_session(&session_id, &title)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ChatSessionDto {
        session_id,
        title,
        updated_at: String::new(),
        message_count: 0,
        conversation_id: conv_id,
    })
}

#[tauri::command]
pub async fn chat_list_sessions(limit: Option<usize>) -> Result<Vec<ChatSessionDto>, String> {
    let db = gui_db().await?;
    let lim = limit.unwrap_or(40);
    let rows = db
        .chat_list_gui_sessions(lim)
        .await
        .map_err(|e| e.to_string())?;
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
    session_id: String,
    limit: Option<usize>,
) -> Result<Vec<ChatMessageDto>, String> {
    let db = gui_db().await?;
    let lim = limit.unwrap_or(500);
    let rows = db
        .chat_get_gui_messages(&session_id, lim)
        .await
        .map_err(|e| e.to_string())?;
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
}

#[tauri::command]
pub async fn chat_append_message<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    input: ChatAppendInput,
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
) -> Result<i64, String> {
    if input.session_id.trim().is_empty() {
        return Err("session_id must not be empty".to_string());
    }
    if input.role.trim().is_empty() {
        return Err("role must not be empty".to_string());
    }
    let db = gui_db().await?;
    let conv_id = db
        .chat_ensure_gui_session(&input.session_id, "Chat")
        .await
        .map_err(|e| e.to_string())?;
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
        .map_err(|e| e.to_string())?;

    // Secretary: detect actionable intent in user messages and submit to hopper.
    // Fire-and-forget — errors here must never fail the chat message save.
    if let Some(classified) = vox_orchestrator::secretary::classify(&input.role, &input.content) {
        let session_id = input.session_id.clone();
        let app_handle_clone = app_handle.clone();
        let daemon: Arc<PersistentDaemon> = daemon.inner().clone();
        tokio::spawn(async move {
            use vox_foundation::protocol::orch_daemon_method;
            use vox_orchestrator::orch_daemon::OrchDaemonClient;

            let params = serde_json::json!({
                "description": classified.intent,
                "file_manifest": [],
                "priority": null,
                "session_id": session_id,
                "allow_duplicate": false,
                "model_hint": null,
                "dry_run": null,
                "active_skill": null,
            });
            let submit_result = async {
                let addr = daemon.ensure().await?;
                let client = match daemon.token().await {
                    Some(token) => OrchDaemonClient::with_token(addr, token),
                    None => OrchDaemonClient::new(addr),
                };
                client
                    .call(orch_daemon_method::SUBMIT_TASK, params)
                    .await
                    .map_err(|e| e.to_string())
            }
            .await;
            match submit_result {
                Ok(raw) => {
                    let item_id = raw
                        .get("task_id")
                        .and_then(|v| v.as_u64())
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    crate::commands::orchestrator::emit_secretary_proposed(
                        &app_handle_clone,
                        crate::commands::orchestrator::SecretaryProposedPayload {
                            item_id,
                            intent: classified.intent,
                            confidence_pct: classified.confidence_pct,
                        },
                    );
                    // Also ping the tasks list so it refreshes immediately.
                    crate::commands::orchestrator::emit_tasks_changed(&app_handle_clone);
                }
                Err(e) => {
                    // Daemon unavailable or rejected — log and move on.
                    tracing::debug!("secretary: failed to submit task: {e}");
                }
            }
        });
    }

    Ok(msg_id)
}

#[tauri::command]
pub async fn chat_rename_session(session_id: String, title: String) -> Result<(), String> {
    let db = gui_db().await?;
    let conv_id = db
        .chat_find_gui_conversation_id(&session_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;
    db.chat_rename_conversation(conv_id, &title)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn chat_archive_session(session_id: String) -> Result<(), String> {
    let db = gui_db().await?;
    let conv_id = db
        .chat_find_gui_conversation_id(&session_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;
    db.chat_archive_conversation(conv_id)
        .await
        .map_err(|e| e.to_string())
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
        let input = ChatAppendInput {
            session_id: "   ".to_string(),
            role: "user".to_string(),
            content: "hi".to_string(),
            task_id: None,
            model_id: None,
        };
        let daemon = app.state::<Arc<PersistentDaemon>>();
        let err = chat_append_message(app.handle().clone(), input, daemon)
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
}
