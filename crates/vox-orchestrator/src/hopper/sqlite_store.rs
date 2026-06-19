//! Persistent HopperIntake backed by vox-db `hopper_inbox` (Hp-T5).

use crate::events::{AgentEventKind, EventBus, HopperItemId};
use crate::hopper::store::{AdmittedReplay, HopperError, HopperIntake};
use crate::hopper::types::{
    IntakeItem, IntakeSource, ItemState, PriorityHint, PriorityOverrideRecord, now_micros,
};
use crate::types::{PrioritySource, TaskPriority};
use std::sync::Arc;
use vox_db::HopperInboxRow;

pub struct SqliteHopper {
    db: Arc<vox_db::VoxDb>,
    bus: Arc<EventBus>,
}

impl SqliteHopper {
    pub fn new(db: Arc<vox_db::VoxDb>) -> Self {
        Self {
            db,
            bus: Arc::new(EventBus::new(16)),
        }
    }

    pub fn with_bus(db: Arc<vox_db::VoxDb>, bus: Arc<EventBus>) -> Self {
        Self { db, bus }
    }
}

fn row_to_item(row: HopperInboxRow) -> IntakeItem {
    let item_id = HopperItemId(row.item_id);
    let intent = row.intent;
    let affinity_hints = serde_json::from_str(&row.affinity_json).unwrap_or_default();
    let priority_hint = PriorityHint::Unspecified;
    let source = serde_json::from_str(&row.source).unwrap_or(IntakeSource::Developer);
    let session_id = row.session_id;
    let classified_priority = TaskPriority::from_u8(row.priority as u8);
    let state = serde_json::from_str(&row.state).unwrap_or(ItemState::Inbox);

    let priority_source = if state == ItemState::Overridden {
        PrioritySource::Developer
    } else {
        PrioritySource::Orchestrator
    };

    IntakeItem {
        item_id,
        intent,
        affinity_hints,
        priority_hint,
        source,
        session_id,
        classified_priority,
        priority_source,
        confidence: 0.85,
        privacy_class: "local-only".into(),
        state,
        submitted_at: row.submitted_at as u64,
        override_history: vec![],
    }
}

