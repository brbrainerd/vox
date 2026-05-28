//! Lightweight three-state circuit breaker for Turso write operations.
//!
//! Activated by `VOX_DB_CIRCUIT_BREAKER=1`. When the breaker is **Open**,
//! write callers receive [`CircuitBreakerError::Open`] immediately so they can
//! buffer locally or enqueue for retry rather than hammering a degraded primary.
//! Gated paths include coordination writes (locks, heartbeats, lineage append/prune), CAS
//! (`store`, `bind_name`, `take_db_snapshot`), agent sessions (`create_session`, `close_session`,
//! `record_agent_event`, `record_task_reliability_observation`, `append_session_event`, `log_interaction`, `submit_feedback`),
//! Codex skill manifests (`publish_skill`, `unpublish_skill`, `record_skill_execution`), **`chat_*`**
//! Codex user chat / tool calls / usage counters / topics, generic actor state (`save_actor_state_generic`),
//! registry preference bulk `DELETE`, research ingest (`knowledge_nodes` / `snippets`) and
//! `codex_capability_map` inserts, `populi_training_run` writes, legacy JSONL import row
//! `DELETE`/`INSERT` (not txn `BEGIN`/`COMMIT`/`ROLLBACK` / `PRAGMA`), `legacy_import_extras`,
//! agent memory /
//! knowledge / search-ingest / embeddings (`save_memory`, `delete_memories_created_before`,
//! `upsert_knowledge_node`, `create_knowledge_edge`, `upsert_search_document`,
//! `replace_search_document_chunks`, `store_embedding`), publication manifest writes
//! (`upsert_publication_manifest`, approvals, attempts, status events), planning graph writes,
//! news publish/approval paths, mens cloud throughput job rows, information-theoretic questioning
//! tables (sessions, events, options, `belief_state_json` merges), and Ludus / `gamify_*` CRUD
//! (profiles, quests, battles, counters, A2A inbox send/claim/ack/prune, oplog, `actor_state`,
//! file locks, teaching profiles), behavioral learning (`behavior_events`, `learned_patterns`,
//! `user_preferences`, `snippets`), durable workflow journal rows (`workflow_activity_log`), optional
//! retention `DELETE … datetime(...)` helpers, scholarly pipeline tables (submissions, media assets),
//! external submission jobs/attempts and remote snapshot/link/revision sidecars, and MCP
//! `chat_transcripts` inserts, Codex graph / research session writes (`workflow_executions`,
//! `execution_log`, `codex_change_log`, `research_sessions`, conversation tables), and research
//! metrics / endpoint reliability / eval / corpus / PM mirror package rows, build observability
//! (`build_run` / crate samples / warnings), `components` registration, TOESTUB auxiliary tables,
//! and schemaless [`crate::collection::Collection`] DML (`insert` / `patch` / `replace` / `delete`;
//! not `ensure_table` DDL).
//!
//! State machine: **Closed** → (N consecutive failures) → **Open** → (reset timeout) → **HalfOpen** → (first success) → **Closed** / (any failure) → **Open**.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub use vox_db_types::CircuitState;

/// Error emitted when the circuit is open.
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError {
    /// The circuit is open; the operation was not attempted.
    #[error("DB circuit breaker is open — too many consecutive failures")]
    Open,
}

impl From<CircuitBreakerError> for String {
    fn from(e: CircuitBreakerError) -> Self {
        e.to_string()
    }
}

/// Thread-safe circuit breaker for database write paths.
///
/// Enabled only when `VOX_DB_CIRCUIT_BREAKER=1` in the environment.
/// When disabled, [`DbCircuitBreaker::call`] always executes the action.
#[derive(Clone, Debug)]
pub struct DbCircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<AtomicU32>,
    failure_threshold: u32,
    reset_timeout: Duration,
    last_failure: Arc<RwLock<Option<Instant>>>,
    enabled: bool,
}

impl DbCircuitBreaker {
    /// Returns `true` when `VOX_DB_CIRCUIT_BREAKER=1` (or `true`).
    #[must_use]
    pub fn enabled_from_env() -> bool {
        vox_config::db_circuit_breaker_env_enabled()
    }

