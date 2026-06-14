use std::future::Future;
use std::time::Duration;
use tokio::time;
use tracing;
use vox_telemetry::{
    METRIC_TYPE_SANDBOX_TIMEOUT_KILL, ResearchMetricEvent, TelemetryEvent, record_event,
};

/// Emit one `sandbox.timeout_kill` telemetry event.
///
/// Fires on every timeout-induced termination (both retried and terminal). The
/// `terminal` flag in `metadata_json` lets consumers distinguish whether this
/// kill ended the activity or was followed by a retry.
fn emit_sandbox_timeout_kill(
    activity_name: &str,
    activity_id: &str,
    attempt: u32,
    max_attempts: u32,
    timeout: Duration,
    terminal: bool,
) {
    let metadata_json = serde_json::json!({
        "activity_name": activity_name,
        "activity_id": activity_id,
        "attempt": attempt,
        "max_attempts": max_attempts,
        "timeout_ms": timeout.as_millis() as u64,
        "terminal": terminal,
    })
    .to_string();
    record_event!(&TelemetryEvent::ResearchMetric(ResearchMetricEvent {
        session_id: format!("sandbox:{activity_id}"),
        metric_type: METRIC_TYPE_SANDBOX_TIMEOUT_KILL.into(),
        metric_value: Some(timeout.as_millis() as f64),
        metadata_json: Some(metadata_json),
    }));
}

/// Options that control activity execution behavior.
/// These map directly to the `with { ... }` syntax in Vox source.
#[derive(Debug, Clone)]
pub struct ActivityOptions {
    /// Maximum number of retry attempts (0 = no retries).
    pub retries: u32,
    /// Timeout for each individual attempt.
    pub timeout: Option<Duration>,
    /// Initial backoff delay between retries.
    pub initial_backoff: Duration,
    /// Maximum backoff delay (caps exponential growth).
    pub max_backoff: Duration,
    /// Backoff multiplier for exponential backoff.
    pub backoff_multiplier: f64,
    /// Unique identifier for this activity execution (for idempotency).
    pub activity_id: Option<String>,
}

impl Default for ActivityOptions {
    fn default() -> Self {
        Self {
            retries: 0,
            timeout: None,
            initial_backoff: vox_config::timeouts::D_100MS,
            max_backoff: vox_config::timeouts::D_60S,
            backoff_multiplier: 2.0,
            activity_id: None,
        }
    }
}

impl ActivityOptions {
    /// Returns default options (no retries, no per-attempt timeout).
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets how many retries to perform after the first failed attempt.
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Sets a wall-clock timeout for each attempt.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the per-attempt timeout from a whole number of seconds.
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout = Some(Duration::from_secs(secs));
        self
    }

    /// Sets the delay before the first retry after a failure.
    pub fn with_initial_backoff(mut self, backoff: Duration) -> Self {
        self.initial_backoff = backoff;
        self
    }

    /// Sets the upper bound for exponential backoff growth.
    pub fn with_max_backoff(mut self, max: Duration) -> Self {
        self.max_backoff = max;
        self
    }

    /// Sets the multiplicative factor applied between backoff steps.
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Sets a stable idempotency key for this activity run.
    pub fn with_activity_id(mut self, id: String) -> Self {
        self.activity_id = Some(id);
        self
    }

    /// Parse a duration string like "10s", "500ms", "2m".
    pub fn parse_duration(s: &str) -> Option<Duration> {
        let s = s.trim();
        if let Some(rest) = s.strip_suffix("ms") {
            rest.parse::<u64>().ok().map(Duration::from_millis)
        } else if let Some(rest) = s.strip_suffix('s') {
            rest.parse::<u64>().ok().map(Duration::from_secs)
        } else if let Some(rest) = s.strip_suffix('m') {
            rest.parse::<u64>()
                .ok()
                .map(|m| Duration::from_secs(m * 60))
        } else if let Some(rest) = s.strip_suffix('h') {
            rest.parse::<u64>()
                .ok()
                .map(|h| Duration::from_secs(h * 3600))
        } else {
            // Try as plain seconds
            s.parse::<u64>().ok().map(Duration::from_secs)
        }
    }
}

/// Result of an activity execution.
#[derive(Debug)]
pub enum ActivityResult<T> {
    /// Activity completed successfully.
    Ok(T),
    /// Activity failed after all retries exhausted.
    Failed(ActivityError),
    /// Activity was cancelled.
    Cancelled,
}

