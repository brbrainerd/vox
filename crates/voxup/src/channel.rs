//! Resolve the latest Vox release from the GitHub Releases API.

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::Deserialize;

const API_LATEST: &str = "https://api.github.com/repos/vox-foundation/vox/releases/latest";

#[derive(Debug, Deserialize, Clone)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub assets: Vec<GhAsset>,
}

impl ReleaseInfo {
    pub fn find_asset(&self, name: &str) -> Option<&GhAsset> {
        self.assets.iter().find(|a| a.name == name)
    }
}

/// Returns the expected archive name for the given release tag on this platform.
///
/// `tag` is the raw GitHub tag string, e.g. `"v0.7.0"`. Asset names in GitHub
/// Releases retain the `v` prefix exactly as the tag was pushed — so must we.
pub fn asset_name(tag: &str) -> String {
    let target = env!("TARGET");
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!("vox-{tag}-{target}.{ext}")
}

const CLIENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub fn make_client() -> Result<Client> {
    // drift-allow(reqwest-bypass): voxup is a standalone binary with no vox_http_client dep
    let mut builder = Client::builder().timeout(CLIENT_TIMEOUT);

    // Support GITHUB_TOKEN/GH_TOKEN for auth header
    if let Ok(token) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")) {
        if !token.trim().is_empty() {
            let mut headers = reqwest::header::HeaderMap::new();
            const BEARER_PREFIX: &str = "Bearer "; // drift-allow(bearer-header-inline): voxup standalone, no vox_http_client dep
            if let Ok(auth_val) = reqwest::header::HeaderValue::from_str(&format!("{BEARER_PREFIX}{token}"))
            {
                headers.insert(reqwest::header::AUTHORIZATION, auth_val);
                builder = builder.default_headers(headers);
            }
        }
    }

    builder.build().context("failed to build reqwest Client")
}

pub async fn fetch_latest(client: &Client) -> Result<ReleaseInfo> {
    #[derive(Deserialize)]
    struct GhRelease {
        tag_name: String,
        assets: Vec<GhAsset>,
    }
    let rel: GhRelease = client
        .get(API_LATEST)
        .header("User-Agent", concat!("voxup/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("GET GitHub releases/latest")?
        .error_for_status()
        .context("GitHub API returned an error")?
        .json()
        .await
        .context("parse GitHub release JSON")?;
    let version = rel.tag_name.trim_start_matches('v').to_string();
    if version.is_empty() {
        bail!("release tag {:?} has no version after 'v'", rel.tag_name);
    }
    Ok(ReleaseInfo {
        tag: rel.tag_name,
        version,
        assets: rel.assets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_name_contains_version_and_target() {
        let name = asset_name("v0.7.0");
        assert!(name.starts_with("vox-v0.7.0-"), "got: {name}");
        assert!(name.contains(env!("TARGET")), "got: {name}");
    }

    #[test]
    fn asset_name_has_correct_extension() {
        let name = asset_name("v1.2.3");
        if cfg!(windows) {
            assert!(name.ends_with(".zip"), "got: {name}");
        } else {
            assert!(name.ends_with(".tar.gz"), "got: {name}");
        }
    }

    #[test]
    fn find_asset_locates_by_exact_name() {
        let info = ReleaseInfo {
            tag: "v0.7.0".into(),
            version: "0.7.0".into(),
            assets: vec![
                GhAsset {
                    name: "checksums.txt".into(),
                    browser_download_url: "https://example.com/checksums.txt".into(),
                    size: 512,
                },
                GhAsset {
                    name: "vox-0.7.0-x86_64-unknown-linux-gnu.tar.gz".into(),
                    browser_download_url: "https://example.com/vox.tar.gz".into(),
                    size: 4_000_000,
                },
            ],
        };
        assert_eq!(
            info.find_asset("checksums.txt").unwrap().name,
            "checksums.txt"
        );
        assert!(info.find_asset("nonexistent.zip").is_none());
    }

    #[test]
    fn asset_name_includes_v_prefix_to_match_ci_artifact_filename() {
        // CI calls: artifact_filename("vox", "v0.7.0", target) → "vox-v0.7.0-{target}.{ext}"
        // voxup must look for: "vox-v0.7.0-{target}.{ext}" — WITH the 'v'
        // The CI-side contract is locked in vox-cli/release_build.rs::artifact_filename_contract_is_stable
        let name = asset_name("v0.7.0");
        assert!(
            name.starts_with("vox-v0.7.0-"),
            "asset_name must keep the 'v' prefix to match CI artifact names, got: {name}"
        );
    }

    #[test]
    fn test_make_client_creates_client() {
        let _client = make_client().unwrap();
    }
}
