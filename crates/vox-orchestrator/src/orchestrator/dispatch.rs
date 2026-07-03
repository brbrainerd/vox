//! Hopper → agent-queue dispatch: pure mapping + the dispatcher loop.

use crate::hopper::types::IntakeItem;
use crate::types::{AgentId, AgentTask, TaskId, TaskPriority};
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
///
/// T1.1 (hopper wiring, harness reliability spec Phase 1): this is the real
/// production admit→dispatch site (spawned from `Orchestrator::new`,
/// `orchestrator/core/mod.rs`). `enqueue` now returns the `AgentId` the task
/// was actually routed to (or `None` if no agent was available), which lets
/// this loop call the real `HopperIntake::assign` — previously unreachable
/// from any production call site — and durably record `HopperAssign`
/// alongside it. `oplog` is optional so existing unit tests that construct
/// `run_dispatcher` without a durable log keep working unchanged.
pub async fn run_dispatcher(
    rx: tokio::sync::broadcast::Receiver<crate::events::AgentEvent>,
    hopper: Arc<dyn crate::hopper::store::HopperIntake>,
    enqueue: impl Fn(AgentTask) -> Option<AgentId> + Send + 'static,
    max_events: Option<usize>,
) {
    run_dispatcher_with_oplog(rx, hopper, enqueue, max_events, None).await
}

/// Same as [`run_dispatcher`], additionally recording `HopperAssign` (and, on
/// first sight of an item, `HopperAdmit`) to the durable op-log when `oplog`
/// is `Some`. Split out so production wiring (which has an oplog handle) and
/// existing tests (which don't) share one implementation.
pub async fn run_dispatcher_with_oplog(
    mut rx: tokio::sync::broadcast::Receiver<crate::events::AgentEvent>,
    hopper: Arc<dyn crate::hopper::store::HopperIntake>,
    enqueue: impl Fn(AgentTask) -> Option<AgentId> + Send + 'static,
    max_events: Option<usize>,
    oplog: Option<Arc<std::sync::RwLock<crate::oplog::OpLog>>>,
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
                if let Some(log) = oplog.as_ref() {
                    record_hopper_op(
                        log,
                        crate::oplog::OperationKind::HopperAdmit {
                            item_id: item.item_id.0.clone(),
                        },
                        format!("Hopper item {} admitted", item.item_id.0),
                    );
                }

                let task = intake_to_task(&item);
                if let Some(agent_id) = enqueue(task) {
                    // Real production caller of `HopperIntake::assign` (previously
                    // unreachable outside tests — see hopper/store.rs). Only record
                    // the `HopperAssign` oplog entry when the real assign actually
                    // succeeded — mirrors the `HopperComplete` gating in
                    // `task_dispatch/complete/success/mod.rs` (`hopper.complete(..)
                    // .await.is_ok()`), so the oplog never claims a hopper state
                    // transition that didn't really happen.
                    if hopper
                        .assign(&item.item_id, agent_id.to_string())
                        .await
                        .is_ok()
                    {
                        if let Some(log) = oplog.as_ref() {
                            record_hopper_op(
                                log,
                                crate::oplog::OperationKind::HopperAssign {
                                    item_id: item.item_id.0.clone(),
                                    task_id: stable_hash(&item.item_id.0),
                                },
                                format!(
                                    "Hopper item {} assigned to agent {}",
                                    item.item_id.0, agent_id
                                ),
                            );
                        }
                    } else {
                        tracing::warn!(
                            item_id = %item.item_id.0,
                            %agent_id,
                            "hopper.assign failed; not recording HopperAssign oplog entry"
                        );
                    }
                }
            }

            seen += 1;
            if Some(seen) == max_events {
                break;
            }
        }
    }
}

