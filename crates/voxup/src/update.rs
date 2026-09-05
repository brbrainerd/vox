//! `voxup update` — check for a newer Vox release and upgrade if found.

use anyhow::{Context, Result};
use semver::Version;
use std::path::Path;
use tracing::info;

pub fn parse_version_output(s: &str) -> Result<Version> {
    for token in s.split_whitespace() {
        let clean = token.trim_start_matches('v');
        if let Ok(v) = Version::parse(clean) {
            return Ok(v);
        }
    }
    anyhow::bail!("could not find a semver string in: {:?}", s)
}

pub fn read_installed_version(vox_bin: &Path) -> Result<Version> {
    let output = std::process::Command::new(vox_bin)
        .arg("--version")
        .output()
        .with_context(|| format!("run {} --version", vox_bin.display()))?;
    parse_version_output(&String::from_utf8_lossy(&output.stdout))
}

pub async fn run_update() -> Result<bool> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    let real_vox = crate::proxy::resolve_vox_bin(&home);
    if !real_vox.exists() {
        anyhow::bail!("real vox binary not found under toolchains — run `voxup install` first");
    }
    let installed = read_installed_version(&real_vox)?;
    info!("Installed: {installed}");
    let client = crate::channel::make_client()?;
    let release = crate::channel::fetch_latest(&client).await?;
    let latest = Version::parse(&release.version)
        .with_context(|| format!("parse remote version {:?}", release.version))?;
    info!("Latest:    {latest}");
    if latest <= installed {
        println!("✅ Vox {installed} is already up to date.");
        return Ok(false);
    }
    println!("⬆  Upgrading {installed} → {latest}…");
    crate::install::run_install("default", None).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_output_handles_standard_format() {
        let v = parse_version_output("vox 0.6.0\n").unwrap();
        assert_eq!(v, Version::new(0, 6, 0));
    }

    #[test]
    fn parse_version_output_strips_leading_v() {
        let v = parse_version_output("vox v0.7.1\n").unwrap();
        assert_eq!(v, Version::new(0, 7, 1));
    }

    #[test]
    fn parse_version_output_fails_on_garbage() {
        let err = parse_version_output("no version here").unwrap_err();
        assert!(err.to_string().contains("could not find a semver"));
    }

    #[test]
    fn parse_version_output_works_with_build_metadata() {
        let v = parse_version_output("vox 0.6.0 (2026-06-18 abc1234)").unwrap();
        assert_eq!(v, Version::new(0, 6, 0));
    }
}
