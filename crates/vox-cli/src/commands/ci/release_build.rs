use crate::utils::release_artifacts::{
    artifact_filename as release_artifact_filename, checksum_line, is_windows_target,
    package_tar_gz, package_zip, sha256_file,
};
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use vox_cli_ci::cmd_enums::ReleasePackage;

/// Supported release triples (SSOT: `vox-install-policy`; keep workflow/docs aligned via `vox ci command-compliance`).
pub use crate::utils::install_policy::SUPPORTED_RELEASE_TARGETS;

/// Crates the release builder shells `cargo build -p` for. Asserted against the
/// workspace by `every_release_package_exists_in_the_workspace`.
pub(crate) const RELEASE_PACKAGES: &[&str] = &["vox-cli", "vox-ml-cli"];

pub(crate) fn validate_release_target(target: &str) -> Result<()> {
    if SUPPORTED_RELEASE_TARGETS.contains(&target) {
        Ok(())
    } else {
        Err(anyhow!(
            "unsupported release target `{target}`; supported: {}",
            SUPPORTED_RELEASE_TARGETS.join(", ")
        ))
    }
}

pub fn run(
    repo_root: &Path,
    target: &str,
    version: Option<&str>,
    out_dir: &Path,
    package: ReleasePackage,
) -> Result<()> {
    validate_release_target(target).context("release-build target")?;
    let artifact_version = version.unwrap_or(env!("CARGO_PKG_VERSION"));
    let out_dir_abs = resolve_out_dir(repo_root, out_dir);
    fs::create_dir_all(&out_dir_abs)
        .with_context(|| format!("create out dir {}", out_dir_abs.display()))?;

    let mut checksum_lines = Vec::new();
    let want_vox = matches!(package, ReleasePackage::Vox | ReleasePackage::All);
    let want_mens = matches!(package, ReleasePackage::Mens | ReleasePackage::All);

    if want_vox {
        let artifact_name = build_and_package_binary(
            repo_root,
            out_dir_abs.as_path(),
            target,
            artifact_version,
            "vox-cli",
            executable_name(target),
            "vox",
        )?;
        let digest = sha256_file(&out_dir_abs.join(&artifact_name))?;
        checksum_lines.push(checksum_line(&digest, &artifact_name));
    }
    if want_mens {
        let mens_bin = plugin_executable_name(target, "vox-ml-cli");
        let artifact_name = build_and_package_binary(
            repo_root,
            out_dir_abs.as_path(),
            target,
            artifact_version,
            "vox-ml-cli",
            &mens_bin,
            "vox-ml-cli",
        )?;
        let digest = sha256_file(&out_dir_abs.join(&artifact_name))?;
        checksum_lines.push(checksum_line(&digest, &artifact_name));
    }
    let checksums = out_dir_abs.join("checksums.txt");
    fs::write(&checksums, checksum_lines.join(""))
        .with_context(|| format!("write checksum manifest {}", checksums.display()))?;

    println!("release-build complete");
    println!("  target: {target}");
    println!("  package: {:?}", package);
    println!("  checksums: {}", checksums.display());
    Ok(())
}

fn resolve_out_dir(repo_root: &Path, out_dir: &Path) -> PathBuf {
    if out_dir.is_absolute() {
        out_dir.to_path_buf()
    } else {
        repo_root.join(out_dir)
    }
}

fn executable_name(target: &str) -> &'static str {
    if is_windows_target(target) {
        "vox.exe"
    } else {
        "vox"
    }
}

/// Plugin binary name resolution for `vox-ml-cli` archives.
///
/// Returns an owned `String` rather than `&'static str` because plugin names
/// are dynamic (any `vox-<name>` pattern), unlike the fixed core/bootstrap names.
fn plugin_executable_name(target: &str, plugin: &str) -> String {
    if is_windows_target(target) {
        format!("{plugin}.exe")
    } else {
        plugin.to_string()
    }
}

