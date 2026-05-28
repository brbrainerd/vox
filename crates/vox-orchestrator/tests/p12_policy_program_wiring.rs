//! P12 orchestrator-policy wiring smoke tests (contract loaders + routing helpers).

use std::collections::HashMap;

use vox_orchestrator::calibration::{BanditArm, ContextualBandit, arm_stats_from_bandit};
use vox_orchestrator::circuit_breaker::CircuitBreakerState;
use vox_orchestrator::models::{
    ModelCapabilities, ModelRegistry, ModelSpec, PricingSource, ProviderType, StrengthTag,
};
use vox_orchestrator::privacy_router::{
    PrivacyLevel, PrivacyRouter, PrivacyRoutingPolicy, model_supports_privacy_local_inference,
};
use vox_orchestrator::types::TaskCategory;
use vox_orchestrator::{
    InferenceConfig, OrchestrationFeatureFlags, OrchestratorPolicy, OrchestratorPolicyConfig,
    PolicyContext, TierProfile,
};
use vox_orchestrator::{RegistryModelResolutionParams, resolve_model_with_registry_fallbacks};

fn dummy_llm(id: &str, provider_type: ProviderType, provider: &str) -> ModelSpec {
    ModelSpec {
        id: id.to_string(),
        canonical_slug: id.to_string(),
        provider: provider.to_string(),
        provider_type,
        max_tokens: 8192,
        cost_per_1k: 0.01,
        cost_per_1k_input: 0.01,
        cost_per_1k_output: 0.01,
        observed_cost_per_1k: None,
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: PricingSource::Bootstrap,
        is_free: false,
        strengths: vec![StrengthTag::Codegen],
        capabilities: ModelCapabilities::default(),
        supported_parameters: vec![],
    }
}

#[test]
fn embedded_feature_flags_parse_false_defaults() {
    let f = OrchestrationFeatureFlags::from_embedded_contract();
    assert!(!f.contextual_bandit_enabled());
}

#[test]
fn dei_resolve_respects_registry_arm_stats_shape() {
    let mut reg = ModelRegistry::new();
    reg.clear();
    reg.register(dummy_llm("cloud-a", ProviderType::OpenRouter, "openrouter"));
    reg.register(dummy_llm("cloud-b", ProviderType::OpenRouter, "openrouter"));
    let mut stats = HashMap::new();
    stats.insert("cloud-a".to_string(), (10_u32, 0_u32));
    reg.inject_arm_stats(stats);

    // Force the Thompson exploration branch (skip `best_for_with_filter` primary path).
    let cfg = InferenceConfig {
        tier: TierProfile::Manual("definitely-not-in-registry".into()),
        ..InferenceConfig::default()
    };
    let params = RegistryModelResolutionParams {
        task: TaskCategory::CodeGen,
        complexity: 5,
        ..Default::default()
    };
    let resolved =
        resolve_model_with_registry_fallbacks(&reg, None, cfg, "hello world", None, params, None)
            .expect("resolve");
    assert!(
        matches!(resolved.0.id.as_str(), "cloud-a" | "cloud-b"),
        "Thompson exploration should return one of the filtered codegen models; got {}",
        resolved.0.id
    );
    assert!(
        reg.arm_stats_snapshot().contains_key("cloud-a"),
        "resolution path must use injected registry arm stats (non-empty snapshot)"
    );
}

#[test]
fn privacy_requires_local_filters_cloud_candidates() {
    let mut reg = ModelRegistry::new();
    reg.clear();
    reg.register(dummy_llm("cloud-a", ProviderType::OpenRouter, "openrouter"));
    let mut ollama = dummy_llm("local-a", ProviderType::Ollama, "ollama");
    ollama.is_free = true;
    reg.register(ollama);

    let cfg = InferenceConfig::default();
    let params = RegistryModelResolutionParams {
        task: TaskCategory::CodeGen,
        complexity: 5,
        privacy_requires_local: true,
        ..Default::default()
    };
    let resolved =
        resolve_model_with_registry_fallbacks(&reg, None, cfg, "hello world", None, params, None)
            .expect("resolve");
    assert_eq!(resolved.0.id, "local-a");
}

#[test]
fn bandit_arm_stats_map_matches_engine_contract() {
    let bandit = ContextualBandit::new(vec![BanditArm::new("m-a"), BanditArm::new("m-b")]);
    let m = arm_stats_from_bandit(&bandit);
    assert_eq!(m.len(), 2);
}

#[test]
fn privacy_router_local_inference_predicate_matches_ollama() {
    let m = dummy_llm("x", ProviderType::Ollama, "ollama");
    assert!(model_supports_privacy_local_inference(&m));
    let router = PrivacyRouter::new(PrivacyRoutingPolicy::default());
    let kept = router.filter_models(
        PrivacyLevel::Private,
        vec![
            dummy_llm("r", ProviderType::OpenRouter, "openrouter"),
            dummy_llm("l", ProviderType::Ollama, "ollama"),
        ],
    );
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].id, "l");
}

#[test]
fn orchestrator_policy_embedded_defaults_are_non_tripping() {
    let mut p = OrchestratorPolicy::new(OrchestratorPolicyConfig::default());
    let d = p.evaluate(&PolicyContext {
        circuit_breaker: CircuitBreakerState {
            no_progress_loops: 9,
            ..Default::default()
        },
        ..PolicyContext::default()
    });
    assert!(d.circuit_trip.is_none());
}
