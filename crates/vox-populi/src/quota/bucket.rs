//! In-memory token bucket with reputation EMA (P5-T3b).

use std::collections::HashMap;
use std::time::Instant;

use super::spec::{QuotaPolicy, ReputationEma};

/// Per-peer token-bucket + reputation state.
#[derive(Debug)]
pub struct PeerBucket {
    tokens: f64,
    last_refill: Instant,
    pub reputation: ReputationEma,
    policy: QuotaPolicy,
}

impl PeerBucket {
    pub fn new(policy: QuotaPolicy) -> Self {
        let tokens = policy.capacity as f64;
        Self {
            tokens,
            last_refill: Instant::now(),
            reputation: ReputationEma::default(),
            policy,
        }
    }

    /// Attempt to consume `n` tokens. Returns `true` if allowed.
    pub fn try_consume(&mut self, n: u64) -> bool {
        self.refill();
        if self.tokens >= n as f64 {
            self.tokens -= n as f64;
            true
        } else {
            false
        }
    }

    /// Record a job outcome and update reputation EMA.
    pub fn record_outcome(&mut self, success: bool) {
        self.reputation.update(success);
    }

    fn refill(&mut self) {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        self.tokens =
            (self.tokens + elapsed * self.policy.refill_per_sec).min(self.policy.capacity as f64);
        self.last_refill = Instant::now();
    }
}

/// In-memory registry of per-pubkey quotas.
#[derive(Debug, Default)]
pub struct QuotaRegistry {
    buckets: HashMap<String, PeerBucket>,
    default_policy: QuotaPolicy,
}

impl QuotaRegistry {
    pub fn new(default_policy: QuotaPolicy) -> Self {
        Self {
            buckets: HashMap::new(),
            default_policy,
        }
    }

    pub fn try_consume(&mut self, pubkey_hex: &str, n: u64) -> bool {
        let policy = self.default_policy.clone();
        self.buckets
            .entry(pubkey_hex.to_string())
            .or_insert_with(|| PeerBucket::new(policy))
            .try_consume(n)
    }

    pub fn record_outcome(&mut self, pubkey_hex: &str, success: bool) {
        let policy = self.default_policy.clone();
        self.buckets
            .entry(pubkey_hex.to_string())
            .or_insert_with(|| PeerBucket::new(policy))
            .record_outcome(success);
    }

    pub fn reputation(&self, pubkey_hex: &str) -> f64 {
        self.buckets
            .get(pubkey_hex)
            .map(|b| b.reputation.value)
            .unwrap_or(1.0)
    }
}
