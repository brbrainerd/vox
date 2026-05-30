//! AI-judged audit of git commit history.
//!
//! See `docs/superpowers/specs/2026-05-28-effort-audit-core-design.md`.

pub mod config;
pub mod range;
pub mod walk;
pub mod shape;
pub mod judge;
pub mod hybrid;
pub mod output;
pub mod pipeline;

pub use pipeline::run;
