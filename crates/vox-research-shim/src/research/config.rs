//! Re-export of shared configuration types for the research pipeline.
//!
//! `orchestrator/config.rs` references these as `super::super::config::*`
//! (path: `dei_shim::research::config`). All types are defined in their
//! respective stub modules and re-exported here for ergonomic imports.

pub use super::gate::{GateConfig, RoutingThresholds};
pub use super::provider::ProviderConfig;
pub use super::verifier::VerifierConfig;
