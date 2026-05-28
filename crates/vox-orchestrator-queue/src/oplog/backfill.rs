//! Unknown-parent op-fragment hold queue with bounded capacity and DLQ spill (P3-T8).
//!
//! When a gossip reply arrives with fragments whose parents are not yet in the local op-log,
//! those fragments are held here until all parents are observed. On overflow the oldest entries
//! spill to `convergence_op_log_backfill_dlq` in vox-db for later reconciliation.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::oplog::persist::PersistContext;

/// Sizing and policy for the backfill hold queue.
#[derive(Clone, Debug)]
pub struct BackfillConfig {
    /// Maximum number of held fragments before evicting oldest (default: 1024).
    pub max_entries: usize,
    /// Maximum total blob bytes before evicting oldest (default: 64 KiB).
    pub max_bytes: usize,
    /// Milliseconds after which a fragment expires to the DLQ regardless of parents
    /// (reserved for future sweeper; not enforced by [`BackfillBuffer::insert`]).
    pub max_age_ms: u64,
}

impl Default for BackfillConfig {
    fn default() -> Self {
        Self {
            max_entries: 1024,
            max_bytes: 64 * 1024,
            max_age_ms: 600_000,
        }
    }
}

/// A single op-fragment held while waiting for its parents.
#[derive(Clone, Debug)]
pub struct HeldFragment {
    pub op_id: u64,
    pub parent_op_ids: Vec<u64>,
    /// Opaque serialized fragment bytes (e.g. JSON-encoded `OpFragmentBlob`).
    pub blob: Vec<u8>,
    /// Wall clock when this fragment was first received (unix ms).
    pub received_at_ms: u64,
}

struct Inner {
    fifo: VecDeque<HeldFragment>,
    /// parent op_id → list of dependent op_ids waiting for it.
    by_parent: BTreeMap<u64, Vec<u64>>,
    bytes: usize,
    /// Set of op_ids confirmed present in the local op-log.
    known: HashSet<u64>,
}

/// Bounded hold buffer for op-fragments whose parents are not yet known.
pub struct BackfillBuffer {
    cfg: BackfillConfig,
    inner: Arc<Mutex<Inner>>,
    persist: Option<Arc<PersistContext>>,
}

impl BackfillBuffer {
    pub fn new(cfg: BackfillConfig) -> Self {
        Self {
            cfg,
            inner: Arc::new(Mutex::new(Inner {
                fifo: VecDeque::new(),
                by_parent: BTreeMap::new(),
                bytes: 0,
                known: HashSet::new(),
            })),
            persist: None,
        }
    }

    /// Attach a persistence context so overflows spill to the DLQ.
    pub fn with_persist(mut self, ctx: Arc<PersistContext>) -> Self {
        self.persist = Some(ctx);
        self
    }

    /// Accept a fragment whose parents are not all known yet.
    ///
    /// If the buffer is at capacity, evicts the oldest entry first, spilling it
    /// to the DLQ if a [`PersistContext`] is attached.
    pub async fn insert(&self, frag: HeldFragment) {
        // Collect victims outside the lock so we don't await while holding it.
        let mut victims: Vec<HeldFragment> = Vec::new();
        {
            let mut g = self.inner.lock().await;
            while g.fifo.len() >= self.cfg.max_entries
                || g.bytes.saturating_add(frag.blob.len()) > self.cfg.max_bytes
            {
                match g.fifo.pop_front() {
                    Some(victim) => {
                        g.bytes = g.bytes.saturating_sub(victim.blob.len());
                        victims.push(victim);
                    }
                    None => break,
                }
            }
            for parent in &frag.parent_op_ids {
                if !g.known.contains(parent) {
                    g.by_parent.entry(*parent).or_default().push(frag.op_id);
                }
            }
            g.bytes = g.bytes.saturating_add(frag.blob.len());
            g.fifo.push_back(frag);
        }
        // DLQ spill (best-effort — errors are logged and discarded).
        if let Some(p) = &self.persist {
            for victim in &victims {
                if let Err(e) = spill_to_dlq(p, victim).await {
                    tracing::warn!(op_id = victim.op_id, error = %e, "backfill DLQ spill failed");
                } else {
                    tracing::debug!(op_id = victim.op_id, "backfill fragment spilled to DLQ");
                }
            }
        }
    }

    /// Record `op_id` as now present in the local op-log so dependents can be released.
    pub async fn mark_known(&self, op_id: u64) {
        self.inner.lock().await.known.insert(op_id);
    }

    /// Release all fragments that were waiting solely on `parent` (and whose remaining
    /// parents are now also known). Returns the op_ids of released fragments.
    pub async fn try_release_for(&self, parent: u64) -> Vec<u64> {
        let mut g = self.inner.lock().await;
        if !g.known.contains(&parent) {
            return Vec::new();
        }
        let dependents = g.by_parent.remove(&parent).unwrap_or_default();
        if dependents.is_empty() {
            return Vec::new();
        }
        let mut released = Vec::new();
        // Drain and rebuild fifo to avoid borrow conflicts.
        let old_fifo = std::mem::take(&mut g.fifo);
        for f in old_fifo {
            if dependents.contains(&f.op_id) && f.parent_op_ids.iter().all(|p| g.known.contains(p))
            {
                released.push(f.op_id);
                g.bytes = g.bytes.saturating_sub(f.blob.len());
            } else {
                g.fifo.push_back(f);
            }
        }
        released
    }

