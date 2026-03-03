//! # vox-orchestrator
//!
//! Multi-agent file-affinity queue system for the Vox programming language.
//!
//! Routes tasks to agents based on **file ownership** — ensuring only one agent
//! writes to any given file at a time. Prevents race conditions and lost updates
//! when multiple AI agents work concurrently across a Vox workspace.
//!
//! ## Architecture
//!
//! ```text
//!   User Request
//!       │
//!       ▼
//!   Orchestrator ──► FileAffinityMap ──► route to Agent
//!       │                                    │
//!       ▼                                    ▼
//!   BulletinBoard ◄──── AgentQueue ──► FileLockManager
//! ```
//!
//! ## Features
//!
//! - `runtime` — Actor-based agents using `vox-runtime` Scheduler/Supervisor
//! - `toestub-gate` — Post-task quality validation using TOESTUB (on by default)
//! - `lsp` — LSP diagnostic integration for file ownership info

pub mod a2a;
pub mod affinity;
pub mod budget;
pub mod bulletin;
pub mod compaction;
pub mod config;
pub mod conflicts;
pub mod context;
pub mod continuation;
pub mod jj_backend;
pub mod events;
pub mod gate;
pub mod groups;
pub mod handoff;
pub mod heartbeat;
pub mod locks;
pub mod memory;
pub mod memory_search;
pub mod models;
pub mod monitor;
pub mod oplog;
pub mod orchestrator;
pub mod schema;
pub mod qa;
pub mod queue;
pub mod rebalance;
pub mod scope;
pub mod security;
pub mod services;
pub mod session;
pub mod snapshot;
pub mod state;
pub mod summary;
pub mod types;
pub mod usage;
pub mod workspace;

#[cfg(test)]
mod tests_agent_session;

#[cfg(feature = "toestub-gate")]
pub mod validation;

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(feature = "lsp")]
pub mod lsp;

// Re-export key public types for ergonomic access.
pub use budget::{AgentBudgetAllocation, BudgetManager, ContextBudget};
pub use compaction::{
    CompactionConfig, CompactionEngine, CompactionResult, CompactionStrategy, Turn,
};
pub use config::{OrchestratorConfig, ScalingProfile};
pub use conflicts::{ConflictId, ConflictManager, ConflictResolution, FileConflict};
pub use context::ContextStore;
pub use continuation::{ContinuationEngine, ContinuationStrategy};
pub use jj_backend::{ContentMerge, DagNodeId, MergeSide, OperationDag};
pub use events::{AgentActivity, AgentEvent, AgentEventKind, EventBus};
pub use gate::{Gate, GateResult, BudgetGate};
pub use handoff::HandoffPayload;
pub use heartbeat::{AgentHeartbeat, HeartbeatMonitor, HeartbeatPolicy, StalenessLevel};
pub use memory::{DailyLog, LongTermMemory, MemoryConfig, MemoryManager, SearchHit};
pub use memory_search::{HybridSearchHit, MemorySearchEngine};
pub use monitor::AiMonitor;
pub use oplog::{OpLog, OperationEntry, OperationId, OperationKind};
pub use orchestrator::{Orchestrator, TaskTraceStep};
pub use scope::{ScopeCheckResult, ScopeEnforcement, ScopeGuard};
pub use security::{
    AuditEntry, AuditLog, AuditResult, PolicyRule, SecurityAction, SecurityGuard, SecurityPolicy,
};
pub use services::{
    MessageGateway, PolicyCheckResult, PolicyEngine, RouteResult, RoutingService, ScalingAction,
    ScalingService,
};
pub use session::{Session, SessionConfig, SessionManager, SessionState};
pub use snapshot::{SnapshotId, SnapshotStore};
pub use summary::SummaryManager;
pub use types::{
    A2AMessage, A2AMessageType, AgentId, AgentMessage, AgentTask, FileAffinity, MessageEnvelope,
    MessageId, MessagePriority, TaskId, TaskPriority, TaskStatus, ThreadId, VcsContext,
};
pub use workspace::{AgentWorkspace, ChangeId, ChangeStatus, WorkspaceManager};
