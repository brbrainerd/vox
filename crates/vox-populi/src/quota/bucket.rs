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

#[cfg(test)]
mod semcov_wave15_tests {
    use super::*;
    use crate::quota::spec::{QuotaPolicy, ReputationEma};

    fn tight_policy() -> QuotaPolicy {
        QuotaPolicy {
            capacity: 10,
            refill_per_sec: 0.0, // no refill so tokens are finite and predictable
        }
    }

    // ── PeerBucket ───────────────────────────────────────────────────────────

    #[test]
    fn zero_capacity_bucket_rejects_every_request() {
        // Catches: try_consume returning true when tokens == 0 and n == 0 (unsigned
        // underflow or ">= 0" always-true for u64).
        let mut b = PeerBucket::new(QuotaPolicy {
            capacity: 0,
            refill_per_sec: 0.0,
        });
        assert!(!b.try_consume(1), "zero-capacity bucket must reject consume(1)");
    }

    #[test]
    fn consume_exactly_capacity_succeeds_then_next_fails() {
        // Catches: off-by-one where consuming exactly `capacity` tokens either
        // fails when it should succeed, or leaves the bucket reporting 0 remaining
        // but still allowing another consume.
        let cap = 5;
        let mut b = PeerBucket::new(QuotaPolicy {
            capacity: cap,
            refill_per_sec: 0.0,
        });
        assert!(b.try_consume(cap), "consuming exactly capacity must succeed");
        assert!(!b.try_consume(1), "bucket should be empty after full drain");
    }

    #[test]
    fn reputation_converges_toward_zero_on_all_failures() {
        // Catches: EMA update ignoring the signal (e.g. always multiplying by alpha
        // without adding the signal term), leaving reputation stuck at 1.0 on failures.
        let mut ema = ReputationEma::default(); // starts at 1.0
        for _ in 0..200 {
            ema.update(false);
        }
        assert!(
            ema.value < 0.01,
            "after 200 failures reputation must be near 0, got {}",
            ema.value
        );
    }

    #[test]
    fn reputation_ordering_matters_success_then_failure_differs_from_reverse() {
        // Catches: EMA commutativity bug — if update() were commutative, the order
        // of outcomes would not affect the final reputation.
        let mut forward = ReputationEma::default();
        let mut reverse = ReputationEma::default();
        // forward: 3 successes then 3 failures
        for &ok in &[true, true, true, false, false, false] {
            forward.update(ok);
        }
        // reverse: 3 failures then 3 successes
        for &ok in &[false, false, false, true, true, true] {
            reverse.update(ok);
        }
        assert!(
            (forward.value - reverse.value).abs() > 1e-9,
            "EMA must NOT be commutative: forward={}, reverse={}",
            forward.value,
            reverse.value
        );
    }

    // ── QuotaRegistry ───────────────────────────────────────────────────────

    #[test]
    fn unknown_peer_reputation_defaults_to_one() {
        // Catches: reputation() returning 0.0 (the "not found" branch uses the wrong
        // default) instead of the optimistic 1.0.
        let r = QuotaRegistry::new(tight_policy());
        assert_eq!(
            r.reputation("never-seen"),
            1.0,
            "unseen peer must have default reputation 1.0"
        );
    }

    #[test]
    fn registry_isolates_buckets_per_peer() {
        // Catches: shared token state across keys (e.g. the HashMap key is ignored
        // and a single global bucket is used).
        let mut r = QuotaRegistry::new(tight_policy());
        // Drain peer A's bucket completely.
        for _ in 0..10 {
            r.try_consume("peer-a", 1);
        }
        // Peer B must still have full capacity.
        assert!(
            r.try_consume("peer-b", 1),
            "draining peer-a must not affect peer-b's bucket"
        );
    }

    #[test]
    fn record_outcome_does_not_create_phantom_bucket_for_reputation_query() {
        // Catches: record_outcome inserting a zero-EMA bucket that then makes
        // reputation() return 0.0 for a peer that just had one outcome recorded
        // (the insert path must start at the default EMA value, not 0.0).
        let mut r = QuotaRegistry::new(tight_policy());
        r.record_outcome("peer-x", true); // one success
        let rep = r.reputation("peer-x");
        // After exactly one success starting from EMA=1.0:
        // new = 0.1*1 + 0.9*1 = 1.0 — so reputation should still be 1.0.
        assert!(
            (rep - 1.0).abs() < 1e-9,
            "one success from initial EMA=1 must keep reputation=1.0, got {rep}"
        );
    }
}
