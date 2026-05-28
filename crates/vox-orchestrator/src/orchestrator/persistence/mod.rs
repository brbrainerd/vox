//! Persistence degradation outbox: periodic drain/prune and health counters.
//!
//! Entries are stored at `lifecycle::PERSISTENCE_OUTBOX_KEY` as a JSON array of objects
//! with `lane`, `error`, `first_seen_unix_ms`, and `retry_count`.

pub(crate) mod lifecycle;
pub(crate) mod replay;

use crate::types::AgentId;
pub(crate) use lifecycle::{
    PERSISTENCE_OUTBOX_KEY, PERSISTENCE_OUTBOX_LIFECYCLE_KEY, PersistenceOutboxLifecycleParams,
    ack_persistence_outbox_lane, run_persistence_outbox_lifecycle_pass,
};

impl crate::orchestrator::Orchestrator {
    pub(crate) async fn tick_persistence_outbox_lifecycle(&self) {
        let now_ms = crate::types::now_unix_ms();
        let params = PersistenceOutboxLifecycleParams::default();
        let mut stats = {
            let store = crate::sync_lock::rw_read(&*self.context_store);
            run_persistence_outbox_lifecycle_pass(&store, now_ms, &params)
        };
        let replay_stats = self.try_replay_persistence_outbox().await;
        let store = crate::sync_lock::rw_read(&*self.context_store);
        stats.queued = store
            .get(PERSISTENCE_OUTBOX_KEY)
            .and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(&raw).ok())
            .map(|v| v.len())
            .unwrap_or(0);

        let health = serde_json::json!({
            "queued": stats.queued,
            "pruned_last_run": stats.pruned_last_run,
            "retried_last_run": stats.retried_last_run,
            "replayed_last_run": replay_stats.replayed_last_run,
            "replay_failed_last_run": replay_stats.replay_failed_last_run,
            "replay_failed_by_op": replay_stats.replay_failed_by_op,
            "last_run_unix_ms": now_ms,
        });
        if let Ok(raw) = serde_json::to_string(&health) {
            store.set(
                AgentId(0),
                PERSISTENCE_OUTBOX_LIFECYCLE_KEY.to_string(),
                raw,
                0,
            );
        }

        if stats.pruned_last_run > 0
            || stats.retried_last_run > 0
            || replay_stats.replayed_last_run > 0
            || replay_stats.replay_failed_last_run > 0
        {
            tracing::debug!(
                queued = stats.queued,
                pruned = stats.pruned_last_run,
                retried = stats.retried_last_run,
                replayed = replay_stats.replayed_last_run,
                replay_failed = replay_stats.replay_failed_last_run,
                "persistence outbox lifecycle tick"
            );
        }
    }

    async fn try_replay_persistence_outbox(&self) -> replay::ReplayStats {
        let Some(db) = self.db() else {
            return replay::ReplayStats::default();
        };
        let key = PERSISTENCE_OUTBOX_KEY.to_string();
        let raw = {
            let store = crate::sync_lock::rw_read(&*self.context_store);
            match store.get(&key) {
                Some(raw) => raw,
                None => return replay::ReplayStats::default(),
            }
        };
        let mut queue = serde_json::from_str::<Vec<serde_json::Value>>(&raw).unwrap_or_default();
        if queue.is_empty() {
            return replay::ReplayStats::default();
        }

        let mut replayed = 0u64;
        let mut replay_failed = 0u64;
        let mut replay_failed_by_op: std::collections::BTreeMap<String, u64> =
            std::collections::BTreeMap::new();
        let mut kept = Vec::with_capacity(queue.len());
        for entry in queue.drain(..) {
            if replayed as usize >= replay::MAX_REPLAY_PER_TICK {
                kept.push(entry);
                continue;
            }
            if replay::replay_one_entry(&db, &entry).await {
                replayed = replayed.saturating_add(1);
            } else {
                replay_failed = replay_failed.saturating_add(1);
                let op = replay::replay_op_label(&entry);
                *replay_failed_by_op.entry(op).or_insert(0) += 1;
                kept.push(entry);
            }
        }
        let store = crate::sync_lock::rw_read(&*self.context_store);
        if let Ok(raw) = serde_json::to_string(&kept) {
            store.set(AgentId(0), key, raw, 0);
        }
        replay::ReplayStats {
            replayed_last_run: replayed,
            replay_failed_last_run: replay_failed,
            replay_failed_by_op,
        }
    }
}
