//! In-memory hopper store — Option A (Hp-T1).
//!
//! `InMemoryHopper` is the single-machine implementation. It uses a
//! `tokio::sync::RwLock<Vec<IntakeItem>>` so every method is async and the
//! API surface is identical to what Option B (persistent) would expose.
//!
//! Hp-T5 will introduce the vox-db `hopper_inbox` table; at that point the
//! dashboard adapter switches to the persistent impl by swapping the underlying
//! `Arc<dyn HopperIntake>` — the HTTP handlers don't change.

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::events::{AgentEventKind, EventBus, HopperItemId};
use crate::types::{PrioritySource, TaskPriority};

use super::capability::DeveloperOverride;
use super::types::{
    IntakeItem, IntakeSource, ItemState, PriorityHint, PriorityOverrideRecord, now_micros,
};

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum HopperError {
    #[error("item not found: {0}")]
    NotFound(String),
    #[error("item is in terminal state and cannot be modified")]
    Terminal,
}

// ── HopperIntake trait ────────────────────────────────────────────────────────

/// Public interface shared by the in-memory (Option A) and persistent (Option B)
/// implementations. Dashboard HTTP handlers program against this trait.
#[async_trait::async_trait]
pub trait HopperIntake: Send + Sync {
    /// Submit a new intake item. Returns the admitted item.
    async fn submit(
        &self,
        intent: String,
        affinity_hints: Vec<String>,
        priority_hint: PriorityHint,
        source: IntakeSource,
        session_id: Option<String>,
    ) -> IntakeItem;

    /// Return all items in Inbox state.
    async fn inbox(&self) -> Vec<IntakeItem>;

    /// Return all items in Assigned state.
    async fn assigned(&self) -> Vec<IntakeItem>;

    /// Return all items in terminal states (Done | Overridden).
    async fn history(&self) -> Vec<IntakeItem>;

    /// Override the classified priority.
    ///
    /// Requires a `DeveloperOverride` capability token — the caller must have
    /// minted one via `DeveloperOverrideMint` before calling this method.
    async fn reprioritize(
        &self,
        item_id: &HopperItemId,
        new_priority: TaskPriority,
        cap: DeveloperOverride,
    ) -> Result<IntakeItem, HopperError>;

    /// Mark an item as assigned to an agent.
    async fn assign(
        &self,
        item_id: &HopperItemId,
        agent_id: String,
    ) -> Result<IntakeItem, HopperError>;

    /// Mark an item as done.
    async fn complete(&self, item_id: &HopperItemId) -> Result<IntakeItem, HopperError>;
}

// ── InMemoryHopper ────────────────────────────────────────────────────────────

pub struct InMemoryHopper {
    items: RwLock<Vec<IntakeItem>>,
    bus: Arc<EventBus>,
}

impl InMemoryHopper {
    pub fn new(bus: Arc<EventBus>) -> Self {
        Self {
            items: RwLock::new(vec![]),
            bus,
        }
    }

    /// Construct without an event bus (useful in unit tests).
    pub fn headless() -> Self {
        Self {
            items: RwLock::new(vec![]),
            bus: Arc::new(EventBus::new(16)),
        }
    }
}

#[async_trait::async_trait]
impl HopperIntake for InMemoryHopper {
    async fn submit(
        &self,
        intent: String,
        affinity_hints: Vec<String>,
        priority_hint: PriorityHint,
        source: IntakeSource,
        session_id: Option<String>,
    ) -> IntakeItem {
        let item = IntakeItem::new(intent, affinity_hints, priority_hint, source, session_id);

        self.bus.emit(AgentEventKind::HopperItemAdmitted {
            item_id: item.item_id.clone(),
            classified_priority: item.classified_priority,
            classified_affinity: item
                .affinity_hints
                .iter()
                .map(std::path::PathBuf::from)
                .collect(),
            confidence: item.confidence,
            session_id: item.session_id.clone(),
        });

        self.items.write().await.push(item.clone());
        item
    }

    async fn inbox(&self) -> Vec<IntakeItem> {
        self.items
            .read()
            .await
            .iter()
            .filter(|i| matches!(i.state, ItemState::Inbox))
            .cloned()
            .collect()
    }

    async fn assigned(&self) -> Vec<IntakeItem> {
        self.items
            .read()
            .await
            .iter()
            .filter(|i| matches!(i.state, ItemState::Assigned { .. }))
            .cloned()
            .collect()
    }

    async fn history(&self) -> Vec<IntakeItem> {
        self.items
            .read()
            .await
            .iter()
            .filter(|i| matches!(i.state, ItemState::Done | ItemState::Overridden))
            .cloned()
            .collect()
    }

