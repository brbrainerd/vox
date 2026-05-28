//! MCP-style registry resolution — canonical implementation for model pick/fallback.
//!
//! `crate::dei_shim::selection` re-exports these symbols for legacy module paths; do not duplicate logic there.

use crate::config::CostPreference;
use crate::mode::{InferenceConfig, TierProfile};
use crate::models::{ModelRegistry, ModelSpec, StrengthTag, task_category_strength};
use crate::privacy_router::model_supports_privacy_local_inference;
use crate::types::TaskCategory;
use crate::usage::RemainingBudget;

#[must_use]
pub fn infer_prompt_capability_hints(prompt: &str) -> (bool, bool) {
    let p = prompt.to_lowercase();
    let requires_vision = false;
    let requires_web_search = p.contains("web search")
        || p.contains("search web")
        || p.contains("search the web")
        || p.contains("look up")
        || p.contains("lookup ")
        || p.contains(" google ")
        || p.contains("bing ")
        || p.contains(" browse ")
        || p.contains("browse the")
        || p.contains("duckduckgo")
        || p.contains("perplexity")
        || p.contains("on the internet")
        || p.contains("live data")
        || p.contains("real-time")
        || p.contains("real time")
        || p.contains("stock price")
        || p.contains("weather today")
        || p.contains("latest ")
        || p.contains("current event")
        || p.contains("today's ")
        || p.contains(" news");
    (requires_vision, requires_web_search)
}

#[derive(Debug, Clone, Copy)]
pub struct RegistryModelResolutionParams {
    pub task: TaskCategory,
    pub complexity: u8,
    pub free_tier_latency_critical: bool,
    pub free_tier_fill_in_middle: bool,
    pub allow_cheapest_fallback: bool,
    pub force_web_search_for_task: bool,
    pub context_fill_ratio: Option<f32>,
    pub privacy_requires_local: bool,
}

impl Default for RegistryModelResolutionParams {
    fn default() -> Self {
        Self {
            task: TaskCategory::CodeGen,
            complexity: 5,
            free_tier_latency_critical: false,
            free_tier_fill_in_middle: false,
            allow_cheapest_fallback: false,
            force_web_search_for_task: false,
            context_fill_ratio: None,
            privacy_requires_local: false,
        }
    }
}

fn inference_predicate(
    m: &ModelSpec,
    cfg: &InferenceConfig,
    params: &RegistryModelResolutionParams,
    free_only: bool,
    vis: bool,
    manual_enforced_id: Option<&str>,
) -> bool {
    match &cfg.tier {
        TierProfile::Automatic => {}
        TierProfile::Manual(id) => {
            // Only pin when the manual id exists in the registry; unknown ids degrade so callers
            // can still reach Thompson / free-tier fallbacks (see P12 wiring tests).
            if manual_enforced_id == Some(id.as_str()) && &m.id != id {
                return false;
            }
        }
        TierProfile::BringYourOwnKey { provider } => {
            let needle = provider.to_ascii_lowercase();
            if !m.provider.to_ascii_lowercase().contains(&needle) {
                return false;
            }
        }
    }
    if free_only && !m.is_free {
        return false;
    }
    if vis && !m.capabilities.supports_vision {
        return false;
    }
    if cfg.modalities.web_search && !m.capabilities.supports_web_search {
        return false;
    }
    if params.privacy_requires_local && !model_supports_privacy_local_inference(m) {
        return false;
    }
    true
}

fn model_matches_task_strength(m: &ModelSpec, task: TaskCategory) -> bool {
    let strength = task_category_strength(task);
    m.strengths
        .iter()
        .any(|s| *s == strength || *s == StrengthTag::Generalist)
}

