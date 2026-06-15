use crate::PublisherConfig;
use crate::contract::DEFAULT_GITHUB_REST_BASE;
use crate::types::{ForgeConfig, ForgePostType, UnifiedNewsItem};
use anyhow::{Result, anyhow};
use vox_forge::GitForgeProvider;
use vox_forge::github::GitHubProvider;
use vox_forge::types::{NewDiscussionOrIssue, NewRelease};

pub async fn post(
    publisher_cfg: &PublisherConfig,
    token: &str,
    item: &UnifiedNewsItem,
    config: &ForgeConfig,
) -> Result<String> {
    let rest_base = publisher_cfg
        .forge_rest_base
        .clone()
        .unwrap_or_else(|| DEFAULT_GITHUB_REST_BASE.to_string());
    let base = format!("{}/", rest_base.trim_end_matches('/'));

    let provider = GitHubProvider::with_base(token, &base)
        .map_err(|e| anyhow!("Failed to initialize GitHub provider: {e}"))?;

    match config.post_type {
        ForgePostType::Discussion => post_discussion(&provider, item, config).await,
        ForgePostType::Release => post_release(&provider, item, config).await,
    }
}

async fn post_release(
    provider: &impl GitForgeProvider,
    item: &UnifiedNewsItem,
    config: &ForgeConfig,
) -> Result<String> {
    let parts: Vec<&str> = config.repo.split('/').collect();
    if parts.len() != 2 {
        return Err(anyhow!(
            "Invalid target repo format, expected owner/repo, got {}",
            config.repo
        ));
    }
    let owner = parts[0];
    let repo = parts[1];

    let tag_name = config
        .release_tag
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(item.id.as_str());

    let release_req = NewRelease {
        tag_name,
        name: &item.title,
        body: &item.content_markdown,
        draft: config.draft,
    };

    let url = provider
        .create_release(owner, repo, release_req)
        .await
        .map_err(|e| anyhow!("Failed to create release: {e}"))?;

    tracing::info!("Created GitHub Release: {}", url);
    Ok(url)
}

async fn post_discussion(
    provider: &impl GitForgeProvider,
    item: &UnifiedNewsItem,
    config: &ForgeConfig,
) -> Result<String> {
    let parts: Vec<&str> = config.repo.split('/').collect();
    if parts.len() != 2 {
        return Err(anyhow!(
            "Invalid target repo format, expected owner/repo, got {}",
            config.repo
        ));
    }
    let owner = parts[0];
    let repo = parts[1];
    let category_name = config
        .discussion_category
        .as_deref()
        .map(str::trim)
        .ok_or_else(|| anyhow!("discussion_category required"))?;

    let req = NewDiscussionOrIssue {
        title: &item.title,
        body: &item.content_markdown,
        category: Some(category_name),
    };

    let url = provider
        .create_discussion_or_issue(owner, repo, req)
        .await
        .map_err(|e| anyhow!("Failed to create discussion: {e}"))?;

    tracing::info!("Created GitHub Discussion: {}", url);
    Ok(url)
}