/// Error from activity execution.
#[derive(Debug, thiserror::Error)]
pub enum ActivityError {
    /// A single attempt exceeded its configured timeout.
    #[error("activity timed out after {0:?}")]
    Timeout(Duration),

    /// All attempts failed; includes the final attempt count and last error text.
    #[error("activity failed after {attempts} attempts: {last_error}")]
    RetriesExhausted {
        /// Number of attempts that were made.
        attempts: u32,
        /// Display string of the last error returned by the activity closure.
        last_error: String,
    },

    /// A non-retryable or wrapped execution failure.
    #[error("activity execution error: {0}")]
    ExecutionError(String),
}

/// Tracks the state of an activity execution for observability.
#[derive(Debug, Clone)]
pub struct ActivityExecution {
    /// Stable id for this run (from options or generated).
    pub activity_id: String,
    /// Current attempt number (1-based).
    pub attempt: u32,
    /// Maximum attempts allowed (initial + retries).
    pub max_attempts: u32,
    /// When this execution started.
    pub started_at: std::time::Instant,
    /// High-level lifecycle state.
    pub status: ActivityStatus,
}

/// Coarse lifecycle state of an activity for metrics and tracing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityStatus {
    /// At least one attempt is in flight.
    Running,
    /// Completed with a value.
    Succeeded,
    /// Failed without a successful value.
    Failed,
    /// Stopped due to timeout.
    TimedOut,
    /// Waiting to retry after backoff.
    Retrying,
}

/// Execute an async activity function with the given options.
///
/// This is the core runtime function that compiled `with { ... }` expressions
/// call into. It handles retries, timeouts, and exponential backoff.
pub async fn execute_activity<F, Fut, T, E>(
    name: &str,
    options: &ActivityOptions,
    f: F,
) -> ActivityResult<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let max_attempts = options.retries + 1;
    let mut current_backoff = options.initial_backoff;
    let mut last_error = String::new();

    let activity_id = options
        .activity_id
        .clone()
        .unwrap_or_else(|| format!("{}-{}", name, crate::simple_id::simple_hex_id()));

    for attempt in 1..=max_attempts {
        tracing::info!(
            activity_id = %activity_id,
            attempt = attempt,
            max_attempts = max_attempts,
            "Executing activity '{}'",
            name
        );

        let result = match options.timeout {
            Some(timeout) => {
                match time::timeout(timeout, f()).await {
                    Ok(inner) => inner,
                    Err(_) => {
                        tracing::warn!(
                            activity_id = %activity_id,
                            attempt = attempt,
                            timeout = ?timeout,
                            "Activity '{}' timed out",
                            name
                        );
                        let terminal = attempt >= max_attempts;
                        emit_sandbox_timeout_kill(
                            name,
                            &activity_id,
                            attempt,
                            max_attempts,
                            timeout,
                            terminal,
                        );
                        if !terminal {
                            // Retry on timeout
                            current_backoff =
                                vox_foundation::primitives::backoff::next_exponential_backoff_duration(
                                    current_backoff,
                                    options.backoff_multiplier,
                                    options.max_backoff,
                                );
                            time::sleep(current_backoff).await;
                            continue;
                        }
                        return ActivityResult::Failed(ActivityError::Timeout(timeout));
                    }
                }
            }
            None => f().await,
        };

        match result {
            Ok(value) => {
                tracing::info!(
                    activity_id = %activity_id,
                    attempt = attempt,
                    "Activity '{}' succeeded",
                    name
                );
                return ActivityResult::Ok(value);
            }
            Err(e) => {
                last_error = e.to_string();
                tracing::warn!(
                    activity_id = %activity_id,
                    attempt = attempt,
                    error = %last_error,
                    "Activity '{}' failed",
                    name
                );

                if attempt < max_attempts {
                    tracing::info!(
                        activity_id = %activity_id,
                        next_backoff = ?current_backoff,
                        "Retrying activity '{}' after backoff",
                        name
                    );
                    time::sleep(current_backoff).await;
                    current_backoff =
                        vox_foundation::primitives::backoff::next_exponential_backoff_duration(
                            current_backoff,
                            options.backoff_multiplier,
                            options.max_backoff,
                        );
                }
            }
        }
    }

    ActivityResult::Failed(ActivityError::RetriesExhausted {
        attempts: max_attempts,
        last_error,
    })
}

