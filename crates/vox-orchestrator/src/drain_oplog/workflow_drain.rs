//! Workflow drain op-log entries and dispatcher predicate.
//!
//! Phase 2: in-memory only. Phase 3 will swap the backing `HashMap` for a
//! vox-db-backed durable op-log. The trait shape stays the same; only the
//! constructor differs.

use std::collections::HashMap;

/// Event recorded when a workflow content-hash enters drain mode.
#[derive(Debug, Clone, Copy)]
pub struct WorkflowDrainStarted {
    /// SHA3-512 content hash of the workflow bundle being drained.
    pub fn_hash: [u8; 64],
    /// Wall-clock time the drain was initiated (Unix milliseconds).
    pub started_at_unix_ms: u64,
}

/// In-memory drain state keyed by workflow `fn_hash`.
///
/// Insert via [`record_drain`][Self::record_drain]; query via
/// [`is_draining`][Self::is_draining] / [`may_start_new_run`][Self::may_start_new_run].
#[derive(Debug, Default)]
pub struct WorkflowDrainState {
    drained: HashMap<[u8; 64], WorkflowDrainStarted>,
}

impl WorkflowDrainState {
    /// Record that a workflow version is entering drain mode.
    pub fn record_drain(&mut self, evt: WorkflowDrainStarted) {
        self.drained.insert(evt.fn_hash, evt);
    }

    /// Returns `true` if the given hash is currently draining.
    pub fn is_draining(&self, fn_hash: &[u8; 64]) -> bool {
        self.drained.contains_key(fn_hash)
    }

    /// Dispatcher predicate. Returns `true` when a new run MAY be started.
    pub fn may_start_new_run(&self, fn_hash: &[u8; 64]) -> bool {
        !self.is_draining(fn_hash)
    }

    /// Snapshot of all currently draining entries.
    pub fn snapshot(&self) -> Vec<WorkflowDrainStarted> {
        self.drained.values().copied().collect()
    }
}
