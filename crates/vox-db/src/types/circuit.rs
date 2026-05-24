//! Circuit-breaker state enum used by `vox_db::circuit_breaker::DbCircuitBreaker`.
//!
//! Moved from `vox_db::circuit_breaker` so consumers can pattern-match on the
//! state without pulling in the heavy `vox-db` crate. The breaker driver
//! (`vox_db::circuit_breaker::DbCircuitBreaker`) stays in `vox-db` because it
//! owns `Arc<RwLock<...>>` and `AtomicU32`. The companion error type
//! (`CircuitBreakerError`) also stays in `vox-db` because it depends on
//! `thiserror`, which is not a `vox-db-types` dependency.

/// Three states a circuit breaker can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CircuitState {
    /// Normal operation — calls pass through.
    Closed,
    /// Tripped — calls are rejected immediately.
    Open,
    /// Probe phase — one call is allowed to test recovery.
    HalfOpen,
}
