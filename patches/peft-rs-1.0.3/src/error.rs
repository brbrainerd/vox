//! Error types for peft-rs.

use thiserror::Error;

/// Result type alias for peft-rs operations.
pub type Result<T> = std::result::Result<T, PeftError>;

/// Errors that can occur in peft-rs operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PeftError {
    /// Invalid configuration parameter.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Shape mismatch in tensor operation.
    #[error("shape mismatch: expected {expected:?}, got {actual:?}")]
    ShapeMismatch {
        /// Expected shape
        expected: Vec<usize>,
        /// Actual shape
        actual: Vec<usize>,
    },

    /// Dimension mismatch.
    #[error("dimension mismatch: {message}")]
    DimensionMismatch {
        /// Descriptive message
        message: String,
    },

    /// Adapter not found.
    #[error("adapter not found: {name}")]
    AdapterNotFound {
        /// Name of the missing adapter
        name: String,
    },

    /// Adapter already exists.
    #[error("adapter already exists: {name}")]
    AdapterExists {
        /// Name of the duplicate adapter
        name: String,
    },

    /// Weight loading error.
    #[error("failed to load weights: {0}")]
    WeightLoad(String),

    /// I/O error (file operations, serialization, etc.).
    #[error("I/O error: {0}")]
    Io(String),

    /// Device mismatch.
    #[error("device mismatch: tensors must be on the same device")]
    DeviceMismatch,

    /// Underlying candle error.
    #[error("candle error: {0}")]
    Candle(#[from] candle_core::Error),
}
