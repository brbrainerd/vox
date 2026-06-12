//! Automated discovery producers (Task 14 + 15).
//!
//! Producers turn raw repository activity into DRAFT finding candidates that a
//! human reviews and (separately) approves. Nothing in this module publishes,
//! approves, or fabricates evidence: every signal is derived deterministically
//! from observable inputs, and absence is propagated as `None`/skip rather than
//! a synthetic score.
//!
//! - [`commit_watcher`] — pure signal extraction from a commit view, plus a
//!   `CommitView` shape the CLI fills from `git log --numstat`.
//! - [`code_uniqueness`] — pure uniqueness math + Rust snippet extraction, plus
//!   an async assessor seam ([`CodeKnnIndex`]) so embedding-distance novelty can
//!   be computed against a vector index when one is configured.

pub mod code_uniqueness;
pub mod commit_watcher;

pub use code_uniqueness::{
    CodeKnnIndex, CodeSnippet, CodeUniquenessAssessment, assess_code_uniqueness, extract_snippets,
    uniqueness_score,
};
pub use commit_watcher::{CommitView, signals_from_commit};
