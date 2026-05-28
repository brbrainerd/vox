use super::params::{
    AgentEventsParams, AttentionSummaryParams, CostHistoryParams, HandoffLineageParams,
    QueueStatusParams,
};

use crate::params::ToolResult;
use crate::server_state::ServerState;
use crate::sync_poison::{poison_rw_read, poison_rw_write};
use vox_orchestrator::{AgentId, TaskId};

const REM_AGENT_ID: &str =
    "Use `spawn_agent` / orchestrator status to list valid agent ids before querying queues.";
const REM_VOXDB_EVENTS: &str = "Configure VoxDb/Turso for the MCP server (or use `vox` CLI with DB enabled) to read Codex history.";
const REM_ORCH_LOCK: &str = "Retry; persistent poisoned-lock errors usually need an MCP restart.";
const REM_ORCH_TASK: &str = "Verify task id and lifecycle state with orchestrator tools; the task may be completed or invalid.";
const REM_ORCH_AGENT_OP: &str =
    "Confirm agent ids via orchestrator status; agents may be retired or paused.";
const REM_ORCH_CONFIG: &str =
    "Patch only valid `OrchestratorConfig` fields; compare against docs and `config_get` output.";

/// Return the queue snapshot for `params.agent_id`.
pub async fn queue_status(state: &ServerState, params: QueueStatusParams) -> String {
    let orch = &state.orchestrator;
    if let Some(queue_lock) = orch.agent_queue(AgentId(params.agent_id)) {
        let json = match poison_rw_read(queue_lock.read(), "read agent queue for queue_status") {
            Ok(guard) => guard.to_json(),
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_LOCK)
                    .to_json();
            }
        };
        ToolResult::ok(json).to_json()
    } else {
        ToolResult::<String>::err_with_remediation(
            format!("Agent {} not found", params.agent_id),
            REM_AGENT_ID,
        )
        .to_json()
    }
}

/// Count exclusive/read locks currently held across the orchestrator.
pub async fn lock_status(state: &ServerState) -> String {
    let orch = &state.orchestrator;
    let count = orch.lock_manager().active_lock_count();
    ToolResult::ok(format!("{} active locks", count)).to_json()
}

/// Aggregate token and USD spend tracked in agent budgets.
pub async fn budget_status(state: &ServerState) -> String {
    let orch = &state.orchestrator;

    // Total up from agents
    let mut total_tokens = 0;
    let mut total_cost = 0.0;

    let bh = orch.budget_handle();
    for agent_id in orch.agent_ids() {
        let budget = match poison_rw_read(bh.read(), "read budget for budget_status") {
            Ok(guard) => guard.check_budget(agent_id),
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_LOCK)
                    .to_json();
            }
        };
        if let Some(budget) = budget {
            total_tokens += budget.tokens_used;
            total_cost += budget.cost_usd;
        }
    }

    ToolResult::ok(format!(
        "Total usage: {} tokens, cost: ${:.4}",
        total_tokens, total_cost
    ))
    .to_json()
}

/// Cancel a task by numeric id (wrapper around orchestrator APIs).
pub async fn cancel_task(state: &ServerState, params: crate::params::CancelTaskParams) -> String {
    let orch = &state.orchestrator;

    if let Err(e) = orch.cancel_task(TaskId(params.task_id)) {
        return ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_TASK).to_json();
    }
    ToolResult::ok(format!("Task {} cancelled", params.task_id)).to_json()
}

/// Change the priority of a queued task.
pub async fn reorder_task(state: &ServerState, params: crate::params::ReorderTaskParams) -> String {
    let orch = &state.orchestrator;

    let priority = match params.priority.as_str() {
        "urgent" => vox_orchestrator::TaskPriority::Urgent,
        "background" => vox_orchestrator::TaskPriority::Background,
        _ => vox_orchestrator::TaskPriority::Normal,
    };

    if let Err(e) = orch.reorder_task(TaskId(params.task_id), priority) {
        return ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_TASK).to_json();
    }
    ToolResult::ok(format!(
        "Task {} reordered to {:?}",
        params.task_id, priority
    ))
    .to_json()
}

/// Drop all queued (not running) tasks for an agent.
pub async fn drain_agent(state: &ServerState, params: crate::params::DrainAgentParams) -> String {
    match state
        .orchestrator
        .drain_agent(vox_orchestrator::AgentId(params.agent_id))
    {
        Ok(tasks) => ToolResult::ok(format!(
            "Drained {} tasks from agent {}",
            tasks.len(),
            params.agent_id
        ))
        .to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_TASK).to_json()
        }
    }
}

