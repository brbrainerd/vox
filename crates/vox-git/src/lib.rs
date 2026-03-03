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
//!     └── GitForgeProvider ──► GitHub / GitLab / Gitea / Forgejo
//!             (via vox-forge)
//! ```
//!
//! ## Design principles
//! - **No C**: `gix` only. Never `git2` (which wraps libgit2).
//! - **Pure Rust TLS**: reqwest with `rustls-tls` feature.
//! - **Forge-agnostic**: git operations here; platform API calls go to `vox-forge`.

pub mod bridge;
pub mod object;
pub mod refs;
pub mod sync;

pub use bridge::GitBridge;
pub use sync::{FetchResult, PushResult, SyncDirection};