fn build_and_package_binary(
    repo_root: &Path,
    out_dir_abs: &Path,
    target: &str,
    artifact_version: &str,
    package_name: &str,
    built_bin_name: &str,
    archive_name: &str,
) -> Result<String> {
    let mut cmd = Command::new(super::cargo_bin());
    cmd.current_dir(repo_root).args([
        "build",
        "-p",
        package_name,
        "--release",
        "--locked",
        "--target",
        target,
    ]);
    // The shipped `vox` binary keeps full on-disk retrieval (tantivy full-text +
    // web-scrape). That stack is gated behind the `heavy-retrieval` feature, which
    // is OFF by default so lean dev/CI/mobile builds stay slim (WS2-T3). Re-enable
    // it here so end users are unaffected. Other packages don't have the feature.
    if package_name == "vox-cli" {
        cmd.args(["--features", "heavy-retrieval"]);
    }
    let status = cmd
        .status()
        .with_context(|| format!("spawn cargo build for {package_name} release artifact"))?;
    if !status.success() {
        return Err(anyhow!(
            "cargo build failed for crate {package_name} target {target} with status {status}"
        ));
    }

    let built_binary = repo_root
        .join("target")
        .join(target)
        .join("release")
        .join(built_bin_name);
    if !built_binary.is_file() {
        return Err(anyhow!(
            "built binary not found at {}",
            built_binary.display()
        ));
    }

    let artifact_name = release_artifact_filename(archive_name, artifact_version, target);
    let artifact_path = out_dir_abs.join(&artifact_name);
    if is_windows_target(target) {
        package_zip(&built_binary, &artifact_path, built_bin_name)?;
    } else {
        package_tar_gz(&built_binary, &artifact_path, built_bin_name)?;
    }
    println!("  artifact: {}", artifact_path.display());
    Ok(artifact_name)
}

#[cfg(test)]
mod tests {
    use crate::utils::release_artifacts::artifact_filename;
    use vox_bounded_fs::read_utf8_path_capped;

    use super::{checksum_line, executable_name, plugin_executable_name, validate_release_target};

    #[test]
    fn unsupported_target_errors() {
        let err = validate_release_target("riscv64-unknown-linux-gnu").unwrap_err();
        assert!(
            err.to_string().contains("unsupported release target"),
            "{err}"
        );
    }

    #[test]
    fn executable_name_matches_target_family() {
        assert_eq!(executable_name("x86_64-pc-windows-msvc"), "vox.exe");
        assert_eq!(executable_name("x86_64-unknown-linux-gnu"), "vox");
        assert_eq!(executable_name("aarch64-apple-darwin"), "vox");
        assert_eq!(
            plugin_executable_name("x86_64-pc-windows-msvc", "vox-ml-cli"),
            "vox-ml-cli.exe"
        );
        assert_eq!(
            plugin_executable_name("x86_64-unknown-linux-gnu", "vox-ml-cli"),
            "vox-ml-cli"
        );
        assert_eq!(
            plugin_executable_name("aarch64-apple-darwin", "vox-ml-cli"),
            "vox-ml-cli"
        );
    }