    /// Number of fragments currently held.
    pub async fn holding_count(&self) -> usize {
        self.inner.lock().await.fifo.len()
    }
}

async fn spill_to_dlq(ctx: &PersistContext, frag: &HeldFragment) -> Result<(), vox_db::StoreError> {
    let parent_json = serde_json::to_string(&frag.parent_op_ids).unwrap_or_else(|_| "[]".into());
    ctx.db
        .insert_backfill_dlq(
            frag.op_id as i64,
            frag.blob.clone(),
            parent_json,
            frag.received_at_ms as i64,
            "backfill buffer overflow".into(),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fragment(op_id: u64, parents: &[u64]) -> HeldFragment {
        HeldFragment {
            op_id,
            parent_op_ids: parents.to_vec(),
            blob: vec![op_id as u8; 16],
            received_at_ms: 0,
        }
    }

    #[tokio::test]
    async fn fragment_with_unknown_parent_holds_then_releases_when_parent_arrives() {
        let bf = BackfillBuffer::new(BackfillConfig::default());
        let child = make_fragment(2, &[1]);

        // Receive child first — parent 1 is unknown.
        bf.insert(child.clone()).await;
        assert_eq!(bf.holding_count().await, 1);

        // Parent not marked yet: no release.
        let released = bf.try_release_for(1).await;
        assert_eq!(released, Vec::<u64>::new());

        // Mark parent known, then release.
        bf.mark_known(1).await;
        let released = bf.try_release_for(1).await;
        assert_eq!(released, vec![2u64]);
        assert_eq!(bf.holding_count().await, 0);
    }

    #[tokio::test]
    async fn child_with_multiple_parents_waits_for_all() {
        let bf = BackfillBuffer::new(BackfillConfig::default());
        let child = make_fragment(3, &[1, 2]);
        bf.insert(child).await;

        // Mark parent 1 — child still waiting on parent 2.
        bf.mark_known(1).await;
        let released = bf.try_release_for(1).await;
        assert_eq!(released, Vec::<u64>::new());

        // Mark parent 2 — now both parents known; child released.
        bf.mark_known(2).await;
        let released = bf.try_release_for(2).await;
        assert_eq!(released, vec![3u64]);
    }

    #[tokio::test]
    async fn evicts_oldest_when_entry_limit_exceeded() {
        let cfg = BackfillConfig {
            max_entries: 2,
            max_bytes: 1024 * 1024,
            max_age_ms: 0,
        };
        let bf = BackfillBuffer::new(cfg);

        bf.insert(make_fragment(1, &[99])).await;
        bf.insert(make_fragment(2, &[99])).await;
        assert_eq!(bf.holding_count().await, 2);

        // Third insert evicts op 1 (oldest).
        bf.insert(make_fragment(3, &[99])).await;
        assert_eq!(bf.holding_count().await, 2);

        // Release parent 99 — only ops 2 and 3 are still held.
        bf.mark_known(99).await;
        let mut released = bf.try_release_for(99).await;
        released.sort_unstable();
        assert_eq!(released, vec![2u64, 3u64]);
    }

    #[tokio::test]
    async fn evicts_when_byte_budget_exceeded() {
        let cfg = BackfillConfig {
            max_entries: 1024,
            max_bytes: 32,
            max_age_ms: 0,
        };
        let bf = BackfillBuffer::new(cfg);

        // blob = 16 bytes each; two fit (32), third evicts first.
        bf.insert(make_fragment(1, &[99])).await;
        bf.insert(make_fragment(2, &[99])).await;
        assert_eq!(bf.holding_count().await, 2);
        bf.insert(make_fragment(3, &[99])).await;
        assert_eq!(bf.holding_count().await, 2);
    }

    #[tokio::test]
    async fn empty_buffer_returns_empty_on_release() {
        let bf = BackfillBuffer::new(BackfillConfig::default());
        bf.mark_known(5).await;
        let released = bf.try_release_for(5).await;
        assert_eq!(released, Vec::<u64>::new());
    }

    #[tokio::test]
    async fn holding_count_tracks_inserts_and_releases() {
        let bf = BackfillBuffer::new(BackfillConfig::default());
        assert_eq!(bf.holding_count().await, 0);
        bf.insert(make_fragment(10, &[1])).await;
        bf.insert(make_fragment(11, &[1])).await;
        assert_eq!(bf.holding_count().await, 2);
        bf.mark_known(1).await;
        let _ = bf.try_release_for(1).await;
        assert_eq!(bf.holding_count().await, 0);
    }
}
