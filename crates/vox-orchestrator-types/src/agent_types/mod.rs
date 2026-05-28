//! Core orchestrator agent/task identity types.
//!
//! These types are a leaf dependency — no orchestrator internals required.

pub mod file_affinity;
pub mod ids;
pub mod priority_source;
pub mod switch;
pub mod workspace_ids;

pub use file_affinity::{AccessKind, FileAffinity};
pub use ids::{
    AgentId, AgentIdGenerator, BatchId, CorrelationId, CorrelationIdGenerator, IdParseError,
    LockToken, TaskId, TaskIdGenerator, is_zero_f64, now_unix_ms,
};
pub use priority_source::PrioritySource;
pub use switch::{SwitchAccessMode, SwitchAction, SwitchActionType};
pub use workspace_ids::{ChangeId, SnapshotId, SnapshotIdGenerator};
