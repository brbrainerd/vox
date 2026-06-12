//! GUI chat session persistence via `conversations` / `conversation_messages`.

use serde::{Deserialize, Serialize};
use vox_db::DbConnectSurface;
use vox_db::connect_workspace_journey_optional;

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
            let task_id = payload.and_then(|p| {
                serde_json::from_str::<serde_json::Value>(&p)
                    .ok()
                    .and_then(|v| {
                        v.get("task_id")
                            .and_then(|t| t.as_str())
                            .map(str::to_string)
                    })
            });
            ChatMessageDto {
                id,
                role,
                content,
                created_at,
                task_id,
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
}

#[tauri::command]
pub async fn chat_append_message(input: ChatAppendInput) -> Result<i64, String> {
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
    let payload = input
        .task_id
        .map(|t| serde_json::json!({ "task_id": t }).to_string());
    db.chat_append_message(conv_id, &input.role, &input.content, payload.as_deref())
        .await
        .map_err(|e| e.to_string())
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
        let input = ChatAppendInput {
            session_id: "   ".to_string(),
            role: "user".to_string(),
            content: "hi".to_string(),
            task_id: None,
        };
        let err = chat_append_message(input).await.expect_err("empty session");
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
}
