//! # vox-container-types
//!
//! Pure types and traits for OCI container runtime abstraction. Zero I/O, zero
//! process execution — safe to depend on from any layer.
//!
//! - [`ContainerRuntime`] — the abstract OCI backend trait
//! - [`BuildOpts`] / [`RunOpts`] — structured options
//! - [`RuntimePreference`] — backend selection hint
//! - [`exec_grammar`] — pure shell AST parser + risk classifier

pub mod detect;
pub mod exec_grammar;
mod runtime;
mod semcov_wave47_tests;

pub use detect::RuntimePreference;
pub use runtime::{BuildOpts, ContainerRuntime, RunOpts};
