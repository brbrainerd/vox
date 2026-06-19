//! Local, on-demand discovery + dedup engine. Mines repeated `.vox` code blocks,
//! dedups installed skills, and flags MCP↔skill SSOT drift. Advisory only — it
//! never installs, executes, or publishes.

pub mod candidate;
pub mod code_miner;
pub mod options;

pub use candidate::{Candidate, CandidateKind, DraftFrontmatter};
pub use options::DiscoverOptions;
