//! AI-judged audit of git commit history.
//!
//! See `docs/superpowers/specs/2026-05-28-effort-audit-core-design.md`.

pub mod config;
pub mod hybrid;
pub mod judge;
pub mod output;
pub mod pipeline;
pub mod range;
pub mod shape;
pub mod walk;

pub use pipeline::run;
