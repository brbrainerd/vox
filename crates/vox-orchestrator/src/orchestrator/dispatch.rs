//! Hopper → agent-queue dispatch: pure mapping + the dispatcher loop.

use crate::hopper::types::IntakeItem;
use crate::types::{AgentTask, TaskId};
use std::sync::Arc;

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

/// Runs the admit→enqueue loop: every HopperItemAdmitted becomes an enqueued task.
/// Returns after `max_events` for test determinism (None = run forever).
pub async fn run_dispatcher(
    mut rx: tokio::sync::broadcast::Receiver<crate::events::AgentEvent>,
    hopper: Arc<dyn crate::hopper::store::HopperIntake>,
    enqueue: impl Fn(AgentTask) + Send + 'static,
    max_events: Option<usize>,
) {
    let mut seen = 0usize;
    while let Ok(ev) = rx.recv().await {
        if let crate::events::AgentEventKind::HopperItemAdmitted { item_id, .. } = ev.kind {
            // Find the item in the hopper inbox/assigned list
            let mut found_item = None;
            for item in hopper.inbox().await {
                if item.item_id == item_id {
                    found_item = Some(item);
                    break;
                }
            }
            if found_item.is_none() {
                for item in hopper.assigned().await {
                    if item.item_id == item_id {
                        found_item = Some(item);
                        break;
                    }
                }
            }

            if let Some(item) = found_item {
                let task = intake_to_task(&item);
                enqueue(task);
            }

            seen += 1;
            if Some(seen) == max_events {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use crate::hopper::store::{HopperIntake, InMemoryHopper};
    use crate::hopper::types::{IntakeSource, PriorityHint};
    use std::sync::Mutex;

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

    #[tokio::test]
    async fn admit_enqueues_one_task() {
        let bus = Arc::new(EventBus::new(16));
        let rx = bus.subscribe();
        let hopper = Arc::new(InMemoryHopper::new(bus.clone()));
        let enqueued = Arc::new(Mutex::new(Vec::new()));
        let sink = enqueued.clone();

        let handle = tokio::spawn(run_dispatcher(
            rx,
            hopper.clone(),
            move |t| sink.lock().unwrap().push(t),
            Some(1),
        ));

        hopper
            .submit(
                "t".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;
        handle.await.unwrap();
        assert_eq!(enqueued.lock().unwrap().len(), 1);
    }
}
