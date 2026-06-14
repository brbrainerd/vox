use crate::models::{ModelSpec, StrengthTag};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
struct ClassificationResponse {
    /// True if the provider is highly reliable, maintains uptime > 99%, and supports the model cleanly.
    is_stable: bool,
    /// Number between 0.0 and 1.0 expressing uptime/health.
    uptime_score: f32,
    /// Refined list of strengths based on meta-analysis.
    #[serde(default)]
    refined_strengths: Vec<StrengthTag>,
}

/// Applies meta-model classification to dynamically tag model strengths and populate uptime_score.
/// In a real implementation, this would use a fast, cheap model (e.g. Haiku or Flash) to review
/// the incoming catalog metadata and refine the `strengths` array, acting as an AI orchestrator.
/// For now, we simulate this layer by enriching known missing data fields with heuristic health checks.
pub async fn classify_models(models: &mut [ModelSpec]) {
    // If the user explicitly disabled the classifier, no-op.
    if vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenRouterClassifierEnabled)
        .expose()
        .unwrap_or("1")
        == "0"
    {
        return;
    }

    // Simulate API batch processing: in a real implementation we would send batch requests
    // to `OpenRouter/auto` asking an LLM to evaluate the metadata of `models` and return JSON.
    // Here we inject an uptime score based on the provider string as a stand-in for the classifier.

    for m in models.iter_mut() {
        // Only classify if it doesn't already have an uptime score from the catalog.
        if m.capabilities.uptime_score.is_none() {
            // Apply heuristic meta-tagging for uptime.
            let health = match m.provider.as_str() {
                "openai" | "anthropic" | "google" => 0.99,
                "deepseek" => 0.95,
                "openrouter" => 0.99,
                "groq" => 0.98,
                "together" => 0.97,
                _ => 0.85,
            };
            m.capabilities.uptime_score = Some(health);
        }

        // Meta-classification: if model is extremely large context, tag as 'long-context'
        if m.max_tokens >= 128_000 && !m.strengths.contains(&StrengthTag::LongContext) {
            m.strengths.push(StrengthTag::LongContext);
        }
    }
}

#[cfg(test)]
mod semcov_wave1b_tests {
    #![allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn classify_models_tags_uptime_and_long_context() {
        fn fixture(provider: &str, max_tokens: u64) -> crate::models::ModelSpec {
            crate::models::ModelSpec {
                id: "test/model".into(),
                canonical_slug: "test/model".into(),
                provider: provider.into(),
                provider_type: crate::models::ProviderType::OpenRouter,
                max_tokens,
                cost_per_1k: 0.0,
                cost_per_1k_input: 0.0,
                cost_per_1k_output: 0.0,
                is_free: true,
                observed_cost_per_1k: None,
                strengths: vec![crate::models::StrengthTag::Codegen],
                capabilities: crate::models::spec::ModelCapabilities::default(),
                cache_creation_cost_per_1k: 0.0,
                cache_read_cost_per_1k: 0.0,
                supports_prompt_caching: false,
                pricing_source: crate::models::spec::PricingSource::Bootstrap,
                supported_parameters: vec![],
            }
        }

        // Ensure the classifier is enabled regardless of host env. nextest runs each
        // test in its own process, so this env mutation does not leak across tests.
        unsafe { std::env::set_var("VOX_OPEN_ROUTER_CLASSIFIER_ENABLED", "1") };

        let mut models = vec![fixture("openai", 200_000)];
        classify_models(&mut models).await;

        let m = &models[0];
        // Provider "openai" with no catalog uptime -> heuristic 0.99.
        let uptime = m.capabilities.uptime_score.expect("uptime_score populated");
        assert!(
            (uptime - 0.99).abs() < 1e-6,
            "openai heuristic uptime should be 0.99, got {uptime}"
        );

        // max_tokens >= 128_000 -> LongContext tagged exactly once.
        let long_ctx = m
            .strengths
            .iter()
            .filter(|s| **s == crate::models::StrengthTag::LongContext)
            .count();
        assert_eq!(long_ctx, 1, "LongContext should be pushed exactly once");
    }
}
