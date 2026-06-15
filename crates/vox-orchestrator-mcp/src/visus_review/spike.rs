//! Trailing-median spike detection over a JSONL ledger.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerRow {
    pub ts: String,
    pub surfaces_reviewed: usize,
    pub total_review_ms: u64,
    pub total_cost_usd: f64,
    pub model: String,
}

/// True if `this_ms` exceeds `factor` * median(history).
pub fn is_spike(history_ms: &[u64], this_ms: u64, factor: f64) -> (bool, String) {
    if history_ms.is_empty() {
        return (false, "no baseline".into());
    }
    let mut v: Vec<u64> = history_ms.to_vec();
    v.sort_unstable();
    let median = v[v.len() / 2] as f64;
    let threshold = median * factor;
    let spiked = (this_ms as f64) > threshold;
    (
        spiked,
        format!("this={this_ms}ms median={median:.0}ms threshold={threshold:.0}ms"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_history_no_spike() {
        assert!(!is_spike(&[], 9999, 1.5).0);
    }
    #[test]
    fn within_threshold_ok() {
        assert!(!is_spike(&[100, 100, 100], 140, 1.5).0);
    }
    #[test]
    fn over_threshold_spikes() {
        assert!(is_spike(&[100, 100, 100], 200, 1.5).0);
    }
}
