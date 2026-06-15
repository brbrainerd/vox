use crate::params::ToolResult;
use crate::server_state::ServerState;

pub async fn exec_time_query(state: &ServerState, args: serde_json::Value) -> String {
    let tool_key = args.get("tool_key").and_then(|v| v.as_str()).unwrap_or("");
    let repository_id = args
        .get("repository_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&state.repository.repository_id);
    let window_days = args
        .get("window_days")
        .and_then(|v| v.as_u64())
        .unwrap_or(30) as u32;

    let Some(ref db) = state.db else {
        return ToolResult::<serde_json::Value>::err("Database not attached").to_json();
    };
    match db
        .query_tool_latency(tool_key, repository_id, window_days, 2.0)
        .await
    {
        Ok(Some(profile)) => {
            ToolResult::ok(serde_json::to_value(&profile).unwrap_or_default()).to_json()
        }
        Ok(None) => ToolResult::ok(serde_json::json!({ "found": false })).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("db_error: {}", e)).to_json(),
    }
}

pub async fn exec_time_record(state: &ServerState, args: serde_json::Value) -> String {
    let tool_key = args.get("tool_key").and_then(|v| v.as_str()).unwrap_or("");
    let repository_id = args
        .get("repository_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&state.repository.repository_id);
    let duration_ms = args
        .get("duration_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let timeout_budget_ms = args.get("timeout_budget_ms").and_then(|v| v.as_u64());

    let outcome_str = args
        .get("outcome")
        .and_then(|v| v.as_str())
        .unwrap_or("success");
    let outcome = match outcome_str {
        "timeout" => vox_db::ExecOutcome::Timeout,
        "error" => vox_db::ExecOutcome::Error,
        _ => vox_db::ExecOutcome::Success,
    };

    let Some(ref db) = state.db else {
        return ToolResult::<serde_json::Value>::err("Database not attached").to_json();
    };

    let record = vox_db::ExecTimeRecord {
        tool_key,
        repository_id,
        duration_ms,
        timeout_budget_ms,
        compute_tokens_used: None,
        vendor_cost_usd_micros: None,
        attention_cost_ms: None,
        outcome,
    };

    match db.record_exec_time(&record).await {
        Ok(()) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("db_error: {}", e)).to_json(),
    }
}
