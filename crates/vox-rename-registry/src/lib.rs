//! Vox public-identifier rename registry and primitive-tag lookup.
//!
//! Extracted from `vox-compiler` so tooling (arch-check, vox-cli migrate) can
//! use it without pulling in the full compiler crate. Zero workspace deps.

pub mod primitive_tags;
pub mod renames;
