//! Single source of truth for news publishing defaults and validation (no I/O).
//!
//! ## Operator config precedence (`PublisherConfig::from_operator_environment`)
//!
//! 1. **Clavis** resolves syndication credentials via `vox_secrets::resolve_secret` (see `SecretId`
//!    variants such as `VoxNewsTwitterBearer`). Resolution covers canonical env keys, aliases, auth JSON,
//!    and optional Infisical/Vault backends when enabled.
//! 2. **Plain env** remains for non-secret knobs: `VOX_SOCIAL_TWITTER_SUMMARY_MARGIN_CHARS`,
//!    `VOX_SOCIAL_REDDIT_SELFPOST_SUMMARY_MAX`, `VOX_SOCIAL_HN_MODE`, `VOX_SOCIAL_YOUTUBE_DEFAULT_CATEGORY_ID`.
//! 3. **Site layout** (`NewsSiteConfig`): orchestrator and MCP set `base_url` / RSS path from
//!    `[orchestrator.news]`, then **all** operator surfaces should call
//!    [`NewsSiteConfig::merge_operator_env_overrides`] so `VOX_NEWS_SITE_BASE_URL` /
//!    `VOX_NEWS_RSS_FEED_PATH` match the orchestrator. CLI publication uses
//!    [`NewsSiteConfig::from_default_with_operator_env`].
//! 4. **Twitter formatting**: optional `VOX_NEWS_TWITTER_TEXT_CHUNK_MAX` and
//!    `VOX_NEWS_TWITTER_TRUNCATION_SUFFIX` in [`crate::PublisherConfig::from_operator_environment`].

/// Default public site base (GitHub Pages) for generated links and RSS.
pub const DEFAULT_SITE_BASE_URL: &str = "https://voxlang.org";

/// Default path to RSS feed relative to repository root.
pub const DEFAULT_RSS_FEED_PATH: &str = "docs/src/feed.xml";

/// Default canonical GitHub repo for foundation news when templates omit overrides.
pub const DEFAULT_GITHUB_REPO: &str = "vox-foundation/vox";

/// Default Open Collective slug.
pub const DEFAULT_OPENCOLLECTIVE_SLUG: &str = "vox-foundation";

/// Production GitHub REST API base.
pub const DEFAULT_GITHUB_REST_BASE: &str = "https://api.github.com";

/// Production GitHub GraphQL endpoint.
pub const DEFAULT_GITHUB_GRAPHQL_URL: &str = "https://api.github.com/graphql";
/// Site configuration for RSS links and feed file location.
#[derive(Debug, Clone)]
pub struct NewsSiteConfig {
    /// e.g. `https://vox-foundation.github.io/vox` (no trailing slash).
    pub base_url: String,
    /// Path to `feed.xml` under repo root.
    pub rss_feed_path: std::path::PathBuf,
}

impl Default for NewsSiteConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_SITE_BASE_URL.trim_end_matches('/').to_string(),
            rss_feed_path: std::path::PathBuf::from(DEFAULT_RSS_FEED_PATH),
        }
    }
}

impl NewsSiteConfig {
    /// Default site layout with `VOX_NEWS_SITE_BASE_URL` / `VOX_NEWS_RSS_FEED_PATH` overrides when set.
    #[must_use]
    pub fn from_default_with_operator_env() -> Self {
        let mut site = Self::default();
        site.merge_operator_env_overrides();
        site
    }

    /// Apply `VOX_NEWS_SITE_BASE_URL` and `VOX_NEWS_RSS_FEED_PATH` when non-empty.
    pub fn merge_operator_env_overrides(&mut self) {
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxNewsSiteBaseUrl).expose()
        {
            let t = v.trim().trim_end_matches('/').to_string();
            if !t.is_empty() {
                self.base_url = t;
            }
        }
        if let Some(v) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxNewsRssFeedPath).expose()
        {
            let t = v.trim();
            if !t.is_empty() {
                self.rss_feed_path = std::path::PathBuf::from(t);
            }
        }
    }

    #[must_use]
    pub fn news_item_link(&self, news_id: &str) -> String {
        format!("{}/news/{}.html", self.base_url, news_id)
    }

    #[must_use]
    pub fn feed_self_link(&self) -> String {
        format!("{}/feed.xml", self.base_url)
    }
}

/// Validate `owner/repo` format.
pub fn validate_github_repo(repo: &str) -> anyhow::Result<()> {
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        anyhow::bail!("Invalid GitHub repo, expected owner/name, got {:?}", repo);
    }
    Ok(())
}

/// Reject path traversal in logical news ids (filename stems).
pub fn validate_news_id(id: &str) -> anyhow::Result<()> {
    if id.is_empty() {
        anyhow::bail!("news id must not be empty");
    }
    if id.contains("..") || id.contains('/') || id.contains('\\') {
        anyhow::bail!("news id must not contain path segments or '..': {:?}", id);
    }
    Ok(())
}

/// Clamp text to `max_chars` preserving UTF-8 boundaries and adding `...` when truncated.
#[must_use]
pub fn clamp_text(input: &str, max_chars: usize) -> String {
    let normalized = input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let keep = max_chars.saturating_sub(3);
    format!("{}...", normalized.chars().take(keep).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::clamp_text;

    #[test]
    fn clamp_text_preserves_short_strings() {
        let out = clamp_text("hello world", 20);
        assert_eq!(out, "hello world");
    }

    #[test]
    fn clamp_text_truncates_and_appends_ellipsis() {
        let out = clamp_text("alpha beta gamma delta epsilon", 12);
        assert!(out.ends_with("..."));
        assert!(out.chars().count() <= 12);
    }

    #[test]
    fn validate_github_repo_requires_exactly_owner_slash_name() {
        // Catches: validate_github_repo accepting empty halves ("foo/") or extra path segments
        // ("a/b/c"), or rejecting a well-formed "owner/name".
        use super::validate_github_repo;
        assert!(validate_github_repo("anthropics/claude").is_ok());
        assert!(validate_github_repo("foo/").is_err());
        assert!(validate_github_repo("/bar").is_err());
        assert!(validate_github_repo("a/b/c").is_err());
        assert!(validate_github_repo("noslash").is_err());
    }

    #[test]
    fn validate_news_id_rejects_path_traversal() {
        // Catches: validate_news_id failing to block "..", "/" or "\\" (path-escape) or
        // rejecting a clean stem / allowing an empty id.
        use super::validate_news_id;
        assert!(validate_news_id("2026-06-15-release").is_ok());
        assert!(validate_news_id("").is_err());
        assert!(validate_news_id("../etc/passwd").is_err());
        assert!(validate_news_id("a/b").is_err());
        assert!(validate_news_id("a\\b").is_err());
    }
}
