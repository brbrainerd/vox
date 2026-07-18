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

    /// Most-recent `limit` terminal items, newest first (bounded history read).
    pub async fn history_recent(&self, limit: u32) -> Vec<IntakeItem> {
        match self.db.hopper_history_list_recent(limit).await {
            Ok(rows) => rows.into_iter().map(row_to_item).collect(),
            Err(e) => {
                tracing::error!("Failed to list recent history from sqlite hopper: {:?}", e);
                vec![]
            }
        }
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
            .inbox()
            .await
            .into_iter()
            .chain(self.assigned().await)
            .find(|i| &i.item_id == item_id);

        let mut item = match item {
            Some(i) => i,
            None => return Err(HopperError::NotFound(item_id.0.clone())),
        };

        // NOTE: unreachable today. inbox()/assigned() SQL already excludes
        // terminal states, so `item` here is never Done/Overridden/Cancelled
        // — this is not a verified double-completion guard, just defensive
        // parity with the same (equally unreachable) check in cancel(). A
        // real guard would need item lookup widened to all states plus a
        // compare-and-swap update (hopper_update_state is an unconditional
        // UPDATE with no expected-state check), which is a bigger change
        // than this fix's scope — tracked, not silently claimed as done.
        if matches!(
            item.state,
            ItemState::Done | ItemState::Overridden | ItemState::Cancelled
        ) {
            return Err(HopperError::Terminal);
        }

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
            ts_micros: override_at_unix_ms * 1_000,
            actor: override_by_node_id,
            original_priority: old_priority,
            new_priority,
            reason: "Mesh replication override".to_string(),
            audit_id: "mesh-sync".to_string(),
        });

        // Save new priority
        if let Err(e) = self
            .db
            .hopper_update_priority(&item_id.0, new_priority as i64)
            .await
        {
            tracing::error!("Failed to update priority in sqlite hopper: {:?}", e);
        }

        Ok(item)
    }

    async fn replay_transitioned(
        &self,
        item_id: &HopperItemId,
        new_state: ItemState,
    ) -> Result<IntakeItem, HopperError> {
        let item = self
            .inbox()
            .await
            .into_iter()
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

    #[tokio::test]
    async fn complete_marks_inbox_item_done_and_history_lists_it() {
        let db = Arc::new(
            vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
                .await
                .expect("db"),
        );
        let hopper = SqliteHopper::new(db);
        let item = hopper
            .submit(
                "todo done directly from inbox".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;
        let done = hopper
            .complete(&item.item_id)
            .await
            .expect("inbox item completable");
        assert_eq!(done.state, ItemState::Done);
        assert!(hopper.inbox().await.is_empty());
        assert!(
            hopper
                .history()
                .await
                .iter()
                .any(|i| i.item_id == item.item_id)
        );
    }

    #[tokio::test]
    async fn history_recent_is_bounded_and_newest_first() {
        let db = Arc::new(
            vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
                .await
                .expect("db"),
        );
        let hopper = SqliteHopper::new(db);
        for intent in ["first", "second", "third"] {
            let item = hopper
                .submit(
                    intent.into(),
                    vec![],
                    PriorityHint::Normal,
                    IntakeSource::Developer,
                    None,
                )
                .await;
            hopper.complete(&item.item_id).await.expect("completable");
        }
        let recent = hopper.history_recent(2).await;
        assert_eq!(recent.len(), 2, "limit must bound the read");
        // ORDER BY submitted_at DESC ⇒ newest first; "first" (oldest) is cut.
        assert!(recent.iter().all(|i| i.intent != "first"));
    }

    #[tokio::test]
    async fn history_recent_does_not_let_cancellations_starve_done_items() {
        // Regression: history_recent's LIMIT must be scoped to `done` alone.
        // If cancelled/overridden rows shared the same LIMIT budget, a burst
        // of cancellations newer than a completion could push that
        // completed item out of the window entirely, even though it's the
        // only thing hopper_list's Done view actually wants to show.
        let db = Arc::new(
            vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
                .await
                .expect("db"),
        );
        let hopper = SqliteHopper::new(db);

        let completed = hopper
            .submit(
                "the one completed item".into(),
                vec![],
                PriorityHint::Normal,
                IntakeSource::Developer,
                None,
            )
            .await;
        hopper
            .complete(&completed.item_id)
            .await
            .expect("completable");

        // A burst of more-recent cancellations that would previously have
        // consumed the shared LIMIT budget ahead of the completion above.
        for intent in ["cancelled-1", "cancelled-2", "cancelled-3"] {
            let item = hopper
                .submit(
                    intent.into(),
                    vec![],
                    PriorityHint::Normal,
                    IntakeSource::Developer,
                    None,
                )
                .await;
            hopper.cancel(&item.item_id).await.expect("cancellable");
        }

        let recent = hopper.history_recent(1).await;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].intent, "the one completed item");
    }
}
