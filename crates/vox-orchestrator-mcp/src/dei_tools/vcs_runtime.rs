use super::params::{HeartbeatParams, PollEventsParams, RecordCostParams, SubmitTaskParams};
use crate::params::ToolResult;
use crate::server_state::ServerState;
use std::path::PathBuf;

const REM_VCS_SUBMIT: &str =
    "Check orchestrator health, task description validity, and agent/session mapping.";
const REM_SESSION_AGENT_MAP: &str =
    "Call `map_agent_session` or list orchestrator agents so `session_id` maps to a live agent.";

/// Check which agent owns a given file path (async).
pub async fn check_file_owner(state: &ServerState, path: &str) -> String {
    let orch = &state.orchestrator;

    let affinity_map = orch.affinity_map();
    match affinity_map.lookup(&PathBuf::from(path)) {
        Some(agent_id) => ToolResult::ok(format!("owned by agent {agent_id}")).to_json(),
        None => ToolResult::ok("no owner assigned".to_string()).to_json(),
    }
}

/// Unified VCS status: snapshots, oplog, conflicts, workspaces, and changes.
pub async fn vcs_status(state: &ServerState) -> String {
    let orch = &state.orchestrator;

    let snapshot_count =
        vox_orchestrator::sync_lock::rw_read(&*orch.snapshot_store_handle()).count();
    let oplog_count = vox_orchestrator::sync_lock::rw_read(&*orch.oplog_handle()).count();
    let active_conflicts =
        vox_orchestrator::sync_lock::rw_read(&*orch.conflict_manager_handle()).active_count();
    let total_conflicts =
        vox_orchestrator::sync_lock::rw_read(&*orch.conflict_manager_handle()).total_count();
    let active_workspaces = vox_orchestrator::sync_lock::rw_read(&*orch.workspace_manager_handle())
        .list_workspaces()
        .len();
    let active_changes = vox_orchestrator::sync_lock::rw_read(&*orch.workspace_manager_handle())
        .list_changes(None, usize::MAX)
        .len();

    // Build workspace details
    let workspace_details: Vec<serde_json::Value> =
        vox_orchestrator::sync_lock::rw_read(&*orch.workspace_manager_handle())
            .list_workspaces()
            .iter()
            .map(|ws| {
                serde_json::json!({
                    "agent_id": ws.agent_id.0,
                    "base_snapshot": ws.base_snapshot.0,
                    "modified_files": ws.modified_count(),
                    "active_change": ws.active_change.map(|c| c.0),
                })
            })
            .collect();

    // Build recent oplog entries (last 10)
    let recent_ops: Vec<serde_json::Value> =
        vox_orchestrator::sync_lock::rw_read(&*orch.oplog_handle())
            .list(None, 10)
            .iter()
            .map(|op| {
                serde_json::json!({
                    "id": op.id.to_string(),
                    "agent_id": op.agent_id.0,
                    "kind": format!("{:?}", op.kind),
                    "description": op.description,
                    "undone": op.undone,
                })
            })
            .collect();

    // Build active conflict details
    let conflict_details: Vec<serde_json::Value> =
        vox_orchestrator::sync_lock::rw_read(&*orch.conflict_manager_handle())
            .active_conflicts()
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id.to_string(),
                    "path": c.path.display().to_string(),
                    "sides": c.sides.len(),
                    "created_ms": c.created_ms,
                })
            })
            .collect();

    let result = serde_json::json!({
        "snapshots": snapshot_count,
        "oplog_entries": oplog_count,
        "recent_operations": recent_ops,
        "conflicts": {
            "active": active_conflicts,
            "total": total_conflicts,
            "details": conflict_details,
        },
        "workspaces": {
            "active": active_workspaces,
            "details": workspace_details,
        },
        "changes": active_changes,
    });

    ToolResult::ok(result).to_json()
}

fn agent_id_from_kind_json(v: &serde_json::Value) -> u64 {
    for key in ["agent_id", "from"] {
        if let Some(n) = v.get(key).and_then(|x| x.as_u64()) {
            return n;
        }
    }
    if let Some(arr) = v.get("agent_ids").and_then(|x| x.as_array()) {
        if let Some(first) = arr.first().and_then(|x| x.as_u64()) {
            return first;
        }
    }
    0
}

fn agent_event_to_record(
    ev: &vox_orchestrator::AgentEvent,
    repo_id: &str,
) -> vox_gamify::db::AgentEventRecord {
    let mut kind_json = serde_json::to_value(&ev.kind).unwrap_or_default();
    let event_type = kind_json
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown")
        .to_string();
    let agent_id = agent_id_from_kind_json(&kind_json);
    if let Some(obj) = kind_json.as_object_mut() {
        obj.insert(
            "repository_id".to_string(),
            serde_json::Value::String(repo_id.to_string()),
        );
    }
    let payload = serde_json::to_string(&kind_json).unwrap_or_default();
    vox_gamify::db::AgentEventRecord {
        id: ev.id.0 as i64,
        agent_id: agent_id.to_string(),
        event_type,
        payload: Some(payload),
        cli_version: None,
        timestamp: ev.timestamp_ms.to_string(),
    }
}

