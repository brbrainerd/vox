//! Trust rollup inspection tools over connected `VoxDb`.

use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_VOXDB_TRUST: &str = "Attach VoxDb (Turso) to the MCP server via `VOX_DB_PATH` / `VOX_DB_URL` for trust rollup tools.";
const REM_TRUST_ARGS: &str = "Use optional string filters (`entity_type`, `dimension`, `domain`, `repository_id`) and numeric bounds.";
const REM_TRUST_SUMMARY_ARGS: &str = "Set `group_by` to dimension|domain|entity_type|dimension_domain|entity_dimension; optional `limit_groups` 1..500.";
const REM_TRUST_DRIFT_ARGS: &str =
    "Optional `entity_type`, `dimension`, `window_ms` (60s–30d, default 86400000).";
const REM_TRUST_PROPAGATE_ARGS: &str = "`dimension` required; optional `damping` 0–1, `iterations` 1–256, `persist` to write *_propagated observations; set `repository_id_default_workspace` true to scope rollups.";

fn require_db(state: &ServerState) -> Result<&std::sync::Arc<vox_db::VoxDb>, String> {
    state
        .db
        .as_ref()
        .ok_or_else(|| "VoxDb is not connected (trust tools need a Turso-backed DB).".to_string())
}

/// `vox_db_trust_rollups`
///
/// Lists trust rollups with optional scope filters.
pub async fn trust_rollups_list(state: &ServerState, args: serde_json::Value) -> String {
    let db = match require_db(state) {
        Ok(d) => d,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_VOXDB_TRUST)
                .to_json();
        }
    };

    let entity_type = args
        .get("entity_type")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let dimension = args
        .get("dimension")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let domain = args
        .get("domain")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let mut repository_id = args
        .get("repository_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if repository_id.is_none()
        && args
            .get("repository_id_default_workspace")
            .and_then(|v| v.as_bool())
            == Some(true)
    {
        repository_id = Some(state.repository.repository_id.as_str());
    }

    let raw_limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(200);
    if raw_limit <= 0 {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "Invalid `limit`: expected positive integer.",
            REM_TRUST_ARGS,
        )
        .to_json();
    }
    let limit = raw_limit.clamp(1, 10_000);

    match db
        .list_trust_rollups(entity_type, dimension, domain, repository_id, limit)
        .await
    {
        Ok(rows) => {
            let payload_rows = rows
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "entity_type": r.entity_type,
                        "entity_id": r.entity_id,
                        "dimension": r.dimension,
                        "domain": r.domain,
                        "task_class": r.task_class,
                        "provider": r.provider,
                        "model_id": r.model_id,
                        "repository_id": r.repository_id,
                        "score": r.score,
                        "sample_size": r.sample_size,
                        "ewma_alpha": r.ewma_alpha,
                        "updated_at_ms": r.updated_at_ms,
                    })
                })
                .collect::<Vec<_>>();
            ToolResult::ok(serde_json::json!({
                "count": payload_rows.len(),
                "filters": {
                    "entity_type": entity_type,
                    "dimension": dimension,
                    "domain": domain,
                    "repository_id": repository_id,
                    "limit": limit,
                },
                "rows": payload_rows,
            }))
            .to_json()
        }
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
            format!("Failed to list trust rollups: {e}"),
            REM_VOXDB_TRUST,
        )
        .to_json(),
    }
}

/// `vox_db_trust_summary` — grouped aggregates over `trust_rollups`.
pub async fn trust_rollups_summary(state: &ServerState, args: serde_json::Value) -> String {
    let db = match require_db(state) {
        Ok(d) => d,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_VOXDB_TRUST)
                .to_json();
        }
    };

    let entity_type = args
        .get("entity_type")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let dimension = args
        .get("dimension")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let domain = args
        .get("domain")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let mut repository_id = args
        .get("repository_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if repository_id.is_none()
        && args
            .get("repository_id_default_workspace")
            .and_then(|v| v.as_bool())
            == Some(true)
    {
        repository_id = Some(state.repository.repository_id.as_str());
    }

    let group_by = args
        .get("group_by")
        .and_then(|v| v.as_str())
        .unwrap_or("dimension")
        .trim();
    if group_by.is_empty() {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "Missing or empty `group_by`.",
            REM_TRUST_SUMMARY_ARGS,
        )
        .to_json();
    }

    let raw_limit = args
        .get("limit_groups")
        .and_then(|v| v.as_i64())
        .unwrap_or(50);
    if raw_limit <= 0 {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "Invalid `limit_groups`: expected positive integer.",
            REM_TRUST_SUMMARY_ARGS,
        )
        .to_json();
    }
    let limit_groups = raw_limit.clamp(1, 500);

    match db
        .summarize_trust_rollups(
            entity_type,
            dimension,
            domain,
            repository_id,
            group_by,
            limit_groups,
        )
        .await
    {
        Ok(rows) => {
            let items = rows
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "entity_type": r.entity_type,
                        "dimension": r.dimension,
                        "domain": r.domain,
                        "rollup_count": r.rollup_count,
                        "mean_score": r.mean_score,
                        "min_score": r.min_score,
                        "max_score": r.max_score,
                        "sum_sample_size": r.sum_sample_size,
                        "max_updated_at_ms": r.max_updated_at_ms,
                    })
                })
                .collect::<Vec<_>>();
            ToolResult::ok(serde_json::json!({
                "group_by": group_by,
                "group_count": items.len(),
                "filters": {
                    "entity_type": entity_type,
                    "dimension": dimension,
                    "domain": domain,
                    "repository_id": repository_id,
                },
                "limit_groups": limit_groups,
                "groups": items,
            }))
            .to_json()
        }
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
            e.to_string(),
            REM_TRUST_SUMMARY_ARGS,
        )
        .to_json(),
    }
}

