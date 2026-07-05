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
        blake3::hash(&self.snapshot_bytes()).into()
    }

    /// Deterministically encode every registered projection's `snapshot()` output
    /// into a single byte buffer, framed as `(name_len, name, buf_len, buf)*` in
    /// registration order. This is the exact byte sequence [`Self::snapshot_blake3`]
    /// hashes, so re-hashing the buffer this returns always reproduces the same
    /// digest — the buffer is what a checkpoint blob stores, and [`Self::restore_bytes`]
    /// is its inverse.
    pub fn snapshot_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        for p in &self.projections {
            let name = p.name().as_bytes();
            let data = p.snapshot();
            buf.extend_from_slice(&(name.len() as u64).to_be_bytes());
            buf.extend_from_slice(name);
            buf.extend_from_slice(&(data.len() as u64).to_be_bytes());
            buf.extend_from_slice(&data);
        }
        buf
    }

    /// Inverse of [`Self::snapshot_bytes`]: split the framed buffer back into
    /// per-projection slices and call [`Projection::restore`] on each registered
    /// projection whose name matches a frame. Frames for names with no registered
    /// projection are skipped (forward-compatible with future projections).
    pub fn restore_bytes(&self, mut buf: &[u8]) -> Result<(), ProjectionError> {
        use std::collections::HashMap;
        let mut frames: HashMap<String, &[u8]> = HashMap::new();
        while !buf.is_empty() {
            let (name, rest) = take_framed(buf)?;
            let (data, rest) = take_framed(rest)?;
            frames.insert(
                String::from_utf8(name.to_vec())
                    .map_err(|e| ProjectionError::Decode(e.to_string()))?,
                data,
            );
            buf = rest;
        }
        for p in &self.projections {
            if let Some(data) = frames.get(p.name()) {
                p.restore(data)?;
            }
        }
        Ok(())
    }
}

/// Read one `(len: u64 BE, bytes)` frame off the front of `buf`, returning the
/// frame's bytes and the remaining slice.
fn take_framed(buf: &[u8]) -> Result<(&[u8], &[u8]), ProjectionError> {
    if buf.len() < 8 {
        return Err(ProjectionError::Decode(
            "truncated checkpoint buffer: missing length prefix".into(),
        ));
    }
    let (len_bytes, rest) = buf.split_at(8);
    let len = u64::from_be_bytes(len_bytes.try_into().unwrap()) as usize;
    if rest.len() < len {
        return Err(ProjectionError::Decode(
            "truncated checkpoint buffer: frame shorter than declared length".into(),
        ));
    }
    Ok(rest.split_at(len))
}
