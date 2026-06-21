//! Tauri commands for context-window archival (dedup+compress → cold tier).

use turso::params;

/// Tauri event name emitted when a context window is archived to cold tier.
pub const CONTEXT_ARCHIVED_EVENT: &str = "vox://context-archived";

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize, Clone)]
pub struct ContextArchivedPayload {
    pub window_id: String,
    pub tier: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ContextWindowInfoDto {
    pub window_id: String,
    pub tier: String,
    pub item_count: i64,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn now_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn canonical_db() -> Result<vox_db::VoxDb, String> {
    vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Compress and dedup all items in `window_id`, then mark the window cold.
/// Emits `vox://context-archived` on success.
#[tauri::command]
pub async fn archive_context_window(
    window_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let db = canonical_db().await?;
    vox_db::archive::pipeline::archive_window(&db, &window_id, now_unix_secs())
        .await
        .map_err(|e| e.to_string())?;
    let payload = ContextArchivedPayload {
        window_id: window_id.clone(),
        tier: "cold".into(),
    };
    let _ = app_handle.emit(CONTEXT_ARCHIVED_EVENT, payload);
    Ok(())
}

/// Return tier + item count for the given context window.
#[tauri::command]
pub async fn get_context_window_info(window_id: String) -> Result<ContextWindowInfoDto, String> {
    let db = canonical_db().await?;

    // Resolve tier.
    let mut rows = db
        .conn
        .query(
            "SELECT tier FROM context_windows WHERE id = ?1 LIMIT 1",
            params![window_id.as_str()],
        )
        .await
        .map_err(|e| e.to_string())?;
    let tier = match rows.next().await.map_err(|e| e.to_string())? {
        Some(row) => row.get::<String>(0).map_err(|e| e.to_string())?,
        None => return Err(format!("context window '{window_id}' not found")),
    };

    // Count items.
    let mut count_rows = db
        .conn
        .query(
            "SELECT COUNT(*) FROM context_window_items WHERE window_id = ?1",
            params![window_id.as_str()],
        )
        .await
        .map_err(|e| e.to_string())?;
    let item_count = match count_rows.next().await.map_err(|e| e.to_string())? {
        Some(row) => row.get::<i64>(0).map_err(|e| e.to_string())?,
        None => 0,
    };

    Ok(ContextWindowInfoDto {
        window_id,
        tier,
        item_count,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time check: both DTOs implement Serialize.
    fn _assert_serialize() {
        fn _check<T: serde::Serialize>() {}
        _check::<ContextWindowInfoDto>();
        _check::<ContextArchivedPayload>();
    }

    #[test]
    fn context_archived_event_name() {
        assert_eq!(CONTEXT_ARCHIVED_EVENT, "vox://context-archived");
    }
}
