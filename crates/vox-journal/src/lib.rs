//! Generic append-only JSON Lines file journal.
//!
//! Stores a sequence of `serde`-serializable entries one per line in a file.
//! Crash-safe: every successful `append` call flushes + `sync_data`s the
//! handle so the bytes are on the device before the call returns. On
//! `open`, the existing file is fully read and parsed entries are returned
//! to the caller for replay.
//!
//! Designed as the mobile-portable durability substrate underneath:
//!
//! - `vox-workflow-runtime::FileJournalTracker` (real `WorkflowTracker` impl)
//! - `vox-actor-runtime` actor state checkpointing (future)
//! - `vox-runtime-rn::open_file_journal` (uniffi-exported for JS)
//!
//! This crate has zero dependencies on `vox-db`, the workspace-hack crate,
//! or any other host-only build infrastructure. It cross-compiles cleanly to
//! every Android + iOS architecture.

#![warn(missing_docs, missing_debug_implementations)]

mod file;

pub use file::{FileJournal, JournalError};
