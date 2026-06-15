//! Tombstone gossip for revoked attestations (P5-T2c).

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct RevocationGossip {
    revoked_at: HashMap<String, Instant>,
    retention: Duration,
}

impl RevocationGossip {
    pub fn new(retention: Duration) -> Self {
        Self {
            revoked_at: HashMap::new(),
            retention,
        }
    }

    pub fn tombstone(&mut self, pubkey_hex: String) {
        self.revoked_at.insert(pubkey_hex, Instant::now());
    }

    pub fn is_revoked(&self, pubkey_hex: &str) -> bool {
        self.revoked_at.contains_key(pubkey_hex)
    }

    /// Garbage-collect tombstones older than `retention`.
    pub fn gc(&mut self) {
        let now = Instant::now();
        let retention = self.retention;
        self.revoked_at
            .retain(|_, t| now.saturating_duration_since(*t) < retention);
    }
}

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn tombstone_then_is_revoked_reports_true_only_for_that_key() {
        // Catches: is_revoked ignoring the key argument (e.g. returning
        // `!map.is_empty()` for any input) so an unrelated key reads as revoked.
        let mut g = RevocationGossip::new(Duration::from_secs(3600));
        g.tombstone("aa11".to_string());
        assert!(g.is_revoked("aa11"), "tombstoned key must read revoked");
        assert!(
            !g.is_revoked("bb22"),
            "untombstoned key must NOT read revoked"
        );
    }

    #[test]
    fn gc_with_zero_retention_evicts_all_tombstones() {
        // Catches: gc using `<=` vs `<` or wrong comparison direction so a
        // zero-retention window keeps (rather than drops) every tombstone.
        let mut g = RevocationGossip::new(Duration::from_secs(0));
        g.tombstone("dead".to_string());
        g.gc();
        assert!(
            !g.is_revoked("dead"),
            "zero retention must evict tombstone on gc"
        );
    }

    #[test]
    fn gc_with_long_retention_keeps_fresh_tombstone() {
        // Catches: gc over-aggressively retaining nothing (e.g. inverted retain
        // predicate) so a brand-new tombstone is dropped under a 1-hour window.
        let mut g = RevocationGossip::new(Duration::from_secs(3600));
        g.tombstone("fresh".to_string());
        g.gc();
        assert!(
            g.is_revoked("fresh"),
            "fresh tombstone must survive gc under long retention"
        );
    }
}
