//! `vox-forge` — Platform-agnostic Git forge integration for Vox.
//!
//! Abstracts Git forges (primarily **GitHub**) behind a single trait.
//! GitLab is **deprecated (2026-06-03)** and no longer supported.
//! All forge-specific API logic lives in the per-platform modules below;
//! callers only depend on [`GitForgeProvider`].
//!
//! ## Forge coverage
//! | Forge    | Feature flag | API basis        | Self-hostable | Status |
//! |----------|-------------|------------------|---------------|--------|
//! | GitHub   | `github`    | REST + GraphQL   | Enterprise only | Supported |
//! | GitLab   | `gitlab`    | REST             | ✅ CE (free) | **Deprecated (2026-06-03)** — unsupported, slated for removal |
//!
//! > **GitLab is no longer supported.** The `gitlab` module is retained behind
//! > the (deprecated) `gitlab` feature and emits a runtime warning when
//! > constructed. New integrations must use GitHub.
//!
//! ## Platform independence
//! All internal Vox code uses `ChangeRequest` instead of "PR" or "MR".

/// Error types for forge HTTP/auth/parse failures.
pub mod error;
/// [`GitForgeProvider`](provider::GitForgeProvider) trait and registry.
pub mod provider;
/// Forge-neutral DTOs (change requests, labels, webhooks, …).
pub mod types;

// Platform implementations — compiled only when the relevant feature is enabled.
/// GitHub REST (`api.github.com` or Enterprise base URL).
#[cfg(feature = "github")]
pub mod github;
/// GitLab REST API. **DEPRECATED (2026-06-03): no longer supported; slated for removal.**
#[cfg(feature = "gitlab")]
pub mod gitlab;

pub use error::ForgeError;
pub use provider::GitForgeProvider;
pub use types::{
    ChangeRequest, ChangeRequestId, ChangeRequestState, ChangeRequestStatus, ForgeRepoInfo,
    ForgeUser, Label, NewChangeRequest, Review, ReviewState, WebhookEvent,
};
