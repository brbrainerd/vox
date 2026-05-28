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

pub mod primitives;
pub mod protocol;
pub mod tracing;