    /// Create with explicit settings.
    #[must_use]
    pub fn new(failure_threshold: u32, reset_timeout: Duration, enabled: bool) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(AtomicU32::new(0)),
            failure_threshold,
            reset_timeout,
            last_failure: Arc::new(RwLock::new(None)),
            enabled,
        }
    }

    /// Create from `VOX_DB_CIRCUIT_BREAKER` env with sensible defaults (5 failures, 30 s reset).
    #[must_use]
    pub fn from_env() -> Self {
        Self::new(5, vox_config::timeouts::D_30S, Self::enabled_from_env())
    }

    /// Current circuit state (without advancing the state machine).
    #[must_use]
    pub fn state(&self) -> CircuitState {
        if !self.enabled {
            return CircuitState::Closed;
        }
        let s = *self.state.read().unwrap_or_else(|p| p.into_inner());
        if s == CircuitState::Open {
            // Check if timeout has elapsed → transition to HalfOpen
            let last = self.last_failure.read().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = *last {
                if t.elapsed() >= self.reset_timeout {
                    drop(last);
                    *self.state.write().unwrap_or_else(|p| p.into_inner()) = CircuitState::HalfOpen;
                    return CircuitState::HalfOpen;
                }
            }
        }
        s
    }

    fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        *self.state.write().unwrap_or_else(|p| p.into_inner()) = CircuitState::Closed;
    }

    fn record_failure(&self) {
        let prev = self.failure_count.fetch_add(1, Ordering::Relaxed);
        *self.last_failure.write().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
        if prev + 1 >= self.failure_threshold {
            *self.state.write().unwrap_or_else(|p| p.into_inner()) = CircuitState::Open;
        }
    }

    /// Execute `action` through the breaker.
    ///
    /// - **Closed / HalfOpen**: attempt the action. Success → Closed; failure → Open.
    /// - **Open**: return `Err(CircuitBreakerError::Open)` without calling `action`.
    /// - **Disabled**: always execute; errors are passed through unchanged without tripping.
    pub async fn call<F, Fut, T, E>(&self, action: F) -> Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: From<CircuitBreakerError>,
    {
        if !self.enabled {
            return action().await;
        }
        match self.state() {
            CircuitState::Open => Err(E::from(CircuitBreakerError::Open)),
            CircuitState::Closed | CircuitState::HalfOpen => {
                let result = action().await;
                match &result {
                    Ok(_) => self.record_success(),
                    Err(_) => self.record_failure(),
                }
                result
            }
        }
    }

    /// Number of consecutive failures recorded.
    #[must_use]
    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Relaxed)
    }
}

impl Default for DbCircuitBreaker {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn closed_to_open_after_threshold() {
        let cb = DbCircuitBreaker::new(3, vox_config::timeouts::D_60S, true);
        for _ in 0..3 {
            let _: Result<(), String> = cb
                .call(|| async { Err::<(), _>(CircuitBreakerError::Open.to_string()) })
                .await;
        }
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn open_returns_error_without_calling() {
        let cb = DbCircuitBreaker::new(1, vox_config::timeouts::D_60S, true);
        // Trip it
        let _: Result<(), String> = cb.call(|| async { Err::<(), _>("fail".to_string()) }).await;
        // Now should be open and not call action
        let mut called = false;
        let result: Result<(), String> = cb
            .call(|| async {
                called = true;
                Ok(())
            })
            .await;
        assert!(!called);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn success_resets_count() {
        let cb = DbCircuitBreaker::new(5, vox_config::timeouts::D_60S, true);
        // One failure
        let _: Result<(), String> = cb.call(|| async { Err("oops".to_string()) }).await;
        assert_eq!(cb.failure_count(), 1);
        // One success
        let _: Result<(), String> = cb.call(|| async { Ok(()) }).await;
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn disabled_always_passes_through() {
        let cb = DbCircuitBreaker::new(1, vox_config::timeouts::D_60S, false);
        // Failures don't trip
        for _ in 0..10 {
            let _: Result<(), String> = cb.call(|| async { Err("x".to_string()) }).await;
        }
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
