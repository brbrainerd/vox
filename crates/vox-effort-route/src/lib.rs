//! Routes effort-audit findings to verified, drafted enforcement artifacts.
//!
//! See `docs/superpowers/specs/2026-05-30-effort-route-design.md`.

pub mod bucket;
pub mod cluster;
pub mod config;
pub mod embed;
pub mod emit;
pub mod load;
pub mod pipeline;
pub mod pricing;
pub mod route;

pub use pipeline::run;
