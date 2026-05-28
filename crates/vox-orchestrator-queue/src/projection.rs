//! `Projection`: read-side derived state rebuilt deterministically from the op-log.
//!
//! Every projection (locks, affinity, capabilities, kudos) implements this trait.
//! At startup the orchestrator loads the latest `Checkpoint` blob, hydrates
//! each projection's state, then replays every op with `op_id > checkpoint.op_id_hi`.
//!
//! The trait is **not async** — projections run on the same task that records ops
//! to keep replay deterministic. I/O-heavy projections may queue async side-effects.

use std::any::Any;

use crate::oplog::OperationEntry;

pub trait Projection: Send + Sync + Any {
    /// Stable name used in dashboards / metrics / checkpoint blob keys.
    fn name(&self) -> &'static str;

    /// Apply a single op. MUST be deterministic: same entry always produces same state delta.
    fn apply(&self, entry: &OperationEntry);

    /// Deterministically encode current state for checkpoint hashing.
    fn snapshot(&self) -> Vec<u8>;

    /// Reset state from a checkpoint snapshot.
    fn restore(&self, snapshot: &[u8]) -> Result<(), ProjectionError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("snapshot decode: {0}")]
    Decode(String),
}

/// Registry of all active projections for a daemon instance.
#[derive(Default)]
pub struct ProjectionRegistry {
    projections: Vec<Box<dyn Projection>>,
}

impl ProjectionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a projection. Returns `self` for builder-style chaining.
    pub fn with<P: Projection + 'static>(mut self, p: P) -> Self {
        self.projections.push(Box::new(p));
        self
    }

    /// Apply an op to every registered projection.
    ///
    /// Async so callers can `await` without blocking the executor, even though
    /// the current implementation is synchronous internally.
    pub async fn apply(&self, entry: &OperationEntry) {
        for p in &self.projections {
            p.apply(entry);
        }
    }

    /// Blake3 over the concatenated deterministic snapshots of all projections.
    /// Two registries with identical state sequences must return identical hashes.
    pub fn snapshot_blake3(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        for p in &self.projections {
            let buf = p.snapshot();
            hasher.update(p.name().as_bytes());
            hasher.update(&(buf.len() as u64).to_be_bytes());
            hasher.update(&buf);
        }
        *hasher.finalize().as_bytes()
    }
}
