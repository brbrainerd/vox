#[cfg(test)]
mod llm_usage_key_tests {
    use crate::models::{ModelSpec, ProviderType};
    use crate::usage::LlmUsageKey;

    #[test]
    fn openrouter_free_maps_to_aggregate_free_bucket() {
        let m = ModelSpec {
            id: "qwen/qwen3-coder:free".into(),
            canonical_slug: "qwen-free".into(),
            provider: "qwen".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 1,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        };
        assert_eq!(
            m.llm_usage_key(),
            LlmUsageKey {
                provider: "openrouter".into(),
                model: ":free".into(),
            }
        );
    }

    #[test]
    fn openrouter_paid_uses_full_model_id() {
        let m = ModelSpec {
            id: "anthropic/claude-sonnet-4.5".into(),
            canonical_slug: "claude".into(),
            provider: "anthropic".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 1,
            cost_per_1k: 0.01,
            cost_per_1k_input: 0.01,
            cost_per_1k_output: 0.01,
            is_free: false,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        };
        assert_eq!(
            m.llm_usage_key(),
            LlmUsageKey {
                provider: "openrouter".into(),
                model: "anthropic/claude-sonnet-4.5".into(),
            }
        );
    }

    #[test]
    fn ollama_maps_to_star_model() {
        let m = ModelSpec {
            id: "llama3.2".into(),
            canonical_slug: "llama".into(),
            provider: "ollama".into(),
            provider_type: ProviderType::Ollama,
            max_tokens: 1,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        };
        assert_eq!(
            m.llm_usage_key(),
            LlmUsageKey {
                provider: "ollama".into(),
                model: "*".into(),
            }
        );
    }

    #[test]
    fn google_direct_uses_google_provider_and_model_id() {
        let m = ModelSpec {
            id: "gemini-2.0-flash-lite".into(),
            canonical_slug: "gemini".into(),
            provider: "google".into(),
            provider_type: ProviderType::GoogleDirect,
            max_tokens: 1,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        };
        assert_eq!(
            m.llm_usage_key(),
            LlmUsageKey {
                provider: "google".into(),
                model: "gemini-2.0-flash-lite".into(),
            }
        );
    }
}

#[cfg(test)]
mod key_guard_tests {
    use crate::config::CostPreference;
    use crate::models::ModelRegistry;
    use crate::types::TaskCategory;

    #[test]
    fn premium_alias_resolves_to_available_model_when_anthropic_key_absent() {
        // SAFETY: standard test env modification
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("VOX_ANTHROPIC_API_KEY");
        }
        let registry = ModelRegistry::new();
        // Default codegen premium-alias is `anthropic/claude-opus-4.7` (GA, 2026-Q2 refresh).
        // Mythos preview was retired from the catalog 2026-05-15.

        let best = registry.best_for(TaskCategory::CodeGen, 5, CostPreference::Performance);
        assert!(
            best.is_some(),
            "Should find a fallback model even if key is missing"
        );

        // Without an Anthropic API key, the router must NOT pick a direct-Anthropic model
        // (Opus 4.7 / Haiku 4.5 / Sonnet 4.6 via Anthropic Direct). It should fall through
        // to an OpenRouter or open-source rank-matched paid model.
        let m = best.unwrap();
        assert_ne!(
            m.id, "claude-mythos-preview-20260407",
            "Mythos preview was retired; should not appear in the registry at all"
        );
        assert_ne!(
            m.id, "anthropic/claude-opus-4.7",
            "Should not pick a direct-Anthropic model when Anthropic API key is missing"
        );
    }
}

#[cfg(test)]
mod premium_alias_tests {
    use crate::models::ModelConfig;
    use std::collections::HashSet;

    #[test]
    fn default_premium_alias_targets_exist_in_models_list() {
        let cfg = ModelConfig::default();
        let ids: HashSet<_> = cfg.models.iter().map(|m| m.id.as_str()).collect();
        for (k, v) in &cfg.premium_alias {
            assert!(
                ids.contains(v.as_str()),
                "premium_alias {k} -> {v} not in default models list"
            );
        }
    }
}

#[cfg(test)]
mod registry_filter_tests {
    use crate::config::CostPreference;
    use crate::models::{ModelRegistry, ModelSpec, ProviderType};
    use crate::types::TaskCategory;

    #[test]
    fn best_free_for_with_filter_skips_ollama() {
        let mut r = ModelRegistry::default();
        r.register(ModelSpec {
            id: "llama-local".into(),
            canonical_slug: "llama-local".into(),
            provider: "ollama".into(),
            provider_type: ProviderType::Ollama,
            max_tokens: 8192,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![crate::models::generated::StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        });
        r.register(ModelSpec {
            id: "gemini-2.0-flash-lite".into(),
            canonical_slug: "gemini-2.0-flash-lite".into(),
            provider: "google".into(),
            provider_type: ProviderType::GoogleDirect,
            max_tokens: 1_000_000,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![crate::models::generated::StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        });
        let picked = r
            .best_for_with_filter(
                TaskCategory::CodeGen,
                2,
                CostPreference::Economy,
                |m| m.is_free && !matches!(m.provider_type, ProviderType::Ollama),
                None,
            )
            .expect("non-ollama free");
        assert_eq!(picked.id, "gemini-2.0-flash-lite");
    }

    #[test]
    fn cheapest_free_with_filter_stable_tiebreak_on_id() {
        let mut r = ModelRegistry::default();
        r.register(ModelSpec {
            id: "z-free".into(),
            canonical_slug: "z-free".into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 1000,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![crate::models::generated::StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        });
        r.register(ModelSpec {
            id: "a-free".into(),
            canonical_slug: "a-free".into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 1000,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![crate::models::generated::StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        });
        let picked = r.cheapest_free_with_filter(|_| true).expect("free model");
        assert_eq!(picked.id, "a-free");
    }

    #[test]
    fn cheapest_with_filter_stable_tiebreak_on_id() {
        let mut r = ModelRegistry::default();
        r.register(ModelSpec {
            id: "z-paid".into(),
            canonical_slug: "z-paid".into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 1000,
            cost_per_1k: 0.01,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: false,
            observed_cost_per_1k: None,
            strengths: vec![crate::models::generated::StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        });
        r.register(ModelSpec {
            id: "a-paid".into(),
            canonical_slug: "a-paid".into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 1000,
            cost_per_1k: 0.01,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: false,
            observed_cost_per_1k: None,
            strengths: vec![crate::models::generated::StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        });
        let picked = r.cheapest_with_filter(|_| true).expect("paid model");
        assert_eq!(picked.id, "a-paid");
    }
}
