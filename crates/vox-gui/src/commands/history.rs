//! Tauri commands for the history and clip manager.
//! Accesses the `history_entries` table via the `vox_db` history store api.

use tauri::{AppHandle, Emitter};
use vox_db::history_store::{self, HistoryEntry};

async fn open_db() -> Result<vox_db::Codex, String> {
    let config = vox_db::DbConfig::resolve_for_mesh().map_err(|e| e.to_string())?;
    vox_db::Codex::connect(config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn history_list(
    kind: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<HistoryEntry>, String> {
    let db = open_db().await?;
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    let repo_ctx = vox_repository::discover_repository_or_fallback(&cwd);
    let repo_id = repo_ctx.repository_id;

    history_store::list_entries(&db, &repo_id, kind.as_deref(), limit.unwrap_or(100))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn history_add(
    app: AppHandle,
    kind: String,
    text: String,
    source: String,
) -> Result<i64, String> {
    let db = open_db().await?;
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    let repo_ctx = vox_repository::discover_repository_or_fallback(&cwd);
    let repo_id = repo_ctx.repository_id;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let id = history_store::add_entry(&db, &repo_id, &kind, &text, "", now, &source)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("vox://history-changed", ());
    Ok(id)
}

#[tauri::command]
pub async fn history_search(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<HistoryEntry>, String> {
    let db = open_db().await?;
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    let repo_ctx = vox_repository::discover_repository_or_fallback(&cwd);
    let repo_id = repo_ctx.repository_id;

    history_store::search_entries(&db, &repo_id, &query, limit.unwrap_or(100))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn history_pin(app: AppHandle, id: i64, pinned: bool) -> Result<(), String> {
    let db = open_db().await?;
    history_store::pin_entry(&db, id, pinned)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("vox://history-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn history_delete(app: AppHandle, id: i64) -> Result<(), String> {
    let db = open_db().await?;
    history_store::delete_entry(&db, id)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("vox://history-changed", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_open_db_works() {
        // Just verify db connectivity helper doesn't panic
        let _ = open_db().await;
    }
}
