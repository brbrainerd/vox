//! Unified Vox compiler pipeline.
//!
//! This crate consolidates all core compiler stages: lexing, parsing,
//! AST definition, HIR lowering, type checking, and code generation.
//!
//! **Generated Rust/TS outputs** are subject to the same premature-completion policy as hand-written
//! code: after emitting a tree, run `vox ci completion-audit` (optionally scoped to the output root)
//! or extend CI to scan `target/` / app output dirs; see `contracts/operations/completion-policy.v1.yaml`.

pub mod annotations;
pub mod app_contract;
/// AST re-exported from the standalone `vox-ast` leaf crate (`crate::ast::*` == `vox_ast::*`).
pub use vox_ast as ast;
pub mod ast_eval;
pub mod builtin_registry;
pub mod canonical_json;
pub mod codegen_ts;
pub mod contract_ir;
pub mod eval;
pub mod fmt;
pub mod generated_vox;
pub mod hir;
pub mod language_surface;
pub mod lexer;
pub mod llm_prompt;
pub mod lowering_shared;
pub mod module;
pub mod parser;
pub mod pipeline;
pub mod react_bridge;
pub mod required_capabilities;
pub mod runtime_projection;
pub mod rust_interop_support;
pub mod serialization;
pub mod shell_projection;
pub mod tokens;
pub mod typeck;
pub mod web_prefixes;

/// Adversarial semantic-coverage tests — wave 17.
#[cfg(test)]
mod semcov_wave17_tests;

/// Structural pipeline-gap regression tests (pattern #1: silent-drop catch-all —
/// the headline top-level-`let` → `Decl::Const` → lowering bug).
#[cfg(test)]
mod semcov_struct_pipeline_tests;

/// Re-export of common types if needed.
pub use ast::decl::Module;
/// Re-export parser-backed AST evaluation (replaces regex-based vox-eval constructs).
pub use ast_eval::{AstEvalReport, ast_eval};
pub use hir::{HirModule, TypedCoreIR_v2};
pub use typeck::checker::Checker;

/// Re-export the canonical formatter so callers use `vox_compiler::format(src)`.
pub use fmt::format;
/// Re-export canonical compact serializer for deterministic `.vox` output.
pub use serialization::canonicalize_vox;
