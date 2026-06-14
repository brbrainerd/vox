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
            row("fast/model", Some(200)),    // excellent -> 1.0
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

/// Adversarial tests for registry invariants, cost calculation, penalty logic,
/// scoreboard routing, and pricing-source trust hierarchy.
#[cfg(test)]
mod semcov_wave34_tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use crate::config::CostPreference;
    use crate::models::registry::{ModelRegistry, ModelScore};
    use crate::models::scoring::{
        budget_match, efficiency_score, latency_score, model_budget_hint, quality_score,
        scoreboard_feedback_boost, throughput_score,
    };
    use crate::models::spec::{ModelCapabilities, PricingSource};
    use crate::models::{ModelSpec, ProviderType, StrengthTag};
    use crate::types::TaskCategory;
    use crate::usage::RemainingBudget;

    const D_5MS: std::time::Duration = std::time::Duration::from_millis(5);
    const D_60S: std::time::Duration = std::time::Duration::from_secs(60);

    // ── helpers ────────────────────────────────────────────────────────────────

    fn spec(id: &str, cost: f64, is_free: bool) -> ModelSpec {
        ModelSpec {
            id: id.into(),
            canonical_slug: id.into(),
            provider: "test".into(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 8192,
            cost_per_1k: cost,
            cost_per_1k_input: cost,
            cost_per_1k_output: cost,
            is_free,
            observed_cost_per_1k: None,
            strengths: vec![StrengthTag::Codegen, StrengthTag::Generalist],
            capabilities: ModelCapabilities::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    fn score_row(
        _model_id: &str,
        success_rate: f64,
        quality: f64,
        p50: Option<i64>,
        n: i64,
    ) -> ModelScore {
        ModelScore {
            success_rate,
            quality_score: quality,
            cost_per_success_usd: None,
            p50_latency_ms: p50,
            n_calls: n,
        }
    }

    fn remaining_budget(
        provider: &str,
        model: &str,
        remaining: u32,
        rate_limited: bool,
    ) -> RemainingBudget {
        RemainingBudget {
            provider: provider.into(),
            model: model.into(),
            calls_used: 10,
            daily_limit: 100,
            remaining,
            cost_today: 0.5,
            rate_limited,
        }
    }

    // ── 1. penalty expiry ──────────────────────────────────────────────────────

    #[test]
    fn penalty_expires_after_duration_elapses() {
        // Catches: is_penalized never clearing, treating expired entries as still active
        let mut r = ModelRegistry::default();
        r.record_penalty(
            "model-x".into(),
            TaskCategory::CodeGen,
            Duration::from_millis(1),
        );
        // Give the penalty a moment to expire (it's 1 ms, so sleep is safe here).
        std::thread::sleep(D_5MS);
        assert!(
            !r.is_penalized("model-x", TaskCategory::CodeGen),
            "expired penalty must not block the model"
        );
    }

    #[test]
    fn penalty_blocks_model_during_active_window() {
        // Catches: is_penalized returning false for a freshly set penalty
        let mut r = ModelRegistry::default();
        r.record_penalty(
            "blocked".into(),
            TaskCategory::Research,
            D_60S,
        );
        assert!(
            r.is_penalized("blocked", TaskCategory::Research),
            "active penalty must block model"
        );
        // Different category — must NOT be penalized
        assert!(
            !r.is_penalized("blocked", TaskCategory::CodeGen),
            "penalty is category-scoped"
        );
    }

    #[test]
    fn penalty_does_not_affect_different_model() {
        // Catches: penalty key collision (e.g., using only category as key)
        let mut r = ModelRegistry::default();
        r.record_penalty(
            "alpha".into(),
            TaskCategory::CodeGen,
            D_60S,
        );
        assert!(
            !r.is_penalized("beta", TaskCategory::CodeGen),
            "penalty on alpha must not leak to beta"
        );
    }

    // ── 2. cost_estimate correctness ───────────────────────────────────────────

    #[test]
    fn cost_estimate_scales_linearly_with_token_count() {
        // Catches: off-by-1000 divisor, integer truncation in cost formula
        let mut r = ModelRegistry::default();
        r.register(spec("cheap/model", 2.0, false));
        let c1k = r.cost_estimate("cheap/model", 1_000).unwrap();
        let c2k = r.cost_estimate("cheap/model", 2_000).unwrap();
        assert!(
            (c1k - 2.0).abs() < 1e-9,
            "1 000 tokens at $2/1k -> $2.00, got {c1k}"
        );
        assert!((c2k - 4.0).abs() < 1e-9, "2 000 tokens -> $4.00, got {c2k}");
    }

    #[test]
    fn cost_estimate_returns_none_for_unknown_model() {
        // Catches: panicking or returning 0.0 for missing model instead of None
        let r = ModelRegistry::default();
        assert!(
            r.cost_estimate("does/not/exist", 1_000).is_none(),
            "unknown model must return None"
        );
    }

    #[test]
    fn cost_estimate_zero_for_free_model() {
        // Catches: free-model guard missing → billing phantom cost
        let mut r = ModelRegistry::default();
        r.register(spec("free/model", 0.0, true));
        let cost = r.cost_estimate("free/model", 100_000).unwrap();
        assert_eq!(cost, 0.0, "free model must always cost $0");
    }

    // ── 3. Telemetry pricing-source immunity ───────────────────────────────────

    #[test]
    fn register_does_not_overwrite_telemetry_pricing() {
        // Catches: re-register clobbering Telemetry price with a stale Bootstrap value
        let mut r = ModelRegistry::default();
        let mut s = spec("telem/model", 0.005, false);
        s.pricing_source = PricingSource::Telemetry;
        r.register(s);

        // Try to overwrite with a cheap Bootstrap price
        let cheap = spec("telem/model", 0.001, false); // Bootstrap by default
        r.register(cheap);

        let got = r.get("telem/model").unwrap();
        assert_eq!(
            got.pricing_source,
            PricingSource::Telemetry,
            "Telemetry source must survive re-register"
        );
        assert!(
            (got.cost_per_1k - 0.005).abs() < 1e-9,
            "Telemetry price must not be overwritten by Bootstrap re-register"
        );
    }

    #[test]
    fn apply_litellm_pricing_does_not_downgrade_telemetry_source() {
        // Catches: apply_litellm_pricing skipping the Telemetry guard, replacing observed prices
        use crate::catalog::LiteLLMPricingEntry;

        let mut r = ModelRegistry::default();
        let mut s = spec("vendor/model", 0.05, false);
        s.pricing_source = PricingSource::Telemetry;
        r.register(s);

        let mut entries = HashMap::new();
        entries.insert(
            "vendor/model".to_string(),
            LiteLLMPricingEntry {
                cost_per_1k_input: Some(0.001),
                cost_per_1k_output: Some(0.001),
                cache_creation_cost_per_1k: None,
                cache_read_cost_per_1k: None,
                supports_prompt_caching: None,
                litellm_provider: None,
            },
        );
        r.apply_litellm_pricing(&entries);

        let got = r.get("vendor/model").unwrap();
        assert_eq!(
            got.pricing_source,
            PricingSource::Telemetry,
            "apply_litellm_pricing must not downgrade Telemetry to LiteLLM"
        );
        assert!(
            (got.cost_per_1k - 0.05).abs() < 1e-9,
            "Telemetry price must be preserved"
        );
    }

    // ── 4. scoreboard bandit arm tracking ─────────────────────────────────────

    #[test]
    fn record_bandit_outcome_increments_correct_counter() {
        // Catches: success/failure counter swapped, saturating_add not used → panic on overflow
        let mut r = ModelRegistry::default();
        r.record_bandit_outcome("m", true);
        r.record_bandit_outcome("m", true);
        r.record_bandit_outcome("m", false);
        let stats = r.arm_stats_snapshot();
        assert_eq!(stats["m"], (2, 1), "2 successes, 1 failure");
    }

    #[test]
    fn record_bandit_outcome_saturates_at_u32_max() {
        // Catches: wrapping_add panic in release mode; saturating_add must be used
        let mut r = ModelRegistry::default();
        r.inject_arm_stats({
            let mut m = HashMap::new();
            m.insert("edge".to_string(), (u32::MAX, 0));
            m
        });
        // Must not panic
        r.record_bandit_outcome("edge", true);
        let stats = r.arm_stats_snapshot();
        assert_eq!(
            stats["edge"].0,
            u32::MAX,
            "saturating_add must not overflow"
        );
    }

    // ── 5. agent override round-trip ───────────────────────────────────────────

    #[test]
    fn agent_override_round_trips_and_is_isolated_per_agent() {
        // Catches: overrides stored globally instead of per-agent-id
        let mut r = ModelRegistry::default();
        r.set_override(1, "model-a".into());
        r.set_override(2, "model-b".into());
        assert_eq!(r.get_override(1).as_deref(), Some("model-a"));
        assert_eq!(r.get_override(2).as_deref(), Some("model-b"));
        assert!(r.get_override(99).is_none(), "unknown agent returns None");
    }

    // ── 6. efficiency_score boundaries ────────────────────────────────────────

    #[test]
    fn efficiency_score_is_one_for_free_model() {
        // Catches: division-by-zero or wrong branch when cost == 0.0
        let s = spec("free/x", 0.0, true);
        assert_eq!(efficiency_score(&s), 1.0, "zero cost -> max efficiency");
    }

    #[test]
    fn efficiency_score_strictly_decreases_as_cost_rises() {
        // Catches: inverted cost formula (higher cost returns higher score)
        let cheap = spec("cheap", 0.001, false);
        let expensive = spec("pricey", 10.0, false);
        assert!(
            efficiency_score(&cheap) > efficiency_score(&expensive),
            "cheaper model must score higher efficiency"
        );
    }

    #[test]
    fn efficiency_score_uses_blended_input_output_when_present() {
        // Catches: blended path ignoring cost_per_1k_input/output, defaulting to legacy field
        let mut s = spec("asym", 0.0, false); // cost_per_1k = 0 (would give 1.0)
        s.cost_per_1k_input = 5.0;
        s.cost_per_1k_output = 5.0;
        let score = efficiency_score(&s);
        assert!(
            score < 1.0,
            "non-zero input+output cost must reduce efficiency below 1.0 (got {score})"
        );
    }

    // ── 7. quality_score token scaling ────────────────────────────────────────

    #[test]
    fn quality_score_clamps_to_unit_interval() {
        // Catches: log10 producing values > 1.0 that escape the clamp
        let mut small = spec("tiny", 0.0, true);
        small.max_tokens = 1;
        let mut huge = spec("huge", 0.0, false);
        huge.max_tokens = u64::MAX;
        let qs = quality_score(&small);
        let qh = quality_score(&huge);
        assert!(
            (0.0..=1.0).contains(&qs),
            "quality_score for tiny out of range: {qs}"
        );
        assert!(
            (0.0..=1.0).contains(&qh),
            "quality_score for huge out of range: {qh}"
        );
    }

    #[test]
    fn quality_score_free_vs_paid_ordering() {
        // Catches: free-model penalty ignored (both models return identical quality_score)
        let free_m = spec("f", 0.0, true);
        let mut paid_m = spec("p", 0.01, false);
        paid_m.max_tokens = free_m.max_tokens; // same tokens, only free flag differs
        let qf = quality_score(&free_m);
        let qp = quality_score(&paid_m);
        assert!(
            qp > qf,
            "paid model must outscore free model (same token count): {qp} vs {qf}"
        );
    }

    // ── 8. budget_match wildcard semantics ────────────────────────────────────

    #[test]
    fn budget_match_wildcard_matches_any_model() {
        // Catches: wildcard "*" treated as a literal model id, never matching
        assert!(
            budget_match("*", "any/model:free"),
            "* must match any model"
        );
        assert!(budget_match("*", "another"), "* must match another");
    }

    #[test]
    fn budget_match_free_suffix_matches_only_free_models() {
        // Catches: :free sentinel matching paid model ids (regression in suffix check)
        assert!(
            budget_match(":free", "qwen/qwen3:free"),
            ":free must match :free models"
        );
        assert!(
            !budget_match(":free", "qwen/qwen3"),
            ":free must NOT match paid model"
        );
    }

    // ── 9. model_budget_hint aggregation ──────────────────────────────────────

    #[test]
    fn model_budget_hint_picks_max_remaining_across_matching_rows() {
        // Catches: returning first-match instead of max remaining capacity
        let s = spec("openrouter/model-x", 0.01, false);
        let hints = vec![
            remaining_budget("openrouter", "openrouter/model-x", 5, false),
            remaining_budget("openrouter", "openrouter/model-x", 50, false),
            remaining_budget("openrouter", "openrouter/model-x", 20, false),
        ];
        let (rem, rl) = model_budget_hint(&s, Some(&hints));
        assert_eq!(rem, 50, "must take max remaining across matching rows");
        assert!(!rl);
    }

    #[test]
    fn model_budget_hint_returns_zero_when_no_matching_provider() {
        // Catches: leaking budget from wrong provider into the model's hint.
        // GoogleDirect models get provider="google" in their LlmUsageKey; a hint
        // scoped to provider="openrouter" must NOT match them.
        let mut s = spec("gemini-2.0-flash-lite", 0.0, true);
        s.provider_type = ProviderType::GoogleDirect;
        let hints = vec![remaining_budget("openrouter", "*", 99, false)];
        let (rem, rl) = model_budget_hint(&s, Some(&hints));
        assert_eq!(rem, 0, "provider mismatch must yield 0 remaining");
        assert!(!rl);
    }

    // ── 10. scoreboard_feedback_boost zero-guard ───────────────────────────────

    #[test]
    fn scoreboard_feedback_boost_returns_zero_when_no_score() {
        // Catches: None score returning a non-zero constant that pollutes routing
        let s = spec("m", 0.01, false);
        let cfg = vox_config::load_model_routing_config();
        let boost = scoreboard_feedback_boost(&s, None, &cfg.quality_weights);
        assert_eq!(boost, 0.0, "no scoreboard data must give zero boost");
    }

    #[test]
    fn scoreboard_feedback_boost_returns_zero_for_zero_calls() {
        // Catches: n_calls guard missing → division-by-zero or phantom boost from empty scoreboard
        let s = spec("m", 0.01, false);
        let sc = score_row("m", 1.0, 1.0, Some(100), 0); // n_calls = 0
        let cfg = vox_config::load_model_routing_config();
        let boost = scoreboard_feedback_boost(&s, Some(&sc), &cfg.quality_weights);
        assert_eq!(boost, 0.0, "zero n_calls must give zero boost");
    }

    #[test]
    fn scoreboard_feedback_boost_is_positive_for_good_model() {
        // Catches: weight normalization bug that produces negative boost for high-quality models
        let s = spec("star", 0.005, false);
        let sc = score_row("star", 1.0, 1.0, Some(200), 10);
        let cfg = vox_config::load_model_routing_config();
        let boost = scoreboard_feedback_boost(&s, Some(&sc), &cfg.quality_weights);
        assert!(
            boost > 0.0,
            "perfect success/quality score must yield a positive boost (got {boost})"
        );
    }

    // ── 11. registry clear / empty invariants ─────────────────────────────────

    #[test]
    fn clear_removes_all_models_including_registered_ones() {
        // Catches: clear() only resetting an internal flag but not the HashMap
        let mut r = ModelRegistry::default();
        r.register(spec("a", 0.01, false));
        r.register(spec("b", 0.02, false));
        r.clear();
        assert!(
            r.list_models().is_empty(),
            "clear() must empty the registry"
        );
        assert!(r.get("a").is_none(), "get() must return None after clear");
        assert!(
            r.cheapest().is_none(),
            "cheapest() must return None on empty registry"
        );
    }

    // ── 12. throughput_score fallback and cap ─────────────────────────────────

    #[test]
    fn throughput_score_caps_at_one_for_very_high_rpm() {
        // Catches: missing upper clamp → score > 1.0 inflating composite score
        let mut s = spec("firehose", 0.0, true);
        s.capabilities.rate_limit_rpm = Some(u32::MAX);
        let ts = throughput_score(&s);
        assert!(
            ts <= 1.0,
            "throughput_score must be clamped to ≤ 1.0, got {ts}"
        );
    }

    #[test]
    fn throughput_score_uses_fallback_when_rpm_absent() {
        // Catches: returning 0.0 instead of the documented THROUGHPUT_FALLBACK_RPM default
        let s = spec("unknown-rpm", 0.0, true);
        let ts = throughput_score(&s);
        assert!(ts > 0.0, "missing RPM must use fallback (>0), got {ts}");
        assert!(ts <= 1.0, "fallback must also be ≤ 1.0, got {ts}");
    }

    // ── 13. best_for returns None on empty registry ───────────────────────────

    #[test]
    fn best_for_returns_none_when_registry_is_empty() {
        // Catches: unwrap() or default fallback returning a ghost spec on empty registry
        let mut r = ModelRegistry::default();
        r.clear();
        let result = r.best_for(TaskCategory::CodeGen, 5, CostPreference::Economy);
        assert!(
            result.is_none(),
            "empty registry must return None from best_for"
        );
    }

    // ── 14. inject_pricing_catalog confidence gate ────────────────────────────

    #[test]
    fn inject_pricing_catalog_ignores_low_confidence_rows() {
        // Catches: confidence guard absent → low-confidence stale data overwriting valid prices
        use vox_db::store::types::ModelPricingCatalogRow;

        let mut r = ModelRegistry::default();
        r.register(spec("p/model", 0.10, false));

        let row = ModelPricingCatalogRow {
            model_id: "p/model".into(),
            provider: "test".into(),
            confidence: "low".into(),
            observed_blended_per_1k: Some(0.001), // would be a big downgrade
            observed_input_per_1k: None,
            observed_output_per_1k: None,
            catalog_input_per_1k: 0.0,
            catalog_output_per_1k: 0.0,
            n_provider_reported: 0,
            n_estimated: 0,
            n_free: 0,
            last_observed_at_ms: None,
            updated_at_ms: 0,
        };
        r.inject_pricing_catalog(vec![row]);

        let got = r.get("p/model").unwrap();
        assert!(
            (got.cost_per_1k - 0.10).abs() < 1e-9,
            "low-confidence row must not overwrite existing price (got {})",
            got.cost_per_1k
        );
    }

    #[test]
    fn inject_pricing_catalog_applies_high_confidence_rows() {
        // Catches: confidence guard rejecting high-confidence telemetry data
        use vox_db::store::types::ModelPricingCatalogRow;

        let mut r = ModelRegistry::default();
        r.register(spec("q/model", 0.10, false));

        let row = ModelPricingCatalogRow {
            model_id: "q/model".into(),
            provider: "test".into(),
            confidence: "high".into(),
            observed_blended_per_1k: Some(0.03),
            observed_input_per_1k: None,
            observed_output_per_1k: None,
            catalog_input_per_1k: 0.0,
            catalog_output_per_1k: 0.0,
            n_provider_reported: 0,
            n_estimated: 0,
            n_free: 0,
            last_observed_at_ms: None,
            updated_at_ms: 0,
        };
        r.inject_pricing_catalog(vec![row]);

        let got = r.get("q/model").unwrap();
        assert!(
            (got.cost_per_1k - 0.03).abs() < 1e-9,
            "high-confidence row must update the price (got {})",
            got.cost_per_1k
        );
    }

    // ── 15. latency score boundary at excellent/poor thresholds ──────────────

    #[test]
    fn latency_score_boundary_excellent_gives_one() {
        // Catches: off-by-one at threshold boundary returning < 1.0 at exactly excellent_ms
        let cfg = vox_config::load_model_routing_config();
        let excellent_ms = cfg.latency_bands.excellent_ms as u32;
        let mut s = spec("at-excellent", 0.0, true);
        s.capabilities.latency_p50_ms = Some(excellent_ms);
        let score = latency_score(&s);
        assert_eq!(
            score, 1.0,
            "p50 == excellent_ms must give score 1.0 (got {score})"
        );
    }

    #[test]
    fn latency_score_at_poor_threshold_gives_zero() {
        // Catches: >= comparison replaced with > causing score > 0.0 at the poor boundary
        let cfg = vox_config::load_model_routing_config();
        let poor_ms = cfg.latency_bands.poor_ms as u32;
        let mut s = spec("at-poor", 0.0, true);
        s.capabilities.latency_p50_ms = Some(poor_ms);
        let score = latency_score(&s);
        assert_eq!(
            score, 0.0,
            "p50 == poor_ms must give score 0.0 (got {score})"
        );
    }

    // ── 16. inject_scoreboard_latency clamps i64 to u32 without panic ─────────

    #[test]
    fn inject_scoreboard_latency_clamps_extreme_i64_to_u32_max() {
        // Catches: unchecked i64→u32 cast overflowing/panicking for very large latency values
        use vox_db::store::types::ModelScoreboardRow;

        let mut r = ModelRegistry::default();
        r.register(spec("slow-outlier", 0.0, true));

        let row = ModelScoreboardRow {
            model_id: "slow-outlier".into(),
            task_category: "code_gen".into(),
            strength_tag: "general".into(),
            window_days: 7,
            n_calls: 1,
            success_rate: 1.0,
            p50_latency_ms: Some(i64::MAX), // far beyond u32::MAX
            p99_latency_ms: None,
            cost_per_success_usd: None,
            quality_score: 1.0,
            updated_at_ms: 0,
            success_count: 1,
            cumulative_cost_usd: 0.0,
        };
        let updated = r.inject_scoreboard_latency(&[row]);
        assert_eq!(updated, 1, "extreme positive i64 must still be accepted");
        // Must not panic; value is clamped to u32::MAX
        let got = r.get("slow-outlier").unwrap();
        assert_eq!(
            got.capabilities.latency_p50_ms,
            Some(u32::MAX),
            "i64::MAX must clamp to u32::MAX"
        );
    }
}
