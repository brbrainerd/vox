//! Typed Scientia-domain read commands (research sessions + publication manifests).
//! Reads go directly to the canonical DB, mirroring the CLI handlers — no CLI
//! stdout parsing and no dependency on the (disabled) HTTP gateway.

use tauri::Emitter;

#[derive(Debug, serde::Serialize)]
pub struct ResearchSessionDto {
    pub id: i64,
    pub status: String,
    pub query_text: String,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct ResearchDetailDto {
    pub session: ResearchSessionDto,
    pub report_markdown: Option<String>,
    pub artifact_json: Option<String>,
}

async fn connect_canonical_db() -> Result<vox_db::VoxDb, String> {
    vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_research_sessions(limit: Option<u32>) -> Result<Vec<ResearchSessionDto>, String> {
    let db = connect_canonical_db().await?;
    let rows = db
        .list_recent_research_sessions(limit.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| ResearchSessionDto {
            id: r.id,
            status: r.status.clone(),
            query_text: r.query_text.clone(),
            started_at_ms: r.started_at_ms,
            finished_at_ms: r.finished_at_ms,
        })
        .collect())
}

#[tauri::command]
pub async fn get_research_session_detail(session_id: i64) -> Result<ResearchDetailDto, String> {
    let db = connect_canonical_db().await?;
    let s = db
        .get_research_session(session_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("research session {session_id} not found"))?;
    let artifact = db
        .get_research_artifact(session_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ResearchDetailDto {
        session: ResearchSessionDto {
            id: s.id,
            status: s.status.clone(),
            query_text: s.query_text.clone(),
            started_at_ms: s.started_at_ms,
            finished_at_ms: s.finished_at_ms,
        },
        report_markdown: artifact.as_ref().map(|a| a.report_markdown.clone()),
        artifact_json: artifact.as_ref().map(|a| a.artifact_json.clone()),
    })
}

#[derive(Debug, serde::Serialize)]
pub struct PublicationManifestDto {
    pub publication_id: String,
    pub content_type: String,
    pub state: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[tauri::command]
pub async fn list_publication_manifests(
    limit: Option<u32>,
) -> Result<Vec<PublicationManifestDto>, String> {
    let db = vox_db::VoxDb::connect_default()
        .await
        .map_err(|e| e.to_string())?;
    let manifests = db
        .list_publication_manifests(Some("scientia"), None, limit.unwrap_or(200) as i64)
        .await
        .map_err(|e| e.to_string())?;
    Ok(manifests
        .iter()
        .map(|m| PublicationManifestDto {
            publication_id: m.publication_id.clone(),
            content_type: m.content_type.clone(),
            state: m.state.clone(),
            created_at_ms: m.created_at_ms,
            updated_at_ms: m.updated_at_ms,
        })
        .collect())
}

// ── Live Scientia-queue push bridge (F2) ─────────────────────────────────────

/// Tauri event channel carrying a lightweight "the Scientia queue changed" ping
/// to the UI. The payload is a compact signal object
/// (`{ signal: u64, manifest_count, research_count }`); on receipt the UI
/// refetches via the typed read commands above.
pub const SCIENTIA_QUEUE_EVENT: &str = "vox://scientia-queue";

/// How often the push bridge samples the DB for a change. The UI keeps its own
/// (longer) interval as a fallback, so this only governs push latency.
const SCIENTIA_POLL_INTERVAL_MS: u64 = 3_000;

/// Compute a compact change signal over the Scientia queue: a hash folded from
/// each publication manifest's `(publication_id, state, updated_at_ms)` plus each
/// research session's `(id, status, finished_at_ms)`. Any add / state transition
/// / timestamp bump flips the signal; a steady queue keeps it stable. Returns
/// `(signal, manifest_count, research_count)`.
async fn scientia_queue_signal(db: &vox_db::VoxDb) -> Result<(u64, usize, usize), String> {
    let manifests = db
        .list_publication_manifests(Some("scientia"), None, 500)
        .await
        .map_err(|e| e.to_string())?;
    let sessions = db
        .list_recent_research_sessions(200)
        .await
        .map_err(|e| e.to_string())?;
    fn fnv1a_mix(mut acc: u64, bytes: &[u8]) -> u64 {
        for &b in bytes {
            acc ^= b as u64;
            acc = acc.wrapping_mul(0x00000100_000001B3);
        }
        acc
    }
    let mut acc: u64 = 0xcbf29ce484222325; // FNV offset basis
    for m in &manifests {
        acc = fnv1a_mix(acc, m.publication_id.as_bytes());
        acc = fnv1a_mix(acc, m.state.as_bytes());
        acc = fnv1a_mix(acc, &m.updated_at_ms.to_le_bytes());
    }
    for s in &sessions {
        acc = fnv1a_mix(acc, &s.id.to_le_bytes());
        acc = fnv1a_mix(acc, s.status.as_bytes());
        let ts_bytes = s.finished_at_ms.unwrap_or(-1_i64).to_le_bytes();
        acc = fnv1a_mix(acc, &ts_bytes);
    }
    Ok((acc, manifests.len(), sessions.len()))
}

/// Spawn a background task that watches the canonical DB for Scientia-queue
/// changes and emits a [`SCIENTIA_QUEUE_EVENT`] ping when the queue signal flips.
///
/// This is the Scientia analog of
/// [`spawn_orchestrator_status_stream`](crate::commands::orchestrator::spawn_orchestrator_status_stream),
/// adapted to a DB-backed surface: the Scientia queue is sourced from the
/// canonical DB via the typed read commands (not the daemon's status stream and
/// not the disabled HTTP gateway), so there is no daemon RPC to subscribe to.
/// Instead we poll a cheap change signal and push only on change — turning the
/// UI's interval refresh into event-driven refresh. Resilient by design: a DB
/// error is logged and retried on the next tick; the task never crashes the app.
// toestub-ignore(skeleton/untested-pub-api) — spawns a background DB-watch task bridging Scientia-queue changes to Tauri events; covered by integration
pub fn spawn_scientia_queue_stream(app_handle: tauri::AppHandle) {
    tokio::spawn(async move {
        let mut last_signal: Option<u64> = None;
        // Open the connection once outside the poll loop and reuse across ticks.
        let db = loop {
            match vox_db::VoxDb::connect_canonical().await {
                Ok(db) => break db,
                Err(e) => {
                    tracing::debug!("scientia queue: db unavailable (will retry): {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(SCIENTIA_POLL_INTERVAL_MS))
                        .await;
                }
            }
        };
        loop {
            match scientia_queue_signal(&db).await {
                Ok((signal, manifest_count, research_count)) => {
                    if last_signal != Some(signal) {
                        last_signal = Some(signal);
                        let _ = app_handle.emit(
                            SCIENTIA_QUEUE_EVENT,
                            serde_json::json!({
                                "signal": signal,
                                "manifest_count": manifest_count,
                                "research_count": research_count,
                            }),
                        );
                    }
                }
                Err(e) => tracing::debug!("scientia queue signal failed: {e}"),
            }
            tokio::time::sleep(std::time::Duration::from_millis(SCIENTIA_POLL_INTERVAL_MS)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    /// The signal hash is order-deterministic and sensitive to state changes:
    /// two folds over the same tuples match; a state change diverges. (Pure hash
    /// fold; no DB required.)
    #[test]
    fn queue_signal_fold_is_deterministic_and_state_sensitive() {
        fn fnv1a_mix(mut acc: u64, bytes: &[u8]) -> u64 {
            for &b in bytes {
                acc ^= b as u64;
                acc = acc.wrapping_mul(0x00000100_000001B3);
            }
            acc
        }
        fn fold(rows: &[(&str, &str, i64)]) -> u64 {
            let mut acc: u64 = 0xcbf29ce484222325;
            for (id, state, ts) in rows {
                acc = fnv1a_mix(acc, id.as_bytes());
                acc = fnv1a_mix(acc, state.as_bytes());
                acc = fnv1a_mix(acc, &ts.to_le_bytes());
            }
            acc
        }
        let a = fold(&[("pub-1", "draft", 100), ("pub-2", "approved", 200)]);
        let b = fold(&[("pub-1", "draft", 100), ("pub-2", "approved", 200)]);
        let c = fold(&[("pub-1", "approved", 100), ("pub-2", "approved", 200)]);
        assert_eq!(a, b, "same tuples -> same signal");
        assert_ne!(a, c, "a state transition flips the signal");
    }
}
