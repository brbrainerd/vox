//! Resolve the latest Vox release from the GitHub Releases API.

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::Deserialize;

// NOT `/releases/latest`: that endpoint EXCLUDES pre-releases and 404s while every
// published release is a pre-release, which is the case today — an end-to-end
// install run failed here even after the shell wrapper was fixed, because the
// wrapper and this binary each resolve the release independently. List releases
// and pick the newest published, non-draft one instead.
const API_RELEASES: &str = "https://api.github.com/repos/vox-foundation/vox/releases?per_page=20";
// Tag lookup for a specific prerelease (e.g. a nightly), which the listing
// above intentionally cannot name.
const API_TAGS: &str = "https://api.github.com/repos/vox-foundation/vox/releases/tags";
const CLIENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

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

pub fn make_client() -> Result<Client> {
    let mut builder = Client::builder().timeout(CLIENT_TIMEOUT);

    // Support GITHUB_TOKEN/GH_TOKEN for auth header
    if let Ok(token) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN"))
        && !token.trim().is_empty()
    {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(auth_val) = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")) {
            headers.insert(reqwest::header::AUTHORIZATION, auth_val);
            builder = builder.default_headers(headers);
        }
    }

    builder.build().context("failed to build reqwest Client")
}

/// A single release as returned by either endpoint above.
#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
    #[serde(default)]
    draft: bool,
    /// `None` while a release is still a draft. Ordering on this rather than
    /// on the API's default `created_at` matters: `created_at` is the tag's
    /// commit date, so a hotfix cut from an older commit would otherwise win.
    #[serde(default)]
    published_at: Option<String>,
}

fn into_release_info(rel: GhRelease) -> Result<ReleaseInfo> {
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

fn get(client: &Client, url: &str) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header("User-Agent", concat!("voxup/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
}

pub async fn fetch_latest(client: &Client) -> Result<ReleaseInfo> {
    let releases: Vec<GhRelease> = get(client, API_RELEASES)
        .send()
        .await
        .context("GET GitHub releases")?
        .error_for_status()
        .context("GitHub API returned an error")?
        .json()
        .await
        .context("parse GitHub release JSON")?;

    let rel = releases
        .into_iter()
        .filter(|r| !r.draft && r.published_at.is_some())
        .max_by(|a, b| a.published_at.cmp(&b.published_at))
        .context(
            "no published release found (all drafts, or the repository has no releases yet)",
        )?;
    into_release_info(rel)
}

/// Fetch a specific release by its exact git tag (e.g. a nightly prerelease
/// like `v0.6.0-nightly.4812`). This is how `voxup install --tag <tag>`
/// fetches one for local use, without requiring a full local `cargo build`.
///
/// `fetch_latest` lists releases rather than calling `/releases/latest`,
/// because that endpoint excludes prereleases and 404s while every published
/// release is one -- so a tag lookup is the only way to name a specific nightly.
pub async fn fetch_by_tag(client: &Client, tag: &str) -> Result<ReleaseInfo> {
    let url = format!("{API_TAGS}/{tag}");
    let rel: GhRelease = get(client, &url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .context("GitHub API returned an error")?
        .json()
        .await
        .context("parse GitHub release JSON")?;
    into_release_info(rel)
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
