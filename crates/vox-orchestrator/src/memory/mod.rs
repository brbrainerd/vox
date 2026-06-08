//! Persistent memory system for Vox agents.
//!
//! Inspired by OpenClaw's file-first memory model:
//! - **Daily logs** (`memory/YYYY-MM-DD.md`) — append-only per-session notes
//! - **MEMORY.md** — curated long-term knowledge indexed by heading
//! - **MemoryManager** — coordinates daily logs + MEMORY.md + VoxDb embeddings,
//!   bootstraps agent context on startup, and flushes critical state before
//!   compaction to prevent knowledge loss. Durable SSOT for agent rows is **Codex**
//!   (`vox_db::Codex`); file logs are a complementary human-editable layer.

pub mod account_registry;
mod config;
mod error;
mod long_term;
mod manager;
mod project_file;
mod search_hit;
#[cfg(test)]
mod tests;
mod time;

pub use account_registry::AccountMemoryRegistry;
pub use config::MemoryConfig;
pub use error::MemoryError;
pub use long_term::LongTermMemory;
pub use manager::{MemoryFact, MemoryManager};
pub use project_file::{MAX_IMPORT_DEPTH, load_project_context};
pub use search_hit::SearchHit;
