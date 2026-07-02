//! Hopper → agent-queue dispatch: pure mapping + the dispatcher loop.

use crate::hopper::types::IntakeItem;
use crate::types::{AgentTask, TaskId, TaskPriority};
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

/// Runs the priority/cancel cascade loop:
/// - HopperItemOverridden: updates the task's priority on agent queue.
/// - HopperItemCancelled: cancels the task on agent queue.
pub async fn run_cascade(
    mut rx: tokio::sync::broadcast::Receiver<crate::events::AgentEvent>,
    on_reprioritize: impl Fn(TaskId, TaskPriority) + Send + 'static,
    on_cancel: impl Fn(TaskId) + Send + 'static,
    max_events: Option<usize>,
) {
    let mut seen = 0usize;
    while let Ok(ev) = rx.recv().await {
        match ev.kind {
            crate::events::AgentEventKind::HopperItemOverridden {
                item_id,
                developer_priority,
                ..
            } => {
                let task_id = TaskId(stable_hash(&item_id.0));
                on_reprioritize(task_id, developer_priority);
                seen += 1;
            }
            crate::events::AgentEventKind::HopperItemCancelled { item_id } => {
                let task_id = TaskId(stable_hash(&item_id.0));
                on_cancel(task_id);
                seen += 1;
            }
            _ => {}
        }
        if Some(seen) == max_events {
            break;
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

    #[tokio::test]
    async fn override_event_triggers_reprioritize_callback() {
        use crate::events::AgentEventKind;
        let bus = Arc::new(EventBus::new(16));
        let rx = bus.subscribe();
        let reprioritized = Arc::new(Mutex::new(Vec::new()));
        let sink = reprioritized.clone();
        let handle = tokio::spawn(run_cascade(
            rx,
            move |id, _p| sink.lock().unwrap().push(id),
            |_| {},
            Some(1),
        ));
        bus.emit(AgentEventKind::HopperItemOverridden {
            item_id: crate::events::HopperItemId("test_item".to_string()),
            original_priority: TaskPriority::Normal,
            developer_priority: TaskPriority::Urgent,
            delta_seconds_since_admit: 0,
        });
        handle.await.unwrap();
        assert_eq!(reprioritized.lock().unwrap().len(), 1);
        assert_eq!(
            reprioritized.lock().unwrap()[0],
            TaskId(stable_hash("test_item"))
        );
    }

    #[tokio::test]
    async fn cancel_event_triggers_cancel_callback() {
        use crate::events::AgentEventKind;
        let bus = Arc::new(EventBus::new(16));
        let rx = bus.subscribe();
        let cancelled = Arc::new(Mutex::new(Vec::new()));
        let sink = cancelled.clone();
        let handle = tokio::spawn(run_cascade(
            rx,
            |_id, _p| {},
            move |id| sink.lock().unwrap().push(id),
            Some(1),
        ));
        bus.emit(AgentEventKind::HopperItemCancelled {
            item_id: crate::events::HopperItemId("test_item".to_string()),
        });
        handle.await.unwrap();
        assert_eq!(cancelled.lock().unwrap().len(), 1);
        assert_eq!(
            cancelled.lock().unwrap()[0],
            TaskId(stable_hash("test_item"))
        );
    }
}