/// Execute an activity and return a standard `Result` envelope for generated code paths.
///
/// This keeps failure/cancellation on the value channel instead of forcing panic-based handling.
pub async fn execute_activity_result<F, Fut, T, E>(
    name: &str,
    options: &ActivityOptions,
    f: F,
) -> Result<T, String>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    match execute_activity(name, options, f).await {
        ActivityResult::Ok(v) => Ok(v),
        ActivityResult::Failed(e) => Err(e.to_string()),
        ActivityResult::Cancelled => Err("activity cancelled".to_string()),
    }
}

#[cfg(test)]
mod semcov_wave11_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    // ── Builder pattern ──────────────────────────────────────────────────────

    #[test]
    fn builder_chain_sets_all_fields() {
        // Catches: a builder method that silently drops its assignment (mut self mistake)
        let opts = ActivityOptions::new()
            .with_retries(3)
            .with_timeout(vox_config::timeouts::D_5S)
            .with_initial_backoff(vox_config::timeouts::D_50MS)
            .with_max_backoff(vox_config::timeouts::D_30S)
            .with_backoff_multiplier(1.5)
            .with_activity_id("chain-id".to_string());
        assert_eq!(opts.retries, 3);
        assert_eq!(opts.timeout, Some(vox_config::timeouts::D_5S));
        assert_eq!(opts.initial_backoff, vox_config::timeouts::D_50MS);
        assert_eq!(opts.max_backoff, vox_config::timeouts::D_30S);
        // Compare to a sentinel that differs from both the default (2.0) and 0.0
        assert!(
            (opts.backoff_multiplier - 1.5).abs() < f64::EPSILON,
            "backoff_multiplier was not stored: got {}",
            opts.backoff_multiplier
        );
        assert_eq!(opts.activity_id.as_deref(), Some("chain-id"));
    }

    #[test]
    fn with_retries_zero_is_valid() {
        // Catches: a guard that rejects 0 retries, or a default that ignores the call
        let opts = ActivityOptions::new().with_retries(0);
        assert_eq!(
            opts.retries, 0,
            "with_retries(0) must store 0, not the default"
        );
    }

    #[test]
    fn with_retries_u32_max_is_accepted() {
        // Catches: silent truncation or overflow in a .saturating_add path
        let opts = ActivityOptions::new().with_retries(u32::MAX);
        assert_eq!(opts.retries, u32::MAX);
    }

    #[test]
    fn with_timeout_secs_zero_stores_zero_duration() {
        // Catches: a guard that converts 0 → None instead of Duration::ZERO
        let opts = ActivityOptions::new().with_timeout_secs(0);
        assert_eq!(
            opts.timeout,
            Some(Duration::ZERO),
            "with_timeout_secs(0) must produce Some(Duration::ZERO)"
        );
    }

    #[test]
    fn with_timeout_secs_and_with_timeout_agree() {
        // Catches: off-by-one in the secs conversion
        let a = ActivityOptions::new().with_timeout_secs(7).timeout;
        let b = ActivityOptions::new()
            .with_timeout(vox_config::timeouts::D_7S)
            .timeout;
        assert_eq!(
            a, b,
            "with_timeout_secs(n) != with_timeout(Duration::from_secs(n))"
        );
    }

    #[test]
    fn with_backoff_multiplier_zero_is_stored() {
        // Catches: a clamp that silently replaces 0.0 with 1.0 or the default 2.0
        let opts = ActivityOptions::new().with_backoff_multiplier(0.0);
        assert!(
            opts.backoff_multiplier == 0.0,
            "backoff_multiplier(0.0) was rejected or clamped: {}",
            opts.backoff_multiplier
        );
    }

    #[test]
    fn with_max_backoff_zero_is_stored() {
        // Catches: a guard that ignores Duration::ZERO and keeps the default 60s
        let opts = ActivityOptions::new().with_max_backoff(Duration::ZERO);
        assert_eq!(
            opts.max_backoff,
            Duration::ZERO,
            "with_max_backoff(ZERO) must store ZERO, not the default"
        );
    }

    // ── parse_duration ───────────────────────────────────────────────────────

    #[test]
    fn parse_duration_5s_exact() {
        // Catches: off-by-one or wrong unit in the 's' branch
        assert_eq!(
            ActivityOptions::parse_duration("5s"),
            Some(vox_config::timeouts::D_5S),
            "\"5s\" must parse to exactly 5 seconds"
        );
    }

    #[test]
    fn parse_duration_100ms_exact() {
        // Catches: treating "ms" as seconds or dropping two digits
        assert_eq!(
            ActivityOptions::parse_duration("100ms"),
            Some(Duration::from_millis(100)),
            "\"100ms\" must parse to exactly 100 milliseconds"
        );
    }

    #[test]
    fn parse_duration_empty_string_is_none() {
        // Catches: a fallback plain-seconds parse that converts "" to Some(ZERO)
        assert_eq!(
            ActivityOptions::parse_duration(""),
            None,
            "empty string must return None"
        );
    }

    #[test]
    fn parse_duration_unknown_unit_is_none() {
        // Catches: a greedy suffix strip that returns Some for "5x" via the plain-int fallback
        assert_eq!(
            ActivityOptions::parse_duration("5x"),
            None,
            "\"5x\" (unknown unit) must return None"
        );
    }

    #[test]
    fn parse_duration_whitespace_only_is_none() {
        // Catches: trim() turning "   " into "" then succeeding as a plain-int
        assert_eq!(
            ActivityOptions::parse_duration("   "),
            None,
            "whitespace-only string must return None"
        );
    }

    #[test]
    fn parse_duration_negative_literal_is_none() {
        // Catches: signed parse path that accepts "-1s" and wraps to a large Duration
        assert_eq!(
            ActivityOptions::parse_duration("-1s"),
            None,
            "negative literal must return None"
        );
    }

    #[test]
    fn parse_duration_float_literal_is_none() {
        // Catches: a float→int truncation that silently accepts "1.5s"
        assert_eq!(
            ActivityOptions::parse_duration("1.5s"),
            None,
            "fractional seconds must return None (no float support)"
        );
    }

    #[test]
    fn parse_duration_minutes_60x_multiplier() {
        // Catches: wrong multiplier (e.g. 100 instead of 60) in the 'm' branch
        assert_eq!(
            ActivityOptions::parse_duration("2m"),
            Some(Duration::from_secs(120)),
            "\"2m\" must be exactly 120 seconds"
        );
    }

    #[test]
    fn parse_duration_hours_3600x_multiplier() {
        // Catches: wrong multiplier in the 'h' branch (e.g. 60 instead of 3600)
        assert_eq!(
            ActivityOptions::parse_duration("1h"),
            Some(Duration::from_secs(3600)),
            "\"1h\" must be exactly 3600 seconds"
        );
    }

    #[test]
    fn parse_duration_ms_suffix_beats_s_suffix() {
        // Catches: strip_suffix('s') firing on "100ms" and yielding "100m" (60x error)
        let ms = ActivityOptions::parse_duration("100ms").expect("must parse");
        assert!(
            ms < Duration::from_secs(1),
            "\"100ms\" parsed to {:?}, expected < 1s (suffix order bug)",
            ms
        );
    }

    // ── ActivityStatus enum ──────────────────────────────────────────────────

    #[test]
    fn activity_status_variants_are_distinct() {
        // Catches: accidentally derived Eq implementation that collapses variants
        let statuses = [
            ActivityStatus::Running,
            ActivityStatus::Succeeded,
            ActivityStatus::Failed,
            ActivityStatus::TimedOut,
            ActivityStatus::Retrying,
        ];
        for (i, a) in statuses.iter().enumerate() {
            for (j, b) in statuses.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b, "same variant must equal itself");
                } else {
                    assert_ne!(
                        a, b,
                        "distinct variants must not be equal: {:?} == {:?}",
                        a, b
                    );
                }
            }
        }
    }

    // ── execute_activity ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn execute_activity_immediate_success_calls_closure_once() {
        // Catches: an off-by-one that invokes the closure an extra time even on success
        let call_count = Arc::new(AtomicU32::new(0));
        let c = call_count.clone();
        let opts = ActivityOptions::new();
        let result = execute_activity("once", &opts, move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok::<_, String>("done")
            }
        })
        .await;
        match result {
            ActivityResult::Ok(v) => assert_eq!(v, "done"),
            other => panic!("Expected Ok, got {:?}", other),
        }
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "closure must be called exactly once on immediate success"
        );
    }

    #[tokio::test]
    async fn execute_activity_retries_exhausted_reports_correct_attempt_count() {
        // Catches: max_attempts = retries (off-by-one) instead of retries + 1
        let opts = ActivityOptions::new()
            .with_retries(2)
            .with_initial_backoff(vox_config::timeouts::D_1MS);
        let result =
            execute_activity("count-check", &opts, || async { Err::<(), _>("boom") }).await;
        match result {
            ActivityResult::Failed(ActivityError::RetriesExhausted { attempts, .. }) => {
                assert_eq!(
                    attempts, 3,
                    "with_retries(2) must yield 3 total attempts (1 initial + 2 retries)"
                );
            }
            other => panic!("Expected RetriesExhausted, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn execute_activity_last_error_reflects_final_attempt() {
        // Catches: last_error captured from attempt 1 rather than the final attempt
        let call_count = Arc::new(AtomicU32::new(0));
        let c = call_count.clone();
        let opts = ActivityOptions::new()
            .with_retries(1)
            .with_initial_backoff(vox_config::timeouts::D_1MS);
        let result = execute_activity("last-err", &opts, move || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst) + 1;
                Err::<(), String>(format!("attempt {n}"))
            }
        })
        .await;
        match result {
            ActivityResult::Failed(ActivityError::RetriesExhausted { last_error, .. }) => {
                assert_eq!(
                    last_error, "attempt 2",
                    "last_error must come from the final attempt, not the first"
                );
            }
            other => panic!("Expected RetriesExhausted, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn execute_activity_with_retries_zero_makes_exactly_one_call() {
        // Catches: retries=0 being treated as "infinite" or as 1 extra attempt
        let call_count = Arc::new(AtomicU32::new(0));
        let c = call_count.clone();
        let opts = ActivityOptions::new()
            .with_retries(0)
            .with_initial_backoff(vox_config::timeouts::D_1MS);
        let _result = execute_activity("zero-retry", &opts, move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>("fail")
            }
        })
        .await;
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "with_retries(0) must call the closure exactly once"
        );
    }

    #[tokio::test]
    async fn execute_activity_timeout_returns_timeout_error_not_retries_exhausted() {
        // Catches: a bug that maps a timed-out run to RetriesExhausted instead of Timeout
        let opts = ActivityOptions::new().with_timeout(vox_config::timeouts::D_10MS);
        let result = execute_activity("timeout-variant", &opts, || async {
            tokio::time::sleep(vox_config::timeouts::D_60S).await;
            Ok::<_, String>("never")
        })
        .await;
        match result {
            ActivityResult::Failed(ActivityError::Timeout(d)) => {
                assert_eq!(
                    d,
                    vox_config::timeouts::D_10MS,
                    "Timeout must carry the configured timeout duration"
                );
            }
            other => panic!("Expected Timeout variant, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn execute_activity_succeeds_on_second_attempt_when_one_retry_allowed() {
        // Catches: a retry loop that exits before the second attempt
        let call_count = Arc::new(AtomicU32::new(0));
        let c = call_count.clone();
        let opts = ActivityOptions::new()
            .with_retries(1)
            .with_initial_backoff(vox_config::timeouts::D_1MS);
        let result = execute_activity("second-attempt", &opts, move || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst) + 1;
                if n == 1 {
                    Err("first attempt fails".to_string())
                } else {
                    Ok("second attempt succeeds")
                }
            }
        })
        .await;
        match result {
            ActivityResult::Ok(v) => assert_eq!(v, "second attempt succeeds"),
            other => panic!("Expected Ok on second attempt, got {:?}", other),
        }
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn execute_activity_result_wraps_success_as_ok() {
        // Catches: execute_activity_result mapping ActivityResult::Ok to Err
        let opts = ActivityOptions::new();
        let result =
            execute_activity_result("wrap-ok", &opts, || async { Ok::<_, String>(42) }).await;
        match result {
            Ok(v) => assert_eq!(v, 42),
            Err(e) => panic!("Expected Ok(42), got Err({e})"),
        }
    }

    #[tokio::test]
    async fn execute_activity_result_wraps_failure_as_err_with_nonempty_message() {
        // Catches: execute_activity_result returning Ok on failure, or an empty error string
        let opts = ActivityOptions::new().with_initial_backoff(vox_config::timeouts::D_1MS);
        let result =
            execute_activity_result("wrap-err", &opts, || async { Err::<(), _>("kaboom") }).await;
        match result {
            Err(msg) => assert!(
                !msg.is_empty(),
                "error message must not be empty on failure"
            ),
            Ok(_) => panic!("Expected Err on failure, got Ok"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_default_options() {
        let opts = ActivityOptions::default();
        assert_eq!(opts.retries, 0);
        assert!(opts.timeout.is_none());
        assert_eq!(opts.initial_backoff, vox_config::timeouts::D_100MS);
        assert_eq!(opts.max_backoff, vox_config::timeouts::D_60S);
        assert_eq!(opts.backoff_multiplier, 2.0);
        assert!(opts.activity_id.is_none());
    }

    #[test]
    fn test_builder_pattern() {
        let opts = ActivityOptions::new()
            .with_retries(3)
            .with_timeout_secs(10)
            .with_initial_backoff(vox_config::timeouts::D_200MS)
            .with_activity_id("test-123".to_string());

        assert_eq!(opts.retries, 3);
        assert_eq!(opts.timeout, Some(vox_config::timeouts::D_10S));
        assert_eq!(opts.initial_backoff, vox_config::timeouts::D_200MS);
        assert_eq!(opts.activity_id, Some("test-123".to_string()));
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            ActivityOptions::parse_duration("10s"),
            Some(vox_config::timeouts::D_10S)
        );
        assert_eq!(
            ActivityOptions::parse_duration("500ms"),
            Some(vox_config::timeouts::D_500MS)
        );
        assert_eq!(
            ActivityOptions::parse_duration("2m"),
            Some(vox_config::timeouts::D_120S)
        );
        assert_eq!(
            ActivityOptions::parse_duration("1h"),
            Some(vox_config::timeouts::D_3600S)
        );
        assert_eq!(
            ActivityOptions::parse_duration("30"),
            Some(vox_config::timeouts::D_30S)
        );
        assert_eq!(ActivityOptions::parse_duration("invalid"), None);
    }

    #[test]
    fn test_next_backoff_capped() {
        let opts = ActivityOptions::new().with_max_backoff(vox_config::timeouts::D_5S);
        // Starting from 4s with 2x multiplier should cap at 5s
        let result = vox_foundation::primitives::backoff::next_exponential_backoff_duration(
            Duration::from_secs(4),
            opts.backoff_multiplier,
            opts.max_backoff,
        );
        assert_eq!(result, vox_config::timeouts::D_5S);
    }

    #[tokio::test]
    async fn test_execute_activity_success() {
        let opts = ActivityOptions::new();
        let result = execute_activity("test", &opts, || async { Ok::<_, String>("hello") }).await;

        match result {
            ActivityResult::Ok(v) => assert_eq!(v, "hello"),
            other => panic!("Expected Ok, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_execute_activity_retry_then_success() {
        let counter = Arc::new(AtomicU32::new(0));
        let opts = ActivityOptions::new()
            .with_retries(3)
            .with_initial_backoff(vox_config::timeouts::D_1MS); // fast for tests

        let counter_clone = counter.clone();
        let result = execute_activity("retry-test", &opts, move || {
            let c = counter_clone.clone();
            async move {
                let attempt = c.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt < 3 {
                    Err(format!("failing on attempt {}", attempt))
                } else {
                    Ok("success after retries")
                }
            }
        })
        .await;

        match result {
            ActivityResult::Ok(v) => assert_eq!(v, "success after retries"),
            other => panic!("Expected Ok after retries, got {:?}", other),
        }
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_execute_activity_all_retries_exhausted() {
        let opts = ActivityOptions::new()
            .with_retries(2)
            .with_initial_backoff(vox_config::timeouts::D_1MS);

        let result = execute_activity("fail-test", &opts, || async {
            Err::<(), _>("always fails")
        })
        .await;

        match result {
            ActivityResult::Failed(ActivityError::RetriesExhausted {
                attempts,
                last_error,
            }) => {
                assert_eq!(attempts, 3); // 1 initial + 2 retries
                assert_eq!(last_error, "always fails");
            }
            other => panic!("Expected RetriesExhausted, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_execute_activity_timeout() {
        let opts = ActivityOptions::new().with_timeout(vox_config::timeouts::D_10MS);

        let result = execute_activity("timeout-test", &opts, || async {
            tokio::time::sleep(vox_config::timeouts::D_10S).await;
            Ok::<_, String>("should not reach")
        })
        .await;

        match result {
            ActivityResult::Failed(ActivityError::Timeout(_)) => { /* expected */ }
            other => panic!("Expected Timeout, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_execute_activity_result_maps_failure_to_err() {
        let opts = ActivityOptions::new();
        let result =
            execute_activity_result("map-fail", &opts, || async { Err::<(), _>("boom") }).await;
        assert!(result.is_err());
    }
}
