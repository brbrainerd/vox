//! Standalone TypeScript codegen crate (sources also embedded in `vox-codegen` via `#[path]`).
#![allow(clippy::collapsible_if)]

#[cfg(feature = "standalone")]
#[path = "mod.rs"]
pub mod codegen_ts;

#[cfg(feature = "standalone")]
pub use codegen_ts::*;
