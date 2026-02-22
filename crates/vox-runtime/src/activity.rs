use std::future::Future;
use std::time::Duration;
use tokio::time;
use tracing;

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
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            activity_id: None,
        }
    }
}

impl ActivityOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout = Some(Duration::from_secs(secs));
        self
    }

    pub fn with_initial_backoff(mut self, backoff: Duration) -> Self {
        self.initial_backoff = backoff;
        self
    }

    pub fn with_max_backoff(mut self, max: Duration) -> Self {
        self.max_backoff = max;
        self
    }

    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

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
    #[error("activity timed out after {0:?}")]
    Timeout(Duration),

    #[error("activity failed after {attempts} attempts: {last_error}")]
    RetriesExhausted { attempts: u32, last_error: String },

    #[error("activity execution error: {0}")]
    ExecutionError(String),
}

/// Tracks the state of an activity execution for observability.
#[derive(Debug, Clone)]
pub struct ActivityExecution {
    pub activity_id: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub started_at: std::time::Instant,
    pub status: ActivityStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityStatus {
    Running,
    Succeeded,
    Failed,
    TimedOut,
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
        .unwrap_or_else(|| format!("{}-{}", name, uuid_simple()));

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
                        if attempt < max_attempts {
                            // Retry on timeout
                            current_backoff = next_backoff(current_backoff, options);
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
                    current_backoff = next_backoff(current_backoff, options);
                }
            }
        }
    }

    ActivityResult::Failed(ActivityError::RetriesExhausted {
        attempts: max_attempts,
        last_error,
    })
}

/// Calculate the next backoff duration using exponential backoff with cap.
fn next_backoff(current: Duration, options: &ActivityOptions) -> Duration {
    let next = Duration::from_secs_f64(current.as_secs_f64() * options.backoff_multiplier);
    std::cmp::min(next, options.max_backoff)
}

/// Generate a simple unique ID (no external dep required).
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", nanos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_default_options() {
        let opts = ActivityOptions::default();
        assert_eq!(opts.retries, 0);
        assert!(opts.timeout.is_none());
        assert_eq!(opts.initial_backoff, Duration::from_millis(100));
        assert_eq!(opts.max_backoff, Duration::from_secs(60));
        assert_eq!(opts.backoff_multiplier, 2.0);
        assert!(opts.activity_id.is_none());
    }

    #[test]
    fn test_builder_pattern() {
        let opts = ActivityOptions::new()
            .with_retries(3)
            .with_timeout_secs(10)
            .with_initial_backoff(Duration::from_millis(200))
            .with_activity_id("test-123".to_string());

        assert_eq!(opts.retries, 3);
        assert_eq!(opts.timeout, Some(Duration::from_secs(10)));
        assert_eq!(opts.initial_backoff, Duration::from_millis(200));
        assert_eq!(opts.activity_id, Some("test-123".to_string()));
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            ActivityOptions::parse_duration("10s"),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            ActivityOptions::parse_duration("500ms"),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            ActivityOptions::parse_duration("2m"),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            ActivityOptions::parse_duration("1h"),
            Some(Duration::from_secs(3600))
        );
        assert_eq!(
            ActivityOptions::parse_duration("30"),
            Some(Duration::from_secs(30))
        );
        assert_eq!(ActivityOptions::parse_duration("invalid"), None);
    }

    #[test]
    fn test_next_backoff_capped() {
        let opts = ActivityOptions::new().with_max_backoff(Duration::from_secs(5));
        // Starting from 4s with 2x multiplier should cap at 5s
        let result = next_backoff(Duration::from_secs(4), &opts);
        assert_eq!(result, Duration::from_secs(5));
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
            .with_initial_backoff(Duration::from_millis(1)); // fast for tests

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
            .with_initial_backoff(Duration::from_millis(1));

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
        let opts = ActivityOptions::new().with_timeout(Duration::from_millis(10));

        let result = execute_activity("timeout-test", &opts, || async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok::<_, String>("should not reach")
        })
        .await;

        match result {
            ActivityResult::Failed(ActivityError::Timeout(_)) => { /* expected */ }
            other => panic!("Expected Timeout, got {:?}", other),
        }
    }
}
