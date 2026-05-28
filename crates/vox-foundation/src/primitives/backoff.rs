//! Pure exponential backoff — no async, no policy loops.

use std::time::Duration;

/// Next duration after multiplying `current` by `multiplier`, capped at `max` (activity retry style).
#[must_use]
pub fn next_exponential_backoff_duration(
    current: Duration,
    multiplier: f64,
    max: Duration,
) -> Duration {
    let next = Duration::from_secs_f64(current.as_secs_f64() * multiplier);
    std::cmp::min(next, max)
}

/// Next sleep interval in milliseconds: double `current_ms`, clamped to `[min_ms, max_ms]` (sidecar poll style).
#[must_use]
pub fn next_backoff_ms_double_clamped(current_ms: u64, min_ms: u64, max_ms: u64) -> u64 {
    current_ms.saturating_mul(2).clamp(min_ms, max_ms)
}

/// After a failed attempt with 1-based index `failed_attempt`, return delay `base_ms * 2^(failed_attempt - 1)` in ms.
///
/// `max_exponent` caps the power-of-two factor (e.g. `6` → factor ≤ 64). Result is clamped to `max_ms`.
#[must_use]
pub fn backoff_ms_geometric_attempt(
    failed_attempt: u32,
    base_ms: u64,
    max_ms: u64,
    max_exponent: u32,
) -> u64 {
    let exp = failed_attempt.saturating_sub(1).min(max_exponent);
    base_ms.saturating_mul(2u64.pow(exp)).min(max_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn exponential_duration_respects_cap() {
        let opts_max = Duration::from_secs(5);
        let out = next_exponential_backoff_duration(Duration::from_secs(4), 2.0, opts_max);
        assert_eq!(out, opts_max);
    }

    #[test]
    fn ms_double_clamped_matches_openclaw_style() {
        assert_eq!(next_backoff_ms_double_clamped(500, 100, 30_000), 1_000);
        assert_eq!(next_backoff_ms_double_clamped(20_000, 100, 30_000), 30_000);
    }

    #[test]
    fn geometric_matches_social_retry_powers() {
        assert_eq!(backoff_ms_geometric_attempt(1, 800, 60_000, 6), 800);
        assert_eq!(backoff_ms_geometric_attempt(2, 800, 60_000, 6), 1_600);
        assert_eq!(backoff_ms_geometric_attempt(7, 800, 60_000, 6), 51_200);
    }
}
