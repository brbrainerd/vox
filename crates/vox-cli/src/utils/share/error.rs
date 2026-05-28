//! Error types for `vox share`.

use thiserror::Error;

pub type ShareResult<T> = Result<T, ShareError>;

#[derive(Debug, Error)]
pub enum ShareError {
    #[error("backend `{0}` is not available: {1}")]
    BackendUnavailable(&'static str, String),

    #[error("tunnel creation failed: {0}")]
    TunnelCreate(String),

    #[error("tunnel disconnected unexpectedly: {0}")]
    TunnelDisconnected(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("config: {0}")]
    Config(String),

    #[error("invalid backend: {0}")]
    InvalidBackend(String),
}
