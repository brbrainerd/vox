use crate::{ServerState, ToolResult, AgentInfo, StatusResponse};
use schemars::JsonSchema;
use serde::Deserialize;
use vox_gamify::companion::Companion;
use vox_gamify::db::{list_companions, upsert_companion};
use std::path::PathBuf;
use vox_orchestrator::{AgentId, TaskId, OrchestratorConfig};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueueStatusParams {
    pub agent_id: u64,
}

pub async fn queue_status(state: &ServerState, params: QueueStatusParams) -> String {
    let orch = state.orchestrator.lock().await;
    if let Some(queue) = orch.agent_queue(AgentId(params.agent_id)) {
        ToolResult::ok(queue.to_json()).to_json()
    } else {
        ToolResult::<String>::err(format!("Agent {} not found", params.agent_id)).to_json()
    }
}

pub async fn lock_status(state: &ServerState) -> String {
    let orch = state.orchestrator.lock().await;
    let count = orch.lock_manager().active_lock_count();
    ToolResult::ok(format!("{} active locks", count)).to_json()
}

pub async fn budget_status(state: &ServerState) -> String {
    let orch = state.orchestrator.lock().await;

    // Total up from agents
    let mut total_tokens = 0;
    let mut total_cost = 0.0;

    for agent_id in orch.agent_ids() {
        if let Some(budget) = orch.budget().check_budget(agent_id) {
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CancelTaskParams {
    pub task_id: u64,
}

pub async fn cancel_task(state: &ServerState, params: crate::CancelTaskParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    if let Err(e) = orch.cancel_task(TaskId(params.task_id)) {
        return ToolResult::<String>::err(format!("{}", e)).to_json();
    }
    ToolResult::ok(format!("Task {} cancelled", params.task_id)).to_json()
}

pub async fn reorder_task(state: &ServerState, params: crate::ReorderTaskParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    let priority = match params.priority.as_str() {
        "urgent" => vox_orchestrator::TaskPriority::Urgent,
        "background" => vox_orchestrator::TaskPriority::Background,
        _ => vox_orchestrator::TaskPriority::Normal,
    };

    if let Err(e) = orch.reorder_task(TaskId(params.task_id), priority) {
        return ToolResult::<String>::err(format!("{}", e)).to_json();
    }
    ToolResult::ok(format!(
        "Task {} reordered to {:?}",
        params.task_id, priority
    ))
    .to_json()
}

pub async fn drain_agent(state: &ServerState, params: crate::DrainAgentParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    match orch.drain_agent(vox_orchestrator::AgentId(params.agent_id)) {
        Ok(tasks) => ToolResult::ok(format!(
            "Drained {} tasks from agent {}",
            tasks.len(),
            params.agent_id
        ))
        .to_json(),
        Err(e) => ToolResult::<String>::err(format!("{}", e)).to_json(),
    }
}

pub async fn rebalance(state: &ServerState) -> String {
    let mut orch = state.orchestrator.lock().await;
    let moved = orch.rebalance();
    ToolResult::ok(format!("Rebalanced {} tasks", moved)).to_json()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentEventsParams {
    pub agent_id: u64,
}

pub async fn agent_events(state: &ServerState, params: AgentEventsParams) -> String {
    if let Some(db) = &state.db {
        match vox_gamify::db::get_events(db, &params.agent_id.to_string(), None).await {
            Ok(events) => ToolResult::ok(events).to_json(),
            Err(e) => ToolResult::<String>::err(format!("DB error: {}", e)).to_json(),
        }
    } else {
        ToolResult::<String>::err("Database not configured, cannot fetch past events.").to_json()
    }
}

pub async fn map_agent_session(
    state: &ServerState,
    params: crate::MapAgentSessionParams,
) -> String {
    let mut orch = state.orchestrator.lock().await;

    match orch.map_agent_session(
        vox_orchestrator::AgentId(params.agent_id),
        params.session_id.clone(),
    ) {
        Ok(_) => ToolResult::ok(format!(
            "Mapped agent session {} to agent {}",
            params.session_id, params.agent_id
        ))
        .to_json(),
        Err(e) => ToolResult::<String>::err(format!("{}", e)).to_json(),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CostHistoryParams {
    pub limit_per_agent: Option<i64>,
}

pub async fn cost_history(state: &ServerState, params: CostHistoryParams) -> String {
    if let Some(db) = &state.db {
        let limit = params.limit_per_agent.unwrap_or(100);
        let mut all_records = Vec::new();

        let agent_ids = {
            let orch = state.orchestrator.lock().await;
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
        ToolResult::<String>::err("Database not configured, cannot fetch cost history.").to_json()
    }
}

pub async fn file_graph(state: &ServerState) -> String {
    let orch = state.orchestrator.lock().await;

    let map = orch.affinity_map().as_json();
    ToolResult::ok(map).to_json()
}

pub async fn config_get(state: &ServerState) -> String {
    let orch = state.orchestrator.lock().await;
    let orch_cfg = serde_json::to_value(orch.config().clone()).unwrap_or_default();

    // Load VoxConfig SSOT (toolchain settings) and merge on top
    let vox_cfg = vox_config::VoxConfig::load();
    let toolchain = vox_cfg.to_map();

    let mut merged = serde_json::Map::new();
    merged.insert("orchestrator".to_string(), orch_cfg);
    merged.insert("toolchain".to_string(), serde_json::to_value(toolchain).unwrap_or_default());

    ToolResult::ok(serde_json::Value::Object(merged)).to_json()
}

pub async fn config_set(state: &ServerState, params: serde_json::Value) -> String {
    let mut orch = state.orchestrator.lock().await;

    let config = orch.config().clone();
    let mut current_json = serde_json::to_value(&config).unwrap_or_default();

    if let (serde_json::Value::Object(ref mut current), serde_json::Value::Object(patch)) =
        (&mut current_json, params)
    {
        for (k, v) in patch {
            current.insert(k, v);
        }
    }

    match serde_json::from_value::<vox_orchestrator::config::OrchestratorConfig>(current_json) {
        Ok(new_config) => {
            *orch.config_mut() = new_config.clone();
            ToolResult::ok(new_config).to_json()
        }
        Err(e) => ToolResult::<String>::err(format!("invalid config fields: {e}")).to_json(),
    }
}


pub async fn orchestrator_start(state: &ServerState) -> String {
    let orch = state.orchestrator.lock().await;

    // In a real execution, we would start AgentFleet by moving Orchestrator into it.
    // However, since we hold it in ServerState inside a Mutex, we simply return
    // that the orchestrator is running and report the number of agents.
    let count = orch.agent_ids().len();
    ToolResult::ok(format!(
        "AgentFleet is running with {} active agents.",
        count
    ))
    .to_json()
}

/// Get a full snapshot of the orchestrator's state.
pub async fn orchestrator_status(state: &ServerState) -> String {
    let (
        status,
        scaling_profile,
        effective_scale_up_threshold,
        vcs_snapshot_count,
        vcs_oplog_count,
        vcs_active_conflicts,
        vcs_active_workspaces,
        vcs_active_changes,
    ) = {
        let orch = state.orchestrator.lock().await;
        let cfg = orch.config();
        let effective = cfg.scaling_threshold as f64 * cfg.scaling_profile.threshold_multiplier();
        (
            orch.status(),
            Some(format!("{:?}", cfg.scaling_profile).to_lowercase()),
            Some(effective),
            orch.snapshot_store().count(),
            orch.oplog().count(),
            orch.conflict_manager().active_count(),
            orch.workspace_manager().list_workspaces().len(),
            orch.workspace_manager()
                .list_changes(None, usize::MAX)
                .len(),
        )
    };

    let agents: Vec<AgentInfo> = status
        .agents
        .iter()
        .map(|a| AgentInfo {
            id: a.id.0,
            name: a.name.clone(),
            queued: a.queued,
            completed: a.completed,
            paused: a.paused,
        })
        .collect();

    let companion = {
        // Try to load from DB for persistence
        let id = "vox-orchestrator";
        let mut comp = if let Some(db) = &state.db {
            match list_companions(db, "user").await {
                Ok(comps) => comps.into_iter().find(|c: &Companion| c.id == id),
                Err(_) => None,
            }
        } else {
            None
        }
        .unwrap_or_else(|| {
            vox_gamify::companion::Companion::new(id, "user", "Vox Orchestrator", "vox")
        });

        comp.ascii_sprite = Some("🧑‍💻".to_string());
        Some(comp)
    };

    let scaling_line = match (scaling_profile.as_ref(), effective_scale_up_threshold) {
        (Some(prof), Some(eff)) => format!(
            "**Scaling:** profile={}, effective scale-up threshold={:.1}\n\n",
            prof, eff
        ),
        _ => String::new(),
    };
    let mut markdown = format!(
        "### 🤖 Vox Orchestrator Status\n\n**Agents Active:** {}\n**Tasks In Progress:** {}\n**Tasks Completed:** {}\n\n{}",
        status.agents.len(),
        status.agents.iter().map(|a| a.queued).sum::<usize>(),
        status.total_completed,
        scaling_line
    );

    if let Some(ref c) = companion {
        markdown.push_str("#### 🧬 Code Companion\n\n");
        markdown.push_str(&format!(
            "```\n{}\n```\n",
            c.ascii_sprite.as_deref().unwrap_or("")
        ));
        markdown.push_str(&format!("**{}** {}\n", c.name, c.mood.emoji()));
        markdown.push_str(&format!(
            "HP: `{}`\n",
            c.render_status_bar(15).split("HP: ").last().unwrap_or("")
        ));
    }

    markdown.push_str("\n#### 📋 Agent Queue\n\n");
    for a in &agents {
        let status_icon = if a.paused {
            "⏸️"
        } else if a.queued > 0 {
            "⚙️"
        } else {
            "💤"
        };
        markdown.push_str(&format!(
            "- {} **{}** (Queued: {}, Done: {})\n",
            status_icon, a.name, a.queued, a.completed
        ));
    }

    let response = StatusResponse {
        agent_count: status.agents.len(),
        in_progress: status.agents.iter().map(|a| a.queued).sum(),
        completed: status.total_completed,
        agents,
        scaling_profile,
        effective_scale_up_threshold,
        companion,
        markdown_summary: Some(markdown),
        snapshot_count: vcs_snapshot_count,
        oplog_count: vcs_oplog_count,
        active_conflicts: vcs_active_conflicts,
        active_workspaces: vcs_active_workspaces,
        active_changes: vcs_active_changes,
    };

    ToolResult::ok(response).to_json()
}

/// Check which agent owns a given file path (async).
pub async fn check_file_owner(state: &ServerState, path: &str) -> String {
    let orch = state.orchestrator.lock().await;

    let affinity_map = orch.affinity_map();
    match affinity_map.lookup(&PathBuf::from(path)) {
        Some(agent_id) => ToolResult::ok(format!("owned by agent {agent_id}")).to_json(),
        None => ToolResult::ok("no owner assigned".to_string()).to_json(),
    }
}

/// Unified VCS status: snapshots, oplog, conflicts, workspaces, and changes.
pub async fn vcs_status(state: &ServerState) -> String {
    let orch = state.orchestrator.lock().await;

    let snapshot_count = orch.snapshot_store().count();
    let oplog_count = orch.oplog().count();
    let active_conflicts = orch.conflict_manager().active_count();
    let total_conflicts = orch.conflict_manager().total_count();
    let active_workspaces = orch.workspace_manager().list_workspaces().len();
    let active_changes = orch
        .workspace_manager()
        .list_changes(None, usize::MAX)
        .len();

    // Build workspace details
    let workspace_details: Vec<serde_json::Value> = orch
        .workspace_manager()
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
    let recent_ops: Vec<serde_json::Value> = orch
        .oplog()
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
    let conflict_details: Vec<serde_json::Value> = orch
        .conflict_manager()
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PollEventsParams {
    pub limit: Option<i64>,
}

pub async fn poll_events(state: &ServerState, params: PollEventsParams) -> String {
    if let Some(db) = &state.db {
        let limit = params.limit.unwrap_or(50);
        let mut all_events = Vec::new();
        let agent_ids = {
            let orch = state.orchestrator.lock().await;
            orch.agent_ids()
        };

        for id in agent_ids {
            if let Ok(records) =
                vox_gamify::db::get_events(db, &id.0.to_string(), Some(limit)).await
            {
                all_events.extend(records);
            }
        }

        let mut transient = Vec::new();
        {
            let mut q = state.transient_events.lock().await;
            transient = std::mem::take(&mut *q);
        }

        for ev in transient {
            let (agent_id, event_type) = match &ev.kind {
                vox_orchestrator::AgentEventKind::TokenStreamed { agent_id, .. } => (agent_id.0, "TokenStreamed"),
                _ => (0, "Unknown"),
            };
            let payload = serde_json::to_string(&ev.kind).unwrap_or_default();
            all_events.push(vox_gamify::db::AgentEventRecord {
                id: ev.id.0 as i64,
                agent_id: agent_id.to_string(),
                event_type: event_type.to_string(),
                payload: Some(payload),
                timestamp: ev.timestamp_ms.to_string(),
            });
        }

        all_events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all_events.truncate(limit as usize);
        ToolResult::ok(all_events).to_json()
    } else {
        ToolResult::<String>::err("DB not configured").to_json()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitTaskParams {
    pub description: String,
    pub affinites: Option<Vec<String>>,
    pub agent_id: Option<u64>,
}

pub async fn submit_task(state: &ServerState, params: SubmitTaskParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    let affinities = params
        .affinites
        .unwrap_or_default()
        .into_iter()
        .map(vox_orchestrator::FileAffinity::write)
        .collect();
    let agent_id = params.agent_id.map(vox_orchestrator::AgentId);

    match orch.submit_task(&params.description, affinities, None).await {
        Ok(task_id) => ToolResult::ok(format!("Submitted task {}", task_id.0)).to_json(),
        Err(e) => ToolResult::<String>::err(format!("Submit failed: {}", e)).to_json(),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HeartbeatParams {
    pub session_id: String,
}

pub async fn heartbeat(state: &ServerState, params: HeartbeatParams) -> String {
    let mut orch = state.orchestrator.lock().await;

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
        ToolResult::<String>::err("No agent mapped to this session").to_json()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordCostParams {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub cost_usd: f64,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

pub async fn record_cost(state: &ServerState, params: RecordCostParams) -> String {
    let (target_id, event_bus) = {
        let orch = state.orchestrator.lock().await;

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
            let _ = vox_gamify::db::insert_cost_record(
                db,
                &id.0.to_string(),
                Some(&params.session_id),
                &params.provider,
                Some(&params.model),
                params.input_tokens as i64,
                params.output_tokens as i64,
                params.cost_usd,
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
        });
        ToolResult::ok(format!(
            "Cost {:.4} recorded for agent {}",
            params.cost_usd, id.0
        ))
        .to_json()
    } else {
        ToolResult::<String>::err("No agent mapped to this session").to_json()
    }
}
