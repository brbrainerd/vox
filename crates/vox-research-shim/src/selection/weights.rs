/// Configurable scoring weights. Tune here or load from config.
#[derive(Debug, Clone)]
pub struct ScoringWeights {
    /// Bonus given if the model strengths match the task category.
    pub task_match: f64,
    /// Bonus for models that support native JSON mode.
    pub supports_json: f64,
    /// Bonus for models that support direct file input (multimodal).
    pub supports_file_input: f64,
    /// Bonus for models with vision/image capabilities.
    pub supports_vision: f64,
    /// Bonus for models with integrated web search.
    pub supports_web_search: f64,
    /// Context window bonus per 100,000 tokens of capacity.
    pub context_bonus_per_100k: f64,
    /// Hard cap on context window bonus score.
    pub max_context_cap: f64,
    /// Generation limit bonus per 16,000 tokens of output capacity.
    pub max_tokens_bonus_per_16k: f64,
    /// Hard cap on output generation bonus.
    pub max_tokens_cap: f64,
    /// Preference bonus for models routed through OpenRouter.
    pub openrouter_bonus: f64,
    /// Penalty for models using Google Direct API (encourages normalization).
    pub google_direct_penalty: f64,
    /// Slight penalty for local Ollama models (prefers cloud if budget allows).
    pub ollama_penalty: f64,
    /// Multiplier for per-token cost penalty in Economy mode.
    pub economy_cost_penalty_factor: f64,
    /// Maximum penalty deduction for high-cost models.
    pub economy_cost_cap: f64,
    /// Bonus for truly free models in Economy mode.
    pub economy_free_bonus: f64,
    /// Bonus for Pro-tier models in Performance mode.
    pub performance_pro_tier: f64,
    /// Bonus for Fast-tier models in Performance mode.
    pub performance_fast_tier: f64,
    /// Penalty for Free-tier models in Performance mode.
    pub performance_free_penalty: f64,
    /// Specific context bonus for the free-only selection pool.
    pub free_tier_context_bonus: f64,
    /// Specific JSON bonus for the free-only selection pool.
    pub free_tier_structured_output_bonus: f64,
    /// Specific vision bonus for the free-only selection pool.
    pub free_tier_vision_bonus: f64,
    /// Bonus for direct free-tier providers (Cerebras, Mistral, DeepSeek, SambaNova).
    /// Slightly below `openrouter_bonus` so OpenRouter (with its own fallback routing) is preferred
    /// when available, but above `ollama_penalty` or `google_direct_penalty`.
    pub direct_free_bonus: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            task_match: 10.0,
            supports_json: 1.0,
            supports_file_input: 1.0,
            supports_vision: 1.0,
            supports_web_search: 1.5,
            context_bonus_per_100k: 1.0,
            max_context_cap: 15.0,
            max_tokens_bonus_per_16k: 1.0,
            max_tokens_cap: 8.0,
            openrouter_bonus: 3.0,
            google_direct_penalty: -5.0,
            ollama_penalty: -1.0,
            economy_cost_penalty_factor: 10_000.0,
            economy_cost_cap: 20.0,
            economy_free_bonus: 3.0,
            performance_pro_tier: 3.0,
            performance_fast_tier: 1.0,
            performance_free_penalty: -1.0,
            free_tier_context_bonus: 0.1,
            free_tier_structured_output_bonus: 2.0,
            free_tier_vision_bonus: 1.0,
            direct_free_bonus: 2.0,
        }
    }
}
