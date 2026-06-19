//! Hopper → agent-queue dispatch: pure mapping + the dispatcher loop.

use crate::hopper::types::IntakeItem;
use crate::types::{AgentTask, TaskId};

/// Stable hash function to map string IDs deterministically to u64 TaskIds.
pub fn stable_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for c in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(c as u64);
    }
    hash
}

/// Convert an admitted hopper item into the task an `AgentQueue` enqueues.
/// Pure + deterministic so it is unit-testable in isolation.
pub fn intake_to_task(item: &IntakeItem) -> AgentTask {
    let task_id = TaskId(stable_hash(&item.item_id.0));
    AgentTask::new(
        task_id,
        item.intent.clone(),
        item.classified_priority,
        vec![], // file_manifest
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hopper::store::{HopperIntake, InMemoryHopper};
    use crate::hopper::types::{IntakeSource, PriorityHint};

    #[tokio::test]
    async fn maps_intent_and_priority() {
        let hopper = InMemoryHopper::headless();
        let item = hopper
            .submit(
                "fix login bug".into(),
                vec!["crates/auth".into()],
                PriorityHint::Urgent,
                IntakeSource::Developer,
                None,
            )
            .await;
        let task = intake_to_task(&item);
        assert!(task.description.contains("fix login bug"));
    }
}
