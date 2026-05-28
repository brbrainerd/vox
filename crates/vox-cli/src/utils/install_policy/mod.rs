//! Single source of truth for **install / update** policy strings shared by `vox-bootstrap`,
//! `vox upgrade`, `vox ci release-build`, and compliance guards.
//!
//! Keep [`SUPPORTED_RELEASE_TARGETS`] aligned with `.github/workflows/release-binaries.yml` and
//! `docs/src/ci/binary-release-contract.md` (enforced by `vox ci command-compliance`).

/// Repository-relative directory of the primary `vox` CLI crate (`cargo install --path …`).
pub const SOURCE_INSTALL_CLI_REL_PATH: &str = "crates/vox-cli";

/// `cargo …` arguments for a reproducible install from a local checkout (uses workspace `Cargo.lock`).
pub const CARGO_INSTALL_CLI_FROM_SOURCE: &[&str] =
    &["install", "--locked", "--path", SOURCE_INSTALL_CLI_REL_PATH];

/// Default GitHub **owner** for release downloads (`vox-bootstrap`, `vox upgrade --provider github`).
pub const DEFAULT_RELEASE_GITHUB_OWNER: &str = "vox-foundation";

/// Default GitHub **repository** name for release downloads.
pub const DEFAULT_RELEASE_GITHUB_REPO: &str = "vox";

/// Rust target triples for which release archives are built and published.
pub const SUPPORTED_RELEASE_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
];

/// Managed OpenClaw sidecar executable base name installed alongside `vox`.
pub const OPENCLAW_SIDECAR_BIN_BASENAME: &str = "openclaw-gateway";

/// Candidate filename prefixes searched in release `checksums.txt` for managed sidecar install.
pub const OPENCLAW_SIDECAR_ASSET_PREFIXES: &[&str] = &["openclaw-gateway-", "openclaw-"];

/// Opt-out environment variable for managed OpenClaw sidecar installs.
pub const VOX_OPENCLAW_SIDECAR_DISABLE_ENV: &str = "VOX_OPENCLAW_SIDECAR_DISABLE";

/// Compile-time host triple when it matches a supported release target; used by `vox-bootstrap`
/// to pick a prebuilt asset. Returns [`None`] on unsupported hosts.
pub fn host_triple_for_release_binary_install() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Some("x86_64-unknown-linux-gnu");
    }
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Some("x86_64-pc-windows-msvc");
    }
    if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        return Some("x86_64-apple-darwin");
    }
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Some("aarch64-apple-darwin");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_install_argv_includes_locked_and_path() {
        assert_eq!(CARGO_INSTALL_CLI_FROM_SOURCE[0], "install");
        assert_eq!(CARGO_INSTALL_CLI_FROM_SOURCE[1], "--locked");
        assert_eq!(CARGO_INSTALL_CLI_FROM_SOURCE[2], "--path");
        assert_eq!(
            CARGO_INSTALL_CLI_FROM_SOURCE[3],
            SOURCE_INSTALL_CLI_REL_PATH
        );
    }

    #[test]
    fn supported_targets_nonempty_unique() {
        assert!(!SUPPORTED_RELEASE_TARGETS.is_empty());
        let mut v = SUPPORTED_RELEASE_TARGETS.to_vec();
        let n = v.len();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), n, "duplicate entries in SUPPORTED_RELEASE_TARGETS");
    }
}
