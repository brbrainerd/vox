//! UI-agnostic terminal + agent engine. Front-ends (ratatui TUI, GUI Console)
//! render its blocks/events and submit input back. Adapts vox-orchestrator;
//! never reimplements the agent loop.
//!
//! # Stable public surface (v0.1)
//!
//! | Module      | Key types / fns                                              |
//! |-------------|--------------------------------------------------------------|
//! | `block`     | `Block`, `BlockId`, `BlockKind`, `BlockStatus`, `Stream`     |
//! | `input`     | `InputIntent`, `classify()`                                  |
//! | `osc633`    | `Osc633Parser`, `Osc633Event`, `decode_command()`            |
//! | `pty`       | `spawn_pty()`, `PtyHandle`, `ShellBackend`, `default_shell()`|
//! | `session`   | `Session`, `SessionEvent`                                    |
//! | `transcript`| `TranscriptEvent`, `TranscriptKind`, `JournalSink`           |
//! | `agent`     | `translate_event()`, `AgentAdapterConfig`                    |
//! | `vox_interp`| `eval_line()`                                                |

pub const CRATE_NAME: &str = "vox-terminal-core";

pub mod agent;
pub mod block;
pub mod input;
pub mod osc633;
pub mod pty;
pub mod session;
pub mod transcript;
pub mod vox_interp;

// Flat re-exports for ergonomic use by front-ends.
pub use block::{Block, BlockId, BlockKind, BlockStatus, OutputChunk, Stream};
pub use input::{InputIntent, classify};
pub use osc633::{Osc633Event, Osc633Parser};
pub use pty::{PtyHandle, ShellBackend, ShellKind, default_shell, spawn_pty};
pub use session::{Session, SessionEvent};
pub use transcript::{JournalSink, NullSink, TranscriptEvent, TranscriptKind, TranscriptSink};
pub use agent::{AgentAdapterConfig, translate_event};
pub use vox_interp::eval_line;
