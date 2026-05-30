//! Routes effort-audit findings to verified, drafted enforcement artifacts.
//!
//! See `docs/superpowers/specs/2026-05-30-effort-route-design.md`.

pub mod config;
pub mod load;
pub mod bucket;
pub mod cluster;
pub mod route;
pub mod emit;
pub mod pipeline;

pub use pipeline::run;
