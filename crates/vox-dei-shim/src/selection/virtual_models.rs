use vox_config::{OPENROUTER_AUTO, OPENROUTER_FREE};
use vox_orchestrator::models::{
    ModelCapabilities, ModelSpec, ModelTier, PricingSource, ProviderType, StrengthTag,
};

/// Virtual/synthetic models. Merged into registry when conditions are met.
/// Single definition for openrouter/auto, openrouter/free, and any future virtual models.
pub fn virtual_models() -> Vec<ModelSpec> {
    vec![
        ModelSpec {
            id: OPENROUTER_AUTO.to_string(),
            canonical_slug: String::new(),
            provider: "openrouter".to_string(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 65_536,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            cost_per_1k: 0.0,
            observed_cost_per_1k: None,
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::Bootstrap,
            is_free: false,
            supported_parameters: vec![
                "max_tokens".to_string(),
                "temperature".to_string(),
                "tools".to_string(),
                "tool_choice".to_string(),
                "reasoning".to_string(),
                "web_search_options".to_string(),
            ],
            strengths: vec![
                StrengthTag::Codegen,
                StrengthTag::Debugging,
                StrengthTag::Research,
                StrengthTag::Review,
            ],
            capabilities: ModelCapabilities {
                supports_vision: true,
                supports_json: true,
                supports_native_tools: true,
                supports_tool_use: true,
                supports_reasoning: true,
                supports_web_search: true,
                supports_image_generation: false,
                supports_audio_input: false,
                supports_audio_output: false,
                supports_file_input: false,
                supports_jsonl: false,
                max_context: 1_000_000,
                tier: ModelTier::Pro,
                rate_limit_rpm: None,
                rate_limit_rpd: None,
                latency_p50_ms: None,
                is_moderated: false,
                uptime_score: None,
            },
        },
        ModelSpec {
            id: OPENROUTER_FREE.to_string(),
            canonical_slug: String::new(),
            provider: "openrouter".to_string(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 65_536,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            cost_per_1k: 0.0,
            observed_cost_per_1k: None,
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::Bootstrap,
            is_free: true,
            supported_parameters: vec!["max_tokens".to_string(), "temperature".to_string()],
            strengths: vec![
                StrengthTag::Codegen,
                StrengthTag::Debugging,
                StrengthTag::Research,
                StrengthTag::Review,
            ],
            capabilities: ModelCapabilities {
                supports_vision: true,
                supports_json: true,
                supports_native_tools: true,
                supports_tool_use: true,
                supports_reasoning: true,
                supports_web_search: false,
                supports_image_generation: false,
                supports_audio_input: false,
                supports_audio_output: false,
                supports_file_input: false,
                supports_jsonl: false,
                max_context: 1_000_000,
                tier: ModelTier::Free,
                // OpenRouter aggregates free-tier limits; treat as conservative placeholders.
                rate_limit_rpm: Some(20),
                rate_limit_rpd: Some(50),
                latency_p50_ms: None,
                is_moderated: false,
                uptime_score: None,
            },
        },
    ]
}

/// Returns the `openrouter/auto` virtual model if OpenRouter models exist in the registry.
pub fn openrouter_auto_model(has_openrouter_models: bool) -> Option<ModelSpec> {
    if has_openrouter_models {
        virtual_models().into_iter().next()
    } else {
        None
    }
}

/// Returns the `openrouter/free` virtual model if OpenRouter models exist in the registry.
pub fn openrouter_free_model(has_openrouter_models: bool) -> Option<ModelSpec> {
    if has_openrouter_models {
        virtual_models().into_iter().nth(1)
    } else {
        None
    }
}
