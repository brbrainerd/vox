//! Interrupt infrastructure for stopping in-progress local tasks.
//!
//! # Current state (Track B, 2026-06-20)
//!
//! The interrupt path is **partially implemented**:
//!
//! - `Orchestrator::interrupt_flags` — an `RwLock<HashMap<TaskId, Arc<AtomicBool>>>` stores a
//!   per-task flag.  Set to `true` by [`Orchestrator::interrupt_task`].
//! - `orch.interrupt_task` daemon method constant is in `vox-foundation/src/protocol.rs`.
//! - Daemon handler dispatches to `interrupt_task` in `orch_daemon/mod.rs`.
//! - Tauri command `interrupt_orchestrator_task` is registered in `vox-gui/src/main.rs`.
//!
//! # What is still missing (TODO for next session)
//!
//! ## 1. Register the flag when a task starts
//!
//! In `runtime.rs` (or wherever `TaskProcessor::process` is called), before handing the task
//! to the processor, create a flag and insert it:
//!
//! ```rust,ignore
//! let flag = Arc::new(AtomicBool::new(false));
//! crate::sync_lock::rw_write(&orch.interrupt_flags).insert(task.id, Arc::clone(&flag));
//! let result = processor.process(agent_id, task).await;
//! crate::sync_lock::rw_write(&orch.interrupt_flags).remove(&task_id);
//! result
//! ```
//!
//! ## 2. Thread `CancellationToken` (or `Arc<AtomicBool>`) into `TaskProcessor::process`
//!
//! The trait signature is currently:
//! ```rust,ignore
//! async fn process(&self, agent_id: AgentId, task: AgentTask) -> anyhow::Result<TaskId>;
//! ```
//!
//! Add a cancellation parameter — either `Arc<AtomicBool>` (zero new deps) or
//! `tokio_util::sync::CancellationToken` (cleaner async integration):
//! ```rust,ignore
//! async fn process(
//!     &self,
//!     agent_id: AgentId,
//!     task: AgentTask,
//!     cancel: Arc<AtomicBool>,
//! ) -> anyhow::Result<TaskId>;
//! ```
//!
//! Both `StubTaskProcessor` and `AiTaskProcessor` must be updated.
//!
//! ## 3. Poll the flag in `AiTaskProcessor::process`
//!
//! In `runtime.rs::AiTaskProcessor::process`, the streaming loop receives tokens from the
//! LLM.  After each batch/chunk, check:
//! ```rust,ignore
//! if cancel.load(Ordering::Acquire) {
//!     tracing::info!("Task {} interrupted by user", task_id);
//!     return Err(anyhow::anyhow!("task interrupted"));
//! }
//! ```
//!
//! ## 4. Emit `TaskCancelled` with `path = "local_interrupt"` on abort
//!
//! Reuse `emit_task_cancelled(task_id, agent_id, "local_interrupt")` from
//! `orchestrator/agent/lifecycle_ops.rs`.
//!
//! ## 5. Release locks on abort
//!
//! Mirror the lock-release pattern in `cancel_task` (affinity map + file lock manager + scope
//! guard revoke) when the interrupt path is taken.
//!
//! # Files to touch
//!
//! | File | Change |
//! |------|--------|
//! | `src/runtime.rs` | `TaskProcessor::process` signature + flag registration + polling in `AiTaskProcessor` |
//! | `src/orchestrator/agent/lifecycle_ops.rs` | lock release in `interrupt_task` when task is in-progress |
//! | `Cargo.toml` (optional) | add `tokio-util = { features = ["sync"] }` if using `CancellationToken` |
