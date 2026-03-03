use crate::{ServerState, ToolResult};
use schemars::JsonSchema;
use serde::Deserialize;
use vox_gamify::companion::{Companion, Interaction};
use vox_gamify::db::{list_companions, upsert_companion};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckMoodParams {
    pub agent_id: u64,
}

pub async fn check_mood(state: &ServerState, params: CheckMoodParams) -> String {
    let id = format!("agent-{}", params.agent_id);
    let user_id = vox_db::paths::local_user_id();

    if let Some(db) = &state.db {
        match list_companions(db, &user_id).await {
            Ok(comps) => {
                if let Some(c) = comps.into_iter().find(|c: &Companion| c.id == id) {
                    return ToolResult::ok(c).to_json();
                }
            }
            Err(e) => tracing::warn!("failed to list companions from DB: {}", e),
        }
    }

    // Fallback/Initial create
    let companion = Companion::new(&id, &user_id, format!("Agent {}", params.agent_id), "vox");

    // Auto-save if DB exists
    if let Some(db) = &state.db {
        let _ = upsert_companion(db, &companion).await;
    }

    ToolResult::ok(companion).to_json()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentStatusParams {
    pub agent_id: u64,
}

pub async fn agent_status(state: &ServerState, params: AgentStatusParams) -> String {
    let id = format!("agent-{}", params.agent_id);
    let user_id = vox_db::paths::local_user_id();
    let mut companion = if let Some(db) = &state.db {
        match list_companions(db, &user_id).await {
            Ok(comps) => comps.into_iter().find(|c: &Companion| c.id == id),
            Err(_) => None,
        }
    } else {
        None
    }
    .unwrap_or_else(|| Companion::new(&id, &user_id, format!("Agent {}", params.agent_id), "vox"));

    let orch = state.orchestrator.lock().await;

    if let Some(queue) = orch.agent_queue(vox_orchestrator::AgentId(params.agent_id)) {
        let hp_bar = companion.render_status_bar(10);
        let markdown = format!(
            "### 🤖 Agent {} Status\n\n**{}**\n\n**Stats:**\n- Queue Depth: `{}`\n- Tasks Done: `{}`\n\n**Activity:** {}",
            params.agent_id,
            hp_bar,
            queue.len(),
            queue.completed_count(),
            if !queue.is_empty() { "Processing tasks... ⚙️" } else { "Idle 💤" }
        );
        ToolResult::ok(markdown).to_json()
    } else {
        ToolResult::<String>::err("Agent not found").to_json()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentContinueParams {
    pub agent_id: u64,
}

pub async fn agent_continue(state: &ServerState, params: AgentContinueParams) -> String {
    let mut orch = state.orchestrator.lock().await;
    orch.tick(); // Triggers auto-continuations for idle agents
    ToolResult::ok(format!(
        "Agent {} triggered for continuation",
        params.agent_id
    ))
    .to_json()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentAssessParams {
    pub agent_id: u64,
}

pub async fn agent_assess(state: &ServerState, params: AgentAssessParams) -> String {
    let mut ms_per_task: usize = 45_000;
    let user_id = vox_db::paths::local_user_id();

    if let Some(db) = &state.db {
        if let Ok(Some(pref)) = db
            .store()
            .get_user_preference(&user_id, "task.estimate_ms")
            .await
        {
            if let Ok(val) = pref.parse::<usize>() {
                ms_per_task = val;
            }
        }
    }

    let orch = state.orchestrator.lock().await;

    if let Some(queue) = orch.agent_queue(vox_orchestrator::AgentId(params.agent_id)) {
        let active = queue.len();
        let completed = queue.completed_count();

        let estimate_s = (active * ms_per_task) / 1000;
        ToolResult::ok(format!(
            "Agent {} has {} pending and {} completed tasks. Est remaining time: {}s",
            params.agent_id, active, completed, estimate_s
        ))
        .to_json()
    } else {
        ToolResult::<String>::err("Agent not found").to_json()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentHandoffParams {
    pub from_agent_id: u64,
    pub to_agent_id: u64,
    pub plan_summary: String,
    #[serde(default)]
    pub unresolved_objectives: Vec<String>,
    #[serde(default)]
    pub verification_criteria: Vec<String>,
}

pub async fn agent_handoff(state: &ServerState, params: AgentHandoffParams) -> String {
    let orch = state.orchestrator.lock().await;
    let mut payload = vox_orchestrator::handoff::HandoffPayload::new(
        vox_orchestrator::AgentId(params.from_agent_id),
        Some(vox_orchestrator::AgentId(params.to_agent_id)),
        &params.plan_summary,
    );
    payload.unresolved_objectives = params.unresolved_objectives;
    payload.verification_criteria = params.verification_criteria;
    vox_orchestrator::handoff::execute_handoff(&payload, orch.event_bus());

    ToolResult::ok(format!(
        "Handoff initiated from agent {} to agent {}: {}",
        params.from_agent_id, params.to_agent_id, params.plan_summary
    ))
    .to_json()
}
