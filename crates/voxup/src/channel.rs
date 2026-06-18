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

pub fn asset_name(version: &str) -> String {
    let target = env!("TARGET");
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!("vox-{version}-{target}.{ext}")
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
        let name = asset_name("0.7.0");
        assert!(name.starts_with("vox-0.7.0-"), "got: {name}");
        assert!(name.contains(env!("TARGET")), "got: {name}");
    }

    #[test]
    fn asset_name_has_correct_extension() {
        let name = asset_name("1.2.3");
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
}