/// Re-run the global task balancer and report how many tasks moved.
pub async fn rebalance(state: &ServerState) -> String {
    let moved = state.orchestrator.rebalance();
    ToolResult::ok(format!("Rebalanced {} tasks", moved)).to_json()
}

/// Return historical [`vox_gamify::db::AgentEventRecord`] rows when Codex is configured.
pub async fn agent_events(state: &ServerState, params: AgentEventsParams) -> String {
    if let Some(db) = &state.db {
        match vox_gamify::db::get_events(db, &params.agent_id.to_string(), None).await {
            Ok(events) => ToolResult::ok(events).to_json(),
            Err(e) => ToolResult::<String>::err_with_remediation(
                format!("DB error: {}", e),
                REM_VOXDB_EVENTS,
            )
            .to_json(),
        }
    } else {
        ToolResult::<String>::err_with_remediation(
            "Database not configured, cannot fetch past events.",
            REM_VOXDB_EVENTS,
        )
        .to_json()
    }
}

/// Bind `params.session_id` to `params.agent_id` inside the orchestrator.
pub async fn map_agent_session(
    state: &ServerState,
    params: crate::params::MapAgentSessionParams,
) -> String {
    let orch = &state.orchestrator;

    match orch.map_agent_session(
        vox_orchestrator::AgentId(params.agent_id),
        params.session_id.clone(),
    ) {
        Ok(_) => ToolResult::ok(format!(
            "Mapped agent session {} to agent {}",
            params.session_id, params.agent_id
        ))
        .to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_AGENT_OP).to_json()
        }
    }
}

/// Merge recent cost rows across orchestrator + agents (requires Codex).
pub async fn cost_history(state: &ServerState, params: CostHistoryParams) -> String {
    if let Some(db) = &state.db {
        let limit = params.limit_per_agent.unwrap_or(100);
        let mut all_records = Vec::new();

        let agent_ids = {
            let orch = &state.orchestrator;
            orch.agent_ids()
        };

        // Also add orchestrator
        let mut ids = vec![
            "vox-orchestrator".to_string(),
            "0".to_string(),
            "master".to_string(),
        ];
        for a in agent_ids {
            ids.push(a.0.to_string());
        }

        for id in ids {
            if let Ok(records) = vox_gamify::db::list_cost_records(db, &id, limit).await {
                all_records.extend(records);
            }
        }

        // Sort globally by timestamp descending
        all_records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        // Take total limit
        all_records.truncate(limit as usize * 2); // just some bounding

        ToolResult::ok(all_records).to_json()
    } else {
        ToolResult::<String>::err_with_remediation(
            "Database not configured, cannot fetch cost history.",
            REM_VOXDB_EVENTS,
        )
        .to_json()
    }
}

/// Dump the affinity map as JSON (path → owner relationships).
/// Serialize the orchestrator affinity map to JSON (path keys → owning agent ids).
pub async fn file_graph(state: &ServerState) -> String {
    let orch = &state.orchestrator;

    let map = orch.affinity_map().as_json();
    ToolResult::ok(map).to_json()
}

/// Return the attention summary for the last `params.hours`.
pub async fn attention_summary(state: &ServerState, params: AttentionSummaryParams) -> String {
    if let Some(db) = &state.db {
        let tracker = vox_orchestrator::attention_tracker::AttentionTracker::new(db);
        let hours = params.hours.unwrap_or(24);
        let since_ms = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64)
            .saturating_sub(hours * 3600 * 1000);

        match tracker.session_summary(since_ms).await {
            Ok(summary) => ToolResult::ok(summary).to_json(),
            Err(e) => ToolResult::<String>::err_with_remediation(
                format!("Attention tracking error: {}", e),
                REM_VOXDB_EVENTS,
            )
            .to_json(),
        }
    } else {
        ToolResult::<String>::err_with_remediation(
            "Database not configured, cannot fetch attention summary.",
            REM_VOXDB_EVENTS,
        )
        .to_json()
    }
}

/// Return the agent-to-agent handoff lineage from Codex.
pub async fn handoff_lineage(state: &ServerState, params: HandoffLineageParams) -> String {
    if let Some(db) = &state.db {
        let repo = vox_orchestrator::lineage::repository_id();
        let limit = params.limit.unwrap_or(50);
        match db
            .list_orchestration_lineage_events(&repo, Some("task_delegated"), limit as i64)
            .await
        {
            Ok(events) => ToolResult::ok(events).to_json(),
            Err(e) => ToolResult::<String>::err_with_remediation(
                format!("Lineage DB error: {}", e),
                REM_VOXDB_EVENTS,
            )
            .to_json(),
        }
    } else {
        ToolResult::<String>::err_with_remediation(
            "Database not configured, cannot fetch handoff lineage.",
            REM_VOXDB_EVENTS,
        )
        .to_json()
    }
}

