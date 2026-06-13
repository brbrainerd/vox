//! A2: SCIENTIA research runs dispatched against the single persistent
//! orchestrator daemon's async fire-and-forget executor.
//!
//! `research.run` (see [`vox_foundation::protocol::dei_method::RESEARCH_RUN`])
//! creates the session row, spawns the pipeline in the daemon process, and
//! returns `{session_id, task_id, status: "running"}` immediately. Routing
//! through the persistent [`PersistentDaemon`] (not a one-shot stdio daemon)
//! keeps the long-running pipeline alive after this command returns — a
//! one-shot daemon would be torn down mid-flight.

use std::sync::Arc;

use serde_json::{Value, json};
use vox_foundation::protocol::dei_method;
use vox_orchestrator::orch_daemon::OrchDaemonClient;

use crate::commands::daemon::PersistentDaemon;

/// Start a research run asynchronously and return the daemon's
/// `{session_id, task_id, status: "running"}` envelope without waiting for the
/// pipeline to finish. Status transitions are observed by the GUI via the
/// Scientia-queue watcher + session-detail polling.
#[tauri::command]
pub async fn start_research_async(
    daemon: tauri::State<'_, Arc<PersistentDaemon>>,
    query: String,
    scope: Option<String>,
    max_sources: Option<u32>,
    verify_claims: Option<bool>,
) -> Result<Value, String> {
    let addr = daemon.ensure().await.map_err(|e| e.to_string())?;
    OrchDaemonClient::new(addr)
        .call(
            dei_method::RESEARCH_RUN,
            json!({
                "query": query,
                "scope": scope,
                "max_sources": max_sources,
                "verify_claims": verify_claims,
            }),
        )
        .await
        .map_err(|e| e.to_string())
}
