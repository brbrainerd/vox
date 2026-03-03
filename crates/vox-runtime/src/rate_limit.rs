use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// In-memory fixed-window rate limiter for generated services.
#[derive(Clone, Debug)]
pub struct RateLimiter {
    max_requests: usize,
    window: Duration,
    buckets: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Construct from env vars:
    /// - `VOX_RATE_LIMIT_MAX_REQUESTS` (default: 120)
    /// - `VOX_RATE_LIMIT_WINDOW_SECONDS` (default: 60)
    pub fn from_env() -> Self {
        let max_requests = std::env::var("VOX_RATE_LIMIT_MAX_REQUESTS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(120);
        let window_seconds = std::env::var("VOX_RATE_LIMIT_WINDOW_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);
        Self::new(max_requests, Duration::from_secs(window_seconds))
    }

    /// Returns true if this key can proceed in the current window.
    pub fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().expect("rate-limit lock poisoned");
        let entries = buckets.entry(key.to_string()).or_default();
        entries.retain(|instant| now.duration_since(*instant) <= self.window);
        if entries.len() >= self.max_requests {
            return false;
        }
        entries.push(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_blocks_after_threshold() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        assert!(limiter.allow("u1"));
        assert!(limiter.allow("u1"));
        assert!(!limiter.allow("u1"));
    }

    #[test]
    fn from_env_produces_sane_defaults() {
        let limiter = RateLimiter::from_env();
        assert!(limiter.allow("env-key"));
    }
}
