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

/// Cargo profile used for every shipped artifact.
///
/// `[profile.dist]` sets `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`.
/// Plain `--release` is thin-LTO and keeps debuginfo — see spec finding F6.
pub(crate) const DIST_PROFILE: &str = "dist";

/// Where cargo writes a `--target <triple> --profile dist` binary.
pub(crate) fn built_binary_path(repo_root: &Path, target: &str, bin: &str) -> PathBuf {
    repo_root
        .join("target")
        .join(target)
        .join(DIST_PROFILE)
        .join(bin)
}

/// Crates the release builder shells `cargo build -p` for. Asserted against the
/// workspace by `every_release_package_exists_in_the_workspace`.
pub(crate) const RELEASE_PACKAGES: &[&str] = &["vox-cli", "vox-ml-cli", "vox-langtool"];

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
    let want_langtool = matches!(package, ReleasePackage::Langtool | ReleasePackage::All);

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
    if want_langtool {
        let langtool_bin = plugin_executable_name(target, "vox-langtool");
        let artifact_name = build_and_package_binary(
            repo_root,
            out_dir_abs.as_path(),
            target,
            artifact_version,
            "vox-langtool",
            &langtool_bin,
            "vox-langtool",
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

/// Binary name resolution for non-`vox` release archives (`vox-ml-cli`, `vox-langtool`).
///
/// Returns an owned `String` rather than `&'static str` because these names
/// are dynamic (any `vox-<name>` pattern), unlike the fixed core `vox` name.
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
    // Fires because release-binaries.yml runs `cargo run` (debug) — not `--release`.
    // If that invocation is ever release-optimized, promote this to a hard check.
    debug_assert!(
        RELEASE_PACKAGES.contains(&package_name),
        "package '{package_name}' is not in RELEASE_PACKAGES; add it there so \
         `every_release_package_exists_in_the_workspace` can verify it exists"
    );
    let mut cmd = Command::new(super::cargo_bin());
    cmd.current_dir(repo_root).args([
        "build",
        "-p",
        package_name,
        "--profile",
        DIST_PROFILE,
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

    let built_binary = built_binary_path(repo_root, target, built_bin_name);
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
        for (advertised, served) in [
            ("voxup", "docs-astro/public/voxup"),
            ("voxup.ps1", "docs-astro/public/voxup.ps1"),
        ] {
            assert!(
                root.join(served).is_file(),
                "https://voxlang.org/{advertised} is documented but {served} does not exist"
            );
        }
        // The served copies must not drift from the canonical scripts.
        for (served, canonical) in [
            ("docs-astro/public/voxup", "scripts/install.sh"),
            ("docs-astro/public/voxup.ps1", "scripts/install.ps1"),
        ] {
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
            vec![
                "vox".to_string(),
                "mens".to_string(),
                "langtool".to_string(),
                "all".to_string()
            ],
            "ReleasePackage tiers changed; `bootstrap` and `both` built a deleted crate"
        );
    }

    /// Bundle artifacts must be parameterised by BOTH matrix axes, or the 16
    /// uploads collide and the survivor identifies neither bundle nor target.
    #[test]
    fn every_bundle_artifact_name_is_matrix_unique() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = std::fs::read_to_string(root.join(".github/workflows/bundle-release.yml"))
            .expect("read bundle-release.yml");
        let v: serde_yaml::Value =
            serde_yaml::from_str(&text).expect("workflow must be valid YAML");

        let steps = v["jobs"]["build-bundles"]["steps"]
            .as_sequence()
            .expect("build-bundles must have steps");

        let mut checked = 0usize;
        for step in steps {
            let blob = serde_yaml::to_string(step).expect("re-serialise step");
            // Steps that name an artifact: the build (--out) and the release attach (files:).
            let names_artifact = blob.contains("--out") || !step["with"]["files"].is_null();
            if !names_artifact {
                continue;
            }
            checked += 1;
            assert!(
                blob.contains("matrix.bundle") && blob.contains("matrix.target"),
                "bundle artifact name is not parameterised by both matrix axes:\n{blob}"
            );
        }
        assert!(
            checked >= 2,
            "expected at least a build step and an attach step, saw {checked}"
        );
    }

    /// Bundles the x86-64 Linux + Windows matrix deliberately does not build.
    /// `vox-ml-metal` carries an Apple-Metal plugin; `vox-mobile` is status="alpha",
    /// planned v0.8. Adding either would spawn jobs that cannot succeed here.
    const MATRIX_EXCLUDED_BUNDLES: &[&str] = &["vox-ml-metal", "vox-mobile"];

    fn catalog_bundle_ids(catalog_toml: &str) -> Vec<String> {
        let v: toml::Value = catalog_toml.parse().expect("catalog.toml must parse");
        let mut ids: Vec<String> = v
            .get("bundle")
            .and_then(|b| b.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.get("id")?.as_str().map(String::from))
                    .filter(|id| !MATRIX_EXCLUDED_BUNDLES.contains(&id.as_str()))
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        ids
    }

    /// Read the matrix from parsed YAML. A hand-rolled line scanner gives wrong
    /// answers on comments, quoted ids, flow style, anchors, and any second
    /// `bundle:` key elsewhere in the file.
    fn workflow_matrix_bundle_ids(yml: &str) -> Vec<String> {
        let v: serde_yaml::Value = serde_yaml::from_str(yml).expect("workflow must be valid YAML");
        let mut ids: Vec<String> = v["jobs"]["build-bundles"]["strategy"]["matrix"]["bundle"]
            .as_sequence()
            .expect("matrix.bundle must be a list")
            .iter()
            .map(|x| x.as_str().expect("bundle id must be a string").to_string())
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn catalog_bundle_ids_excludes_platform_specific_bundles() {
        let toml_src = "[[bundle]]\nid = \"vox-base\"\n\n[[bundle]]\nid = \"vox-ml-metal\"\n";
        assert_eq!(catalog_bundle_ids(toml_src), vec!["vox-base"]);
    }

    #[test]
    fn bundle_release_matrix_matches_plugin_catalog() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let catalog = std::fs::read_to_string(root.join("crates/vox-plugin-catalog/catalog.toml"))
            .expect("read catalog.toml");
        let wf = std::fs::read_to_string(root.join(".github/workflows/bundle-release.yml"))
            .expect("read bundle-release.yml");

        let expected = catalog_bundle_ids(&catalog);
        let actual = workflow_matrix_bundle_ids(&wf);

        assert_eq!(
            actual,
            expected,
            "bundle-release.yml matrix must match the buildable [[bundle]] ids in \
             catalog.toml (excluding {MATRIX_EXCLUDED_BUNDLES:?}).\n  \
             only in workflow (phantom — fails UnknownBundle every release): {:?}\n  \
             only in catalog (never built): {:?}",
            actual
                .iter()
                .filter(|b| !expected.contains(b))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .filter(|b| !actual.contains(b))
                .collect::<Vec<_>>(),
        );
    }

    /// `[profile.dist]` must not abort on panic. Three non-test paths in the shipped
    /// binary rely on unwinding:
    ///   - vox-actor-runtime/src/supervisor.rs:30,52 — spawn_supervised matches on
    ///     JoinError::is_panic(); under abort a panicking task kills the process and
    ///     every caller silently loses supervision.
    ///   - vox-vcs/src/jj_actor.rs:196,282 — the `guarded!` macro catch_unwinds
    ///     block_on so a panicking jj-lib call returns Err(Unavailable) rather than
    ///     killing the actor loop. `jj` is a default feature of vox-orchestrator,
    ///     which vox-cli takes with defaults, so this ships.
    ///   - vox-search/src/memory_cache.rs:88 — resume_unwind on a spawn_blocking panic.
    ///
    /// Also guards the two other routes abort could arrive by: inheritance from
    /// [profile.release], and a global rustflag.
    #[test]
    fn dist_profile_does_not_abort_on_panic() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
        let v: toml::Value = manifest.parse().expect("workspace Cargo.toml must parse");

        let dist = v["profile"].get("dist").expect("[profile.dist] must exist");
        assert!(
            dist.get("panic").is_none(),
            "[profile.dist] must not set `panic`; abort breaks catch_unwind-based \
             panic containment in supervisor.rs and jj_actor.rs"
        );
        assert!(
            v["profile"]["release"].get("panic").is_none(),
            "[profile.release] sets `panic`; [profile.dist] inherits from it"
        );

        // The optimization settings ARE the point of the profile — keep them.
        assert_eq!(dist.get("lto").and_then(|x| x.as_str()), Some("fat"));
        assert_eq!(
            dist.get("codegen-units").and_then(|x| x.as_integer()),
            Some(1)
        );
        assert_eq!(dist.get("strip").and_then(|x| x.as_str()), Some("symbols"));

        let cargo_cfg =
            std::fs::read_to_string(root.join(".cargo/config.toml")).unwrap_or_default();
        assert!(
            !cargo_cfg.replace(' ', "").contains("panic=abort"),
            ".cargo/config.toml sets panic=abort globally, bypassing the profile"
        );
    }

    /// Release workflows must not float the toolchain. Building shipped artifacts on
    /// `@stable` means users get binaries from a compiler no CI gate ever ran, and
    /// each new stable silently imports its lint wave.
    #[test]
    fn release_workflows_pin_the_toolchain() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pinned = std::fs::read_to_string(root.join("rust-toolchain.toml"))
            .expect("read rust-toolchain.toml");
        let want = pinned
            .lines()
            .find_map(|l| l.trim().strip_prefix("channel = "))
            .map(|v| v.trim().trim_matches('"').to_string())
            .expect("rust-toolchain.toml must declare a channel");

        let floating = concat!("rust-toolchain@", "stable");
        for rel in [
            ".github/workflows/release-binaries.yml",
            ".github/workflows/release-installers.yml",
            ".github/workflows/bundle-release.yml",
            ".github/workflows/release-gui.yml",
        ] {
            let text = std::fs::read_to_string(root.join(rel)).expect("read workflow");
            assert!(
                !text.contains(floating),
                "{rel} floats the toolchain; pin it to {want} (rust-toolchain.toml)"
            );
            if text.contains("dtolnay/rust-toolchain") {
                assert!(
                    text.contains(&format!("toolchain: \"{want}\"")),
                    "{rel} installs a toolchain other than rust-toolchain.toml's {want}"
                );
            }
        }
    }

    /// Build steps that produce a SHIPPED artifact must use --profile dist, and no
    /// shipped-artifact workflow may still read from target/release/.
    ///
    /// Comments are stripped before scanning: release-installers.yml documents the
    /// old command in prose, and a blanket file-level `contains` assertion is
    /// unsatisfiable because of it.
    #[test]
    fn shipped_build_steps_use_the_dist_profile() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let flag = concat!("--", "release");

        // (file, build lines that legitimately stay on release, target/release paths
        //  that legitimately remain)
        let shipped: &[(&str, &[&str], &[&str])] = &[
            // `cargo run … ci release-build` runs the vox-cli TOOL itself (in debug —
            // no --release/--profile on this line), which then shells its own
            // `cargo build --profile dist` internally (see build_and_package_binary).
            // Same shape as bundle-release.yml's `cargo run` exception below.
            (
                ".github/workflows/release-binaries.yml",
                &["cargo run"],
                &[],
            ),
            // voxup is built here only to smoke `--help` and run an install E2E;
            // fat LTO would blow the 30-minute job budget for zero shipped bytes.
            // Because it stays on --release, its output stays at target/release/.
            (
                ".github/workflows/release-installers.yml",
                &["-p voxup"],
                &["target/release/voxup"],
            ),
            // The Tauri sidecar STAGING destination is target/release/vox-<triple>
            // and must not move — tauri.conf.json's externalBin is read by seven
            // consumers. Only the copy SOURCE moves to dist. `bundle` is Tauri's
            // own output dir, unrelated to the cargo profile.
            (
                ".github/workflows/release-gui.yml",
                &[],
                &["target/release/vox-", "target/release/bundle"],
            ),
            // bundle-release.yml needs no exemption: it builds once via
            // `cargo build --profile dist` and every later step (bundle apply/
            // build/verify) runs that dist binary directly via $VOX_BIN, since
            // `vox bundle build` tars std::env::current_exe() into the shipped
            // tarball — no `cargo build`/`cargo run` line remains to allow-list.
            ("Dockerfile", &[], &[]),
        ];

        for (rel, allowed_flags, allowed_paths) in shipped {
            let text = std::fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("read {rel}: {e}"));
            for (i, line) in text.lines().enumerate() {
                let code = line.split('#').next().unwrap_or("");
                if !code.contains("cargo build") && !code.contains("cargo run") {
                    continue;
                }
                if allowed_flags.iter().any(|a| code.contains(a)) {
                    continue;
                }
                assert!(
                    !code.contains(flag),
                    "{rel}:{} builds a shipped artifact with the release profile:\n  {}",
                    i + 1,
                    code.trim()
                );
                assert!(
                    code.contains("--profile dist"),
                    "{rel}:{} builds a shipped artifact without --profile dist:\n  {}",
                    i + 1,
                    code.trim()
                );
            }
            // Path fallout: switching the flag relocates output, so a surviving
            // `target/release` read is a job that dies at the next step.
            for (i, line) in text.lines().enumerate() {
                let code = line.split('#').next().unwrap_or("");
                if !code.contains("target/release") {
                    continue;
                }
                if allowed_paths.iter().any(|a| code.contains(a)) {
                    continue;
                }
                panic!(
                    "{rel}:{} still reads target/release/ after the profile switch:\n  {}",
                    i + 1,
                    code.trim()
                );
            }
        }
    }

    /// The builder writes to and reads from target/<triple>/<profile>/.
    #[test]
    fn built_binary_path_uses_the_dist_profile() {
        let p = super::built_binary_path(
            std::path::Path::new("/repo"),
            "x86_64-unknown-linux-gnu",
            "vox",
        );
        assert!(
            p.ends_with("target/x86_64-unknown-linux-gnu/dist/vox"),
            "built artifacts must be read from the dist profile dir, got {}",
            p.display()
        );
    }

    /// A tag push cannot be gated by GitHub required checks, so the ordering between
    /// verification and publication must be structural. Deleting the dist-verify job
    /// or dropping it from `needs:` would silently publish unverified artifacts.
    #[test]
    fn publish_job_is_gated_on_dist_verification() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = std::fs::read_to_string(root.join(".github/workflows/release-binaries.yml"))
            .expect("read release-binaries.yml");
        let v: serde_yaml::Value =
            serde_yaml::from_str(&text).expect("workflow must be valid YAML");

        assert!(
            !v["jobs"]["dist-verify"].is_null(),
            "the dist-verify job was removed; artifacts would publish unverified"
        );
        let needs = v["jobs"]["publish"]["needs"]
            .as_sequence()
            .expect("publish must declare a needs: list");
        let names: Vec<&str> = needs.iter().filter_map(|n| n.as_str()).collect();
        assert!(
            names.contains(&"dist-verify"),
            "publish no longer needs dist-verify (needs: {names:?}); a tag would \
             publish while verification was still running, or after it failed"
        );
    }

    /// `install.sh` must abort, not continue, when no SHA-256 tool exists. Executes
    /// the real `verify_checksum` with every hash tool reported missing.
    #[cfg(unix)]
    #[test]
    fn install_sh_aborts_when_no_hash_tool_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let sh = std::fs::read_to_string(root.join("scripts/install.sh")).expect("read install.sh");

        // Source the helpers without running main(), then shadow `command -v` so
        // every hash tool reports missing.
        let harness = format!(
            "{}\n\
             command() {{ if [ \"$1\" = \"-v\" ]; then case \"$2\" in \
               sha256sum|shasum|openssl) return 1;; esac; fi; builtin command \"$@\"; }}\n\
             verify_checksum /dev/null deadbeef\n\
             echo REACHED_INSTALL\n",
            sh.replace("main \"$@\"", "")
        );

        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(&harness)
            .output()
            .expect("spawn sh");

        assert!(
            !out.status.success(),
            "verify_checksum returned success with no hash tool available"
        );
        assert!(
            !String::from_utf8_lossy(&out.stdout).contains("REACHED_INSTALL"),
            "install.sh continued past an unverifiable checksum"
        );

        // POSITIVE CONTROL. Without this the assertions above are vacuous: any
        // harness breakage (a shell lacking `builtin`, a renamed helper, a typo
        // in the sourced text) also produces "failed, no marker" and would be
        // read as "failed closed". Run the same harness WITHOUT shadowing and
        // with the true SHA-256 of an empty file; it must reach the marker. If
        // this half fails, the negative half proves nothing.
        let empty_sha = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let control = format!(
            "{}
verify_checksum /dev/null {empty_sha}
echo REACHED_INSTALL
",
            sh.replace("main \"$@\"", "")
        );
        let ok = std::process::Command::new("sh")
            .arg("-c")
            .arg(&control)
            .output()
            .expect("spawn sh");
        assert!(
            String::from_utf8_lossy(&ok.stdout).contains("REACHED_INSTALL"),
            "positive control failed — the harness cannot reach the success path, so              the negative assertion above is vacuous.
stdout: {}
stderr: {}",
            String::from_utf8_lossy(&ok.stdout),
            String::from_utf8_lossy(&ok.stderr)
        );
    }
}
