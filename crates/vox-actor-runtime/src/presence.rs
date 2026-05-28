//! Cursor / awareness presence over a `Channel` (GA-25).
//!
//! Multiplayer is the composition of channels (GA-13) + CRDT (GA-15) +
//! presence; this module is the presence half. Per C4, no new
//! `multiplayer` keyword — functionality is emergent from a `@collaborative`
//! `RichText` field over a `Channel` plus a `PresenceMap` value type.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// One participant's presence snapshot — typically a cursor position plus an
/// optional selection range and free-form metadata (color, name, status).
#[derive(Debug, Clone)]
pub struct PresenceSnapshot {
    pub user_id: String,
    pub cursor: Option<CursorPosition>,
    pub selection: Option<SelectionRange>,
    /// Free-form per-app metadata (color hex, display name, etc.).
    pub metadata: HashMap<String, String>,
    pub last_heartbeat: SystemTime,
}

/// Logical cursor position. The exact semantics (line/col vs byte-offset vs
/// CRDT path) is delegated to the document model; this type is the channel
/// envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub line: u32,
    pub col: u32,
}

/// Logical selection range. End-exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: CursorPosition,
    pub end: CursorPosition,
}

/// Server-side presence map keyed by `user_id`.
///
/// Stale entries (older than `stale_after`) are removed on every read; this
/// avoids a separate sweeper thread for the simple case.
#[derive(Debug, Clone)]
pub struct PresenceMap {
    inner: Arc<RwLock<HashMap<String, PresenceSnapshot>>>,
    stale_after: Duration,
}

impl PresenceMap {
    pub fn new(stale_after: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            stale_after,
        }
    }

    /// Record a presence heartbeat. Replaces any prior snapshot for `user_id`.
    pub fn upsert(&self, snap: PresenceSnapshot) {
        let mut g = self.inner.write().expect("presence lock");
        g.insert(snap.user_id.clone(), snap);
    }

    /// Remove a participant — typically on Channel disconnect.
    pub fn remove(&self, user_id: &str) {
        let mut g = self.inner.write().expect("presence lock");
        g.remove(user_id);
    }

    /// Snapshot all currently-live participants. Stale entries are evicted as
    /// a side effect.
    pub fn snapshot(&self) -> Vec<PresenceSnapshot> {
        let now = SystemTime::now();
        // First pass: collect stale keys without holding a write lock.
        let stale: Vec<String> = {
            let g = self.inner.read().expect("presence lock");
            g.iter()
                .filter(|(_, s)| {
                    now.duration_since(s.last_heartbeat)
                        .map(|d| d > self.stale_after)
                        .unwrap_or(false)
                })
                .map(|(k, _)| k.clone())
                .collect()
        };
        // Second pass: evict + collect snapshot under a single write lock.
        let mut g = self.inner.write().expect("presence lock");
        for k in stale {
            g.remove(&k);
        }
        g.values().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.inner.read().expect("presence lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn snap(user: &str, line: u32, col: u32, last_heartbeat: SystemTime) -> PresenceSnapshot {
        PresenceSnapshot {
            user_id: user.into(),
            cursor: Some(CursorPosition { line, col }),
            selection: None,
            metadata: HashMap::new(),
            last_heartbeat,
        }
    }

    #[test]
    fn upsert_replaces_prior_snapshot() {
        let map = PresenceMap::new(Duration::from_secs(60));
        let now = SystemTime::now();
        map.upsert(snap("alice", 1, 1, now));
        map.upsert(snap("alice", 2, 5, now));
        let snaps = map.snapshot();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].cursor.unwrap().line, 2);
    }

    #[test]
    fn remove_drops_participant() {
        let map = PresenceMap::new(Duration::from_secs(60));
        let now = SystemTime::now();
        map.upsert(snap("alice", 0, 0, now));
        map.upsert(snap("bob", 0, 0, now));
        map.remove("alice");
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn snapshot_evicts_stale_entries() {
        let map = PresenceMap::new(Duration::from_millis(1));
        let past = SystemTime::now() - Duration::from_secs(10);
        map.upsert(snap("ghost", 0, 0, past));
        // Tiny sleep would be flaky; instead, give the snapshot a now that's
        // definitely past stale_after by reusing the constructor — `past` is
        // already 10s old, well past 1ms stale_after.
        let snaps = map.snapshot();
        assert!(snaps.is_empty(), "stale entry should be evicted");
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn empty_map_is_empty() {
        let map = PresenceMap::new(Duration::from_secs(60));
        assert!(map.is_empty());
    }
}
