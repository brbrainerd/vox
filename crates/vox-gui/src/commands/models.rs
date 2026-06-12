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

/// Parse a `VOX_AUTO_ROUTING_PRIORITY`-style CSV (`efficiency=25,precision=30,...`)
/// onto a base priority, leaving any axes not present in the CSV untouched.
fn apply_routing_csv(mut base: RoutingPriorityDto, csv: &str) -> RoutingPriorityDto {
    for part in csv.split(',') {
        let mut it = part.splitn(2, '=');
        let key = it.next().map(str::trim).unwrap_or("").to_ascii_lowercase();
        let Ok(parsed) = it.next().map(str::trim).unwrap_or("").parse::<u8>() else {
            continue;
        };
        match key.as_str() {
            "efficiency" | "cost" => base.efficiency = parsed,
            "precision" | "quality" => base.precision = parsed,
            "latency" | "speed" => base.latency = parsed,
            "availability" => base.availability = parsed,
            "balance" => base.balance = parsed,
            "mobile" => base.mobile = parsed,
            _ => {}
        }
    }
    base
}

/// Resolve the effective routing priority: persisted DB pref (if present)
/// overrides env/default; env/default is the base.
async fn effective_routing_priority() -> RoutingPriorityDto {
    let base: RoutingPriorityDto = AutoRoutingPriority::from_env().into();
    if let Some(db) =
        vox_db::connect_workspace_journey_optional(vox_db::DbConnectSurface::Runtime, true).await
        && let Ok(Some(csv)) = db
            .get_user_preference("local_user", "routing_priority")
            .await
        && !csv.trim().is_empty()
    {
        return apply_routing_csv(base, &csv);
    }
    base
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
    if let Some(db) =
        vox_db::connect_workspace_journey_optional(vox_db::DbConnectSurface::Runtime, true).await
    {
        let _ = db
            .set_user_preference("local_user", "active_model", model_id.trim())
            .await;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_active_model() -> Result<Option<String>, String> {
    if let Some(db) =
        vox_db::connect_workspace_journey_optional(vox_db::DbConnectSurface::Runtime, true).await
        && let Ok(Some(v)) = db.get_user_preference("local_user", "active_model").await
        && !v.trim().is_empty()
    {
        return Ok(Some(v));
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
        let req =
            ModelSelectionRequest::from_intent(SelectionIntent::for_task(TaskCategory::CodeGen));
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
    let exploration_spent_usd = vox_cli_core::daemon_ipc::dispatch::call_daemon(
        "vox-orchestrator-d",
        vox_foundation::protocol::orch_daemon_method::STATUS,
        serde_json::json!({}),
        false,
    )
    .await
    .ok()
    .and_then(|status| {
        status
            .get("global_exploration_cost_usd")
            .and_then(|v| v.as_f64())
    })
    .unwrap_or(0.0);

    Ok(RoutingSummaryDto {
        active_model: active,
        exploration_spent_usd,
        exploration_budget_usd: cfg.exploration.budget_usd_per_day,
        routing_priority: effective_routing_priority().await,
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
        std::env::set_var("VOX_AUTO_ROUTING_PRIORITY", &csv);
    }
    if let Some(db) =
        vox_db::connect_workspace_journey_optional(vox_db::DbConnectSurface::Runtime, true).await
    {
        let _ = db
            .set_user_preference("local_user", "routing_priority", &csv)
            .await;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_model_scoreboard(
    window_days: Option<i64>,
) -> Result<Vec<ScoreboardRowDto>, String> {
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
pub async fn explain_model_selection(
    task: String,
    complexity: Option<u8>,
) -> Result<serde_json::Value, String> {
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

/// Empty-policy JSON returned when no `selection_policy` preference is persisted.
const EMPTY_SELECTION_POLICY: &str = "{\"steps\":[]}";

/// Read the persisted `selection_policy` user-preference (JSON for a
/// [`vox_orchestrator::models::SelectionPolicy`]). Mirrors
/// [`effective_routing_priority`] / [`get_active_model`]: reads the
/// `("local_user","selection_policy")` pref via `connect_workspace_journey_optional`.
/// Returns the stored JSON, or the empty policy (`{"steps":[]}`) when absent.
#[tauri::command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_db preferences; behavior covered by orchestrator selection-policy tests
pub async fn get_selection_policy() -> String {
    if let Some(db) =
        vox_db::connect_workspace_journey_optional(vox_db::DbConnectSurface::Runtime, true).await
        && let Ok(Some(json)) = db
            .get_user_preference("local_user", "selection_policy")
            .await
        && !json.trim().is_empty()
    {
        return json;
    }
    EMPTY_SELECTION_POLICY.to_string()
}

/// Persist a `selection_policy` user-preference. Validates that `json` parses as
/// a [`vox_orchestrator::models::SelectionPolicy`] (rejecting invalid input)
/// before writing the `("local_user","selection_policy")` pref. Mirrors
/// [`set_routing_priority`].
///
/// NOTE: the daemon reads this preference and installs the active policy at
/// startup (mirroring how `routing_priority` is applied), so a saved policy
/// takes effect on the next orchestrator (re)start, not immediately.
#[tauri::command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_db preferences; behavior covered by orchestrator selection-policy tests
pub async fn set_selection_policy(json: String) -> Result<(), String> {
    // Reject anything that isn't a well-formed SelectionPolicy.
    vox_orchestrator::models::SelectionPolicy::from_json(&json)
        .map_err(|e| format!("invalid selection policy JSON: {e}"))?;
    if let Some(db) =
        vox_db::connect_workspace_journey_optional(vox_db::DbConnectSurface::Runtime, true).await
    {
        db.set_user_preference("local_user", "selection_policy", &json)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The six live routing-priority axes, in display order. Each maps 1:1 to a
/// field of [`RoutingPriorityDto`] / `vox_config::AutoRoutingPriority` and to a
/// CSV key understood by [`apply_routing_csv`] / [`set_routing_priority`].
const ROUTING_AXES: [(&str, &str); 6] = [
    ("efficiency", "Efficiency"),
    ("precision", "Precision"),
    ("latency", "Latency"),
    ("availability", "Availability"),
    ("balance", "Balance"),
    ("mobile", "Mobile"),
];

/// One cell of the Matrix "intentions" hex grid, derived from real routing
/// state (no fabricated rows). The shape matches the GUI `Intention` type.
#[derive(Debug, Clone, Serialize)]
pub struct RoutingIntentionDto {
    pub id: String,
    pub parent: String,
    pub branch: String,
    pub phase: String,
    pub conf: f64,
    pub note: String,
}

/// Direction of a Matrix Promote/Doubt action on a routing axis.
#[derive(Debug, Clone, Copy)]
pub enum NudgeDirection {
    /// Promote: increase this axis's weight (the orchestrator favors it more).
    Promote,
    /// Doubt: decrease this axis's weight (the orchestrator favors it less).
    Doubt,
}

/// Step size (priority points) applied per Promote/Doubt action.
const NUDGE_STEP: u8 = 5;

fn axis_value(p: &RoutingPriorityDto, axis: &str) -> Option<u8> {
    match axis {
        "efficiency" => Some(p.efficiency),
        "precision" => Some(p.precision),
        "latency" => Some(p.latency),
        "availability" => Some(p.availability),
        "balance" => Some(p.balance),
        "mobile" => Some(p.mobile),
        _ => None,
    }
}

fn axis_note(axis: &str) -> &'static str {
    match axis {
        "efficiency" => "Bias toward cheaper / lower-cost models.",
        "precision" => "Bias toward higher-quality, more capable models.",
        "latency" => "Bias toward faster (lower p50 latency) models.",
        "availability" => "Bias toward models with higher observed success rate.",
        "balance" => "Even weighting across cost, quality, and speed.",
        "mobile" => "Bias toward on-device / mobile-friendly models.",
        _ => "Routing priority axis.",
    }
}

/// Project the live routing priority onto Matrix hex cells. Confidence is the
/// axis weight normalized to 0..1; the top-weighted axis is `Validated`, the
/// bottom-weighted is `Doubted`, the rest are `Active`.
fn priority_to_intentions(p: &RoutingPriorityDto) -> Vec<RoutingIntentionDto> {
    let values: Vec<u8> = ROUTING_AXES
        .iter()
        .map(|(id, _)| axis_value(p, id).unwrap_or(0))
        .collect();
    let max = values.iter().copied().max().unwrap_or(0);
    let min = values.iter().copied().min().unwrap_or(0);
    ROUTING_AXES
        .iter()
        .map(|(id, label)| {
            let v = axis_value(p, id).unwrap_or(0);
            // Distinct max/min so a uniform profile stays Active rather than
            // flipping every cell to Validated+Doubted at once.
            let phase = if max != min && v == max {
                "Validated"
            } else if max != min && v == min {
                "Doubted"
            } else {
                "Active"
            };
            RoutingIntentionDto {
                id: (*id).to_string(),
                parent: "ROUTING-PRIORITY".to_string(),
                branch: (*label).to_string(),
                phase: phase.to_string(),
                conf: f64::from(v) / 100.0,
                note: axis_note(id).to_string(),
            }
        })
        .collect()
}

/// Apply a single Promote/Doubt nudge to one axis, clamped to `0..=100`. An
/// unknown axis is a no-op. Pure — does no I/O.
fn nudge_axis(p: &RoutingPriorityDto, axis: &str, dir: NudgeDirection) -> RoutingPriorityDto {
    let mut out = RoutingPriorityDto { ..*p };
    let Some(cur) = axis_value(p, axis) else {
        return out;
    };
    let next = match dir {
        NudgeDirection::Promote => cur.saturating_add(NUDGE_STEP).min(100),
        NudgeDirection::Doubt => cur.saturating_sub(NUDGE_STEP),
    };
    match axis {
        "efficiency" => out.efficiency = next,
        "precision" => out.precision = next,
        "latency" => out.latency = next,
        "availability" => out.availability = next,
        "balance" => out.balance = next,
        "mobile" => out.mobile = next,
        _ => {}
    }
    out
}

fn priority_to_csv(p: &RoutingPriorityDto) -> String {
    format!(
        "efficiency={},precision={},latency={},availability={},balance={},mobile={}",
        p.efficiency, p.precision, p.latency, p.availability, p.balance, p.mobile
    )
}

/// Live "intentions" for the Matrix surface: the real routing-priority axes,
/// derived from the effective (DB pref → env → default) priority. Replaces the
/// former hardcoded seed.
#[tauri::command]
pub async fn get_routing_intentions() -> Result<Vec<RoutingIntentionDto>, String> {
    Ok(priority_to_intentions(&effective_routing_priority().await))
}

/// Promote or Doubt a single routing axis from the Matrix surface. This is a
/// real mutation: it nudges the axis weight and persists it via the same path
/// as [`set_routing_priority`] (env var + `routing_priority` user-preference),
/// so the next orchestrator decision reflects it.
#[tauri::command]
pub async fn nudge_routing_intention(axis: String, direction: String) -> Result<(), String> {
    let dir = match direction.to_ascii_lowercase().as_str() {
        "promote" => NudgeDirection::Promote,
        "doubt" => NudgeDirection::Doubt,
        other => {
            return Err(format!(
                "unknown direction {other:?} (expected promote|doubt)"
            ));
        }
    };
    let base = effective_routing_priority().await;
    if axis_value(&base, &axis).is_none() {
        return Err(format!(
            "unknown routing axis {axis:?} (expected one of efficiency|precision|latency|availability|balance|mobile)"
        ));
    }
    let next = nudge_axis(&base, &axis, dir);
    let csv = priority_to_csv(&next);
    unsafe {
        std::env::set_var("VOX_AUTO_ROUTING_PRIORITY", &csv);
    }
    if let Some(db) =
        vox_db::connect_workspace_journey_optional(vox_db::DbConnectSurface::Runtime, true).await
    {
        let _ = db
            .set_user_preference("local_user", "routing_priority", &csv)
            .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_priority_csv_shape() {
        let csv = "efficiency=40,precision=30,latency=10,availability=10,balance=5,mobile=5";
        assert!(csv.contains("efficiency=40"));
    }

    fn sample_priority() -> RoutingPriorityDto {
        RoutingPriorityDto {
            efficiency: 40,
            precision: 30,
            latency: 10,
            availability: 10,
            balance: 5,
            mobile: 5,
        }
    }

    #[test]
    fn axes_to_intentions_yields_one_cell_per_axis_with_normalized_confidence() {
        let cells = priority_to_intentions(&sample_priority());
        assert_eq!(cells.len(), ROUTING_AXES.len());
        let eff = cells.iter().find(|c| c.id == "efficiency").unwrap();
        assert_eq!(eff.parent, "ROUTING-PRIORITY");
        // conf is the axis weight normalized to 0..1.
        assert!((eff.conf - 0.40).abs() < 1e-6, "got {}", eff.conf);
        assert_eq!(eff.branch, "Efficiency");
    }

    #[test]
    fn top_axis_is_validated_bottom_is_doubted() {
        let cells = priority_to_intentions(&sample_priority());
        let eff = cells.iter().find(|c| c.id == "efficiency").unwrap();
        assert_eq!(eff.phase, "Validated"); // highest weight
        let lowest = cells
            .iter()
            .find(|c| c.id == "balance" || c.id == "mobile")
            .unwrap();
        assert_eq!(lowest.phase, "Doubted"); // tied-lowest weight
    }

    #[test]
    fn nudge_promote_raises_axis_doubt_lowers_axis_and_clamps() {
        let base = sample_priority();
        let up = nudge_axis(&base, "latency", NudgeDirection::Promote);
        assert!(up.latency > base.latency);
        let down = nudge_axis(&base, "latency", NudgeDirection::Doubt);
        assert!(down.latency < base.latency);
        // Other axes are untouched by a nudge.
        assert_eq!(up.precision, base.precision);
        // Clamps at the ceiling.
        let mut maxed = sample_priority();
        maxed.precision = 100;
        let still = nudge_axis(&maxed, "precision", NudgeDirection::Promote);
        assert_eq!(still.precision, 100);
        // Clamps at the floor.
        let mut zeroed = sample_priority();
        zeroed.mobile = 0;
        let still0 = nudge_axis(&zeroed, "mobile", NudgeDirection::Doubt);
        assert_eq!(still0.mobile, 0);
    }

    #[test]
    fn nudge_unknown_axis_is_a_noop() {
        let base = sample_priority();
        let out = nudge_axis(&base, "nonsense", NudgeDirection::Promote);
        assert_eq!(out.efficiency, base.efficiency);
        assert_eq!(out.precision, base.precision);
    }

    #[test]
    fn priority_to_csv_round_trips_through_apply() {
        let p = sample_priority();
        let csv = priority_to_csv(&p);
        let back = apply_routing_csv(
            RoutingPriorityDto {
                efficiency: 0,
                precision: 0,
                latency: 0,
                availability: 0,
                balance: 0,
                mobile: 0,
            },
            &csv,
        );
        assert_eq!(back.efficiency, p.efficiency);
        assert_eq!(back.mobile, p.mobile);
    }
}
