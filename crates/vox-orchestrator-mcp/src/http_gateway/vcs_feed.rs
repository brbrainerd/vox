//! WebSocket topic for live VCS isolation-policy changes.
//!
//! Unlike `scientia.queue.changed` (a background DB poller), isolation policy is
//! **user-driven**: the only mutation path is `POST /api/v2/vcs/isolation/strategy`.
//! That handler publishes a [`super::scientia_feed::TopicMessage`] on this topic
//! after writing the live `IsolationPlan`, so subscribed dashboards re-fetch. A
//! background poller is intentionally not required for P4 (conflict-driven pushes
//! can be a follow-up).

/// Canonical topic name for isolation-policy changes (sibling to
/// [`super::scientia_feed::SCIENTIA_QUEUE_CHANGED`]).
pub(crate) const VCS_ISOLATION_CHANGED: &str = "vcs.isolation.changed";

#[cfg(test)]
mod tests {
    use super::super::scientia_feed::{TOPIC_CHANNEL_CAPACITY, TopicMessage, should_forward};
    use super::*;
    use std::collections::HashSet;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn isolation_topic_is_forwarded_only_to_subscribers() {
        let (tx, _rx0) = broadcast::channel::<TopicMessage>(TOPIC_CHANNEL_CAPACITY);
        let mut rx = tx.subscribe();
        tx.send(TopicMessage {
            topic: VCS_ISOLATION_CHANGED.to_string(),
            data: serde_json::json!({ "strategy_default": "shared_branch" }),
        })
        .expect("send should succeed with a live subscriber");
        let msg = rx.recv().await.expect("subscriber should receive message");

        let mut subs = HashSet::new();
        subs.insert(VCS_ISOLATION_CHANGED.to_string());
        assert!(should_forward(&subs, &msg.topic));

        let empty = HashSet::new();
        assert!(!should_forward(&empty, &msg.topic));
    }
}
