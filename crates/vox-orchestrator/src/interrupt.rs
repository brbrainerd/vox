//! Interrupt infrastructure for stopping in-progress local tasks.
//!
//! # Current state (Track B, complete — 2026-06-20)
//!
//! The local-task interrupt path is **fully wired end to end**:
//!
//! 1. **Flag registration.** When [`crate::runtime::ActorAgent`] dequeues a task and is about to
//!    run it, it creates `Arc<AtomicBool>` and inserts it into
//!    [`Orchestrator::interrupt_flags`] keyed by `TaskId` (via `crate::sync_lock::rw_write`).
//!    The entry is removed after `process` returns.
//! 2. **Threaded into the processor.** [`crate::runtime::TaskProcessor::process`] takes a
//!    `cancel: Arc<AtomicBool>` parameter. Both `StubTaskProcessor` and `AiTaskProcessor`
//!    implement it.
//! 3. **Polling.** `AiTaskProcessor` polls `cancel.load(Ordering::Acquire)` at the top of every
//!    phase and after every streamed chunk (in `run_phase_stream`). `StubTaskProcessor` checks
//!    the flag at entry so the cancel path is unit-testable.
//! 4. **Abort telemetry.** On abort, [`Orchestrator::abort_interrupted_task`] emits
//!    `orch.task.cancelled` with `path = "local_interrupt"` (mirrors `emit_task_cancelled`).
//! 5. **Lock release.** `abort_interrupted_task` mirrors `cancel_task`'s release path: it revokes
//!    the file lock, affinity, and scope-guard claims for the task's write files (unless another
//!    task still claims them), drops the task assignment, and clears the interrupt flag.
//!
//! # Trigger surface
//!
//! - [`Orchestrator::interrupt_task`] sets the flag if the task is in-progress (registered), or
//!   falls back to `cancel_task` for still-queued tasks.
//! - Daemon method constant `orch.interrupt_task` lives in `vox-foundation/src/protocol.rs`;
//!   the daemon handler dispatches it; the Tauri command `interrupt_orchestrator_task` is
//!   registered in `vox-gui/src/main.rs`.
//!
//! # Design notes
//!
//! `Arc<AtomicBool>` is used rather than `tokio_util::sync::CancellationToken` to avoid adding a
//! dependency. Polling is cooperative: a phase that is mid-`await` on inference completes that
//! single inference before the next poll observes the flag, so abort latency is bounded by one
//! streamed chunk (or one phase boundary if the backend does not stream).
