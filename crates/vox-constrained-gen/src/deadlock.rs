//! Deadlock watchdog for grammar-constrained samplers.
//!
//! Wraps any [`ConstrainedSampler`] and enforces a per-step time budget.
//! If `mask_logits` exceeds the budget, the watchdog returns a
//! [`ConstrainedGenError::Timeout`] so the caller can retry with exponential
//! backoff or fall back to unconstrained generation.
//!
//! **Research rationale (Grammar Constraints §4.4):** Deadlocks are *systemic*
//! vulnerabilities, not edge cases. A watchdog is mandatory infrastructure.

use std::time::Instant;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{ConstrainedGenError, ConstrainedSampler, Result, SamplerState};

/// Watchdog configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogConfig {
    /// Maximum wall-clock time (ms) for a single `mask_logits` call.
    pub timeout_ms: u64,
    /// Maximum consecutive deadlock/timeout errors before giving up.
    pub max_retries: usize,
    /// Base backoff delay (ms) between retries (doubled each time).
    pub backoff_base_ms: u64,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5_000,
            max_retries: 3,
            backoff_base_ms: 100,
        }
    }
}

/// Wraps a [`ConstrainedSampler`] with timeout enforcement.
pub struct DeadlockWatchdog<S: ConstrainedSampler> {
    inner: S,
    config: WatchdogConfig,
}

impl<S: ConstrainedSampler> DeadlockWatchdog<S> {
    pub fn new(inner: S, config: WatchdogConfig) -> Self {
        Self { inner, config }
    }
}

impl<S: ConstrainedSampler> ConstrainedSampler for DeadlockWatchdog<S> {
    fn mask_logits(
        &self,
        logits: &[f32],
        state: &SamplerState,
        token_strings: &[String],
    ) -> Result<(Vec<f32>, SamplerState)> {
        let start = Instant::now();

        let result = self.inner.mask_logits(logits, state, token_strings);

        let elapsed_ms = start.elapsed().as_millis() as u64;
        if elapsed_ms > self.config.timeout_ms {
            let position = match state {
                SamplerState::Earley(_) => 0,
                SamplerState::Pda(_) => 0,
                SamplerState::Empty => 0,
            };

            warn!(
                elapsed_ms,
                limit_ms = self.config.timeout_ms,
                sampler = self.inner.name(),
                "constrained-gen watchdog timeout"
            );
            return Err(ConstrainedGenError::Timeout {
                position,
                elapsed_ms,
                limit_ms: self.config.timeout_ms,
            });
        }

        // If the inner sampler deadlocked, log and propagate.
        if let Err(ConstrainedGenError::Deadlock {
            position,
            ref partial_output,
        }) = result
        {
            warn!(
                position,
                partial_len = partial_output.len(),
                sampler = self.inner.name(),
                "constrained-gen deadlock detected"
            );
        }

        result
    }

    fn initial_state(&self) -> SamplerState {
        self.inner.initial_state()
    }

    fn name(&self) -> &'static str {
        "deadlock-watchdog"
    }
}

