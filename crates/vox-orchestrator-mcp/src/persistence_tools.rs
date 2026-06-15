//! Orchestrator persistence outbox inspection tools.

use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_CTX: &str = "Run this tool against a live orchestrator session that has context-store persistence health keys.";
const KEY_LIFECYCLE: &str = "orchestrator/persistence_outbox_lifecycle";
const KEY_QUEUE: &str = "orchestrator/persistence_outbox";

fn read_context_json(state: &ServerState, key: &str) -> Result<serde_json::Value, String> {
    let store = state.orchestrator.context_store();
    let raw = vox_orchestrator::sync_lock::rw_read(&*store)
        .get(key)
        .ok_or_else(|| format!("Context key `{key}` is not set."))?;
    serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|e| format!("Context key `{key}` is not valid JSON: {e}"))
}

/// `vox_orchestrator_persistence_outbox_lifecycle`
///
/// Returns the latest lifecycle health snapshot for the persistence outbox replay loop.
pub async fn persistence_outbox_lifecycle(state: &ServerState, _args: serde_json::Value) -> String {
    match read_context_json(state, KEY_LIFECYCLE) {
        Ok(payload) => ToolResult::ok(serde_json::json!({
            "context_key": KEY_LIFECYCLE,
            "lifecycle": payload,
        }))
        .to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(e, REM_CTX).to_json(),
    }
}

/// `vox_orchestrator_persistence_outbox_queue`
///
/// Returns queued persistence outbox entries with optional lane filtering and replay payload redaction.
pub async fn persistence_outbox_queue(state: &ServerState, args: serde_json::Value) -> String {
    let lane_filter = args
        .get("lane")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100)
        .clamp(1, 1000) as usize;
    let include_replay = args
        .get("include_replay")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let queue = match read_context_json(state, KEY_QUEUE) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_CTX).to_json();
        }
    };
    let Some(entries) = queue.as_array() else {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("Context key `{KEY_QUEUE}` is not a JSON array."),
            REM_CTX,
        )
        .to_json();
    };

    let mut filtered: Vec<serde_json::Value> = entries
        .iter()
        .filter(|entry| {
            if let Some(ref lane) = lane_filter {
                entry
                    .get("lane")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .is_some_and(|s| s == lane)
            } else {
                true
            }
        })
        .cloned()
        .collect();
    if !include_replay {
        for entry in &mut filtered {
            if let Some(obj) = entry.as_object_mut() {
                obj.remove("replay");
            }
        }
    }

    let total_after_filter = filtered.len();
    let start = total_after_filter.saturating_sub(limit);
    let rows = filtered.into_iter().skip(start).collect::<Vec<_>>();

    ToolResult::ok(serde_json::json!({
        "context_key": KEY_QUEUE,
        "lane_filter": lane_filter,
        "include_replay": include_replay,
        "limit": limit,
        "total_after_filter": total_after_filter,
        "returned": rows.len(),
        "rows": rows,
    }))
    .to_json()
}
