//! Quota policy types (P5-T3a).

/// Per-key quota policy.
#[derive(Debug, Clone)]
pub struct QuotaPolicy {
    /// Maximum tokens in the bucket.
    pub capacity: u64,
    /// Refill rate in tokens per second.
    pub refill_per_sec: f64,
}

impl Default for QuotaPolicy {
    fn default() -> Self {
        Self {
            capacity: 10_000,
            refill_per_sec: 100.0,
        }
    }
}

/// Exponential moving average for peer reputation (α = 0.1 default).
#[derive(Debug, Clone)]
pub struct ReputationEma {
    /// Current EMA value in [0.0, 1.0].
    pub value: f64,
    /// Smoothing factor α in (0.0, 1.0].
    pub alpha: f64,
}

impl Default for ReputationEma {
    fn default() -> Self {
        Self {
            value: 1.0,
            alpha: 0.1,
        }
    }
}

impl ReputationEma {
    pub fn update(&mut self, success: bool) {
        let signal = if success { 1.0 } else { 0.0 };
        self.value = self.alpha * signal + (1.0 - self.alpha) * self.value;
    }
}
