//! UI-agnostic terminal + agent engine. Front-ends (ratatui TUI, GUI Console)
//! render its blocks/events and submit input back. Adapts vox-orchestrator;
//! never reimplements the agent loop.
pub const CRATE_NAME: &str = "vox-terminal-core";

pub mod block;
pub mod input;
pub mod osc633;
pub mod pty;
pub mod session;
pub mod transcript;
pub mod vox_interp;
