//! Topic-multiplex WebSocket feed for the HTTP gateway.
//!
//! Provides a real change-source for the `scientia.queue.changed` topic: a
//! background poller recomputes the scientia [`QueueSnapshot`] from the Codex DB
//! every [`POLL_INTERVAL`] and, only when the serialized snapshot **differs**
//! from the previous one, broadcasts a [`TopicMessage`] to subscribed WS clients.
//!
//! This is genuinely driven by DB state — not a fabricated/timer event — so a
//! quiet queue produces no messages.

use std::collections::HashSet;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::broadcast;

use super::dashboard_api::assemble_scientia_queue;

/// Canonical topic name for scientia queue changes.
pub(crate) const SCIENTIA_QUEUE_CHANGED: &str = "scientia.queue.changed";

/// How often the poller recomputes the queue snapshot. Worst-case latency for a
/// change to reach subscribers is one interval.
pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Capacity of the topic broadcast channel.
pub(crate) const TOPIC_CHANNEL_CAPACITY: usize = 64;

/// A topic-tagged payload broadcast to all WS connections; each connection
/// forwards it only if it has subscribed to `topic`.
#[derive(Debug, Clone)]
pub(crate) struct TopicMessage {
    pub topic: String,
    pub data: Value,
}

/// Returns true when a message on `topic` should be forwarded to a connection
/// whose subscription set is `subscribed`.
pub(crate) fn should_forward(subscribed: &HashSet<String>, topic: &str) -> bool {
    subscribed.contains(topic)
}

/// Spawn the background poller that drives the `scientia.queue.changed` topic.
///
/// Tolerates DB errors (logs and continues). Sends only on change, comparing the
/// serialized snapshot to the previously-broadcast value.
pub(crate) fn spawn_scientia_queue_poller(sender: broadcast::Sender<TopicMessage>) {
    tokio::spawn(async move {
        let mut last_serialized: Option<String> = None;
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        loop {
            ticker.tick().await;
            match assemble_scientia_queue().await {
                Ok(snapshot) => {
                    let value = match serde_json::to_value(&snapshot) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!("scientia queue poller: serialize failed: {e}");
                            continue;
                        }
                    };
                    let serialized = value.to_string();
                    let changed = last_serialized.as_deref() != Some(serialized.as_str());
                    if changed {
                        last_serialized = Some(serialized);
                        // A send error means no subscribers are listening — that
                        // is fine; the next change will retry from a fresh state.
                        let _ = sender.send(TopicMessage {
                            topic: SCIENTIA_QUEUE_CHANGED.to_string(),
                            data: value,
                        });
                    }
                }
                Err(e) => {
                    tracing::debug!("scientia queue poller: assemble failed: {e}");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_forward_only_for_subscribed_topics() {
        let mut subs = HashSet::new();
        assert!(!should_forward(&subs, SCIENTIA_QUEUE_CHANGED));
        subs.insert(SCIENTIA_QUEUE_CHANGED.to_string());
        assert!(should_forward(&subs, SCIENTIA_QUEUE_CHANGED));
        assert!(!should_forward(&subs, "some.other.topic"));
    }

    #[tokio::test]
    async fn subscriber_receives_matching_topic_message() {
        let (tx, _rx0) = broadcast::channel::<TopicMessage>(TOPIC_CHANNEL_CAPACITY);
        let mut rx = tx.subscribe();
        tx.send(TopicMessage {
            topic: SCIENTIA_QUEUE_CHANGED.to_string(),
            data: serde_json::json!({ "depth": 3 }),
        })
        .expect("send should succeed with a live subscriber");
        let msg = rx.recv().await.expect("subscriber should receive message");

        let mut subs = HashSet::new();
        subs.insert(SCIENTIA_QUEUE_CHANGED.to_string());
        assert!(should_forward(&subs, &msg.topic));
        assert_eq!(msg.data, serde_json::json!({ "depth": 3 }));

        // A connection subscribed to nothing must not forward it.
        let empty = HashSet::new();
        assert!(!should_forward(&empty, &msg.topic));
    }
}
