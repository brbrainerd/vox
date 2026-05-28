//! Pure scoring heuristics shared across producers.
//!
//! All functions are deterministic and free of I/O. They are unit-tested in
//! the module's `tests` submodule; producer-level integration tests live with
//! each producer.

/// 95th-percentile of an unsorted sample, using nearest-rank.
///
/// Returns `None` for an empty input. The implementation sorts a local copy
/// (O(n log n)) and indexes; for the sample sizes producers care about
/// (≤ a few thousand) this is plenty fast.
pub fn p95(samples: &[u64]) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let mut s: Vec<u64> = samples.to_vec();
    s.sort_unstable();
    let idx = ((s.len() as f64 * 0.95) as usize).min(s.len() - 1);
    Some(s[idx])
}

/// Fractional improvement of `improved` over `baseline`, clamped to `[0, 1]`.
///
/// Returns `0.0` when `baseline == 0` (avoids divide-by-zero; producers
/// should not consider such a sample meaningful).
pub fn fractional_improvement(baseline: f64, improved: f64) -> f64 {
    if baseline <= 0.0 {
        return 0.0;
    }
    let delta = (baseline - improved) / baseline;
    delta.clamp(0.0, 1.0)
}

/// Convert an epoch-ms timestamp to a `YYYY-MM-DD` slug.
pub fn date_slug(now_ms: i64) -> String {
    use chrono::DateTime;
    DateTime::from_timestamp_millis(now_ms)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "1970-01-01".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p95_of_empty_is_none() {
        assert_eq!(p95(&[]), None);
    }

    #[test]
    fn p95_of_singleton_is_the_value() {
        assert_eq!(p95(&[42]), Some(42));
    }

    #[test]
    fn p95_of_100_elements_is_the_95th_value() {
        let samples: Vec<u64> = (1..=100).collect();
        // nearest-rank: idx = (100 * 0.95) as usize = 95, sorted[95] = 96.
        assert_eq!(p95(&samples), Some(96));
    }

    #[test]
    fn fractional_improvement_basic() {
        assert!((fractional_improvement(100.0, 80.0) - 0.20).abs() < 1e-9);
        assert_eq!(fractional_improvement(100.0, 100.0), 0.0);
        // Regression — no improvement.
        assert_eq!(fractional_improvement(100.0, 150.0), 0.0);
    }

    #[test]
    fn fractional_improvement_zero_baseline_is_zero() {
        assert_eq!(fractional_improvement(0.0, 10.0), 0.0);
    }

    #[test]
    fn date_slug_for_known_epoch() {
        // 1_747_000_000_000 ms == 2025-05-11T22:46:40Z.
        let slug = date_slug(1_747_000_000_000);
        assert_eq!(slug, "2025-05-11");
    }

    #[test]
    fn date_slug_for_zero_epoch_is_unix_epoch_date() {
        assert_eq!(date_slug(0), "1970-01-01");
    }
}
