//! Workspace-wide multi-language drift and pattern-repetition linter.
//!
//! Detects stale symbol references, naming drift, copy-paste drift, stale patterns,
//! and diverging implementations across Rust, TypeScript, Vox source files, and the monorepo.

pub mod cache;
pub mod config;
pub mod engine;
pub mod extractor;
pub mod extractors;
pub mod features;
pub mod report;
pub mod rules;
pub mod sweep;

pub use vox_code_audit::rules::Severity;
