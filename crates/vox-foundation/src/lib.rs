//! Utility umbrella crate that consolidates small, low-dependency helpers shared
//! across the Vox workspace.
//!
//! ## Modules
//!
//! - [`primitives`] — cheap trace ids, exponential backoff, AgentOS mutation kinds
//!   (was `vox-primitives`)
//! - [`protocol`] — orchestrator daemon wire-protocol pure-data types
//!   (was `vox-protocol`)
//! - [`tracing`] — `tracing_subscriber` bootstrap presets for CLIs and daemons
//!   (was `vox-tracing-init`)
//!
//! ## Migration
//!
//! Old crates are removed from the workspace; replace dependency entries:
//!
//! | Old dep              | New dep in Cargo.toml |
//! |----------------------|-----------------------|
//! | `vox-primitives`     | `vox-foundation`      |
//! | `vox-protocol`       | `vox-foundation`      |
//! | `vox-tracing-init`   | `vox-foundation`      |
//!
//! Use paths are unchanged — e.g. `vox_foundation::primitives::backoff::expo_backoff`.

/// Classifies a failed build's output as a real compile error, stale artifacts,
/// or host contention. Lives here rather than in a CLI crate so both the CLI and
/// the MCP compiler tools can use it without either taking a wide crate edge.
pub mod build_failure;
pub mod primitives;
pub mod protocol;
pub mod tracing;
