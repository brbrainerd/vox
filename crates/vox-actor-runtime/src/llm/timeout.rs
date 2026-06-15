//! SSOT resolver for the per-request LLM HTTP timeout.
//!
//! Precedence: explicit positive `LlmConfig.timeout_ms` → shared
//! `vox_config::timeouts::HTTP_REQUEST`. A `timeout_ms` of `0` (or unset) falls
//! back to the SSOT default, since `0` would make every request time out instantly.
//! Applied to unary chat/embed calls only (streaming is excluded — a whole-request
//! deadline would cut off long SSE streams).

use std::time::Duration;

use super::types::LlmConfig;

/// Resolve the request timeout for a unary LLM call.
pub(crate) fn request_timeout(config: &LlmConfig) -> Duration {
    match config.timeout_ms {
        Some(ms) if ms > 0 => Duration::from_millis(ms),
        _ => vox_config::timeouts::HTTP_REQUEST,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(timeout_ms: Option<u64>) -> LlmConfig {
        let mut c = LlmConfig::openrouter("test-model");
        c.timeout_ms = timeout_ms;
        c
    }

    #[test]
    fn explicit_timeout_is_used() {
        assert_eq!(
            request_timeout(&cfg(Some(5_000))),
            Duration::from_millis(5_000)
        );
    }

    #[test]
    fn falls_back_to_ssot_default_when_unset() {
        assert_eq!(
            request_timeout(&cfg(None)),
            vox_config::timeouts::HTTP_REQUEST
        );
    }

    #[test]
    fn zero_timeout_falls_back_to_ssot_default() {
        // `Some(0)` would make every request time out instantly; fall back to default.
        assert_eq!(
            request_timeout(&cfg(Some(0))),
            vox_config::timeouts::HTTP_REQUEST
        );
    }
}
