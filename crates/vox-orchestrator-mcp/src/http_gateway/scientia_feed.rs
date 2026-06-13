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

/// Canonical topic name for newly-surfaced discovery candidates (inbox rows).
pub(crate) const SCIENTIA_DISCOVERY_SURFACED: &str = "scientia.discovery.surfaced";

/// Max inbox rows fetched (and broadcast) per poll tick.
pub(crate) const DISCOVERY_BATCH_LIMIT: i64 = 64;

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
        // Highest discovery-inbox id already broadcast. Initialised from the
        // current MAX so a fresh poller does not replay pre-existing rows; on a
        // DB error we leave it at 0 and recover on a later tick.
        let mut last_max_inbox_id: i64 = match vox_db::VoxDb::connect_default().await {
            Ok(db) => db.max_discovery_inbox_id().await.unwrap_or(0),
            Err(e) => {
                tracing::debug!("scientia discovery poller: initial connect failed: {e}");
                0
            }
        };
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        loop {
            ticker.tick().await;

            // Topic 2: newly-surfaced discovery candidates (inbox diff on max id).
            poll_discovery_surfaced(&sender, &mut last_max_inbox_id).await;

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

/// JSON shape broadcast for one inbox row on `scientia.discovery.surfaced`.
/// (`DiscoveryInboxRow` is not `Serialize`, so the wire form is built here.)
fn discovery_row_to_json(row: &vox_db::DiscoveryInboxRow) -> Value {
    serde_json::json!({
        "id": row.id,
        "publication_id": row.publication_id,
        "surfaced_at_ms": row.surfaced_at_ms,
        "intake_tier": row.intake_tier,
        "signal_codes": row.signal_codes,
        "acknowledged_at_ms": row.acknowledged_at_ms,
    })
}

/// One tick of the discovery-surfaced diff: if new inbox rows exist beyond
/// `last_max_inbox_id`, broadcast them (oldest-first) and advance the watermark.
/// DB errors are logged and skipped (the watermark is only advanced on success).
async fn poll_discovery_surfaced(
    sender: &broadcast::Sender<TopicMessage>,
    last_max_inbox_id: &mut i64,
) {
    let db = match vox_db::VoxDb::connect_default().await {
        Ok(db) => db,
        Err(e) => {
            tracing::debug!("scientia discovery poller: connect failed: {e}");
            return;
        }
    };
    let current_max = match db.max_discovery_inbox_id().await {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("scientia discovery poller: max id failed: {e}");
            return;
        }
    };
    if current_max <= *last_max_inbox_id {
        return; // quiet — no new candidates.
    }
    let rows = match db
        .discoveries_since(*last_max_inbox_id, DISCOVERY_BATCH_LIMIT)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::debug!("scientia discovery poller: discoveries_since failed: {e}");
            return;
        }
    };
    if rows.is_empty() {
        // Defensive: max advanced but no rows returned — re-anchor and bail.
        *last_max_inbox_id = current_max;
        return;
    }
    let data = Value::Array(rows.iter().map(discovery_row_to_json).collect());
    // The largest id we actually fetched (rows are id-ascending).
    let new_max = rows.last().map_or(current_max, |r| r.id);
    // No subscribers is fine; the next change retries from the advanced anchor.
    let _ = sender.send(TopicMessage {
        topic: SCIENTIA_DISCOVERY_SURFACED.to_string(),
        data,
    });
    *last_max_inbox_id = new_max;
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

    #[test]
    fn discovery_surfaced_topic_is_independent() {
        // The two scientia topics are distinct: subscribing to one must not
        // forward the other.
        let mut subs = HashSet::new();
        subs.insert(SCIENTIA_DISCOVERY_SURFACED.to_string());
        assert!(should_forward(&subs, SCIENTIA_DISCOVERY_SURFACED));
        assert!(!should_forward(&subs, SCIENTIA_QUEUE_CHANGED));
        assert_ne!(SCIENTIA_DISCOVERY_SURFACED, SCIENTIA_QUEUE_CHANGED);
    }

    #[tokio::test]
    async fn discovery_poller_broadcasts_new_inbox_rows_once() {
        use vox_db::{DbConfig, VoxDb};

        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let (tx, mut rx) = broadcast::channel::<TopicMessage>(TOPIC_CHANNEL_CAPACITY);

        // Mirror the poller's diff logic against an in-memory DB (the real
        // poller connects via connect_default, which a unit test cannot pin).
        let mut last_max = db.max_discovery_inbox_id().await.expect("max");
        assert_eq!(last_max, 0);

        let id = db
            .insert_discovery_inbox(
                "commit-feed",
                1_234,
                "review_suggested",
                r#"["perf_claim"]"#,
            )
            .await
            .expect("insert");

        let current_max = db.max_discovery_inbox_id().await.expect("max2");
        assert!(current_max > last_max);
        let rows = db
            .discoveries_since(last_max, DISCOVERY_BATCH_LIMIT)
            .await
            .expect("since");
        let data = Value::Array(rows.iter().map(discovery_row_to_json).collect());
        tx.send(TopicMessage {
            topic: SCIENTIA_DISCOVERY_SURFACED.to_string(),
            data,
        })
        .expect("send with live subscriber");
        last_max = rows.last().map_or(current_max, |r| r.id);
        assert_eq!(last_max, id);

        let msg = rx.recv().await.expect("subscriber receives message");
        assert_eq!(msg.topic, SCIENTIA_DISCOVERY_SURFACED);
        let arr = msg.data.as_array().expect("payload is an array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["publication_id"], "commit-feed");
        assert_eq!(arr[0]["intake_tier"], "review_suggested");
        assert_eq!(arr[0]["id"], id);

        // A second diff with no new rows produces nothing (quiet inbox).
        let next_max = db.max_discovery_inbox_id().await.expect("max3");
        assert_eq!(next_max, last_max, "no new rows → no advance");
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
