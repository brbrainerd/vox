//! Lazy-download + SHA256-verify + disk-cache for the `cloudflared` binary.
//!
//! Cache path: `$XDG_CACHE_HOME/vox/cloudflared/` (or `~/.cache/vox/cloudflared/` on Unix,
//! `%LOCALAPPDATA%\vox\cloudflared\` on Windows via the `dirs` crate).
//!
//! Override for testing: set `VOX_CLOUDFLARED_PATH` env var to an absolute path.

use crate::error::{ShareError, ShareResult};
use std::path::{Path, PathBuf};

/// Pinned cloudflared version downloaded by `vox share`.
///
/// To update: pick a new release at <https://github.com/cloudflare/cloudflared/releases>,
/// run `sha256sum` on each platform binary, and update `cloudflared_url_and_checksum`.
pub const CLOUDFLARED_VERSION: &str = "2026.3.0";

/// Returns `(download_url, expected_sha256_lowercase_hex)` for the current platform,
/// or `None` if the platform is unsupported.
pub fn cloudflared_url_and_checksum() -> Option<(String, &'static str)> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let (filename, checksum): (&str, &str) = match (os, arch) {
        ("linux", "x86_64") => (
            "cloudflared-linux-amd64",
            "4a9e50e6d6d798e90fcd01933151a90bf7edd99a0a55c28ad18f2e16263a5c30",
        ),
        ("linux", "aarch64") => (
            "cloudflared-linux-arm64",
            "0755ba4cbab59980e6148367fcf53a8f3ec85a97deefd63c2420cf7850769bee",
        ),
        ("macos", "x86_64") => (
            "cloudflared-darwin-amd64.tgz",
            "b91dbec79a3e3809d5508b96d8b0bdfbf3ad7d51f858200228fa3e57100580d9",
        ),
        ("macos", "aarch64") => (
            "cloudflared-darwin-arm64.tgz",
            "633cee0fd41fd2020e17498beecc54811bf4fc99f891c080dc9343eb0f449c60",
        ),
        ("windows", "x86_64") => (
            "cloudflared-windows-amd64.exe",
            "59b12880b24af581cf5b1013db601c7d843b9b097e9c78aa5957c7f39f741885",
        ),
        _ => return None,
    };
    let url = format!(
        "https://github.com/cloudflare/cloudflared/releases/download/{}/{}",
        CLOUDFLARED_VERSION, filename
    );
    Some((url, checksum))
}

/// Returns the path to the cached cloudflared binary, downloading and verifying if needed.
///
/// Respects `VOX_CLOUDFLARED_PATH` env var override (for testing and power users).
pub async fn ensure_cloudflared() -> ShareResult<PathBuf> {
    // Allow override for testing or power users.
    if let Ok(custom) = std::env::var("VOX_CLOUDFLARED_PATH") {
        let p = PathBuf::from(&custom);
        if !p.exists() {
            return Err(ShareError::Config(format!(
                "VOX_CLOUDFLARED_PATH points to a non-existent file: {}",
                custom
            )));
        }
        return Ok(p);
    }

    let (url, expected_sha) = cloudflared_url_and_checksum().ok_or_else(|| {
        ShareError::Config(format!(
            "cloudflared is not available for this platform ({}-{})",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    })?;

    let cache_dir = cloudflared_cache_dir()?;
    std::fs::create_dir_all(&cache_dir)?;

    let bin_name = cached_binary_name();
    let bin_path = cache_dir.join(&bin_name);

    // Re-use cached binary if it already passes the checksum.
    if bin_path.exists() && verify_sha256(&bin_path, expected_sha)? {
        return Ok(bin_path);
    }

    tracing::info!(
        "Downloading cloudflared {} from {}",
        CLOUDFLARED_VERSION,
        url
    );
    download_and_verify(&url, expected_sha, &bin_path).await?;
    make_executable(&bin_path)?;
    Ok(bin_path)
}

/// Directory where the cached binary lives.
pub fn cloudflared_cache_dir() -> ShareResult<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(dirs::cache_dir)
        .ok_or_else(|| ShareError::Config("could not determine cache directory".into()))?;
    Ok(base.join("vox").join("cloudflared"))
}

/// Filename used for the cached binary (includes version + platform).
pub fn cached_binary_name() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let ext = if os == "windows" { ".exe" } else { "" };
    format!("cloudflared-{}-{}-{}{}", CLOUDFLARED_VERSION, os, arch, ext)
}

/// Verify the SHA256 of `path` matches `expected` (hex, case-insensitive).
pub fn verify_sha256(path: &Path, expected: &str) -> ShareResult<bool> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path)?;
    let actual = hex::encode(Sha256::digest(&bytes));
    Ok(actual.eq_ignore_ascii_case(expected))
}

async fn download_and_verify(url: &str, expected_sha: &str, dest: &Path) -> ShareResult<()> {
    let resp = reqwest::get(url)
        .await
        .map_err(|e| ShareError::Config(format!("download {}: {}", url, e)))?;
    if !resp.status().is_success() {
        return Err(ShareError::Config(format!(
            "download {}: HTTP {}",
            url,
            resp.status()
        )));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| ShareError::Config(format!("read download body: {}", e)))?;

    use sha2::{Digest, Sha256};
    let actual = hex::encode(Sha256::digest(&bytes));
    if !actual.eq_ignore_ascii_case(expected_sha) {
        return Err(ShareError::Config(format!(
            "cloudflared SHA256 mismatch: expected {}, got {}",
            expected_sha, actual
        )));
    }

    std::fs::write(dest, &bytes)?;
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> ShareResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(path)?.permissions();
    perm.set_mode(perm.mode() | 0o111);
    std::fs::set_permissions(path, perm)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> ShareResult<()> {
    Ok(())
}
