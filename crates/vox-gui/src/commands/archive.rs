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
        window_id,
        tier: "cold".into(),
    };
    if let Err(e) = app_handle.emit(CONTEXT_ARCHIVED_EVENT, payload) {
        tracing::warn!("failed to emit {CONTEXT_ARCHIVED_EVENT}: {e}");
    }
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

// ── Background archive worker ─────────────────────────────────────────────────

const ARCHIVE_POLL_INTERVAL_MS: u64 = 5_000;

pub fn spawn_archive_worker(app_handle: tauri::AppHandle) {
    tokio::spawn(async move {
        let db = loop {
            match vox_db::VoxDb::connect_canonical().await {
                Ok(db) => break db,
                Err(e) => {
                    tracing::debug!("archive worker: db unavailable (will retry): {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(ARCHIVE_POLL_INTERVAL_MS))
                        .await;
                }
            }
        };
        loop {
            if let Err(e) = run_archive_worker_tick(&db, &app_handle).await {
                tracing::warn!("archive worker tick failed: {e}");
            }
            tokio::time::sleep(std::time::Duration::from_millis(ARCHIVE_POLL_INTERVAL_MS)).await;
        }
    });
}

async fn run_archive_worker_tick(
    db: &vox_db::VoxDb,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    use turso::params;
    // Claim one queued archive run.
    let mut rows = db.conn.query(
        "SELECT id, scope_id FROM processing_runs WHERE run_kind = 'archive_context_window' AND status = 'queued' ORDER BY id ASC LIMIT 1",
        (),
    ).await.map_err(|e| e.to_string())?;

    let Some(row) = rows.next().await.map_err(|e| e.to_string())? else {
        return Ok(()); // nothing queued
    };
    let run_id: i64 = row.get(0).map_err(|e| e.to_string())?;
    let window_id: String = row.get(1).map_err(|e| e.to_string())?;

    // Mark running.
    db.conn.execute(
        "UPDATE processing_runs SET status = 'running', started_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
        params![run_id],
    ).await.map_err(|e| e.to_string())?;

    // Run the archive pipeline.
    let result = vox_db::archive::pipeline::archive_window(db, &window_id, now_unix_secs()).await;

    match result {
        Ok(()) => {
            db.conn.execute(
                "UPDATE processing_runs SET status = 'completed', completed_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
                params![run_id],
            ).await.map_err(|e| e.to_string())?;
            let payload = ContextArchivedPayload {
                window_id,
                tier: "cold".into(),
            };
            if let Err(e) = app_handle.emit(CONTEXT_ARCHIVED_EVENT, payload) {
                tracing::warn!("archive worker: failed to emit event: {e}");
            }
        }
        Err(e) => {
            let err_str = e.to_string();
            db.conn.execute(
                "UPDATE processing_runs SET status = 'failed', error_text = ?2, updated_at = datetime('now') WHERE id = ?1",
                params![run_id, err_str.as_str()],
            ).await.map_err(|e| e.to_string())?;
            return Err(format!("archive_window failed for {window_id}: {err_str}"));
        }
    }
    Ok(())
}
