//! Orchestrator context-store snapshot persistence — JSON, not the DB.
//!
//! The in-memory `ContextStore` is serialized to JSON for crash-recovery
//! snapshots. It is **not** in the DB because:
//!
//! * the snapshot is written on a "fast" path during shutdown, before any
//!   async runtime guarantee,
//! * the data is opaque to other consumers (orchestrator-internal),
//! * keeping it out of the DB avoids schema churn for orchestrator changes.
//!
//! If cross-process visibility is needed, add a `vox_db::ops_orchestrator_context`
//! module instead.

use crate::context::ContextStore;
use crate::types::AgentId;

/// Context key for the outbox queue (must match persistence callsites).
pub(crate) const PERSISTENCE_OUTBOX_KEY: &str = "orchestrator/persistence_outbox";

/// Health / observability for the last lifecycle pass.
pub(crate) const PERSISTENCE_OUTBOX_LIFECYCLE_KEY: &str =
    "orchestrator/persistence_outbox_lifecycle";

#[derive(Debug, Clone)]
pub(crate) struct PersistenceOutboxLifecycleParams {
    /// After this many ms since `first_seen`, an item is considered stale and gains a retry per tick.
    pub stale_after_ms: u64,
    /// Drop items whose first_seen is older than this many ms.
    pub max_age_ms: u64,
    /// Drop items whose `retry_count` is at or above this threshold (after any increment this tick).
    pub max_retries: u64,
}

