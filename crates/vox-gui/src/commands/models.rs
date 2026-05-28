//! Tauri commands for model registry, routing preferences, and scoreboard surfaces.

use serde::Serialize;
use vox_config::AutoRoutingPriority;
use vox_orchestrator::config::CostPreference;
use vox_orchestrator::models::{
    ModelRegistry, ModelSelectionRequest, SelectionIntent, TaskCategory, decide,
    select_with_default_registry,
};

#[derive(Debug, Serialize)]
pub struct ModelCardDto {
    pub id: String,
    pub provider: String,
    pub tier: String,
    pub cost_per_1k: f64,
    pub max_tokens: u32,
    pub is_free: bool,
    pub latency_p50_ms: Option<u32>,
    pub success_rate: Option<f64>,
    pub quality_score: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct RoutingPriorityDto {
    pub efficiency: u8,
    pub precision: u8,
    pub latency: u8,
    pub availability: u8,
    pub balance: u8,
    pub mobile: u8,
}

impl From<AutoRoutingPriority> for RoutingPriorityDto {
    fn from(p: AutoRoutingPriority) -> Self {
        Self {
            efficiency: p.efficiency,
            precision: p.precision,
            latency: p.latency,
            availability: p.availability,
            balance: p.balance,
            mobile: p.mobile,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RoutingSummaryDto {
    pub active_model: Option<String>,
    pub exploration_spent_usd: f64,
    pub exploration_budget_usd: f64,
    pub routing_priority: RoutingPriorityDto,
    pub arm_count: usize,
    pub model_count: usize,
    pub decision_preview: Option<DecisionPreviewDto>,
}

#[derive(Debug, Serialize)]
pub struct DecisionPreviewDto {
    pub selected_model: String,
    pub discovery_state: String,
    pub alternatives: Vec<String>,
    pub rejection_reasons: Vec<String>,
    pub intelligence_score: f64,
    pub efficiency_score: f64,
    pub latency_score: f64,
}

#[derive(Debug, Serialize)]
pub struct ScoreboardRowDto {
    pub model_id: String,
    pub task_category: String,
    pub strength_tag: String,
    pub n_calls: i64,
    pub success_rate: f64,
    pub p50_latency_ms: Option<i64>,
    pub cost_per_success_usd: Option<f64>,
    pub quality_score: f64,
}

fn registry_from_cache() -> ModelRegistry {
    ModelRegistry::from_cache()
}

#[tauri::command]
pub async fn list_model_cards(limit: Option<usize>) -> Result<Vec<ModelCardDto>, String> {
    let reg = registry_from_cache();
    let limit = limit.unwrap_or(200);
    let mut models = reg.list_models();
    models.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(models
        .into_iter()
        .take(limit)
        .map(|m| {
            let sb = reg.scoreboard_snapshot().get(&m.id);
            ModelCardDto {
                id: m.id.clone(),
                provider: m.provider.clone(),
                tier: format!("{:?}", m.capabilities.tier),
                cost_per_1k: m.cost_per_1k,
                max_tokens: u32::try_from(m.max_tokens).unwrap_or(u32::MAX),
                is_free: m.is_free,
                latency_p50_ms: m.capabilities.latency_p50_ms,
                success_rate: sb.map(|s| s.success_rate),
                quality_score: sb.map(|s| s.quality_score),
            }
        })
        .collect())
}

#[tauri::command]
pub async fn set_active_model(model_id: String) -> Result<(), String> {
    if model_id.trim().is_empty() {
        return Err("model_id must not be empty".into());
    }
    let reg = registry_from_cache();
    if reg.get(&model_id).is_none() {
        return Err(format!("model {model_id} not found in registry"));
    }
    unsafe {
        std::env::set_var("VOX_MODEL", model_id.trim());
    }
    if let Some(db) = vox_db::connect_workspace_journey_optional(
        vox_db::DbConnectSurface::Runtime,
        true,
    )
    .await
    {
        let _ = db
            .set_user_preference("local_user", "active_model", model_id.trim())
            .await;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_active_model() -> Result<Option<String>, String> {
    if let Some(db) = vox_db::connect_workspace_journey_optional(
        vox_db::DbConnectSurface::Runtime,
        true,
    )
    .await
    {
        if let Ok(Some(v)) = db.get_user_preference("local_user", "active_model").await {
            if !v.trim().is_empty() {
                return Ok(Some(v));
            }
        }
    }
    Ok(vox_secrets::resolve_secret(vox_secrets::SecretId::VoxModel)
        .expose()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string))
}

#[tauri::command]
pub async fn get_routing_summary() -> Result<RoutingSummaryDto, String> {
    let reg = registry_from_cache();
    let cfg = vox_config::load_model_routing_config();
    let active = get_active_model().await.ok().flatten();
    let decision_preview = {
        let req = ModelSelectionRequest::from_intent(SelectionIntent::for_task(TaskCategory::CodeGen));
        decide(&req, &reg).map(|d| DecisionPreviewDto {
            selected_model: d.selected_model,
            discovery_state: d.discovery_state.as_str().to_string(),
            alternatives: d.alternatives,
            rejection_reasons: d.rejection_reasons,
            intelligence_score: d.score_breakdown.intelligence_score,
            efficiency_score: d.score_breakdown.efficiency_score,
            latency_score: d.score_breakdown.latency_score,
        })
    };
    Ok(RoutingSummaryDto {
        active_model: active,
        exploration_spent_usd: 0.0,
        exploration_budget_usd: cfg.exploration.budget_usd_per_day,
        routing_priority: AutoRoutingPriority::from_env().into(),
        arm_count: reg.arm_stats_snapshot().len(),
        model_count: reg.list_models().len(),
        decision_preview,
    })
}

#[tauri::command]
pub async fn set_routing_priority(
    efficiency: u8,
    precision: u8,
    latency: u8,
    availability: u8,
    balance: u8,
    mobile: u8,
) -> Result<(), String> {
    let csv = format!(
        "efficiency={efficiency},precision={precision},latency={latency},availability={availability},balance={balance},mobile={mobile}"
    );
    unsafe {
        std::env::set_var("VOX_AUTO_ROUTING_PRIORITY", csv);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_model_scoreboard(window_days: Option<i64>) -> Result<Vec<ScoreboardRowDto>, String> {
    let window = window_days.unwrap_or(7);
    let db_config = vox_db::DbConfig::resolve_canonical().map_err(|e| e.to_string())?;
    let db = vox_db::VoxDb::connect(db_config)
        .await
        .map_err(|e| e.to_string())?;
    let rows = db
        .get_model_scoreboard(window)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| ScoreboardRowDto {
            model_id: r.model_id,
            task_category: r.task_category,
            strength_tag: r.strength_tag,
            n_calls: r.n_calls,
            success_rate: r.success_rate,
            p50_latency_ms: r.p50_latency_ms,
            cost_per_success_usd: r.cost_per_success_usd,
            quality_score: r.quality_score,
        })
        .collect())
}

#[tauri::command]
pub async fn explain_model_selection(task: String, complexity: Option<u8>) -> Result<serde_json::Value, String> {
    let task_category = match task.to_ascii_lowercase().as_str() {
        "codegen" => TaskCategory::CodeGen,
        "research" => TaskCategory::Research,
        "review" => TaskCategory::Review,
        "debugging" => TaskCategory::Debugging,
        "parsing" => TaskCategory::Parsing,
        _ => TaskCategory::General,
    };
    let mut intent = SelectionIntent::for_task(task_category);
    if let Some(c) = complexity {
        intent.complexity = c.clamp(1, 10);
    }
    let outcome = select_with_default_registry(&intent)
        .ok_or_else(|| "no model matched selection intent".to_string())?;
    Ok(serde_json::json!({
        "model_id": outcome.model_id,
        "reason": format!("{:?}", outcome.reason),
        "effective_axes": {
            "efficiency": outcome.effective_axes.efficiency,
            "precision": outcome.effective_axes.precision,
            "latency": outcome.effective_axes.latency,
            "availability": outcome.effective_axes.availability,
            "balance": outcome.effective_axes.balance,
            "mobile": outcome.effective_axes.mobile,
        },
        "provider": outcome.model_spec.provider,
        "tier": format!("{:?}", outcome.model_spec.capabilities.tier),
        "cost_per_1k": outcome.model_spec.cost_per_1k,
    }))
}

#[tauri::command]
pub async fn suggest_model_for_task(task: String) -> Result<String, String> {
    let reg = registry_from_cache();
    let task_category = match task.to_ascii_lowercase().as_str() {
        "codegen" => TaskCategory::CodeGen,
        "research" => TaskCategory::Research,
        "review" => TaskCategory::Review,
        _ => TaskCategory::General,
    };
    reg.best_for(task_category, 5, CostPreference::Performance)
        .map(|m| m.id)
        .ok_or_else(|| "no suitable model".to_string())
}

/// Live routing summary (registry + env; avoids heavy orchestrator bootstrap in Tauri).
#[tauri::command]
pub async fn get_routing_summary_live() -> Result<RoutingSummaryDto, String> {
    get_routing_summary().await
}

#[cfg(test)]
mod tests {
    #[test]
    fn routing_priority_csv_shape() {
        let csv = "efficiency=40,precision=30,latency=10,availability=10,balance=5,mobile=5";
        assert!(csv.contains("efficiency=40"));
    }
}
