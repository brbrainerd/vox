use vox_orchestrator::config::CostPreference;
use vox_orchestrator::mode::ResolvedClutch;
use vox_orchestrator::models::{ModelSpec, ModelTier, ProviderType};
use vox_orchestrator::types::TaskCategory;

use super::task_routing::model_matches_task;
use super::weights::ScoringWeights;

/// Select the best model from a list for a given task and preference.
pub fn select_best_model(
    models: &[ModelSpec],
    task: TaskCategory,
    pref: CostPreference,
) -> Option<ModelSpec> {
    let scorer = ModelScorer::default();
    models
        .iter()
        .map(|m| {
            (
                m,
                scorer.score(
                    m,
                    ScoreParams {
                        task_type: task,
                        effective_pref: pref,
                        ..Default::default()
                    },
                ),
            )
        })
        .filter(|(_, score)| score.is_finite())
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(m, _)| m.clone())
}

/// Pluggable model scorer with configurable weights.
#[derive(Debug, Clone, Default)]
pub struct ModelScorer {
    /// Weights used to calculate model scores.
    pub weights: ScoringWeights,
}

/// Parameters for model scoring.
#[derive(Debug, Clone, Default)]
pub struct ScoreParams {
    /// The model to score.
    pub model: Option<ModelSpec>,
    /// The category of task being performed.
    pub task_type: TaskCategory,
    /// Effective cost preference (Economy vs Performance).
    pub effective_pref: CostPreference,
    /// Whether only free models are allowed.
    pub free_only: bool,
    /// Whether vision support is required.
    pub requires_vision: bool,
    /// Whether web search support is required.
    pub requires_web_search: bool,
    /// Whether the model has an equivalent available via OpenRouter (to avoid duplicates).
    pub has_openrouter_equivalent: bool,
    /// Optional resolved clutch for quality-driven bonuses.
    pub mode: Option<ResolvedClutch>,
}

impl ModelScorer {
    /// Score a model for selection. Returns f64::NEG_INFINITY if model does not satisfy constraints.
    /// When `mode` is provided, applies mode-specific bonuses/penalties on top of cost-preference scoring.
    pub fn score(&self, model: &ModelSpec, params: ScoreParams) -> f64 {
        let mut full_params = params;
        full_params.model = Some(model.clone());
        self.score_with_mode(full_params)
    }

    /// Score a model with optional mode-aware bonuses/penalties.
    pub fn score_with_mode(&self, params: ScoreParams) -> f64 {
        let model = params
            .model
            .as_ref()
            .expect("model is required for scoring");
        let task_type = params.task_type;
        let effective_pref = params.effective_pref;
        let free_only = params.free_only;
        let requires_vision = params.requires_vision;
        let requires_web_search = params.requires_web_search;
        let has_openrouter_equivalent = params.has_openrouter_equivalent;
        let mode = params.mode;

        if vox_orchestrator::route_policy::route_policy_exclusion_reason(model).is_some() {
            return f64::NEG_INFINITY;
        }

        if free_only && !model.is_free {
            return f64::NEG_INFINITY;
        }
        if requires_vision && !model.capabilities.supports_vision {
            return f64::NEG_INFINITY;
        }
        if requires_web_search && !model.supports_web_search() {
            return f64::NEG_INFINITY;
        }
        if has_openrouter_equivalent {
            return f64::NEG_INFINITY;
        }

        let w = &self.weights;
        let mut score = 0.0;

        if model_matches_task(model, task_type) {
            score += w.task_match;
        }
        if model.capabilities.supports_json {
            score += w.supports_json;
        }
        if model.capabilities.supports_file_input {
            score += w.supports_file_input;
        }
        if model.capabilities.supports_vision {
            score += w.supports_vision;
        }
        if model.supports_web_search() {
            score += w.supports_web_search;
        }
        score += (model.capabilities.max_context as f64 / 100_000.0 * w.context_bonus_per_100k)
            .min(w.max_context_cap);
        score +=
            (model.max_tokens as f64 / 16_000.0 * w.max_tokens_bonus_per_16k).min(w.max_tokens_cap);

        match model.provider_type {
            ProviderType::OpenRouter | ProviderType::Groq => score += w.openrouter_bonus,
            ProviderType::Cerebras
            | ProviderType::Mistral
            | ProviderType::DeepSeek
            | ProviderType::SambaNova => score += w.direct_free_bonus,
            ProviderType::GoogleDirect => score += w.google_direct_penalty,
            ProviderType::Ollama | ProviderType::VoxLocal => score += w.ollama_penalty,
            ProviderType::PopuliMesh => score += w.openrouter_bonus,
            ProviderType::Anthropic | ProviderType::HuggingFaceRouter | ProviderType::Custom(_) => {
                score += w.openrouter_bonus;
            }
        }

        match effective_pref {
            CostPreference::Economy => {
                score -=
                    (model.cost_per_1k * w.economy_cost_penalty_factor).min(w.economy_cost_cap);
                if model.is_free {
                    score += w.economy_free_bonus;
                }
            }
            CostPreference::Performance => match model.capabilities.tier {
                ModelTier::Pro => score += w.performance_pro_tier,
                ModelTier::Fast => score += w.performance_fast_tier,
                ModelTier::Free => score += w.performance_free_penalty,
                // Unknown / Local / Light / Elite get no tier bonus in Performance mode.
                _ => {}
            },
        }

        if free_only {
            score +=
                (model.capabilities.max_context as f64 / 100_000.0) * w.free_tier_context_bonus;
            if model.capabilities.supports_json || model.capabilities.supports_jsonl {
                score += w.free_tier_structured_output_bonus;
            }
            if model.capabilities.supports_vision {
                score += w.free_tier_vision_bonus;
            }
        }

        // Mode-specific bonuses/penalties
        if let Some(m) = mode {
            score += Self::mode_bonus(m, model);
        }

        score
    }

