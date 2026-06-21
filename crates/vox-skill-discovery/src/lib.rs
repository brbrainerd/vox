//! Local, on-demand discovery + dedup engine. Mines repeated `.vox` code blocks,
//! dedups installed skills, and flags MCP↔skill SSOT drift. Advisory only — it
//! never installs, executes, or publishes.

pub mod candidate;
pub mod catalog;
pub mod code_miner;
pub mod options;
pub mod report;

pub use candidate::{Candidate, CandidateKind, DraftFrontmatter};
pub use catalog::{dedup_skills, validate_ssot};
pub use code_miner::mine_repeated_code;
pub use options::DiscoverOptions;
pub use report::{render_json, render_terminal};
