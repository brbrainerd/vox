//! `vox-git` — Pure-Rust Git bridge for Vox.
//!
//! Uses `gix` (gitoxide) for all Git operations. No C, no libgit2.
//!
//! ## Architecture
//!
//! ```text
//! vox-orchestrator
//!     │
//!     ▼
//! GitBridge (this crate)
//!     │
//!     ├── gix::Repository ── local .git/
//!     │
//!     └── GitForgeProvider ──► GitHub (via `vox-forge`; GitLab deprecated)
//! ```
//!
//! ## Design principles
//! - **No C**: `gix` only. Never `git2` (which wraps libgit2).
//! - **Pure Rust TLS**: reqwest with `rustls-tls` feature.
//! - **Forge-agnostic**: git operations here; platform API calls go to `vox-forge`.

/// High-level repository operations using gitoxide.
pub mod bridge;
/// Git object id / OID helpers.
pub mod object;
/// Audited read-only raw-`git` invocations (unified diffs / numstat).
pub mod read_cmd;
/// Reference (branch/tag) utilities.
pub mod refs;
/// Fetch/push and remote sync orchestration.
pub mod sync;

pub use bridge::GitBridge;
pub use read_cmd::{GitReadError, read_only};
pub use sync::{FetchResult, PushResult, SyncDirection};