    /// Quality-driven score adjustment, keyed off the resolved clutch's quality.
    ///
    /// Preserves the exact deltas of the former `ExecutionModeProfile` path:
    /// - `Flash`   → old `Fast`      bonuses (tier-keyed)
    /// - `Balanced`→ old `Efficient` bonuses (free vs. paid; `LegacyDefault` collapsed in)
    /// - `Premium` → old `Precision` bonuses (tier-keyed)
    ///
    /// The old `Verbose` profile had no `QualityLevel` equivalent and was never
    /// emitted by any production path, so it is dropped.
    fn mode_bonus(resolved: ResolvedClutch, model: &ModelSpec) -> f64 {
        use vox_orchestrator::mode::QualityLevel;
        match resolved.quality {
            QualityLevel::Balanced => {
                if model.is_free {
                    1.0
                } else {
                    -0.5
                }
            }
            QualityLevel::Flash => match model.capabilities.tier {
                ModelTier::Fast => 1.5,
                ModelTier::Free => 0.5,
                ModelTier::Pro => -0.5,
                _ => 0.0,
            },
            QualityLevel::Premium => match model.capabilities.tier {
                ModelTier::Pro => 1.0,
                ModelTier::Fast => -0.3,
                ModelTier::Free => -0.8,
                _ => 0.0,
            },
        }
    }

    /// Score a model using the full `InferenceConfig` — the canonical scoring path.
    ///
    /// Respects tier constraints, capability flags, and quality-derived cost preference.
    /// Returns `f64::NEG_INFINITY` for models that don't satisfy hard constraints.
    pub fn score_with_config(
        &self,
        model: &ModelSpec,
        task_type: TaskCategory,
        cfg: &vox_orchestrator::mode::InferenceConfig,
        has_openrouter_equivalent: bool,
    ) -> f64 {
        use vox_orchestrator::mode::{ClutchProfile, QualityLevel, TierProfile};

        // Manual tier: only score the exact model ID.
        if let TierProfile::Manual(ref id) = cfg.tier {
            return if &model.id == id {
                1_000.0
            } else {
                f64::NEG_INFINITY
            };
        }

        // BYOK tier: filter to the specified provider.
        if let TierProfile::BringYourOwnKey { ref provider } = cfg.tier
            && !model
                .provider
                .to_ascii_lowercase()
                .contains(&provider.to_ascii_lowercase())
        {
            return f64::NEG_INFINITY;
        }

        // Map the quality level to the clutch detent that resolves to the same
        // quality, preserving the former Flash→Fast / Balanced→Efficient /
        // Premium→Precision bonus mapping in `mode_bonus`.
        let mode_hint = Some(
            match cfg.quality {
                QualityLevel::Flash => ClutchProfile::Efficiency,
                QualityLevel::Balanced => ClutchProfile::Balanced,
                QualityLevel::Premium => ClutchProfile::Genius,
            }
            .resolve(),
        );

        self.score_with_mode(ScoreParams {
            model: Some(model.clone()),
            task_type,
            effective_pref: cfg.quality.to_cost_preference(),
            free_only: cfg.is_free_only(),
            requires_vision: cfg.modalities.vision,
            requires_web_search: cfg.modalities.web_search,
            has_openrouter_equivalent,
            mode: mode_hint,
        })
    }
}
