//! HTTP inference errors for LLM providers.

/// Non-success or transport error from an LLM HTTP call.
#[derive(Debug)]
pub(crate) struct HttpInferError {
    /// HTTP status, or `0` for transport / configuration errors.
    pub status: u16,
    pub message: String,
    /// When `true`, the error signals that this adapter cannot handle the request
    /// (e.g. capability mismatch), and the caller should retry with a different adapter.
    /// When `false`, this is a genuine transport or API error that should propagate.
    pub is_capability_gap: bool,
}

impl HttpInferError {
    /// Constructor for ordinary HTTP/transport errors. Sets `is_capability_gap = false`.
    pub(crate) fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            is_capability_gap: false,
        }
    }

    /// Constructor for provider capability gaps (caller should retry with a different adapter).
    pub(crate) fn capability_gap(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            message: message.into(),
            is_capability_gap: true,
        }
    }
}

impl std::fmt::Display for HttpInferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.status == 0 {
            return write!(f, "{}", self.message);
        }
        write!(f, "LLM API error {}: {}", self.status, self.message)
    }
}
