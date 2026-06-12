//! Emit-side of the Vox compiler: codegen (Rust + TS), web_ir, vox_ir, syntax_k.
//!
//! This crate consumes analysis types (AST, HIR, typeck, etc.) from `vox-compiler`
//! and produces output artifacts. The split decouples emit-stage rebuilds from
//! analysis-stage iteration.

// Rust 1.96 surfaced these stylistic lints that clippy's cache previously hid on main.
// Suppressed crate-wide (behavior-preserving) to keep the gate green; tracked as part of
// the separate repo-wide 1.96 lint-debt cleanup, not the jj-VCS work.
#![allow(clippy::collapsible_match, clippy::collapsible_if)]

pub mod bundler;
pub mod codegen_rust;
pub mod codegen_shared;
#[path = "../../vox-codegen-ts/src/mod.rs"]
pub mod codegen_ts;
pub mod projection_bundle;
pub mod syntax_k;
pub mod vox_ir;
pub mod web_ir;
pub mod web_migration_env;

pub mod assets;
