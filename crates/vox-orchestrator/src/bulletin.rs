use tokio::sync::broadcast;

use crate::types::AgentMessage;

/// Default capacity for the bulletin board broadcast channel.
const DEFAULT_BULLETIN_CAPACITY: usize = 256;

/// Cross-agent communication channel using broadcast pub/sub.
///
/// Agents publish messages (file changes, task completions, interrupts)
/// and all other agents receive them. This follows the same pattern as
/// `vox-runtime::SubscriptionManager` but for orchestrator-level events.
#[derive(Clone)]
pub struct BulletinBoard {
    sender: broadcast::Sender<AgentMessage>,
}

impl BulletinBoard {
    /// Create a new bulletin board with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish a message to all subscribed agents.
    pub fn publish(&self, msg: AgentMessage) {
        let count = self.sender.receiver_count();
        tracing::debug!(
            subscribers = count,
            "bulletin board message published: {:?}",
            msg
        );
        // Ignore send errors (no active receivers is fine)
        let _ = self.sender.send(msg);
    }

    /// Subscribe to bulletin board messages.
    /// Returns a receiver that will get all future messages.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentMessage> {
        self.sender.subscribe()
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for BulletinBoard {
    fn default() -> Self {
        Self::new(DEFAULT_BULLETIN_CAPACITY)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentId, TaskId};

    #[tokio::test]
    async fn publish_and_receive() {
        let board = BulletinBoard::new(16);
        let mut rx = board.subscribe();

        board.publish(AgentMessage::TaskCompleted {
            task_id: TaskId(1),
            agent_id: AgentId(1),
        });

        let msg = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .expect("should not timeout")
            .expect("should receive message");

        match msg {
            AgentMessage::TaskCompleted { task_id, agent_id } => {
                assert_eq!(task_id, TaskId(1));
                assert_eq!(agent_id, AgentId(1));
            }
            _ => panic!("wrong message variant"),
        }
    }

    #[tokio::test]
    async fn multiple_subscribers() {
        let board = BulletinBoard::new(16);
        let mut rx1 = board.subscribe();
        let mut rx2 = board.subscribe();

        assert_eq!(board.subscriber_count(), 2);

        board.publish(AgentMessage::DependencyReady { task_id: TaskId(5) });

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[tokio::test]
    async fn no_subscribers_does_not_panic() {
        let board = BulletinBoard::new(16);
        // Publishing with no subscribers should not panic
        board.publish(AgentMessage::Interrupt {
            agent_id: AgentId(1),
            reason: "test".to_string(),
        });
    }
}
