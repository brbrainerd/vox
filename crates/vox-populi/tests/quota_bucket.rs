//! P5-T3: token-bucket drain/refill + reputation EMA tests.
//!
//! Uses bounded polling instead of fixed `thread::sleep` where refill timing is observable via `try_consume`.

use std::time::Duration;

use vox_populi::quota::{
    bucket::{PeerBucket, QuotaRegistry},
    spec::{QuotaPolicy, ReputationEma},
};

fn wait_until_sync(
    label: &str,
    timeout: Duration,
    interval: Duration,
    mut condition: impl FnMut() -> bool,
) {
    let deadline = std::time::Instant::now() + timeout;
    while !condition() {
        if std::time::Instant::now() >= deadline {
            panic!("{label}: timed out after {timeout:?}");
        }
        std::thread::sleep(interval);
    }
}

// --- Token bucket tests ---

#[test]
fn bucket_starts_full() {
    let policy = QuotaPolicy {
        capacity: 100,
        refill_per_sec: 10.0,
    };
    let mut bucket = PeerBucket::new(policy);
    // Should be able to consume up to capacity immediately.
    assert!(
        bucket.try_consume(100),
        "should consume full capacity at start"
    );
}

#[test]
fn bucket_drains_and_rejects_overflow() {
    let policy = QuotaPolicy {
        capacity: 50,
        refill_per_sec: 1.0,
    };
    let mut bucket = PeerBucket::new(policy);
    assert!(bucket.try_consume(50), "should consume all tokens");
    assert!(!bucket.try_consume(1), "empty bucket must reject request");
}

#[test]
fn bucket_partial_drain() {
    let policy = QuotaPolicy {
        capacity: 100,
        refill_per_sec: 0.0,
    };
    let mut bucket = PeerBucket::new(policy);
    assert!(bucket.try_consume(40));
    assert!(bucket.try_consume(60));
    assert!(!bucket.try_consume(1), "bucket is now empty");
}

#[test]
fn bucket_refills_over_time() {
    let policy = QuotaPolicy {
        capacity: 10,
        refill_per_sec: 1_000_000.0,
    };
    let mut bucket = PeerBucket::new(policy);
    // Drain all tokens.
    assert!(bucket.try_consume(10));
    assert!(!bucket.try_consume(1), "empty after drain");

    wait_until_sync(
        "token bucket refilled at least one token after drain",
        Duration::from_secs(2),
        Duration::from_micros(50),
        || bucket.try_consume(1),
    );
}

#[test]
fn bucket_refill_does_not_exceed_capacity() {
    let policy = QuotaPolicy {
        capacity: 5,
        refill_per_sec: 1_000_000.0,
    };
    let mut bucket = PeerBucket::new(policy);
    // Starts full; high refill rate must not push tokens above capacity.
    // Should be able to consume exactly capacity, but not more.
    assert!(bucket.try_consume(5), "capacity tokens available");
    assert!(!bucket.try_consume(1), "must not exceed capacity");
}

// --- Reputation EMA tests ---

#[test]
fn ema_starts_at_one() {
    let ema = ReputationEma::default();
    assert!(
        (ema.value - 1.0).abs() < 1e-9,
        "default value should be 1.0"
    );
}

#[test]
fn ema_moves_toward_zero_on_failures() {
    let mut ema = ReputationEma::default();
    for _ in 0..100 {
        ema.update(false);
    }
    assert!(
        ema.value < 0.01,
        "after 100 failures EMA should be near 0, got {}",
        ema.value
    );
}

#[test]
fn ema_moves_toward_one_on_successes_from_zero() {
    let mut ema = ReputationEma {
        value: 0.0,
        alpha: 0.1,
    };
    for _ in 0..100 {
        ema.update(true);
    }
    assert!(
        ema.value > 0.99,
        "after 100 successes EMA should be near 1, got {}",
        ema.value
    );
}

#[test]
fn ema_partial_recovery() {
    let mut ema = ReputationEma::default();
    // Drive reputation down.
    for _ in 0..50 {
        ema.update(false);
    }
    let low = ema.value;
    assert!(low < 0.1, "should be low after 50 failures");
    // Then recover.
    for _ in 0..50 {
        ema.update(true);
    }
    assert!(ema.value > low, "reputation should recover after successes");
}

#[test]
fn ema_alpha_controls_smoothing() {
    let mut fast = ReputationEma {
        value: 1.0,
        alpha: 0.5,
    };
    let mut slow = ReputationEma {
        value: 1.0,
        alpha: 0.1,
    };
    for _ in 0..5 {
        fast.update(false);
        slow.update(false);
    }
    assert!(fast.value < slow.value, "higher alpha means faster decay");
}

// --- QuotaRegistry tests ---

#[test]
fn registry_tracks_per_key() {
    let policy = QuotaPolicy {
        capacity: 10,
        refill_per_sec: 0.0,
    };
    let mut registry = QuotaRegistry::new(policy);
    assert!(registry.try_consume("peer_a", 10));
    assert!(!registry.try_consume("peer_a", 1), "peer_a exhausted");
    // peer_b is a separate bucket and should be fresh.
    assert!(registry.try_consume("peer_b", 10));
}

#[test]
fn registry_records_outcome_updates_reputation() {
    let policy = QuotaPolicy::default();
    let mut registry = QuotaRegistry::new(policy);
    let key = "peer_test";
    assert!(
        (registry.reputation(key) - 1.0).abs() < 1e-9,
        "unknown key defaults to 1.0"
    );
    for _ in 0..20 {
        registry.record_outcome(key, false);
    }
    assert!(
        registry.reputation(key) < 0.2,
        "reputation should drop after failures"
    );
}