/// Synchronous op-log write for the dispatcher loop: acquires the std
/// `RwLockWriteGuard`, records, and drops the guard — never held across an
/// `.await` (mirrors the non-`_persisted` `OpLog::record` used elsewhere for
/// in-process-only recording; write-through-to-db uses `record_persisted`,
/// which is async — the dispatcher loop stays sync-only here to avoid
/// threading agent_id/db-persist plumbing into a hot broadcast loop).
fn record_hopper_op(
    oplog: &Arc<std::sync::RwLock<crate::oplog::OpLog>>,
    kind: crate::oplog::OperationKind,
    description: String,
) {
    let mut log = match oplog.write() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    log.record(
        crate::types::AgentId(0),
        kind,
        description,
        None,
        None,
        None,
        None,
        None,
        None,
    );
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
            move |t| {
                sink.lock().unwrap().push(t);
                Some(crate::types::AgentId(1))
            },
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

    /// T1.1: the dispatcher, given an oplog handle, records `HopperAdmit` +
    /// `HopperAssign` for an admitted item, and calls the real
    /// `HopperIntake::assign` (previously unreachable from production code).
    #[tokio::test]
    async fn dispatcher_with_oplog_records_admit_and_assign_and_calls_hopper_assign() {
        use crate::hopper::types::ItemState;

        let bus = Arc::new(EventBus::new(16));
        let rx = bus.subscribe();
        let hopper = Arc::new(InMemoryHopper::new(bus.clone()));
        let oplog = Arc::new(std::sync::RwLock::new(crate::oplog::OpLog::new(100)));

        let handle = tokio::spawn(run_dispatcher_with_oplog(
            rx,
            hopper.clone(),
            move |_t| Some(crate::types::AgentId(7)),
            Some(1),
            Some(oplog.clone()),
        ));

        let item = hopper
            .submit(
                "t".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;
        handle.await.unwrap();

        // The real HopperIntake::assign was called: the item transitioned to Assigned.
        let assigned = hopper.assigned().await;
        assert!(
            assigned.iter().any(|i| i.item_id == item.item_id
                && matches!(i.state, ItemState::Assigned { .. })),
            "hopper.assign must have been called by the dispatcher"
        );

        let entries = {
            let log = oplog.read().unwrap();
            log.list(None, 100).into_iter().cloned().collect::<Vec<_>>()
        };
        assert!(
            entries.iter().any(|e| matches!(
                &e.kind,
                crate::oplog::OperationKind::HopperAdmit { item_id } if item_id == &item.item_id.0
            )),
            "expected a HopperAdmit oplog entry"
        );
        assert!(
            entries.iter().any(|e| matches!(
                &e.kind,
                crate::oplog::OperationKind::HopperAssign { item_id, task_id }
                    if item_id == &item.item_id.0 && *task_id == stable_hash(&item.item_id.0)
            )),
            "expected a HopperAssign oplog entry"
        );
    }

    /// Test-only decorator that delegates everything to an inner `InMemoryHopper`
    /// except `assign`, which always fails — used to prove the dispatcher does
    /// NOT record `HopperAssign` when the real assign call errors (Issue 1).
    struct AssignFailingHopper {
        inner: InMemoryHopper,
    }

    #[async_trait::async_trait]
    impl HopperIntake for AssignFailingHopper {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        async fn submit(
            &self,
            intent: String,
            affinity_hints: Vec<String>,
            priority_hint: PriorityHint,
            source: IntakeSource,
            session_id: Option<String>,
        ) -> crate::hopper::types::IntakeItem {
            self.inner
                .submit(intent, affinity_hints, priority_hint, source, session_id)
                .await
        }
        async fn inbox(&self) -> Vec<crate::hopper::types::IntakeItem> {
            self.inner.inbox().await
        }
        async fn assigned(&self) -> Vec<crate::hopper::types::IntakeItem> {
            self.inner.assigned().await
        }
        async fn history(&self) -> Vec<crate::hopper::types::IntakeItem> {
            self.inner.history().await
        }
        async fn reprioritize(
            &self,
            item_id: &crate::events::HopperItemId,
            new_priority: TaskPriority,
            cap: crate::hopper::capability::DeveloperOverride,
        ) -> Result<crate::hopper::types::IntakeItem, crate::hopper::store::HopperError> {
            self.inner.reprioritize(item_id, new_priority, cap).await
        }
        async fn assign(
            &self,
            _item_id: &crate::events::HopperItemId,
            _agent_id: String,
        ) -> Result<crate::hopper::types::IntakeItem, crate::hopper::store::HopperError> {
            Err(crate::hopper::store::HopperError::NotFound(
                "synthetic assign failure (test)".into(),
            ))
        }
        async fn complete(
            &self,
            item_id: &crate::events::HopperItemId,
        ) -> Result<crate::hopper::types::IntakeItem, crate::hopper::store::HopperError> {
            self.inner.complete(item_id).await
        }
        async fn cancel(
            &self,
            item_id: &crate::events::HopperItemId,
        ) -> Result<crate::hopper::types::IntakeItem, crate::hopper::store::HopperError> {
            self.inner.cancel(item_id).await
        }
        async fn replay_admitted(
            &self,
            op: crate::hopper::store::AdmittedReplay,
        ) -> crate::hopper::types::IntakeItem {
            self.inner.replay_admitted(op).await
        }
        async fn replay_overridden(
            &self,
            item_id: &crate::events::HopperItemId,
            new_priority: TaskPriority,
            override_at_unix_ms: u64,
            override_by_node_id: String,
        ) -> Result<crate::hopper::types::IntakeItem, crate::hopper::store::HopperError> {
            self.inner
                .replay_overridden(
                    item_id,
                    new_priority,
                    override_at_unix_ms,
                    override_by_node_id,
                )
                .await
        }
        async fn replay_transitioned(
            &self,
            item_id: &crate::events::HopperItemId,
            new_state: crate::hopper::types::ItemState,
        ) -> Result<crate::hopper::types::IntakeItem, crate::hopper::store::HopperError> {
            self.inner.replay_transitioned(item_id, new_state).await
        }
    }

    /// Issue 1 fix: when the real `hopper.assign()` call fails, the dispatcher
    /// must NOT record a `HopperAssign` oplog entry (it would otherwise claim a
    /// hopper state transition that never actually happened).
    #[tokio::test]
    async fn dispatcher_does_not_record_hopper_assign_when_assign_fails() {
        let bus = Arc::new(EventBus::new(16));
        let rx = bus.subscribe();
        let inner = InMemoryHopper::new(bus.clone());
        let hopper: Arc<dyn HopperIntake> = Arc::new(AssignFailingHopper { inner });
        let oplog = Arc::new(std::sync::RwLock::new(crate::oplog::OpLog::new(100)));

        let handle = tokio::spawn(run_dispatcher_with_oplog(
            rx,
            hopper.clone(),
            move |_t| Some(crate::types::AgentId(7)),
            Some(1),
            Some(oplog.clone()),
        ));

        let item = hopper
            .submit(
                "t".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;
        handle.await.unwrap();

        let entries = {
            let log = oplog.read().unwrap();
            log.list(None, 100).into_iter().cloned().collect::<Vec<_>>()
        };
        // HopperAdmit should still be recorded (assign failure only gates the
        // HopperAssign write).
        assert!(
            entries.iter().any(|e| matches!(
                &e.kind,
                crate::oplog::OperationKind::HopperAdmit { item_id } if item_id == &item.item_id.0
            )),
            "expected a HopperAdmit oplog entry even though assign later failed"
        );
        assert!(
            !entries.iter().any(|e| matches!(
                &e.kind,
                crate::oplog::OperationKind::HopperAssign { item_id, .. } if item_id == &item.item_id.0
            )),
            "must NOT record HopperAssign when the real hopper.assign() call failed; entries: {entries:?}"
        );
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