pub fn resolve_model_with_registry_fallbacks(
    models: &ModelRegistry,
    cost_preference_override: Option<CostPreference>,
    mut cfg: InferenceConfig,
    user_prompt: &str,
    preferred_id: Option<&str>,
    params: RegistryModelResolutionParams,
    availability_hint: Option<&[RemainingBudget]>,
) -> Result<(ModelSpec, bool), String> {
    let (vis, web) = infer_prompt_capability_hints(user_prompt);
    cfg.modalities.vision |= vis;
    cfg.modalities.web_search |= web;
    if params.force_web_search_for_task || params.task == TaskCategory::Research {
        cfg.modalities.web_search = true;
    }
    let free_only = cfg.is_free_only();
    if let Some(id) = preferred_id.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(spec) = models.get(id) {
            if params.privacy_requires_local && !model_supports_privacy_local_inference(&spec) {
                return Err(format!(
                    "Model override '{id}' is not eligible for privacy-local routing"
                ));
            }
            return Ok((spec, free_only));
        }
        return Err(format!(
            "Model override '{id}' is not in the registry; clear the override or pick a valid id from the model list"
        ));
    }
    if let Some(pin) =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRoutingHardPinModel).expose()
    {
        let p = pin.trim();
        if !p.is_empty() {
            if let Some(spec) = models.get(p) {
                if params.privacy_requires_local && !model_supports_privacy_local_inference(&spec) {
                    return Err(
                        "Pinned routing model is not eligible for privacy-local routing".into(),
                    );
                }
                return Ok((spec, free_only));
            }
        }
    }
    let complexity = params.complexity.clamp(1, 10);
    let preference = cost_preference_override.unwrap_or_else(|| cfg.quality.to_cost_preference());

    let manual_enforced_id: Option<&str> = match &cfg.tier {
        TierProfile::Manual(id) if models.get(id).is_some() => Some(id.as_str()),
        _ => None,
    };

    let mut registry_pred =
        |m: &ModelSpec| inference_predicate(m, &cfg, &params, free_only, vis, manual_enforced_id);

    let mut selected = models.best_for_with_filter(
        params.task,
        complexity,
        preference,
        &mut registry_pred,
        None,
    );

    selected = selected.or_else(|| {
        let candidates: Vec<ModelSpec> = models
            .list_models()
            .into_iter()
            .filter(|m| {
                inference_predicate(m, &cfg, &params, free_only, vis, manual_enforced_id)
                    && model_matches_task_strength(m, params.task)
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        if candidates.len() == 1 {
            return Some(candidates.into_iter().next().expect("len 1"));
        }
        let mut engine = crate::routing::ModelSelectionEngine::new(None);
        let arm_stats = models.arm_stats_snapshot().clone();
        engine.pick_with_auto_score_thompson(
            &candidates,
            params.task,
            complexity,
            params.free_tier_latency_critical,
            params.context_fill_ratio,
            preference,
            availability_hint,
            &arm_stats,
            0,
        )
    });

    selected = selected.or_else(|| {
        models.best_free_for_with_filter(params.task, |m| {
            inference_predicate(m, &cfg, &params, true, vis, manual_enforced_id)
        })
    });

    if params.allow_cheapest_fallback {
        selected = selected.or_else(|| models.cheapest());
    }
    let model = selected.ok_or_else(|| "No models available in registry".to_string())?;
    emit_legacy_decision_event(&params, preferred_id, &cfg, &model);
    Ok((model, free_only))
}

/// Emit a [`vox_telemetry::SelectionDecisionEvent`] from the legacy resolver
/// so its decisions show up in the same L3 council report as the new
/// `models::select::select()` path.
///
/// `intent_caller` is set to `"legacy_registry_resolve"` to disambiguate
/// from the new SSOT in dashboards. Axes are reconstructed from the
/// `InferenceConfig.quality` knob; the legacy resolver doesn't carry the
/// 3-axis user knob, so this is a best-effort projection.
fn emit_legacy_decision_event(
    params: &RegistryModelResolutionParams,
    preferred_id: Option<&str>,
    cfg: &InferenceConfig,
    model: &ModelSpec,
) {
    use vox_telemetry::{SelectionDecisionEvent, TelemetryEvent};

    let reason = if preferred_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some()
    {
        "preferred_id"
    } else if vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRoutingHardPinModel)
        .expose()
        .map(|p| !p.trim().is_empty())
        .unwrap_or(false)
    {
        "hard_pin_secret"
    } else {
        "scored"
    };
    // Approximate axes from the legacy CostPreference projection.
    let cost_pref = cfg.quality.to_cost_preference();
    let axes = match cost_pref {
        CostPreference::Economy => (70u8, 15u8, 15u8),
        CostPreference::Performance => (15u8, 15u8, 70u8),
    };
    let event = SelectionDecisionEvent {
        intent_caller: Some("legacy_registry_resolve".to_string()),
        task: format!("{:?}", params.task).to_ascii_lowercase(),
        axes,
        chosen_model: model.id.clone(),
        reason: reason.to_string(),
        premium_alias_key: None,
        repository_id: None,
    };
    vox_telemetry::record_event!(&TelemetryEvent::SelectionDecision(event));
}
