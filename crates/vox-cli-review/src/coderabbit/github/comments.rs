//! Issue comments and review polling.

use std::path::Path;

use anyhow::{Context, Result};
use vox_forge::GitForgeProvider;
use vox_forge::github::GitHubProvider;

use super::api::{forge_token, owner_repo_from_path};

/// Post a comment to trigger CodeRabbit review.
pub async fn trigger_coderabbit(
    token: &str,
    owner: &str,
    repo: &str,
    pr_number: u64,
    full_review: bool,
) -> Result<()> {
    let body = if full_review {
        "@coderabbitai full review"
    } else {
        "@coderabbitai review"
    };

    let url = format!("https://api.github.com/repos/{owner}/{repo}/issues/{pr_number}/comments");
    let client = vox_http_client::client();
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&serde_json::json!({ "body": body }))
        .send()
        .await
        .context("POST issue comment")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitHub API {status}: {text}");
    }
    Ok(())
}

/// Wait for CodeRabbit bot review on a PR (polls GitHub reviews).
pub async fn wait_for_review(pr_number: u64, timeout_secs: u64, path: &Path) -> Result<()> {
    let (owner, repo) = owner_repo_from_path(path)?;
    let token = forge_token()?;
    let provider = GitHubProvider::new(&token).map_err(|e| anyhow::anyhow!("{e}"))?;

    let start = std::time::Instant::now();
    let poll_interval = vox_config::timeouts::D_30S;

    while start.elapsed().as_secs() < timeout_secs {
        let reviews = provider
            .list_reviews(&owner, &repo, pr_number)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let coderabbit_review = reviews.iter().find(|r| {
            let login = r.reviewer.to_lowercase();
            login.contains("coderabbit") || login.contains("code-rabbit")
        });

        if coderabbit_review.is_some() {
            eprintln!("CodeRabbit review completed for PR #{pr_number}");
            return Ok(());
        }

        tokio::time::sleep(poll_interval).await;
    }

    anyhow::bail!("Timeout waiting for CodeRabbit review on PR #{pr_number} ({timeout_secs}s)")
}
