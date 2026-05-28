#[derive(Debug, Clone, Default)]
pub(crate) struct HttpCallMetadata {
    pub provider_request_id: Option<String>,
    pub provider_reported_cost_usd: Option<f64>,
    /// Tokens served from the provider's prompt cache (reads). OpenAI/DeepSeek:
    /// `prompt_tokens_details.cached_tokens`; Anthropic-native: `cache_read_input_tokens`.
    /// `None` = provider didn't report / not applicable.
    pub cache_read_input_tokens: Option<u32>,
    /// Tokens written to populate the provider's prompt cache (creation premium).
    /// Anthropic-native only; `None` for other providers.
    pub cache_creation_input_tokens: Option<u32>,
}

/// Base URL for Ollama (`OLLAMA_HOST` or Mens local default).
pub(crate) fn ollama_base_url() -> String {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOllamaHost)
        .expose()
        .map(|s| s.to_string())
        .unwrap_or_else(vox_config::inference::local_ollama_populi_base_url)
}
