//! Library surface for `vox-arch-check`.
//!
//! The crate is primarily a binary (`src/main.rs` enforces the layered
//! architecture model). This thin library exposes the pieces that are reused
//! by the binary AND exercised directly by integration tests — currently the
//! CR-META criteria-format lint.

pub mod criteria_format;
