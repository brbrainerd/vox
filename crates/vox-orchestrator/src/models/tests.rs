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

#[cfg(test)]
mod scoreboard_latency_injection_tests {
    use crate::models::scoring::latency_score;
    use crate::models::{ModelRegistry, ModelSpec, ProviderType};
    use vox_db::store::types::ModelScoreboardRow;

    fn spec(id: &str) -> ModelSpec {
        ModelSpec {
            id: id.into(),
            canonical_slug: id.into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
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
        }
    }

    fn row(model_id: &str, p50: Option<i64>) -> ModelScoreboardRow {
        ModelScoreboardRow {
            model_id: model_id.into(),
            task_category: "code_gen".into(),
            strength_tag: "general".into(),
            window_days: 7,
            n_calls: 42,
            success_rate: 0.9,
            p50_latency_ms: p50,
            p99_latency_ms: p50.map(|v| v * 2),
            cost_per_success_usd: Some(0.01),
            quality_score: 1.0,
            updated_at_ms: 0,
            success_count: 38,
            cumulative_cost_usd: 0.4,
        }
    }

    #[test]
    fn inject_scoreboard_latency_updates_capability_and_drives_latency_score() {
        let mut r = ModelRegistry::default();
        r.register(spec("fast/model"));
        r.register(spec("slow/model"));

        // Before injection: no measured p50 -> scorer falls back to a provider constant.
        let before = latency_score(&r.get("fast/model").unwrap());

        let rows = vec![
            row("fast/model", Some(200)),   // excellent -> 1.0
            row("slow/model", Some(20_000)), // beyond poor band -> 0.0
            row("unknown/model", Some(300)), // not in registry -> ignored
        ];
        let updated = r.inject_scoreboard_latency(&rows);
        assert_eq!(updated, 2, "only the two registered models are updated");

        let fast = r.get("fast/model").unwrap();
        assert_eq!(fast.capabilities.latency_p50_ms, Some(200));
        assert_eq!(
            latency_score(&fast),
            1.0,
            "measured p50=200ms now drives the score to 1.0"
        );
        // The static-field path is now data-backed; pre-injection fallback differed.
        assert!(latency_score(&fast) >= before - f64::EPSILON);

        let slow = r.get("slow/model").unwrap();
        assert_eq!(slow.capabilities.latency_p50_ms, Some(20_000));
        assert_eq!(latency_score(&slow), 0.0, "measured slow p50 -> score 0.0");
    }

    #[test]
    fn inject_scoreboard_latency_skips_missing_and_nonpositive_p50() {
        let mut r = ModelRegistry::default();
        r.register(spec("m1"));
        r.register(spec("m2"));

        let rows = vec![row("m1", None), row("m2", Some(0))];
        let updated = r.inject_scoreboard_latency(&rows);
        assert_eq!(updated, 0, "None and <=0 p50 are ignored");
        assert_eq!(r.get("m1").unwrap().capabilities.latency_p50_ms, None);
        assert_eq!(r.get("m2").unwrap().capabilities.latency_p50_ms, None);
    }
}
