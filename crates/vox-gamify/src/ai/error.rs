use thiserror::Error;

// ─── Errors ──────────────────────────────────────────────

/// Errors produced by the AI client.
#[derive(Debug, Error)]
pub enum AiError {
    /// All configured providers were exhausted without producing a response.
    #[error("All AI providers failed: {0}")]
    AllProvidersFailed(String),
    /// A network-level HTTP error occurred.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// The provider returned malformed JSON.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    /// The provider returned a 200 response with no content.
    #[error("Empty response from provider")]
    EmptyResponse,
    /// The provider returned a 429 Too Many Requests response.
    #[error("Rate limited by provider: {provider} (retry after {retry_after_secs:?}s)")]
    RateLimited {
        /// Name of the provider that returned the rate limit.
        provider: String,
        /// Seconds until the rate limit expires, if provided by the server.
        retry_after_secs: Option<u64>,
    },
    /// The provider returned a non-2xx HTTP status other than 429 (e.g. 402 quota
    /// exhausted, 500 server error). The body is not valid generated text.
    #[error("Provider {provider} returned HTTP {status}")]
    ProviderStatus {
        /// Name of the provider that returned the error status.
        provider: String,
        /// The HTTP status code.
        status: u16,
    },
}