    #[test]
    fn artifact_filename_contract_is_stable() {
        assert_eq!(
            artifact_filename("vox", "v1.2.3", "x86_64-pc-windows-msvc"),
            "vox-v1.2.3-x86_64-pc-windows-msvc.zip"
        );
        assert_eq!(
            artifact_filename("vox", "v1.2.3", "x86_64-unknown-linux-gnu"),
            "vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"
        );
        assert_eq!(
            artifact_filename("vox", "v1.2.3", "aarch64-apple-darwin"),
            "vox-v1.2.3-aarch64-apple-darwin.tar.gz"
        );
        assert_eq!(
            artifact_filename("vox-ml-cli", "v1.2.3", "x86_64-unknown-linux-gnu"),
            "vox-ml-cli-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    #[test]
    fn checksum_manifest_line_format() {
        let line = checksum_line("deadbeef", "vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz");
        assert_eq!(
            line,
            "deadbeef  vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz\n"
        );
    }

    #[test]
    fn checksum_manifest_supports_multiple_entries() {
        let all = [
            checksum_line("aaa", "vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"),
            checksum_line("bbb", "vox-ml-cli-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"),
        ]
        .join("");
        assert_eq!(
            all,
            "aaa  vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz\nbbb  vox-ml-cli-v1.2.3-x86_64-unknown-linux-gnu.tar.gz\n"
        );
    }

    /// `scripts/install.*` must name every triple users can download; keep aligned with CI matrix.
    #[test]
    fn install_scripts_cover_release_targets() {
        use std::path::PathBuf;

        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let sh = read_utf8_path_capped(&repo_root.join("scripts/install.sh"))
            .expect("read scripts/install.sh");
        let ps1 = read_utf8_path_capped(&repo_root.join("scripts/install.ps1"))
            .expect("read scripts/install.ps1");

        for triple in super::SUPPORTED_RELEASE_TARGETS {
            assert!(
                sh.contains(triple),
                "scripts/install.sh must mention `{triple}` so standalone download resolves the correct asset"
            );
            if triple.ends_with("-pc-windows-msvc") {
                assert!(
                    ps1.contains(triple),
                    "scripts/install.ps1 must mention `{triple}` for Windows prebuilt bootstrap"
                );
            }
        }
    }

    #[test]
    fn release_binaries_workflow_matrix_matches_ssot() {
        use std::collections::BTreeSet;
        use std::path::PathBuf;

        let wf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.github/workflows/release-binaries.yml");
        let yml = read_utf8_path_capped(&wf).expect("read release-binaries.yml");

        let mut from_workflow = BTreeSet::new();
        for line in yml.lines() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("- target:") else {
                continue;
            };
            from_workflow.insert(rest.trim().to_string());
        }

        let mut from_ssot = BTreeSet::new();
        for triple in super::SUPPORTED_RELEASE_TARGETS {
            from_ssot.insert((*triple).to_string());
        }

        assert_eq!(
            from_workflow,
            from_ssot,
            "release-binaries.yml matrix targets must match `crate::utils::install_policy::SUPPORTED_RELEASE_TARGETS` (workflow files: {})",
            wf.display()
        );
    }

    /// The install command we document must resolve. `docs/src/reference/installation.md`
    /// and both script headers advertise https://voxlang.org/voxup ; if nothing is
    /// served there, `curl … | sh` pipes a 404 page into a shell.
    #[test]
    fn documented_install_urls_are_served() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (advertised, served) in [("voxup", "docs-astro/public/voxup"),
                                     ("voxup.ps1", "docs-astro/public/voxup.ps1")] {
            assert!(
                root.join(served).is_file(),
                "https://voxlang.org/{advertised} is documented but {served} does not exist"
            );
        }
        // The served copies must not drift from the canonical scripts.
        for (served, canonical) in [("docs-astro/public/voxup", "scripts/install.sh"),
                                    ("docs-astro/public/voxup.ps1", "scripts/install.ps1")] {
            let a = std::fs::read_to_string(root.join(served)).expect("read served copy");
            let b = std::fs::read_to_string(root.join(canonical)).expect("read canonical script");
            assert_eq!(
                a, b,
                "{served} has drifted from {canonical}; regenerate it in the same commit"
            );
        }
    }

    /// Every crate the release builder shells out to must exist. `vox-bootstrap`
    /// was deleted from the workspace but `--package all` kept building it, so
    /// every release matrix leg failed and no artifact was ever published.
    ///
    /// Reads `RELEASE_PACKAGES` — the same constant `run()` uses — rather than a
    /// hardcoded list, so adding a package to the builder cannot bypass this.
    #[test]
    fn every_release_package_exists_in_the_workspace() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let lock = std::fs::read_to_string(root.join("Cargo.lock")).expect("read Cargo.lock");
        for pkg in super::RELEASE_PACKAGES {
            assert!(
                root.join("crates").join(pkg).is_dir(),
                "release_build shells `cargo build -p {pkg}` but crates/{pkg}/ does not exist"
            );
            assert!(
                lock.contains(&format!("name = \"{pkg}\"")),
                "release_build shells `cargo build -p {pkg}`, absent from Cargo.lock"
            );
        }
    }

    /// `--package all` must still parse, and the retired tiers must not.
    #[test]
    fn release_package_value_enum_matches_the_shipped_tiers() {
        use clap::ValueEnum;
        let names: Vec<String> = vox_cli_ci::cmd_enums::ReleasePackage::value_variants()
            .iter()
            .filter_map(|v| v.to_possible_value().map(|p| p.get_name().to_string()))
            .collect();
        assert_eq!(
            names,
            vec!["vox".to_string(), "mens".to_string(), "all".to_string()],
            "ReleasePackage tiers changed; `bootstrap` and `both` built a deleted crate"
        );
    }
}
