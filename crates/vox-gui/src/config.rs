//! Shared timing constants for the Vox GUI Tauri backend.

/// Scientia queue watcher poll interval (seconds).
pub const SCIENTIA_QUEUE_POLL_SECS: u64 = 3;

/// Orchestrator status stream channel capacity.
pub const ORCH_STATUS_CHANNEL_CAP: usize = 64;

/// Agent events stream channel capacity.
pub const AGENT_EVENTS_CHANNEL_CAP: usize = 256;