    async fn reprioritize(
        &self,
        item_id: &HopperItemId,
        new_priority: TaskPriority,
        cap: DeveloperOverride,
    ) -> Result<IntakeItem, HopperError> {
        let mut items = self.items.write().await;
        let item = items
            .iter_mut()
            .find(|i| &i.item_id == item_id)
            .ok_or_else(|| HopperError::NotFound(item_id.0.clone()))?;

        if matches!(item.state, ItemState::Done | ItemState::Overridden) {
            return Err(HopperError::Terminal);
        }

        let old_priority = item.classified_priority;
        item.classified_priority = new_priority;
        // DeveloperOverride cap was presented — record that this priority is
        // now Developer-sourced. Any subsequent automated policy must not
        // overwrite it without another Developer-level cap (Hp-T3).
        item.priority_source = PrioritySource::Developer;
        item.override_history.push(PriorityOverrideRecord {
            ts_micros: now_micros(),
            actor: cap.actor.clone(),
            original_priority: old_priority,
            new_priority,
            reason: cap.reason.clone(),
            audit_id: cap.audit_id.clone(),
        });

        let out = item.clone();

        self.bus.emit(AgentEventKind::HopperItemOverridden {
            item_id: item_id.clone(),
            original_priority: old_priority,
            developer_priority: new_priority,
            delta_seconds_since_admit: (now_micros().saturating_sub(out.submitted_at)) / 1_000_000,
        });

        Ok(out)
    }

    async fn assign(
        &self,
        item_id: &HopperItemId,
        agent_id: String,
    ) -> Result<IntakeItem, HopperError> {
        let mut items = self.items.write().await;
        let item = items
            .iter_mut()
            .find(|i| &i.item_id == item_id)
            .ok_or_else(|| HopperError::NotFound(item_id.0.clone()))?;
        item.state = ItemState::Assigned { agent_id };
        Ok(item.clone())
    }

    async fn complete(&self, item_id: &HopperItemId) -> Result<IntakeItem, HopperError> {
        let mut items = self.items.write().await;
        let item = items
            .iter_mut()
            .find(|i| &i.item_id == item_id)
            .ok_or_else(|| HopperError::NotFound(item_id.0.clone()))?;
        item.state = ItemState::Done;
        Ok(item.clone())
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hopper::capability::DeveloperOverrideMint;

    fn mint() -> DeveloperOverrideMint {
        DeveloperOverrideMint::new()
    }

    #[tokio::test]
    async fn submit_lands_in_inbox() {
        let h = InMemoryHopper::headless();
        let item = h
            .submit(
                "fix flaky test".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;
        assert_eq!(item.state, ItemState::Inbox);
        assert_eq!(h.inbox().await.len(), 1);
        assert_eq!(h.history().await.len(), 0);
    }

    #[tokio::test]
    async fn reprioritize_requires_developer_override_cap() {
        let h = InMemoryHopper::headless();
        let item = h
            .submit(
                "urgent fix".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;

        let cap = mint().mint("test-user", "needs to go first", "audit-123");
        let updated = h
            .reprioritize(&item.item_id, TaskPriority::Urgent, cap)
            .await
            .unwrap();
        assert_eq!(updated.classified_priority, TaskPriority::Urgent);
        assert_eq!(updated.override_history.len(), 1);
        assert_eq!(updated.override_history[0].audit_id, "audit-123");
    }

    /// Hp-T3 acceptance: initial intake is `Orchestrator`-sourced; after a
    /// `DeveloperOverride` cap the source flips to `Developer`.
    #[tokio::test]
    async fn priority_source_starts_orchestrator_then_flips_to_developer() {
        let h = InMemoryHopper::headless();
        let item = h
            .submit(
                "some work".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;

        // Fresh item: orchestrator classified it.
        assert_eq!(item.priority_source, PrioritySource::Orchestrator);

        // After a DeveloperOverride, priority_source becomes Developer.
        let cap = mint().mint("alice", "critical path", "audit-999");
        let updated = h
            .reprioritize(&item.item_id, TaskPriority::Urgent, cap)
            .await
            .unwrap();
        assert_eq!(updated.priority_source, PrioritySource::Developer);
    }

    /// Hp-T3 acceptance: `PrioritySource` partial order — `Developer > Orchestrator
    /// > LearningPolicy`.
    #[test]
    fn priority_source_partial_order_holds() {
        assert!(PrioritySource::Developer.dominates(PrioritySource::Orchestrator));
        assert!(PrioritySource::Developer.dominates(PrioritySource::LearningPolicy));
        assert!(PrioritySource::Orchestrator.dominates(PrioritySource::LearningPolicy));
        assert!(!PrioritySource::Orchestrator.dominates(PrioritySource::Developer));
        assert!(!PrioritySource::LearningPolicy.dominates(PrioritySource::Developer));
    }

    #[tokio::test]
    async fn reprioritize_terminal_item_returns_error() {
        let h = InMemoryHopper::headless();
        let item = h
            .submit(
                "old task".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Agent,
                None,
            )
            .await;
        h.complete(&item.item_id).await.unwrap();
        let cap = mint().mint("u", "r", "a");
        let result = h
            .reprioritize(&item.item_id, TaskPriority::Urgent, cap)
            .await;
        assert!(matches!(result, Err(HopperError::Terminal)));
    }

    #[tokio::test]
    async fn assign_and_complete_lifecycle() {
        let h = InMemoryHopper::headless();
        let item = h
            .submit(
                "build widget".into(),
                vec![],
                PriorityHint::Unspecified,
                IntakeSource::Webhook,
                None,
            )
            .await;
        h.assign(&item.item_id, "agent-42".into()).await.unwrap();
        assert_eq!(h.assigned().await.len(), 1);
        assert_eq!(h.inbox().await.len(), 0);

        h.complete(&item.item_id).await.unwrap();
        assert_eq!(h.assigned().await.len(), 0);
        assert_eq!(h.history().await.len(), 1);
    }
}