/// Pull recent Gamify rows when Codex is configured; **always** merges in-memory
/// `transient_events` so clients without DB still see streaming/orchestrator events.
pub async fn poll_events(state: &ServerState, params: PollEventsParams) -> String {
    let limit = params.limit.unwrap_or(50).max(1) as usize;
    let mut all_events = Vec::new();

    if let Some(db) = &state.db {
        let lim_i64 = limit as i64;
        let agent_ids = {
            let orch = &state.orchestrator;
            orch.agent_ids()
        };

        for id in agent_ids {
            if let Ok(records) =
                vox_gamify::db::get_events(db, &id.0.to_string(), Some(lim_i64)).await
            {
                all_events.extend(records);
            }
        }
    }

    let repo_id = state.repository.repository_id.clone();
    let transient = {
        let mut q = state.transient_events.lock().await;
        std::mem::take(&mut *q)
    };
    for ev in transient {
        all_events.push(agent_event_to_record(&ev, &repo_id));
    }

    all_events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    all_events.truncate(limit);
    ToolResult::ok(all_events).to_json()
}

/// Submit a task through the orchestrator (simpler shape than [`crate::params::SubmitTaskParams`]).
pub async fn submit_task(state: &ServerState, params: SubmitTaskParams) -> String {
    let orch = &state.orchestrator;

    let affinities = params
        .affinites
        .unwrap_or_default()
        .into_iter()
        .map(vox_orchestrator::FileAffinity::write)
        .collect();
    let _agent_id = params.agent_id.map(vox_orchestrator::AgentId);

    match orch
        .submit_task_with_agent(
            &params.description,
            affinities,
            None,
            None,
            None,
            None,
            params.session_id,
            params.tenant_id,
        )
        .await
    {
        Ok(task_id) => ToolResult::ok(format!("Submitted task {}", task_id.0)).to_json(),
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("Submit failed: {}", e),
            REM_VCS_SUBMIT,
        )
        .to_json(),
    }
}

/// Emit a synthetic busy event for the agent mapped to `session_id`.
pub async fn heartbeat(state: &ServerState, params: HeartbeatParams) -> String {
    let orch = &state.orchestrator;

    // Try finding the agent mapped to this session
    let mut target = None;
    for agent in orch.status().agents {
        if agent.agent_session_id.as_deref() == Some(params.session_id.as_str()) {
            target = Some(vox_orchestrator::AgentId(agent.id.0));
            break;
        }
    }

    if let Some(id) = target {
        // We simulate a heartbeat by emitting a basic event or just resetting their heartbeat timer
        orch.event_bus()
            .emit(vox_orchestrator::AgentEventKind::AgentBusy { agent_id: id });
        ToolResult::ok(format!("Heartbeat received for agent {}", id.0)).to_json()
    } else {
        ToolResult::<String>::err_with_remediation(
            "No agent mapped to this session",
            REM_SESSION_AGENT_MAP,
        )
        .to_json()
    }
}

/// Persist a cost row (when DB present) and emit `CostIncurred` on the orchestrator bus.
pub async fn record_cost(state: &ServerState, params: RecordCostParams) -> String {
    let (target_id, event_bus): (
        Option<vox_orchestrator::AgentId>,
        vox_orchestrator::events::EventBus,
    ) = {
        let orch = &state.orchestrator;

        let mut target = None;
        for agent in orch.status().agents {
            if agent.agent_session_id.as_deref() == Some(params.session_id.as_str()) {
                target = Some(vox_orchestrator::AgentId(agent.id.0));
                break;
            }
        }
        (target, orch.event_bus().clone())
    };

    if let Some(id) = target_id {
        if let Some(db) = &state.db {
            let tenant_id = vox_gamify::db::get_agent_session(db, &params.session_id)
                .await
                .ok()
                .flatten()
                .and_then(|s| s.tenant_id);

            let _ = vox_gamify::db::insert_cost_record(
                db,
                &id.0.to_string(),
                Some(&params.session_id),
                &params.provider,
                Some(&params.model),
                params.input_tokens as i64,
                params.output_tokens as i64,
                params.cost_usd,
                tenant_id.as_deref(),
            )
            .await;
        }

        event_bus.emit(vox_orchestrator::AgentEventKind::CostIncurred {
            agent_id: id,
            provider: params.provider,
            model: params.model,
            input_tokens: params.input_tokens,
            output_tokens: params.output_tokens,
            cost_usd: params.cost_usd,
            temporal_context: None,
        });
        ToolResult::ok(format!(
            "Cost {:.4} recorded for agent {}",
            params.cost_usd, id.0
        ))
        .to_json()
    } else {
        ToolResult::<String>::err_with_remediation(
            "No agent mapped to this session",
            REM_SESSION_AGENT_MAP,
        )
        .to_json()
    }
}
