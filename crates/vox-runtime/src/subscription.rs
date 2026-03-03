//! Reactive subscription manager for table-level change notifications.
//!
//! Uses `tokio::sync::broadcast` channels to notify subscribers when
//! a table's data has been mutated. This powers SSE-based reactive queries.
//!
//! # Architecture
//!
//! ```text
//!   @mutation insert_task()
//!       │
//!       ▼
//!   SubscriptionManager::notify("tasks")
//!       │
//!       ▼
//!   broadcast::Sender<()> ──► all Receivers for "tasks"
//!       │
//!       ▼
//!   SSE endpoint re-runs @query list_tasks()
//!       │
//!       ▼
//!   Client gets updated result
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

/// Default capacity for broadcast channels per table.
const DEFAULT_CHANNEL_CAPACITY: usize = 64;

/// Manages per-table broadcast channels for reactive query subscriptions.
///
/// When a `@mutation` commits, it calls `notify()` with the affected table names.
/// SSE subscription endpoints hold `Receiver` handles and re-run their queries
/// when notified.
#[derive(Clone)]
pub struct SubscriptionManager {
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<()>>>>,
}

impl SubscriptionManager {
    /// Create a new, empty subscription manager.
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Subscribe to change notifications for a specific table.
    /// Returns a `broadcast::Receiver` that fires whenever the table is mutated.
    pub fn subscribe(&self, table: &str) -> broadcast::Receiver<()> {
        let channels = self.channels.read().expect("subscription lock poisoned");
        if let Some(sender) = channels.get(table) {
            return sender.subscribe();
        }
        drop(channels);

        // Table not yet tracked — create a new channel
        let mut channels = self.channels.write().expect("subscription lock poisoned");
        let sender = channels
            .entry(table.to_string())
            .or_insert_with(|| broadcast::channel(DEFAULT_CHANNEL_CAPACITY).0);
        sender.subscribe()
    }

    /// Notify all subscribers that a single table has been mutated.
    pub fn notify(&self, table: &str) {
        let channels = self.channels.read().expect("subscription lock poisoned");
        if let Some(sender) = channels.get(table) {
            let count = sender.receiver_count();
            tracing::debug!(
                table = table,
                subscribers = count,
                "subscription notification fired"
            );
            // Ignore send errors (no active receivers is fine)
            let _ = sender.send(());
        }
    }

    /// Notify all subscribers for multiple tables at once.
    /// Typically called after a `@mutation` commits.
    pub fn notify_tables(&self, tables: &[&str]) {
        let channels = self.channels.read().expect("subscription lock poisoned");
        for table in tables {
            if let Some(sender) = channels.get(*table) {
                let _ = sender.send(());
            }
        }
    }

    /// Subscribe to change notifications for multiple tables.
    /// Returns receivers for each table.
    pub fn subscribe_tables(&self, tables: &[&str]) -> Vec<broadcast::Receiver<()>> {
        tables.iter().map(|t| self.subscribe(t)).collect()
    }

    /// Number of active subscribers for a given table.
    pub fn subscriber_count(&self, table: &str) -> usize {
        self.channels
            .read()
            .expect("subscription lock poisoned")
            .get(table)
            .map(|s| s.receiver_count())
            .unwrap_or(0)
    }

    /// Notify all subscribers for all tracked tables.
    pub fn notify_all(&self) {
        let channels = self.channels.read().expect("subscription lock poisoned");
        for sender in channels.values() {
            let _ = sender.send(());
        }
    }

    /// Remove all subscription channels (for graceful shutdown).
    pub fn unsubscribe_all(&self) {
        self.channels
            .write()
            .expect("subscription lock poisoned")
            .clear();
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe_and_notify() {
        let mgr = SubscriptionManager::new();
        let mut rx = mgr.subscribe("tasks");

        mgr.notify("tasks");

        let result = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await;

        assert!(result.is_ok(), "should receive notification");
    }

    #[tokio::test]
    async fn test_no_notification_for_other_table() {
        let mgr = SubscriptionManager::new();
        let mut rx = mgr.subscribe("tasks");

        mgr.notify("users"); // different table

        let result = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;

        assert!(
            result.is_err(),
            "should NOT receive notification for unrelated table"
        );
    }

    #[tokio::test]
    async fn test_notify_tables_multiple() {
        let mgr = SubscriptionManager::new();
        let mut rx_tasks = mgr.subscribe("tasks");
        let mut rx_users = mgr.subscribe("users");

        mgr.notify_tables(&["tasks", "users"]);

        assert!(rx_tasks.recv().await.is_ok());
        assert!(rx_users.recv().await.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let mgr = SubscriptionManager::new();
        let mut rx1 = mgr.subscribe("tasks");
        let mut rx2 = mgr.subscribe("tasks");

        mgr.notify("tasks");

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }
}
