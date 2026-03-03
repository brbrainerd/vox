//! Error types for `vox-forge`.

use thiserror::Error;

/// Errors that can occur when talking to a Git forge.
#[derive(Debug, Error)]
pub enum ForgeError {
    /// HTTP request failed.
    #[error("HTTP error {status}: {message}")]
    Http { status: u16, message: String },

    /// The forge API returned a rate-limit response.
    #[error("Rate limited by forge (retry after {retry_after_secs}s)")]
    RateLimited { retry_after_secs: u64 },

    /// Authentication failed (bad token, expired, missing scope).
    #[error("Authentication failed: {reason}")]
    Unauthorized { reason: String },

    /// The requested resource was not found.
    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    /// The operation is not supported by this forge.
    #[error("Operation not supported by {forge}: {operation}")]
    Unsupported { forge: String, operation: String },

    /// JSON deserialization error.
    #[error("Failed to parse forge response: {0}")]
    Parse(#[from] serde_json::Error),

    /// Network/transport error.
    #[error("Network error: {0}")]
    Network(String),

    /// Any other error.
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl ForgeError {
    /// Returns `true` if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited { .. } => true,
            Self::Network(_) => true,
            Self::Http { status, .. } if *status >= 500 => true,
            _ => false,
        }
    }
}
