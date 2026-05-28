//! Emit-side of the Vox compiler: codegen (Rust + TS), web_ir, vox_ir, syntax_k.
//!
//! This crate consumes analysis types (AST, HIR, typeck, etc.) from `vox-compiler`
//! and produces output artifacts. The split decouples emit-stage rebuilds from
//! analysis-stage iteration.

pub mod bundler;
pub mod codegen_rust;
pub mod codegen_shared;
pub mod codegen_ts;
pub mod projection_bundle;
pub mod syntax_k;
pub mod vox_ir;
pub mod web_ir;
pub mod web_migration_env;

pub mod assets;
