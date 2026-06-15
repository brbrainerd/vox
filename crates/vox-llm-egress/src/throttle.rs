//! Per-provider AIMD concurrency throttle for LLM egress.
//!
//! Design (OpenRouter-informed, 2026-06): paid OpenRouter traffic has no
//! platform RPM cap — concurrency is the real dial — while free models 429
//! readily. We bound in-flight requests per provider with a ceiling supplied by
//! the caller (resolved from VoxConfig in `vox_config::resolve_egress`, so this
//! crate stays free of a config dependency), halve the window on 429 (honoring
//! Retry-After / X-RateLimit-Reset as a cooldown), and additively recover one
//! permit per 8 successes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

pub struct ProviderThrottle {
    max_limit: usize,
    current_limit: AtomicUsize,
    in_flight: AtomicUsize,
    success_streak: AtomicUsize,
    cooldown_until: Mutex<Option<Instant>>,
    notify: Notify,
}

/// RAII permit: releases the slot on drop.
pub struct Permit<'a> {
    throttle: &'a ProviderThrottle,
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.throttle.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.throttle.notify.notify_waiters();
    }
}

impl ProviderThrottle {
    pub fn new(max_limit: usize) -> Self {
        let max = max_limit.max(1);
        Self {
            max_limit: max,
            current_limit: AtomicUsize::new(max),
            in_flight: AtomicUsize::new(0),
            success_streak: AtomicUsize::new(0),
            cooldown_until: Mutex::new(None),
            notify: Notify::new(),
        }
    }

    pub fn current_limit(&self) -> usize {
        self.current_limit.load(Ordering::SeqCst)
    }

    /// Wait for a free slot (and for any active cooldown to elapse).
    pub async fn acquire(&self) -> Permit<'_> {
        loop {
            let wait = {
                let guard = self.cooldown_until.lock().expect("throttle lock");
                guard.and_then(|until| until.checked_duration_since(Instant::now()))
            };
            if let Some(d) = wait {
                tokio::time::sleep(d).await;
                continue;
            }
            // Register the wakeup future BEFORE the admission check. `Notify::
            // notify_waiters()` only wakes already-registered waiters (it stores no
            // permit for a future `notified()`), so a permit dropped between our
            // load and our `.await` would otherwise be a lost wakeup that hangs us.
            let notified = self.notify.notified();
            tokio::pin!(notified);

            let limit = self.current_limit.load(Ordering::SeqCst);
            let cur = self.in_flight.load(Ordering::SeqCst);
            if cur < limit
                && self
                    .in_flight
                    .compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                return Permit { throttle: self };
            }
            notified.await;
        }
    }

    /// Multiplicative decrease + optional header-driven cooldown.
    pub fn on_rate_limited(&self, retry_after: Option<Duration>) {
        let limit = self.current_limit.load(Ordering::SeqCst);
        self.current_limit.store((limit / 2).max(1), Ordering::SeqCst);
        self.success_streak.store(0, Ordering::SeqCst);
        if let Some(d) = retry_after {
            let mut guard = self.cooldown_until.lock().expect("throttle lock");
            let until = Instant::now() + d;
            *guard = Some(guard.map_or(until, |existing| existing.max(until)));
        }
        self.notify.notify_waiters();
    }

    /// Additive increase: +1 permit per 8 consecutive successes, up to max.
    pub fn on_success(&self) {
        let streak = self.success_streak.fetch_add(1, Ordering::SeqCst) + 1;
        if streak.is_multiple_of(8) {
            let limit = self.current_limit.load(Ordering::SeqCst);
            if limit < self.max_limit {
                self.current_limit.store(limit + 1, Ordering::SeqCst);
            }
            self.notify.notify_waiters();
        }
    }
}

static REGISTRY: OnceLock<Mutex<HashMap<String, &'static ProviderThrottle>>> = OnceLock::new();

/// Throttle for `provider`, created on first use with `max_limit` (the caller resolves
/// the limit from VoxConfig — this crate takes no config dependency). The first call's
/// limit wins; later calls reuse the existing per-provider throttle. Leaked
/// intentionally: providers are a small fixed set per process.
pub fn for_provider(provider: &str, max_limit: usize) -> &'static ProviderThrottle {
    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = registry.lock().expect("throttle registry lock");
    if let Some(t) = map.get(provider) {
        return t;
    }
    let throttle: &'static ProviderThrottle = Box::leak(Box::new(ProviderThrottle::new(max_limit)));
    map.insert(provider.to_string(), throttle);
    throttle
}

/// Acquire a permit for `provider`, creating its throttle with `max_limit` on first use.
pub async fn acquire_permit(provider: &str, max_limit: usize) -> Permit<'static> {
    for_provider(provider, max_limit).acquire().await
}

/// Feed a 429 back to `provider`'s throttle (halve window + optional cooldown).
pub fn on_rate_limited(provider: &str, retry_after: Option<Duration>) {
    for_provider(provider, 1).on_rate_limited(retry_after);
}

/// Record a success for `provider`'s throttle (additive recovery).
pub fn on_success(provider: &str) {
    for_provider(provider, 1).on_success();
}

/// Parse `Retry-After` (seconds) or `X-RateLimit-Reset` (epoch ms) into a wait.
pub fn retry_after_from_headers(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    if let Some(v) = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
    {
        return Some(Duration::from_secs(v.min(120)));
    }
    if let Some(reset_ms) = headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u128>().ok())
    {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis();
        if reset_ms > now_ms {
            return Some(Duration::from_millis(((reset_ms - now_ms) as u64).min(120_000)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn permits_bound_concurrency() {
        let t = ProviderThrottle::new(2);
        let g1 = t.acquire().await;
        let _g2 = t.acquire().await;
        let pending = tokio::time::timeout(Duration::from_millis(50), t.acquire()).await;
        assert!(pending.is_err(), "third permit should block at limit 2");
        drop(g1);
        let g3 = tokio::time::timeout(Duration::from_millis(200), t.acquire()).await;
        assert!(g3.is_ok(), "released permit should admit a waiter");
    }

    #[tokio::test]
    async fn rate_limit_halves_and_successes_recover() {
        let t = ProviderThrottle::new(8);
        t.on_rate_limited(None);
        assert_eq!(t.current_limit(), 4);
        t.on_rate_limited(None);
        assert_eq!(t.current_limit(), 2);
        for _ in 0..8 {
            t.on_success();
        }
        assert_eq!(t.current_limit(), 3);
    }

    #[tokio::test]
    async fn cooldown_blocks_until_deadline() {
        let t = ProviderThrottle::new(4);
        t.on_rate_limited(Some(Duration::from_millis(120)));
        let start = std::time::Instant::now();
        let _g = t.acquire().await;
        assert!(
            start.elapsed() >= Duration::from_millis(100),
            "acquire should wait out cooldown"
        );
    }
}
