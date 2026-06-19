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
    let cwd = std::env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    let repo_ctx = vox_repository::discover_repository_or_fallback(&cwd);
    let repo_id = repo_ctx.repository_id;

    history_store::list_entries(
        &db,
        &repo_id,
        kind.as_deref(),
        limit.unwrap_or(100),
    )
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
    let cwd = std::env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    let repo_ctx = vox_repository::discover_repository_or_fallback(&cwd);
    let repo_id = repo_ctx.repository_id;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let id = history_store::add_entry(
        &db,
        &repo_id,
        &kind,
        &text,
        "",
        now,
        &source,
    )
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
    let cwd = std::env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    let repo_ctx = vox_repository::discover_repository_or_fallback(&cwd);
    let repo_id = repo_ctx.repository_id;

    let q = format!("%{}%", query);
    let limit_val = limit.unwrap_or(100) as i64;

    let mut rows = db
        .connection()
        .query(
            "SELECT id, repo_id, kind, text, redacted_text, created_at, pinned, source, token_estimate
             FROM history_entries
             WHERE repo_id = ?1 AND (text LIKE ?2 OR redacted_text LIKE ?2)
             ORDER BY created_at DESC, id DESC
             LIMIT ?3",
            turso::params![repo_id.as_str(), q.as_str(), limit_val],
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        let id: i64 = row.get(0).map_err(|e| e.to_string())?;
        let repo_id: String = row.get(1).map_err(|e| e.to_string())?;
        let kind: String = row.get(2).map_err(|e| e.to_string())?;
        let text: String = row.get(3).map_err(|e| e.to_string())?;
        let redacted_text: String = row.get(4).map_err(|e| e.to_string())?;
        let created_at: i64 = row.get(5).map_err(|e| e.to_string())?;
        let pinned_val: i64 = row.get(6).map_err(|e| e.to_string())?;
        let source: Option<String> = row.get(7).map_err(|e| e.to_string())?;
        let token_estimate: i64 = row.get(8).map_err(|e| e.to_string())?;

        entries.push(HistoryEntry {
            id,
            repo_id,
            kind,
            text,
            redacted_text,
            created_at,
            pinned: pinned_val == 1,
            source,
            token_estimate,
        });
    }
    Ok(entries)
}

#[tauri::command]
pub async fn history_pin(
    app: AppHandle,
    id: i64,
    pinned: bool,
) -> Result<(), String> {
    let db = open_db().await?;
    history_store::pin_entry(&db, id, pinned)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("vox://history-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn history_delete(
    app: AppHandle,
    id: i64,
) -> Result<(), String> {
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
