//! Phase 4.2: `@scheduled` durable runner.
//!
//! A process-global registry of `(name, callback)` pairs and a single tokio
//! task that polls [`vox_db::VoxDb`] for due rows, fires the callback, and
//! updates DB state. Crash-safe because all timer state lives in the DB —
//! restart picks up at the persisted `next_due_at_ms`.
//!
//! ```ignore
//! use std::sync::Arc;
//! use std::time::Duration;
//! use vox_db::{DbConfig, VoxDb};
//!
//! # async fn demo() -> anyhow::Result<()> {
//! let db = Arc::new(VoxDb::connect(DbConfig::Memory).await?);
//! vox_workflow_runtime::scheduled::register(
//!     "ticker",
//!     Duration::from_secs(60),
//!     Arc::new(|| Box::pin(async { Ok(()) })),
//!     db.clone(),
//! ).await?;
//! let handle = vox_workflow_runtime::scheduled::start(db).await?;
//! // ... later ...
//! handle.shutdown().await;
//! # Ok(()) }
//! ```

mod runner;

pub use runner::{Callback, ScheduledHandle, register, start};
