//! Reactive config: a process-wide watch channel bumped on any config write
//! (set_user_config / toml reload / mesh-sync). GUI/agents re-pull instead of poll.
//!
//! Complements the callback-based [`crate::snapshot`] module: `snapshot` invalidates
//! derived caches synchronously; `ConfigWatch` exposes an async-friendly
//! `tokio::sync::watch` channel for subscribers that prefer pull-on-change.

use std::sync::OnceLock;

use tokio::sync::watch;

/// Point-in-time view of the config revision and the keys changed in the last bump.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigSnapshot {
    /// Monotonic revision; advances by 1 per [`ConfigWatch::bump`].
    pub rev: u64,
    /// Keys changed in this bump (empty = general reload — refetch all).
    pub changed_keys: Vec<String>,
}

/// Watch channel for config changes. Clone [`subscribe`](Self::subscribe) receivers
/// to fan out to multiple consumers.
pub struct ConfigWatch {
    tx: watch::Sender<ConfigSnapshot>,
    rx: watch::Receiver<ConfigSnapshot>,
}

impl ConfigWatch {
    /// Create a new watch channel at revision 0.
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(ConfigSnapshot::default());
        Self { tx, rx }
    }

    /// Subscribe to config-change notifications.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<ConfigSnapshot> {
        self.rx.clone()
    }

    /// Advance the revision and notify all subscribers.
    pub fn bump(&self, keys: &[&str]) {
        let mut snap = self.tx.borrow().clone();
        snap.rev += 1;
        snap.changed_keys = keys.iter().map(|k| (*k).to_string()).collect();
        let _ = self.tx.send(snap);
    }

    /// Current snapshot without subscribing.
    #[must_use]
    pub fn current(&self) -> ConfigSnapshot {
        self.tx.borrow().clone()
    }
}

impl Default for ConfigWatch {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL: OnceLock<ConfigWatch> = OnceLock::new();

/// Process-wide config watch channel (lazy-init on first access).
#[must_use]
pub fn global() -> &'static ConfigWatch {
    GLOBAL.get_or_init(ConfigWatch::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_increments_rev_and_records_keys() {
        let w = ConfigWatch::new();
        let rx = w.subscribe();
        assert_eq!(rx.borrow().rev, 0);
        w.bump(&["VOX_WASM_SKILL_FUEL"]);
        assert_eq!(rx.borrow().rev, 1);
        assert_eq!(rx.borrow().changed_keys, vec!["VOX_WASM_SKILL_FUEL"]);
    }
}