impl Default for PersistenceOutboxLifecycleParams {
    fn default() -> Self {
        Self {
            stale_after_ms: 300_000, // 5 minutes
            max_age_ms: 604_800_000, // 7 days
            max_retries: 128,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct OutboxLifecycleStats {
    pub queued: usize,
    pub pruned_last_run: u64,
    pub retried_last_run: u64,
}

/// One maintenance pass: load outbox, prune by age/attempts, bump retries on stale rows, persist.
pub(crate) fn run_persistence_outbox_lifecycle_pass(
    store: &ContextStore,
    now_ms: u64,
    params: &PersistenceOutboxLifecycleParams,
) -> OutboxLifecycleStats {
    let key = PERSISTENCE_OUTBOX_KEY.to_string();
    let mut queue = store
        .get(&key)
        .and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(&raw).ok())
        .unwrap_or_default();

    let mut pruned_last_run: u64 = 0;
    let mut retried_last_run: u64 = 0;
    let mut kept: Vec<serde_json::Value> = Vec::with_capacity(queue.len());

    for item in queue.drain(..) {
        let Some(obj) = item.as_object().cloned() else {
            pruned_last_run = pruned_last_run.saturating_add(1);
            continue;
        };

        let lane = obj
            .get("lane")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let error = obj
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let first_seen = first_seen_unix_ms(&obj).unwrap_or(now_ms);
        let mut retry_count = obj
            .get("retry_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        let age_ms = now_ms.saturating_sub(first_seen);

        if age_ms >= params.max_age_ms {
            pruned_last_run = pruned_last_run.saturating_add(1);
            continue;
        }
        if retry_count >= params.max_retries {
            pruned_last_run = pruned_last_run.saturating_add(1);
            continue;
        }

        if age_ms >= params.stale_after_ms {
            retry_count = retry_count.saturating_add(1);
            retried_last_run = retried_last_run.saturating_add(1);
            if retry_count >= params.max_retries {
                pruned_last_run = pruned_last_run.saturating_add(1);
                continue;
            }
        }

        let mut normalized = obj;
        normalized.insert("lane".to_string(), serde_json::Value::String(lane));
        normalized.insert("error".to_string(), serde_json::Value::String(error));
        normalized.insert(
            "first_seen_unix_ms".to_string(),
            serde_json::Value::from(first_seen),
        );
        normalized.insert(
            "retry_count".to_string(),
            serde_json::Value::from(retry_count),
        );
        normalized.insert("schema_version".to_string(), serde_json::Value::from(1));
        kept.push(serde_json::Value::Object(normalized));
    }

    let stats = OutboxLifecycleStats {
        queued: kept.len(),
        pruned_last_run,
        retried_last_run,
    };

    if let Ok(raw) = serde_json::to_string(&kept) {
        store.set(AgentId(0), key, raw, 0);
    }

    let health = serde_json::json!({
        "queued": stats.queued,
        "pruned_last_run": stats.pruned_last_run,
        "retried_last_run": stats.retried_last_run,
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

    stats
}

/// Acknowledge one recovered lane by removing the oldest matching entry.
pub(crate) fn ack_persistence_outbox_lane(store: &ContextStore, lane: &str) -> bool {
    let lane = lane.trim();
    if lane.is_empty() {
        return false;
    }
    let key = PERSISTENCE_OUTBOX_KEY.to_string();
    let mut queue = store
        .get(&key)
        .and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(&raw).ok())
        .unwrap_or_default();

    let idx = queue.iter().enumerate().find_map(|(i, item)| {
        item.as_object().and_then(|o| {
            o.get("lane")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|s| *s == lane)
                .map(|_| i)
        })
    });
    let Some(i) = idx else {
        return false;
    };
    queue.remove(i);
    if let Ok(raw) = serde_json::to_string(&queue) {
        store.set(AgentId(0), key, raw, 0);
    }
    true
}

fn first_seen_unix_ms(obj: &serde_json::Map<String, serde_json::Value>) -> Option<u64> {
    if let Some(v) = obj.get("first_seen_unix_ms") {
        if let Some(n) = v.as_u64() {
            return Some(n);
        }
    }
    obj.get("first_seen").and_then(|v| v.as_u64()).or_else(|| {
        obj.get("first_seen")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_outbox(store: &ContextStore, items: Vec<serde_json::Value>) {
        let raw = serde_json::to_string(&items).unwrap();
        store.set(AgentId(0), PERSISTENCE_OUTBOX_KEY.to_string(), raw, 0);
    }

    fn load_outbox(store: &ContextStore) -> Vec<serde_json::Value> {
        store
            .get(PERSISTENCE_OUTBOX_KEY)
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    #[test]
    fn ack_persistence_outbox_lane_removes_one_oldest_match() {
        let store = ContextStore::new();
        seed_outbox(
            &store,
            vec![
                serde_json::json!({"lane":"lineage/task_failed","error":"a","first_seen_unix_ms":1u64,"retry_count":0u64}),
                serde_json::json!({"lane":"trust/observation","error":"b","first_seen_unix_ms":2u64,"retry_count":0u64}),
                serde_json::json!({"lane":"lineage/task_failed","error":"c","first_seen_unix_ms":3u64,"retry_count":0u64}),
            ],
        );
        assert!(ack_persistence_outbox_lane(&store, "lineage/task_failed"));
        let q = load_outbox(&store);
        assert_eq!(q.len(), 2);
        assert_eq!(q[0]["lane"], "trust/observation");
        assert_eq!(q[1]["lane"], "lineage/task_failed");
    }

    #[test]
    fn fresh_entry_not_retried_until_stale() {
        let store = ContextStore::new();
        let now = 10_000u64;
        seed_outbox(
            &store,
            vec![serde_json::json!({
                "lane": "test",
                "error": "e",
                "first_seen_unix_ms": now,
                "retry_count": 0u64,
            })],
        );
        let params = PersistenceOutboxLifecycleParams {
            stale_after_ms: 5_000,
            max_age_ms: 1_000_000,
            max_retries: 10,
        };
        let s = run_persistence_outbox_lifecycle_pass(&store, now, &params);
        assert_eq!(s.queued, 1);
        assert_eq!(s.retried_last_run, 0);
        assert_eq!(s.pruned_last_run, 0);
        let q = load_outbox(&store);
        assert_eq!(q[0]["retry_count"], 0);
    }
}
