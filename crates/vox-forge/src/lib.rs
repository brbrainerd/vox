//! `vox-forge` — Platform-agnostic Git forge integration for Vox.
//!
//! Abstracts GitHub, GitLab, Gitea, and Forgejo behind a single trait.
//! All forge-specific API logic lives in the per-platform modules below;
//! callers only depend on `GitForgeProvider`.
//!
//! ## Forge coverage
//! | Forge    | Feature flag | API basis        | Self-hostable |
//! |----------|-------------|------------------|---------------|
//! | GitHub   | `github`    | REST + GraphQL   | Enterprise only |
//! | GitLab   | `gitlab`    | REST             | ✅ CE (free) |
//! | Gitea    | `gitea`     | REST (swagger)   | ✅ Free |
//! | Forgejo  | `gitea`     | REST (compatible)| ✅ Free |
//!
//! ## Platform independence
//! Inspired by the Zig project's 2025 migration from GitHub to Codeberg (Forgejo).
//! All internal Vox code uses `ChangeRequest` instead of "PR" or "MR".

pub mod error;
pub mod provider;
pub mod types;

// Platform implementations — compiled only when the relevant feature is enabled.
#[cfg(feature = "github")]
pub mod github;
#[cfg(feature = "gitlab")]
pub mod gitlab;
#[cfg(feature = "gitea")]
pub mod gitea;

pub use error::ForgeError;
pub use provider::GitForgeProvider;
pub use types::{
    ChangeRequest, ChangeRequestId, ChangeRequestState, ChangeRequestStatus,
    ForgeRepoInfo, ForgeUser, Label, Review, ReviewState, WebhookEvent,
};
