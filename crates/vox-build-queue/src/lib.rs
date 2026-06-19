//! Pure logic for the vox build broker: cargo resolution, fair queueing, metrics.
//!
//! See `docs/superpowers/specs/2026-06-18-unified-build-broker-design.md`.
pub mod env_filter;
pub mod metrics;
pub mod queue;
pub mod resolve;
