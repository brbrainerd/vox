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
        let max_requests = vox_config::env_parse::resolve_config_u64("VOX_RATE_LIMIT_MAX_REQUESTS", 120) as usize;
        let window_seconds = vox_config::env_parse::resolve_config_u64("VOX_RATE_LIMIT_WINDOW_SECONDS", 60);
        Self::new(max_requests, Duration::from_secs(window_seconds))
    }

    /// Returns true if this key can proceed in the current window.
    pub fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let limiter = RateLimiter::new(2, vox_config::timeouts::D_60S);
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
