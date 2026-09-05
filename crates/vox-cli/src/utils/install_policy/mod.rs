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

/// Repository-relative directory of the ML / mesh CLI crate (`vox mens`, `vox populi`).
pub const SOURCE_INSTALL_ML_CLI_REL_PATH: &str = "crates/vox-ml-cli";

/// Cargo feature that `vox-ml-cli` needs for the mesh transport. It is **not** in that
/// crate's `default` feature set (`default = ["mens-base"]`), so a bare
/// `cargo install --path crates/vox-ml-cli` produces a mesh-less binary and `vox populi`
/// cannot serve. Anything that documents or shells the ML CLI source install must pass it.
pub const ML_CLI_MESH_FEATURE: &str = "populi";

/// `cargo …` arguments for a reproducible ML / mesh CLI install from a local checkout.
///
/// Consumers that should read this instead of hardcoding the string (none do yet — wire
/// them up when those call sites are next touched):
/// - `vox doctor`'s remediation hint for a missing `vox-ml-cli` (`crates/vox-cli/src/commands/
///   diagnostics/doctor/checks_standard/tier_deps.rs`), which currently prints a bare
///   "install it" message and a `voxlang.org/install` URL that 404s.
/// - `vox upgrade --source repo`, which reinstalls the CLI but never the ML CLI.
/// - `CONTRIBUTING.md`'s "Beyond `vox` itself" table and
///   `docs/src/reference/installation.md`, kept in sync by review until a doc-lint gate exists.
pub const CARGO_INSTALL_ML_CLI_FROM_SOURCE: &[&str] = &[
    "install",
    "--locked",
    "--path",
    SOURCE_INSTALL_ML_CLI_REL_PATH,
    "--features",
    ML_CLI_MESH_FEATURE,
];

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

    /// A bare `--path crates/vox-ml-cli` yields a mesh-less binary: `populi` is not in
    /// that crate's default features. Guard the feature flag, not just the path.
    #[test]
    fn ml_cli_install_argv_is_locked_and_carries_the_mesh_feature() {
        assert_eq!(
            CARGO_INSTALL_ML_CLI_FROM_SOURCE,
            &[
                "install",
                "--locked",
                "--path",
                "crates/vox-ml-cli",
                "--features",
                "populi",
            ]
        );
        assert_eq!(SOURCE_INSTALL_ML_CLI_REL_PATH, "crates/vox-ml-cli");
        assert_eq!(ML_CLI_MESH_FEATURE, "populi");
    }

    /// Every documented source install is reproducible against the workspace lockfile.
    #[test]
    fn every_source_install_argv_passes_locked() {
        for argv in [CARGO_INSTALL_CLI_FROM_SOURCE, CARGO_INSTALL_ML_CLI_FROM_SOURCE] {
            assert!(
                argv.contains(&"--locked"),
                "source install argv must pass --locked: {argv:?}"
            );
        }
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
