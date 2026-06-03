//! Git remote URL parsing and token resolution.

use std::path::Path;

use anyhow::{Context, Result};
use vox_git::GitBridge;

/// Parse owner/repo from a GitHub URL (https or git).
pub(crate) fn parse_github_owner_repo(url: &str) -> Option<(String, String)> {
    let url = url.trim().trim_end_matches('/');
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("git@github.com:"))?;
    let rest = rest.trim_end_matches(".git");
    let mut parts = rest.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

/// Resolve Forge token: `FORGE_TOKEN` / `GITHUB_TOKEN`.
pub(crate) fn forge_token() -> Result<String> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::ForgeToken)
        .expose()
        .map(std::string::ToString::to_string)
        .context("Forge token required: set FORGE_TOKEN or GITHUB_TOKEN.")
}

pub(crate) fn owner_repo_from_path(path: &Path) -> Result<(String, String)> {
    let bridge = GitBridge::open(path).context("open git repo")?;
    let remote_url = bridge.remote_url().context("remote URL")?;
    parse_github_owner_repo(&remote_url).context("parse GitHub owner/repo from remote URL")
}
