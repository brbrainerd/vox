//! Central model selection: policy + Thompson exploration on top of registry auto-scoring.

use std::collections::HashMap;
use std::sync::OnceLock;

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::config::CostPreference;
use crate::models::ModelSpec;
use crate::models::ModelScore;
use crate::models::ModelTier;
use crate::models::scoring::auto_score_model;
use crate::types::TaskCategory;
use crate::usage::RemainingBudget;

use super::bandit::sample_beta_thompson;
use super::policy::RoutingPolicy;

fn prefer_reasoning_from_secrets() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    *CELL.get_or_init(|| {
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCapabilityPreferReasoning)
            .expose()
            .map(|s| s.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

/// Why this model arm was chosen (for telemetry / outcome recorder).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickReason {
    Manual,
    PolicyExploit,
    PolicyExplore,
    Fallback,
}

/// Engine state: embedded routing policy + RNG for Thompson draws.
pub struct ModelSelectionEngine {
    policy: RoutingPolicy,
    rng: StdRng,
}

impl ModelSelectionEngine {
    /// `seed`: deterministic testing; `None` uses thread RNG as seed source.
    #[must_use]
    pub fn new(seed: Option<u64>) -> Self {
        let rng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_rng(rand::thread_rng())
                .unwrap_or_else(|_| StdRng::seed_from_u64(0xdecafbad)),
        };
        Self {
            policy: RoutingPolicy::load(),
            rng,
        }
    }

    #[must_use]
    pub fn policy(&self) -> &RoutingPolicy {
        &self.policy
    }

    /// Among `candidates`, combine [`auto_score_model`] with a Thompson draw on per-arm Beta
    /// posteriors keyed by `model.id` (successes, failures). Empty `arm_stats` → uninformative prior.
    #[must_use]
    pub fn pick_with_auto_score_thompson(
        &mut self,
        candidates: &[ModelSpec],
        task: TaskCategory,
        complexity: u8,
        free_tier_latency_critical: bool,
        context_fill_ratio: Option<f32>,
        preference: CostPreference,
        availability_hint: Option<&[RemainingBudget]>,
        arm_stats: &HashMap<String, (u32, u32)>,
        scoreboard: &HashMap<String, ModelScore>,
        novel_explores_so_far: u32,
    ) -> Option<ModelSpec> {
        if candidates.is_empty() {
            return None;
        }
        if candidates.len() == 1 {
            return Some(candidates[0].clone());
        }
        let exp = &self.policy.exploration;
        let mut best_i = 0usize;
        let mut best_score = f64::NEG_INFINITY;
        let mut any_explore = false;
        for (i, m) in candidates.iter().enumerate() {
            let sb = scoreboard.get(&m.id);
            let mut base = auto_score_model(
                m,
                complexity,
                free_tier_latency_critical,
                context_fill_ratio,
                preference,
                availability_hint,
                sb,
            );
            if prefer_reasoning_from_secrets() && m.capabilities.supports_reasoning {
                base += 0.02;
            }
            let (s, f) = arm_stats.get(&m.id).copied().unwrap_or((0, 0));
            // Free-by-default: Free and Fast tiers participate equally in the
            // novelty exploration bonus so new free models get a fair trial.
            if self.policy.routing_objective.kind == "quality_first"
                && s + f == 0
                && matches!(
                    m.capabilities.tier,
                    ModelTier::Pro | ModelTier::Fast | ModelTier::Free
                )
            {
                base += 0.06;
            }
            let draw = sample_beta_thompson(&mut self.rng, s, f);
            let lam = if s + f < exp.min_samples_per_arm {
                exp.epsilon_ceiling
            } else {
                exp.epsilon_floor
            };
            if s + f < exp.min_samples_per_arm {
                any_explore = true;
            }
            let mut total = base + lam * draw * 10.0;
            let allow_novel_boost = self.policy.routing_objective.kind == "quality_first"
                && (s + f) == 0
                && novel_explores_so_far < exp.max_concurrent_explorations;
            if allow_novel_boost {
                total *= 1.0 + exp.exploration_boost_for_novel;
            }
            if total > best_score {
                best_score = total;
                best_i = i;
            }
        }
        tracing::debug!(
            target: "vox_routing::decision",
            task = ?task,
            picked_id = %candidates[best_i].id,
            best_score,
            explore_prior = any_explore,
            "model_selection_engine pick"
        );
        Some(candidates[best_i].clone())
    }

    /// Pick reason for telemetry (best-effort: explore if any arm was under-sampled).
    #[must_use]
    pub fn last_pick_reason_for_stats(
        candidates: &[ModelSpec],
        best: &ModelSpec,
        arm_stats: &HashMap<String, (u32, u32)>,
        min_samples: u32,
    ) -> PickReason {
        let (s, f) = arm_stats.get(&best.id).copied().unwrap_or((0, 0));
        if s + f < min_samples {
            return PickReason::PolicyExplore;
        }
        if candidates.iter().any(|m| {
            m.id != best.id
                && arm_stats
                    .get(&m.id)
                    .is_none_or(|&(a, b)| a + b < min_samples)
        }) {
            return PickReason::PolicyExplore;
        }
        PickReason::PolicyExploit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::spec::PricingSource;
    use crate::models::{ModelCapabilities, ModelSpec, ProviderType};

    fn dummy_model(id: &str, cost: f64) -> ModelSpec {
        ModelSpec {
            id: id.to_string(),
            canonical_slug: id.to_string(),
            provider: "test".to_string(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 4096,
            cost_per_1k: cost,
            cost_per_1k_input: cost,
            cost_per_1k_output: cost,
            is_free: false,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: ModelCapabilities::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    #[test]
    fn engine_picks_one_of_two() {
        let mut eng = ModelSelectionEngine::new(Some(7));
        let a = dummy_model("a/x", 0.01);
        let b = dummy_model("b/y", 0.02);
        let stats = HashMap::new();
        let picked = eng
            .pick_with_auto_score_thompson(
                &[a.clone(), b.clone()],
                TaskCategory::CodeGen,
                5,
                false,
                None,
                CostPreference::Performance,
                None,
                &stats,
                &HashMap::new(),
                0,
            )
            .expect("pick");
        assert!(picked.id == "a/x" || picked.id == "b/y");
    }
}
