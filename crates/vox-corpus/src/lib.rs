//! Corpus crate for the Vox workspace: aggregates training data, MCP meta corpora,
//! synthetic-search generators, tool-workflow corpora, and codegen-Vox samples.
//! Used by `vox-ml-cli` and downstream training pipelines.

pub mod arca_replay;
pub mod ast_mutator;
pub mod codegen_vox;
pub mod corpus;
pub mod dataset_snapshot;
pub mod external_review_replay;
pub mod flywheel;
pub mod research_gen;
pub mod rust_to_vox;
#[cfg(test)]
mod semcov_wave30_tests;
pub mod synthetic_gen;
pub mod synthetic_search_gen;
pub mod tool_workflow_corpus;
pub mod training;
