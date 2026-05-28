//! # vox-container-types
//!
//! Pure types and traits for OCI container runtime abstraction. Zero I/O, zero
//! process execution — safe to depend on from any layer.
//!
//! - [`ContainerRuntime`] — the abstract OCI backend trait
//! - [`BuildOpts`] / [`RunOpts`] — structured options
//! - [`RuntimePreference`] — backend selection hint
//! - [`exec_grammar`] — pure shell AST parser + risk classifier

mod runtime;
pub mod detect;
pub mod exec_grammar;

pub use runtime::{BuildOpts, ContainerRuntime, RunOpts};
pub use detect::RuntimePreference;
