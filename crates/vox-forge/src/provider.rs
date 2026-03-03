//! `GitForgeProvider` — the core trait for all forge integrations.
//!
//! Implement this trait to add support for a new Git hosting platform.
//! All methods are async; use `async_trait` for object safety.

use async_trait::async_trait;

use crate::error::ForgeError;
use crate::types::{
    ChangeRequest, ChangeRequestId, ChangeRequestState, ForgeRepoInfo,
    ForgeUser, Label, Review, WebhookEvent,
};

/// A platform-agnostic interface to a Git forge (GitHub, GitLab, Gitea, Forgejo, …).
///
/// Implementations must be `Send + Sync` for use across async task boundaries.
///
/// ## Adding a new forge
/// 1. Create `src/<forge>.rs` with a struct that implements this trait.
/// 2. Add a feature flag `<forge>` in `Cargo.toml`.
/// 3. Gate the module behind `#[cfg(feature = "<forge>")]` in `lib.rs`.
/// 4. Wire the new forge into `ForgeRegistry`.
///
/// ## Terminology
/// Internally Vox uses "ChangeRequest" for what GitHub calls "Pull Request"
/// and GitLab calls "Merge Request". All trait methods use this neutral term.
#[async_trait]
pub trait GitForgeProvider: Send + Sync {
    /// Human-readable name of this forge (e.g., "GitHub", "GitLab").
    fn name(&self) -> &str;

    /// Base URL of the forge API (e.g., "https://api.github.com").
    fn api_base_url(&self) -> &str;

    // ── Repository ─────────────────────────────────────────────────────────

    /// Fetch metadata for a repository.
    async fn repo_info(&self, owner: &str, repo: &str) -> Result<ForgeRepoInfo, ForgeError>;

    // ── Change Requests ────────────────────────────────────────────────────

    /// List open ChangeRequests for a repository.
    async fn list_change_requests(
        &self,
        owner: &str,
        repo: &str,
        state: Option<ChangeRequestState>,
        limit: u32,
    ) -> Result<Vec<ChangeRequest>, ForgeError>;

    /// Get a specific ChangeRequest by number.
    async fn get_change_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<ChangeRequest, ForgeError>;

    /// Open a new ChangeRequest.
    async fn create_change_request(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        source_branch: &str,
        target_branch: &str,
        draft: bool,
    ) -> Result<ChangeRequest, ForgeError>;

    /// Update an existing ChangeRequest's title and/or body.
    async fn update_change_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        title: Option<&str>,
        body: Option<&str>,
        state: Option<ChangeRequestState>,
    ) -> Result<ChangeRequest, ForgeError>;

    /// Merge a ChangeRequest. Returns the merge commit SHA.
    async fn merge_change_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        merge_message: Option<&str>,
    ) -> Result<String, ForgeError>;

    // ── Reviews ────────────────────────────────────────────────────────────

    /// List reviews on a ChangeRequest.
    async fn list_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<Review>, ForgeError>;

    // ── Labels ─────────────────────────────────────────────────────────────

    /// Add labels to a ChangeRequest.
    async fn add_labels(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        labels: &[String],
    ) -> Result<Vec<Label>, ForgeError>;

    // ── Users ──────────────────────────────────────────────────────────────

    /// Get the currently authenticated user.
    async fn current_user(&self) -> Result<ForgeUser, ForgeError>;

    // ── Webhooks ───────────────────────────────────────────────────────────

    /// Parse a raw webhook payload into a `WebhookEvent`.
    ///
    /// The `event_type` is the platform-specific event header
    /// (e.g., `X-GitHub-Event`, `X-Gitlab-Event`).
    fn parse_webhook(
        &self,
        event_type: &str,
        payload: &[u8],
    ) -> Result<WebhookEvent, ForgeError>;

    // ── Health ─────────────────────────────────────────────────────────────

    /// Verify API connectivity. Returns the API rate limit remaining, if applicable.
    async fn health_check(&self) -> Result<Option<u32>, ForgeError>;
}

