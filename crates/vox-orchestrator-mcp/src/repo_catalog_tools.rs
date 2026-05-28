//! Explicit multi-repo catalog and read-only polyrepo query tools.
//!
//! Cross-repo queries may append a **`benchmark_event`** row via `record_query_metric` (latency + trace metadata) when Codex
//! is attached — **no extra env gate** beyond having `VoxDb`. Mesh snapshot Codex mirroring is separate (see `dei_tools::orchestrator_snapshot`).

use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_actor_runtime::supervisor::spawn_supervised_infallible;
use vox_db::TrustObservationInput;

const REM_REPO_CATALOG: &str = "Add `.vox/repositories.yaml` under the current workspace root and keep local repo paths explicit.";
const REM_REPO_QUERY: &str = "Ensure the repo catalog resolves local repositories successfully before running cross-repo queries.";

pub async fn repo_status(state: &ServerState) -> String {
    let status = vox_repository::repo_workspace_status_for_cwd(&state.repository.root);
    ToolResult::ok(status).to_json()
}

/// Fire-and-forget `research_metrics` benchmark row for polyrepo queries (trace ids, backend, counts).
fn record_query_metric(
    state: &ServerState,
    query_kind: &str,
    trace: &vox_repository::CrossRepoQueryTrace,
    result_count: usize,
    skipped_count: usize,
) {
    let Some(db) = state.db.as_ref().cloned() else {
        return;
    };
    let repository_id = state.repository.repository_id.clone();
    let latency_ms = trace.latency_ms;
    let details = serde_json::json!({
        "query_kind": query_kind,
        "trace_id": trace.trace_id,
        "correlation_id": trace.correlation_id,
        "conversation_id": trace.conversation_id,
        "workspace_repository_id": trace.workspace_repository_id,
        "target_repository_ids": trace.target_repository_ids,
        "source_plane": trace.source_plane,
        "query_backend": trace.query_backend,
        "result_count": result_count,
        "skipped_count": skipped_count,
    });
    spawn_supervised_infallible("repo_catalog_metric", async move {
        let _ = db
            .record_benchmark_event(
                &repository_id,
                "cross_repo_query",
                Some(latency_ms as f64),
                Some("milliseconds"),
                Some(details),
            )
            .await;
    });
}

fn record_catalog_refresh_observation(
    state: &ServerState,
    success: bool,
    artifact_ref: Option<&str>,
    metadata_json: serde_json::Value,
) {
    let Some(db) = state.db.as_ref().cloned() else {
        return;
    };
    let repository_id = state.repository.repository_id.clone();
    let artifact_ref = artifact_ref.map(str::to_string);
    let metadata_text = metadata_json.to_string();
    spawn_supervised_infallible("repo_catalog_metric", async move {
        let _ = db
            .record_trust_observation(TrustObservationInput {
                entity_type: "repository",
                entity_id: &repository_id,
                dimension: "repo_catalog_freshness",
                domain: Some("cross_repo_query"),
                task_class: None,
                provider: None,
                model_id: None,
                repository_id: Some(&repository_id),
                source_kind: Some("vox_repo_catalog_refresh"),
                observation_value: if success { 1.0 } else { 0.0 },
                confidence_weight: 1.0,
                sample_size: 1,
                artifact_ref: artifact_ref.as_deref(),
                metadata_json: Some(&metadata_text),
                ewma_alpha: 0.10,
            })
            .await;
    });
}

async fn get_catalog(
    state: &ServerState,
) -> Result<vox_repository::ResolvedRepoCatalog, vox_repository::RepoCatalogError> {
    let manifest_path = vox_repository::repo_catalog_manifest_path(&state.repository.root);
    let mtime = std::fs::metadata(&manifest_path)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    {
        let cache = state.catalog_cache.read().await;
        if let Some(cached) = &*cache {
            if cached.manifest_mtime == mtime {
                return Ok(cached.resolved.clone());
            }
        }
    }

    let resolved = vox_repository::resolve_repo_catalog(&state.repository.root)?;
    let mut cache = state.catalog_cache.write().await;
    *cache = Some(crate::server_state::CachedCatalog {
        resolved: resolved.clone(),
        manifest_mtime: mtime,
    });

    Ok(resolved)
}

pub async fn repo_catalog_list(state: &ServerState) -> String {
    match get_catalog(state).await {
        Ok(catalog) => ToolResult::ok(catalog).to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_REPO_CATALOG).to_json()
        }
    }
}

pub async fn repo_catalog_refresh(state: &ServerState) -> String {
    match vox_repository::refresh_repo_catalog(&state.repository.root) {
        Ok(result) => {
            record_catalog_refresh_observation(
                state,
                true,
                Some(&result.snapshot_path),
                serde_json::json!({
                    "manifest_path": result.manifest_path,
                    "snapshot_path": result.snapshot_path,
                    "repository_count": result.catalog.repositories.len(),
                }),
            );
            // Invalidate the cache by removing it
            *state.catalog_cache.write().await = None;
            ToolResult::ok(result).to_json()
        }
        Err(e) => {
            record_catalog_refresh_observation(
                state,
                false,
                None,
                serde_json::json!({
                    "error": e.to_string(),
                }),
            );
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_REPO_CATALOG).to_json()
        }
    }
}

pub async fn repo_query_text(
    state: &ServerState,
    params: vox_repository::QueryTextParams,
) -> String {
    let catalog = get_catalog(state).await.ok();
    match vox_repository::repo_query_text_with_plane(
        &state.repository.root,
        &params,
        "mcp",
        catalog.as_ref(),
    ) {
        Ok(response) => {
            record_query_metric(
                state,
                "query_text",
                &response.trace,
                response.result_count,
                response.skipped.len(),
            );
            ToolResult::ok(response).to_json()
        }
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_REPO_QUERY).to_json()
        }
    }
}

pub async fn repo_query_file(
    state: &ServerState,
    params: vox_repository::QueryFileParams,
) -> String {
    let catalog = get_catalog(state).await.ok();
    match vox_repository::repo_query_file_with_plane(
        &state.repository.root,
        &params,
        "mcp",
        catalog.as_ref(),
    ) {
        Ok(response) => {
            record_query_metric(
                state,
                "query_file",
                &response.trace,
                response.result_count,
                response.skipped.len(),
            );
            ToolResult::ok(response).to_json()
        }
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_REPO_QUERY).to_json()
        }
    }
}

pub async fn repo_query_history(
    state: &ServerState,
    params: vox_repository::QueryHistoryParams,
) -> String {
    let catalog = get_catalog(state).await.ok();
    match vox_repository::repo_query_history_with_plane(
        &state.repository.root,
        &params,
        "mcp",
        catalog.as_ref(),
    ) {
        Ok(response) => {
            record_query_metric(
                state,
                "query_history",
                &response.trace,
                response.result_count,
                response.skipped.len(),
            );
            ToolResult::ok(response).to_json()
        }
        Err(e) => {
            ToolResult::<String>::err_with_remediation(format!("{e}"), REM_REPO_QUERY).to_json()
        }
    }
}
