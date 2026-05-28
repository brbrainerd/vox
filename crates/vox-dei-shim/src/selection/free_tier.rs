use vox_orchestrator::config::CostPreference;
use vox_orchestrator::models::{ModelSpec, ModelTier, ProviderType};
use vox_orchestrator::types::TaskCategory;

use super::scorer::{ModelScorer, ScoreParams};
use super::weights::ScoringWeights;

/// Request parameters for the `FreeTierRouter`.
#[derive(Debug, Clone, Default)]
pub struct FreeTierRouteRequest {
    /// The category of work (CodeGen, Debugging, etc.)
    pub task: TaskCategory,
    /// Minimum context window required (tokens).
    pub context_tokens: u64,
    /// Whether vision support is strictly required.
    pub requires_vision: bool,
    /// Whether structured output (JSON) is strictly required.
    pub requires_structured_output: bool,
    /// Whether fill-in-the-middle (FIM) support is requested (routes to Mistral/Codestral).
    pub requires_fill_in_middle: bool,
    /// Whether low-latency inference is prioritized (routes to Cerebras/Groq).
    pub latency_critical: bool,
    /// Maximum number of candidates to return for parallel dispatch.
    pub max_candidates: usize,
}

/// A prioritized model candidate for free-tier routing.
#[derive(Debug, Clone)]
pub struct RouteCandidate {
    /// The model specification.
    pub model: ModelSpec,
    /// The provider type for this candidate.
    pub provider_type: ProviderType,
    /// Human-readable rationale for this selection (shown in `vox provider status`).
    pub rationale: &'static str,
}

/// Intelligent router that selects the best free-tier models based on capability constraints.
///
/// It delegates scoring to `ModelScorer` but provides a high-level API for multi-candidate
/// routing and specialized constraint handling (e.g. FIM, Scout-speed).
#[derive(Debug, Clone, Default)]
pub struct FreeTierRouter {
    /// Configurable weights for scoring.
    pub weights: ScoringWeights,
}

impl FreeTierRouter {
    /// Construct a new router with default weights.
    pub fn new() -> Self {
        Self::default()
    }

    /// Select the best available free-tier models for the request.
    ///
    /// Returns a list of candidates ordered by suitability. Non-satisfying models are filtered.
    pub fn route(&self, req: &FreeTierRouteRequest, models: &[ModelSpec]) -> Vec<RouteCandidate> {
        let scorer = ModelScorer {
            weights: self.weights.clone(),
        };

        let mut candidates: Vec<(f64, RouteCandidate)> = models
            .iter()
            .filter(|m| m.is_free)
            .filter_map(|m| {
                // Apply hard constraints
                if req.requires_vision && !m.capabilities.supports_vision {
                    return None;
                }
                if req.requires_structured_output && !m.capabilities.supports_json {
                    return None;
                }
                if req.context_tokens > 0 && m.capabilities.max_context < req.context_tokens {
                    return None;
                }

                // Specialized FIM routing: only route to providers with native FIM support.
                // Mistral (Codestral) is the gold standard; DeepSeek also supports FIM natively.
                if req.requires_fill_in_middle
                    && !matches!(
                        m.provider_type,
                        ProviderType::Mistral | ProviderType::DeepSeek
                    )
                {
                    return None;
                }

                // Initial score from standard model scorer
                let mut score = scorer.score(
                    m,
                    ScoreParams {
                        task_type: req.task,
                        effective_pref: CostPreference::Economy,
                        free_only: true,
                        requires_vision: req.requires_vision,
                        ..Default::default()
                    },
                );

                if !score.is_finite() {
                    return None;
                }

                let mut rationale = "General free-tier candidate";

                // Latency bonus for Fast-tier models (Cerebras, Groq)
                if req.latency_critical && m.capabilities.tier == ModelTier::Fast {
                    score += 5.0;
                    rationale = "Candidate selected for ultra-low latency";
                }

                // Preference for direct providers when keys are likely set
                if m.provider_type == ProviderType::Cerebras && req.latency_critical {
                    rationale = "Best-in-class performance (2600 tok/s)";
                } else if m.provider_type == ProviderType::Mistral && req.requires_fill_in_middle {
                    rationale = "Specialized code completion (FIM)";
                } else if m.provider_type == ProviderType::DeepSeek
                    && req.task == TaskCategory::CodeGen
                {
                    rationale = "Top-tier coding reasoning (DeepSeek V3)";
                } else if m.provider_type == ProviderType::SambaNova && m.id.contains("405B") {
                    rationale = "Large-scale reasoning (405B model)";
                } else if m.capabilities.supports_vision && req.requires_vision {
                    rationale = "Vision-capable free model";
                } else if m.capabilities.max_context >= 1_000_000 {
                    rationale = "Extraordinary context window (1M tokens)";
                }

                Some((
                    score,
                    RouteCandidate {
                        model: m.clone(),
                        provider_type: m.provider_type.clone(),
                        rationale,
                    },
                ))
            })
            .collect();

        // Sort by score descending
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let max = if req.max_candidates > 0 {
            req.max_candidates
        } else {
            1
        };
        candidates.into_iter().take(max).map(|(_, c)| c).collect()
    }
}