// ---------------------------------------------------------------------------
// ForgeRegistry
// ---------------------------------------------------------------------------

/// A runtime registry of available forge providers.
///
/// Used by the orchestrator to dispatch forge operations without knowing
/// which concrete platform is in use.
#[derive(Default)]
pub struct ForgeRegistry {
    providers: Vec<Box<dyn GitForgeProvider>>,
}

impl ForgeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a forge provider.
    pub fn register(&mut self, provider: Box<dyn GitForgeProvider>) {
        self.providers.push(provider);
    }

    /// Get the first registered provider by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&dyn GitForgeProvider> {
        self.providers
            .iter()
            .find(|p| p.name().eq_ignore_ascii_case(name))
            .map(|p| p.as_ref())
    }

    /// List all registered provider names.
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// True if no providers are registered.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A no-op forge provider for testing.
    struct NullForge;

    #[async_trait]
    impl GitForgeProvider for NullForge {
        fn name(&self) -> &str { "NullForge" }
        fn api_base_url(&self) -> &str { "https://null.example.com/api" }

        async fn repo_info(&self, _o: &str, _r: &str) -> Result<ForgeRepoInfo, ForgeError> {
            Err(ForgeError::Unsupported {
                forge: "NullForge".into(),
                operation: "repo_info".into(),
            })
        }
        async fn list_change_requests(&self, _o: &str, _r: &str, _s: Option<ChangeRequestState>, _l: u32)
            -> Result<Vec<ChangeRequest>, ForgeError> { Ok(vec![]) }
        async fn get_change_request(&self, _o: &str, _r: &str, _n: u64)
            -> Result<ChangeRequest, ForgeError> {
            Err(ForgeError::NotFound { resource: "cr".into() })
        }
        async fn create_change_request(&self, _o: &str, _r: &str, _t: &str, _b: &str, _s: &str, _tg: &str, _d: bool)
            -> Result<ChangeRequest, ForgeError> {
            Err(ForgeError::Unsupported { forge: "NullForge".into(), operation: "create_cr".into() })
        }
        async fn update_change_request(&self, _o: &str, _r: &str, _n: u64, _t: Option<&str>, _b: Option<&str>, _s: Option<ChangeRequestState>)
            -> Result<ChangeRequest, ForgeError> {
            Err(ForgeError::Unsupported { forge: "NullForge".into(), operation: "update_cr".into() })
        }
        async fn merge_change_request(&self, _o: &str, _r: &str, _n: u64, _m: Option<&str>)
            -> Result<String, ForgeError> {
            Err(ForgeError::Unsupported { forge: "NullForge".into(), operation: "merge_cr".into() })
        }
        async fn list_reviews(&self, _o: &str, _r: &str, _n: u64)
            -> Result<Vec<Review>, ForgeError> { Ok(vec![]) }
        async fn add_labels(&self, _o: &str, _r: &str, _n: u64, _l: &[String])
            -> Result<Vec<Label>, ForgeError> { Ok(vec![]) }
        async fn current_user(&self) -> Result<ForgeUser, ForgeError> {
            Err(ForgeError::Unauthorized { reason: "no auth".into() })
        }
        fn parse_webhook(&self, _e: &str, _p: &[u8]) -> Result<WebhookEvent, ForgeError> {
            Ok(WebhookEvent::Unknown { event_type: "test".into() })
        }
        async fn health_check(&self) -> Result<Option<u32>, ForgeError> {
            Ok(None)
        }
    }

    #[test]
    fn registry_register_and_lookup() {
        let mut registry = ForgeRegistry::new();
        assert!(registry.is_empty());
        registry.register(Box::new(NullForge));
        assert_eq!(registry.len(), 1);
        assert!(registry.get("NullForge").is_some());
        assert!(registry.get("nullforge").is_some()); // case-insensitive
        assert!(registry.get("GitHub").is_none());
    }

    #[test]
    fn provider_names() {
        let mut registry = ForgeRegistry::new();
        registry.register(Box::new(NullForge));
        assert_eq!(registry.provider_names(), vec!["NullForge"]);
    }
}