/// `vox_db_trust_drift` — recent vs prior window mean on `trust_observations`.
pub async fn trust_observation_drift(state: &ServerState, args: serde_json::Value) -> String {
    let db = match require_db(state) {
        Ok(d) => d,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_VOXDB_TRUST)
                .to_json();
        }
    };
    let entity_type = args
        .get("entity_type")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let dimension = args
        .get("dimension")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let window_ms = args
        .get("window_ms")
        .and_then(|v| v.as_i64())
        .unwrap_or(86_400_000);
    match db
        .trust_observation_drift_two_window(entity_type, dimension, window_ms)
        .await
    {
        Ok(rep) => {
            let mut payload = serde_json::json!({
                "drift": serde_json::to_value(&rep).unwrap_or_default(),
            });
            let include_raw = args
                .get("include_raw_observations")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let include_lineage = args
                .get("include_lineage_for_task")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let task_id = args.get("task_id").and_then(|v| v.as_i64());
            let mut repository_id = args
                .get("repository_id")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            if repository_id.is_none()
                && args
                    .get("repository_id_default_workspace")
                    .and_then(|v| v.as_bool())
                    == Some(true)
            {
                repository_id = Some(state.repository.repository_id.as_str());
            }
            if include_raw {
                let raw_limit = args
                    .get("raw_limit")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(200)
                    .clamp(1, 10_000);
                let since_ms = args.get("since_ms").and_then(|v| v.as_i64());
                let artifact_ref = task_id.map(|id| id.to_string());
                match db
                    .list_trust_observations(
                        entity_type,
                        dimension,
                        None,
                        repository_id,
                        artifact_ref.as_deref(),
                        since_ms,
                        raw_limit,
                    )
                    .await
                {
                    Ok(rows) => {
                        payload["raw_observations"] =
                            serde_json::to_value(rows).unwrap_or_default();
                    }
                    Err(e) => {
                        payload["raw_observations_error"] =
                            serde_json::Value::String(e.to_string());
                    }
                }
            }
            if include_lineage && let (Some(repo), Some(task_id)) = (repository_id, task_id) {
                let lineage_limit = args
                    .get("lineage_limit")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(200)
                    .clamp(1, 500);
                match db
                    .list_orchestration_lineage_for_task(repo, task_id, lineage_limit)
                    .await
                {
                    Ok(rows) => {
                        let items: Vec<serde_json::Value> = rows
                            .into_iter()
                            .map(|(id, kind, created_at_ms)| {
                                serde_json::json!({
                                    "id": id,
                                    "kind": kind,
                                    "created_at_ms": created_at_ms,
                                })
                            })
                            .collect();
                        payload["task_lineage"] = serde_json::Value::Array(items);
                    }
                    Err(e) => {
                        payload["task_lineage_error"] = serde_json::Value::String(e.to_string());
                    }
                }
            }
            ToolResult::ok(payload).to_json()
        }
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
            e.to_string(),
            REM_TRUST_DRIFT_ARGS,
        )
        .to_json(),
    }
}

/// `vox_db_trust_propagate` — domain-clique propagated scores for model rollups.
pub async fn trust_propagate(state: &ServerState, args: serde_json::Value) -> String {
    let db = match require_db(state) {
        Ok(d) => d,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_VOXDB_TRUST)
                .to_json();
        }
    };
    let dimension = args
        .get("dimension")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let Some(dimension) = dimension else {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "Missing non-empty `dimension` (base rollup dimension, e.g. factuality).",
            REM_TRUST_PROPAGATE_ARGS,
        )
        .to_json();
    };
    let explicit_repo = args
        .get("repository_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let use_workspace = args
        .get("repository_id_default_workspace")
        .and_then(|v| v.as_bool())
        == Some(true);
    let repository_id = if use_workspace {
        state.repository.repository_id.as_str()
    } else if let Some(r) = explicit_repo {
        r
    } else {
        state.repository.repository_id.as_str()
    };
    let damping = args.get("damping").and_then(|v| v.as_f64()).unwrap_or(0.82);
    let iterations = args
        .get("iterations")
        .and_then(|v| v.as_u64())
        .unwrap_or(24) as u32;
    let persist = args
        .get("persist")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    match db
        .trust_propagate_model_rollups(repository_id, dimension, damping, iterations, persist)
        .await
    {
        Ok(rows) => ToolResult::ok(serde_json::json!({
            "repository_id": repository_id,
            "base_dimension": dimension,
            "damping": damping,
            "iterations": iterations,
            "persist": persist,
            "count": rows.len(),
            "scores": rows,
        }))
        .to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
            e.to_string(),
            REM_TRUST_PROPAGATE_ARGS,
        )
        .to_json(),
    }
}