/// Merge orchestrator config with on-disk `VoxConfig` toolchain map.
/// Return merged JSON: live `OrchestratorConfig` plus `VoxConfig` toolchain map from disk.
pub async fn config_get(state: &ServerState) -> String {
    let orch = &state.orchestrator;
    let orch_cfg = {
        let handle = orch.config_handle();
        let cfg = match poison_rw_read(handle.read(), "read orchestrator config for config_get") {
            Ok(c) => c,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_LOCK)
                    .to_json();
            }
        };
        serde_json::to_value(&*cfg).unwrap_or_default()
    };

    // Load VoxConfig SSOT (toolchain settings) and merge on top
    let vox_cfg = vox_config::VoxConfig::load();
    let toolchain = vox_cfg.to_map();

    let mut merged = serde_json::Map::new();
    merged.insert("orchestrator".to_string(), orch_cfg);
    merged.insert(
        "toolchain".to_string(),
        serde_json::to_value(toolchain).unwrap_or_default(),
    );

    ToolResult::ok(serde_json::Value::Object(merged)).to_json()
}

/// Patch `OrchestratorConfig` by shallow-merging JSON keys into the live instance.
/// Deep-merge `params` into the current orchestrator JSON config (mutates in-memory orchestrator).
pub async fn config_set(state: &ServerState, params: serde_json::Value) -> String {
    let orch = &state.orchestrator;

    let mut current_json = {
        let handle = orch.config_handle();
        let cfg = match poison_rw_read(handle.read(), "read orchestrator config for config_set") {
            Ok(c) => c,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_LOCK)
                    .to_json();
            }
        };
        serde_json::to_value(&*cfg).unwrap_or_default()
    };

    if let (serde_json::Value::Object(current), serde_json::Value::Object(patch)) =
        (&mut current_json, params)
    {
        for (k, v) in patch {
            current.insert(k, v);
        }
    }

    match serde_json::from_value::<vox_orchestrator::config::OrchestratorConfig>(current_json) {
        Ok(new_config) => {
            match poison_rw_write(
                orch.config_handle().write(),
                "write orchestrator config for config_set",
            ) {
                Ok(mut w) => *w = new_config.clone(),
                Err(e) => {
                    return ToolResult::<String>::err_with_remediation(
                        e.to_string(),
                        REM_ORCH_LOCK,
                    )
                    .to_json();
                }
            }
            ToolResult::ok(new_config).to_json()
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("invalid config fields: {e}"),
            REM_ORCH_CONFIG,
        )
        .to_json(),
    }
}