#[async_trait::async_trait]
impl HopperIntake for SqliteHopper {
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
    ) -> IntakeItem {
        let item = IntakeItem::new(intent, affinity_hints, priority_hint, source, session_id);

        let affinity_json = serde_json::to_string(&item.affinity_hints).unwrap();
        let source_json = serde_json::to_string(&item.source).unwrap();
        let state_json = serde_json::to_string(&item.state).unwrap();
        let priority_int = item.classified_priority as i64;
        let submitted_at_int = item.submitted_at as i64;

        if let Err(e) = self
            .db
            .hopper_submit(
                &item.item_id.0,
                &item.intent,
                &affinity_json,
                priority_int,
                &source_json,
                item.session_id.as_deref(),
                &state_json,
                submitted_at_int,
            )
            .await
        {
            tracing::error!("Failed to submit item to sqlite hopper: {:?}", e);
        }

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

        item
    }

    async fn inbox(&self) -> Vec<IntakeItem> {
        match self.db.hopper_inbox_list().await {
            Ok(rows) => rows.into_iter().map(row_to_item).collect(),
            Err(e) => {
                tracing::error!("Failed to list inbox from sqlite hopper: {:?}", e);
                vec![]
            }
        }
    }

    async fn assigned(&self) -> Vec<IntakeItem> {
        match self.db.hopper_assigned_list().await {
            Ok(rows) => rows.into_iter().map(row_to_item).collect(),
            Err(e) => {
                tracing::error!("Failed to list assigned from sqlite hopper: {:?}", e);
                vec![]
            }
        }
    }

    async fn history(&self) -> Vec<IntakeItem> {
        match self.db.hopper_history_list().await {
            Ok(rows) => rows.into_iter().map(row_to_item).collect(),
            Err(e) => {
                tracing::error!("Failed to list history from sqlite hopper: {:?}", e);
                vec![]
            }
        }
    }

    async fn reprioritize(
        &self,
        item_id: &HopperItemId,
        new_priority: TaskPriority,
        cap: crate::hopper::capability::DeveloperOverride,
    ) -> Result<IntakeItem, HopperError> {
        // Find the item first
        let item = self
            .inbox()
            .await
            .into_iter()
            .chain(self.assigned().await)
            .find(|i| &i.item_id == item_id);

        let mut item = match item {
            Some(i) => i,
            None => return Err(HopperError::NotFound(item_id.0.clone())),
        };

        if matches!(
            item.state,
            ItemState::Done | ItemState::Overridden | ItemState::Cancelled
        ) {
            return Err(HopperError::Terminal);
        }

        let old_priority = item.classified_priority;
        item.classified_priority = new_priority;
        item.priority_source = PrioritySource::Developer;
        item.override_history.push(PriorityOverrideRecord {
            ts_micros: now_micros(),
            actor: cap.actor.clone(),
            original_priority: old_priority,
            new_priority,
            reason: cap.reason.clone(),
            audit_id: cap.audit_id.clone(),
        });

        // Save new priority
        if let Err(e) = self
            .db
            .hopper_update_priority(&item_id.0, new_priority as i64)
            .await
        {
            tracing::error!("Failed to update priority in sqlite hopper: {:?}", e);
        }

        self.bus.emit(AgentEventKind::HopperItemOverridden {
            item_id: item_id.clone(),
            original_priority: old_priority,
            developer_priority: new_priority,
            delta_seconds_since_admit: (now_micros().saturating_sub(item.submitted_at)) / 1_000_000,
        });

        Ok(item)
    }

    async fn assign(
        &self,
        item_id: &HopperItemId,
        agent_id: String,
    ) -> Result<IntakeItem, HopperError> {
        let item = self
            .inbox()
            .await
            .into_iter()
            .find(|i| &i.item_id == item_id);

        let mut item = match item {
            Some(i) => i,
            None => return Err(HopperError::NotFound(item_id.0.clone())),
        };

        item.state = ItemState::Assigned {
            agent_id: agent_id.clone(),
        };
        let state_json = serde_json::to_string(&item.state).unwrap();

        if let Err(e) = self.db.hopper_update_state(&item_id.0, &state_json).await {
            tracing::error!("Failed to update state in sqlite hopper: {:?}", e);
        }

        Ok(item)
    }

    async fn complete(&self, item_id: &HopperItemId) -> Result<IntakeItem, HopperError> {
        let item = self
            .assigned()
            .await
            .into_iter()
            .find(|i| &i.item_id == item_id);

        let mut item = match item {
            Some(i) => i,
            None => return Err(HopperError::NotFound(item_id.0.clone())),
        };

        item.state = ItemState::Done;
        let state_json = serde_json::to_string(&item.state).unwrap();

        if let Err(e) = self.db.hopper_update_state(&item_id.0, &state_json).await {
            tracing::error!("Failed to update state in sqlite hopper: {:?}", e);
        }

        Ok(item)
    }

    async fn cancel(&self, item_id: &HopperItemId) -> Result<IntakeItem, HopperError> {
        let item = self
            .inbox()
            .await
            .into_iter()
            .chain(self.assigned().await)
            .find(|i| &i.item_id == item_id);

        let mut item = match item {
            Some(i) => i,
            None => return Err(HopperError::NotFound(item_id.0.clone())),
        };

        if matches!(
            item.state,
            ItemState::Done | ItemState::Overridden | ItemState::Cancelled
        ) {
            return Err(HopperError::Terminal);
        }

        item.state = ItemState::Cancelled;
        let state_json = serde_json::to_string(&item.state).unwrap();

        if let Err(e) = self.db.hopper_update_state(&item_id.0, &state_json).await {
            tracing::error!("Failed to update state in sqlite hopper: {:?}", e);
        }

        self.bus.emit(AgentEventKind::HopperItemCancelled {
            item_id: item_id.clone(),
        });

        Ok(item)
    }

    async fn replay_admitted(&self, op: AdmittedReplay) -> IntakeItem {
        // Idempotent: check if exists
        let existing = self
            .inbox()
            .await
            .into_iter()
            .chain(self.assigned().await)
            .chain(self.history().await)
            .find(|i| i.item_id == op.item_id);

        if let Some(item) = existing {
            return item;
        }

        let item = IntakeItem::from_replay(
            op.item_id,
            op.classified_priority,
            op.submitted_at_micros,
            op.task_kind,
            op.origin_node_id,
        );

        let affinity_json = serde_json::to_string(&item.affinity_hints).unwrap();
        let source_json = serde_json::to_string(&item.source).unwrap();
        let state_json = serde_json::to_string(&item.state).unwrap();
        let priority_int = item.classified_priority as i64;
        let submitted_at_int = item.submitted_at as i64;

        if let Err(e) = self
            .db
            .hopper_submit(
                &item.item_id.0,
                &item.intent,
                &affinity_json,
                priority_int,
                &source_json,
                item.session_id.as_deref(),
                &state_json,
                submitted_at_int,
            )
            .await
        {
            tracing::error!("Failed to replay item to sqlite hopper: {:?}", e);
        }

        item
    }

    async fn replay_overridden(
        &self,
        item_id: &HopperItemId,
        new_priority: TaskPriority,
        override_at_unix_ms: u64,
        override_by_node_id: String,
    ) -> Result<IntakeItem, HopperError> {
        let item = self.inbox().await.into_iter()
            .chain(self.assigned().await)
            .find(|i| &i.item_id == item_id);

        let mut item = match item {
            Some(i) => i,
            None => return Err(HopperError::NotFound(item_id.0.clone())),
        };

        if matches!(
            item.state,
            ItemState::Done | ItemState::Overridden | ItemState::Cancelled
        ) {
            return Err(HopperError::Terminal);
        }

        let old_priority = item.classified_priority;
        item.classified_priority = new_priority;
        item.priority_source = PrioritySource::Developer;
        item.override_history.push(PriorityOverrideRecord {
            ts_micros: override_at_unix_ms * 1_000,
            actor: override_by_node_id,
            original_priority: old_priority,
            new_priority,
            reason: "Mesh replication override".to_string(),
            audit_id: "mesh-sync".to_string(),
        });

        // Save new priority
        if let Err(e) = self.db.hopper_update_priority(&item_id.0, new_priority as i64).await {
            tracing::error!("Failed to update priority in sqlite hopper: {:?}", e);
        }

        Ok(item)
    }

    async fn replay_transitioned(
        &self,
        item_id: &HopperItemId,
        new_state: ItemState,
    ) -> Result<IntakeItem, HopperError> {
        let item = self.inbox().await.into_iter()
            .chain(self.assigned().await)
            .chain(self.history().await)
            .find(|i| &i.item_id == item_id);

        let mut item = match item {
            Some(i) => i,
            None => return Err(HopperError::NotFound(item_id.0.clone())),
        };

        item.state = new_state;
        let state_json = serde_json::to_string(&item.state).unwrap();

        if let Err(e) = self.db.hopper_update_state(&item_id.0, &state_json).await {
            tracing::error!("Failed to update state in sqlite hopper: {:?}", e);
        }

        Ok(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hopper::store::HopperIntake;
    use crate::hopper::types::{IntakeSource, PriorityHint};

    #[tokio::test]
    async fn submit_then_reload_preserves_inbox() {
        let db = Arc::new(
            vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
                .await
                .expect("db"),
        );
        let hopper = SqliteHopper::new(db.clone());
        hopper
            .submit(
                "persist me".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;
        // Drop and rebuild over the same DB to simulate a restart.
        let reloaded = SqliteHopper::new(db);
        let inbox = reloaded.inbox().await;
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].intent, "persist me");
    }
}