/// Execute `mask_logits` with automatic retry on deadlock/timeout.
///
/// On each failure the function waits `config.backoff_base_ms * 2^attempt` ms
/// (capped at `config.max_retries`). If all retries are exhausted, the last
/// error is returned.
pub fn mask_logits_with_retry(
    sampler: &dyn ConstrainedSampler,
    logits: &[f32],
    state: &SamplerState,
    token_strings: &[String],
    config: &WatchdogConfig,
) -> Result<(Vec<f32>, SamplerState)> {
    let mut last_err = None;

    for attempt in 0..=config.max_retries {
        match sampler.mask_logits(logits, state, token_strings) {
            Ok(result) => return Ok(result),
            Err(e @ ConstrainedGenError::Deadlock { .. })
            | Err(e @ ConstrainedGenError::Timeout { .. }) => {
                warn!(
                    attempt,
                    max = config.max_retries,
                    error = %e,
                    "constrained-gen retry"
                );
                last_err = Some(e);

                if attempt < config.max_retries {
                    let delay = config.backoff_base_ms * (1u64 << attempt.min(10));
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
            }
            Err(e) => return Err(e), // Non-retryable errors propagate immediately
        }
    }

    Err(last_err.unwrap_or_else(|| {
        ConstrainedGenError::Internal("retry loop exited without result".into())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::earley::EarleySampler;

    #[test]
    fn watchdog_wraps_earley() {
        let inner = EarleySampler::from_vox_grammar().unwrap();
        let watchdog = DeadlockWatchdog::new(inner, WatchdogConfig::default());
        assert_eq!(watchdog.name(), "deadlock-watchdog");

        let state = watchdog.initial_state();
        assert!(matches!(state, SamplerState::Earley(_)));
    }

    #[test]
    fn watchdog_passes_through_valid_mask() {
        let inner = EarleySampler::from_vox_grammar().unwrap();
        let watchdog = DeadlockWatchdog::new(inner, WatchdogConfig::default());
        let state = watchdog.initial_state();
        let logits = vec![1.0, 2.0];
        let tokens = vec!["fn".to_string(), "???".to_string()];
        let result = watchdog.mask_logits(&logits, &state, &tokens);
        assert!(result.is_ok());
    }

    #[test]
    fn watchdog_config_defaults() {
        let cfg = WatchdogConfig::default();
        assert_eq!(cfg.timeout_ms, 5_000);
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.backoff_base_ms, 100);
    }
}

#[cfg(test)]
mod semcov_wave4_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::{
        ConstrainedGenError, ConstrainedSampler, Result, SamplerState,
        deadlock::{WatchdogConfig, mask_logits_with_retry},
    };

    /// A sampler that always returns a Deadlock error.
    struct AlwaysDeadlockSampler;
    impl ConstrainedSampler for AlwaysDeadlockSampler {
        fn mask_logits(
            &self,
            _logits: &[f32],
            _state: &SamplerState,
            _token_strings: &[String],
        ) -> Result<(Vec<f32>, SamplerState)> {
            Err(ConstrainedGenError::Deadlock {
                position: 0,
                partial_output: "partial".to_string(),
            })
        }
        fn initial_state(&self) -> SamplerState {
            SamplerState::Empty
        }
        fn name(&self) -> &'static str {
            "always-deadlock"
        }
    }

    /// A sampler that always returns a non-retryable (Internal) error.
    struct AlwaysInternalErrorSampler;
    impl ConstrainedSampler for AlwaysInternalErrorSampler {
        fn mask_logits(
            &self,
            _logits: &[f32],
            _state: &SamplerState,
            _token_strings: &[String],
        ) -> Result<(Vec<f32>, SamplerState)> {
            Err(ConstrainedGenError::Internal("fatal".to_string()))
        }
        fn initial_state(&self) -> SamplerState {
            SamplerState::Empty
        }
        fn name(&self) -> &'static str {
            "always-internal-error"
        }
    }

    /// A sampler that succeeds on the first call.
    struct AlwaysSucceedSampler;
    impl ConstrainedSampler for AlwaysSucceedSampler {
        fn mask_logits(
            &self,
            logits: &[f32],
            _state: &SamplerState,
            _token_strings: &[String],
        ) -> Result<(Vec<f32>, SamplerState)> {
            Ok((logits.to_vec(), SamplerState::Empty))
        }
        fn initial_state(&self) -> SamplerState {
            SamplerState::Empty
        }
        fn name(&self) -> &'static str {
            "always-succeed"
        }
    }

    #[test]
    fn retry_returns_ok_on_success() {
        let sampler = AlwaysSucceedSampler;
        let config = WatchdogConfig {
            timeout_ms: 5_000,
            max_retries: 0,
            backoff_base_ms: 0,
        };
        let logits = vec![1.0f32, 2.0];
        let tokens = vec!["fn".to_string(), "let".to_string()];
        let result =
            mask_logits_with_retry(&sampler, &logits, &SamplerState::Empty, &tokens, &config);
        assert!(result.is_ok(), "expected Ok from successful sampler");
        let (masked, _) = result.unwrap();
        assert_eq!(masked, logits);
    }

    #[test]
    fn retry_exhausts_on_deadlock_and_returns_last_error() {
        let sampler = AlwaysDeadlockSampler;
        // max_retries=0: one attempt, no sleep needed
        let config = WatchdogConfig {
            timeout_ms: 5_000,
            max_retries: 0,
            backoff_base_ms: 0,
        };
        let logits = vec![1.0f32];
        let tokens: Vec<String> = vec![];
        let result =
            mask_logits_with_retry(&sampler, &logits, &SamplerState::Empty, &tokens, &config);
        assert!(
            matches!(result, Err(ConstrainedGenError::Deadlock { .. })),
            "expected Deadlock after exhausting retries"
        );
    }

    #[test]
    fn retry_propagates_non_retryable_error_immediately() {
        let sampler = AlwaysInternalErrorSampler;
        // Even with max_retries=3, Internal errors propagate immediately (no retry)
        let config = WatchdogConfig {
            timeout_ms: 5_000,
            max_retries: 3,
            backoff_base_ms: 0,
        };
        let logits = vec![1.0f32];
        let tokens: Vec<String> = vec![];
        let result =
            mask_logits_with_retry(&sampler, &logits, &SamplerState::Empty, &tokens, &config);
        assert!(
            matches!(result, Err(ConstrainedGenError::Internal(_))),
            "expected Internal error to propagate without retry"
        );
    }

    #[test]
    fn retry_multiple_attempts_on_deadlock() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingSampler {
            count: Arc<AtomicUsize>,
        }
        impl ConstrainedSampler for CountingSampler {
            fn mask_logits(
                &self,
                _logits: &[f32],
                _state: &SamplerState,
                _token_strings: &[String],
            ) -> Result<(Vec<f32>, SamplerState)> {
                self.count.fetch_add(1, Ordering::SeqCst);
                Err(ConstrainedGenError::Deadlock {
                    position: 0,
                    partial_output: String::new(),
                })
            }
            fn initial_state(&self) -> SamplerState {
                SamplerState::Empty
            }
            fn name(&self) -> &'static str {
                "counting"
            }
        }

        let count = Arc::new(AtomicUsize::new(0));
        let sampler = CountingSampler {
            count: Arc::clone(&count),
        };
        let config = WatchdogConfig {
            timeout_ms: 5_000,
            max_retries: 2,
            backoff_base_ms: 0, // no sleep
        };
        let result = mask_logits_with_retry(&sampler, &[], &SamplerState::Empty, &[], &config);
        // max_retries=2 → attempts 0,1,2 → 3 total calls
        assert_eq!(
            count.load(Ordering::SeqCst),
            3,
            "expected 3 total attempts (attempt 0..=max_retries)"
        );
        assert!(matches!(result, Err(ConstrainedGenError::Deadlock { .. })));
    }
}