/// Truthful embedded-runtime probe: MCP holds the orchestrator in-process and may run the
/// embedded [`AgentFleet`](vox_orchestrator::runtime::AgentFleet) loop (see `VOX_MCP_AGENT_FLEET`).
pub async fn orchestrator_start(state: &ServerState) -> String {
    use crate::params::OrchestratorRuntimeProbe;

    let orch = &state.orchestrator;
    let agent_count = orch.agent_ids().len();
    let registered_worker_processes =
        vox_orchestrator::sync_lock::rw_read(&*orch.agent_handles).len();
    let execution_mode = if registered_worker_processes > 0 {
        "workers_attached"
    } else {
        "queue_only"
    };
    let fleet_on = ServerState::mcp_agent_fleet_env_enabled();

    let daemon_reported_agent_count: Option<u64> = None;
    let daemon_status_rpc_error: Option<String> = None;
    let daemon_reported_agent_ids: Option<Vec<u64>> = None;
    let daemon_agent_ids_rpc_error: Option<String> = None;

    let mut embed_ids_sorted: Vec<u64> = orch.agent_ids().iter().map(|a| a.0).collect();
    embed_ids_sorted.sort_unstable();

    let mut honest_message = format!(
        "Embedded orchestrator is active with {agent_count} agent queue(s); \
         {registered_worker_processes} vox-actor-runtime worker process handle(s) registered."
    );
    match (&daemon_reported_agent_count, &daemon_status_rpc_error) {
        (Some(dc), None) if (*dc as usize) != agent_count => {
            honest_message.push_str(&format!(
                " External vox-orchestrator-d orch.status reports agent_count={dc} (embedded {agent_count}); verify single SSOT or enable IPC-first routing."
            ));
        }
        (Some(dc), None) if (*dc as usize) == agent_count => {
            honest_message.push_str(&format!(
                " External vox-orchestrator-d orch.status also reports agent_count={dc}."
            ));
        }
        (_, Some(err)) => {
            honest_message.push_str(&format!(" External orch.status RPC failed: {err}"));
        }
        _ => {}
    }

    if let Some(err) = &daemon_agent_ids_rpc_error {
        honest_message.push_str(&format!(" External orch.agent_ids RPC failed: {err}"));
    } else if let Some(ref d_ids) = daemon_reported_agent_ids {
        let mut d_sorted = d_ids.clone();
        d_sorted.sort_unstable();
        if d_sorted != embed_ids_sorted {
            honest_message.push_str(&format!(
                " External orch.agent_ids (sorted) {d_sorted:?} differs from embedded {embed_ids_sorted:?}."
            ));
        }
    }

    ToolResult::ok(OrchestratorRuntimeProbe {
        honest_message,
        agent_count,
        registered_worker_processes,
        execution_mode: execution_mode.to_string(),
        agent_fleet_loop_running: fleet_on,
        note: if fleet_on {
            "Embedded AgentFleet runs in-process when VOX_MCP_AGENT_FLEET is enabled (default). \
             Disable with VOX_MCP_AGENT_FLEET=0. sync_fleet registers workers so task submit can wake ProcessQueue."
        } else {
            "VOX_MCP_AGENT_FLEET disabled: AgentFleet loop not started at MCP boot; queues only \
             drain if worker handles are registered another way."
        },
        daemon_reported_agent_count,
        daemon_status_rpc_error,
        daemon_reported_agent_ids,
        daemon_agent_ids_rpc_error,
    })
    .to_json()
}

/// Spawn a new orchestrator agent (optionally marked dynamic / auto-retire when idle).
pub async fn spawn_agent(state: &ServerState, params: crate::params::SpawnAgentParams) -> String {
    let out_name = params.name.clone();
    let out_dynamic = params.dynamic.unwrap_or(false);
    let out_parent = params.parent_agent_id;
    let out_source = params.source_task_id;
    let res = if out_dynamic {
        state.orchestrator.spawn_dynamic_agent_with_parent(
            &out_name,
            out_parent.map(vox_orchestrator::AgentId),
            params.delegation_reason.as_deref(),
            out_source.map(vox_orchestrator::TaskId),
            None,
        )
    } else {
        state.orchestrator.spawn_agent(&out_name)
    };
    match res.map_err(|e| e.to_string()) {
        Ok(id) => ToolResult::ok(serde_json::json!({
            "agent_id": id.0,
            "name": out_name,
            "dynamic": out_dynamic,
            "parent_agent_id": out_parent,
            "source_task_id": out_source,
        }))
        .to_json(),
        Err(e) => {
            ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_ORCH_AGENT_OP)
                .to_json()
        }
    }
}

/// Retire an agent (releases locks/affinity, drains queue metadata).
pub async fn retire_agent(state: &ServerState, params: crate::params::AgentIdToolParams) -> String {
    match state
        .orchestrator
        .retire_agent(vox_orchestrator::AgentId(params.agent_id))
        .await
    {
        Ok(remaining_tasks) => ToolResult::ok(serde_json::json!({
            "agent_id": params.agent_id,
            "remaining_tasks": remaining_tasks,
        }))
        .to_json(),
        Err(e) => {
            ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_ORCH_AGENT_OP)
                .to_json()
        }
    }
}

/// Pause an agent's task queue.
pub async fn pause_agent(state: &ServerState, params: crate::params::AgentIdToolParams) -> String {
    match state
        .orchestrator
        .pause_agent(vox_orchestrator::AgentId(params.agent_id))
    {
        Ok(()) => ToolResult::ok(format!("Agent {} paused", params.agent_id)).to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_AGENT_OP).to_json()
        }
    }
}

/// Resume a paused agent queue.
pub async fn resume_agent(state: &ServerState, params: crate::params::AgentIdToolParams) -> String {
    match state
        .orchestrator
        .resume_agent(vox_orchestrator::AgentId(params.agent_id))
    {
        Ok(()) => ToolResult::ok(format!("Agent {} resumed", params.agent_id)).to_json(),
        Err(e) => {
            ToolResult::<String>::err_with_remediation(e.to_string(), REM_ORCH_AGENT_OP).to_json()
        }
    }
}
