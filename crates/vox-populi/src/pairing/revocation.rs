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
