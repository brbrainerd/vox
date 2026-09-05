use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use tracing::{info, warn};

/// Place the real `vox` binary at the canonical user-facing path, and install
/// `voxup` itself alongside it.
///
/// This is extracted from `run_install` so it can be unit-tested independently.
///
/// - `extracted_vox`: The `vox` binary from the downloaded release archive.
/// - `canonical`: `~/.vox/bin/vox[.exe]` — the path users invoke.
/// - `secondary`: `~/.cargo/bin/vox[.exe]` — backward-compat hard-link.
/// - `current_voxup`: The currently running `voxup` process binary.
/// - `voxup_canonical`: `~/.vox/bin/voxup[.exe]` — where voxup lives post-install.
pub(crate) fn place_binaries(
    extracted_vox: &Path,
    canonical: &Path,
    secondary: &Path,
    current_voxup: &Path,
    voxup_canonical: &Path,
) -> Result<()> {
    if let Some(parent) = canonical.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = secondary.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = voxup_canonical.parent() {
        fs::create_dir_all(parent)?;
    }

    replace_file(extracted_vox, canonical)?;
    establish_single_binary(canonical, secondary)?;
    replace_file(current_voxup, voxup_canonical)?;
    info!("voxup installed at {}", voxup_canonical.display());
    Ok(())
}

/// Staging dir for an in-progress extraction of `version` under `cache_dir`.
///
/// Built as a string, not `tc_dir.with_extension("incoming")` — `with_extension`
/// replaces everything after the LAST dot, so "vox-0.7.0" would collide with
/// "vox-0.7.5" at the same "vox-0.7.incoming" staging path, letting one
/// in-progress install's `remove_dir_all` delete another's.
fn staging_dir_for(cache_dir: &Path, version: &str) -> std::path::PathBuf {
    cache_dir.join(format!("vox-{version}.incoming"))
}

pub async fn run_install(profile: &str, tag: Option<&str>) -> Result<()> {
    // Validate tier before touching the network.
    crate::profiles::validate_tier(crate::profiles::PROFILES_YAML, profile)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let profiles = crate::profiles::parse(crate::profiles::PROFILES_YAML)
        .expect("embedded SSOT must be valid");
    if let Some(tier) = profiles.tiers.get(profile) {
        info!("Installing Vox ({profile}) — {}", tier.description);
    }

    let home = dirs::home_dir().context("cannot determine home directory")?;
    let bin_dir = home.join(".vox").join("bin");
    let cache_dir = home.join(".vox").join("toolchains");
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&cache_dir)?;

    let client = crate::channel::make_client()?;
    let release = match tag {
        Some(tag) => {
            info!("Fetching release '{tag}' from GitHub…");
            crate::channel::fetch_by_tag(&client, tag).await?
        }
        None => {
            info!("Fetching latest Vox release from GitHub…");
            crate::channel::fetch_latest(&client).await?
        }
    };
    info!("Release: {} ({})", release.tag, release.version);

    // Download and parse checksums.txt
    let ck_asset = release
        .find_asset("checksums.txt")
        .context("checksums.txt not found in GitHub release assets")?;
    let ck_bytes = crate::download::fetch_bytes(&client, &ck_asset.browser_download_url).await?;
    let ck_text = String::from_utf8(ck_bytes).context("checksums.txt is not valid UTF-8")?;
    let checksums = crate::download::parse_checksums(&ck_text);

    // Resolve the platform archive
    let archive_name = crate::channel::asset_name(&release.tag);
    let ar_asset = release.find_asset(&archive_name).with_context(|| {
        format!(
            "Expected asset '{archive_name}' not in release {}. Available: {}",
            release.tag,
            release
                .assets
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let expected_hash = checksums
        .get(&archive_name)
        .with_context(|| format!("No checksum for '{archive_name}' in checksums.txt"))?;

    info!("Downloading {} ({} bytes)…", archive_name, ar_asset.size);
    let ar_bytes = crate::download::fetch_bytes(&client, &ar_asset.browser_download_url).await?;

    info!("Verifying SHA-256…");
    crate::download::verify_sha256(&ar_bytes, expected_hash)
        .with_context(|| format!("Integrity check failed for {archive_name}"))?;
    info!("Checksum OK");

    // Extract to versioned dir. A mid-extraction failure must not leave a
    // partially-populated version dir behind for the next run, so extract into
    // a sibling staging dir and rename on success.
    let tc_dir = cache_dir.join(format!("vox-{}", release.version));
    let staging = staging_dir_for(&cache_dir, &release.version);
    let _ = fs::remove_dir_all(&staging);
    crate::download::extract(&ar_bytes, &staging, &archive_name)?;
    let _ = fs::remove_dir_all(&tc_dir);
    fs::rename(&staging, &tc_dir)
        .with_context(|| format!("promote {} -> {}", staging.display(), tc_dir.display()))?;

    // Write active version
    fs::write(cache_dir.join("active"), &release.version)
        .with_context(|| format!("failed to write active file in {}", cache_dir.display()))?;

    // Establish canonical binary (the voxup proxy renamed/copied as vox)
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    let extracted_bin = tc_dir.join(exe);
    let canonical = bin_dir.join(exe);
    let secondary = home.join(".cargo").join("bin").join(exe);
    if !extracted_bin.exists() {
        bail!(
            "Extraction succeeded but '{exe}' not found in {}",
            tc_dir.display()
        );
    }
    let voxup_exe = if cfg!(windows) { "voxup.exe" } else { "voxup" };
    let voxup_canonical = bin_dir.join(voxup_exe);
    let current_voxup = std::env::current_exe().context("cannot get current exe path")?;
    place_binaries(
        &extracted_bin,
        &canonical,
        &secondary,
        &current_voxup,
        &voxup_canonical,
    )?;

    // WASM sysroots
    provision_wasm_sysroots(&cache_dir, &release.version).await?;

    // Persistent PATH
    let modified = crate::shell::add_to_path(&home, &bin_dir);
    if modified.is_empty() {
        info!(
            "No shell profiles found. Add {} to your PATH manually.",
            bin_dir.display()
        );
    } else {
        info!("Updated {} shell profile(s).", modified.len());
    }

    println!("\n✅ Vox {} installed!", release.version);
    println!("   vox:   {}", canonical.display());
    println!("   voxup: {}", voxup_canonical.display());
    println!("   Run: vox --version");
    // Name the profile that was actually modified. Hardcoding ~/.bashrc told
    // macOS users (zsh by default, and where voxup now creates ~/.zshrc on a
    // pristine account) to source a file it had not touched — and which usually
    // does not exist there.
    match modified.first() {
        Some(profile) => println!("   Restart your shell or: source {}", profile.display()),
        None => println!(
            "   Add {} to your PATH, then restart your shell",
            bin_dir.display()
        ),
    }
    Ok(())
}

/// A real `vox` executable is multi-megabyte; the legacy proxy stub this
/// replaced was a tiny shell/batch script. Anything below this size is treated
/// as "not a real binary" (a stub or placeholder to be overwritten).
const MIN_REAL_BINARY_BYTES: u64 = 64 * 1024;

/// True when `path` is an existing file large enough to be a real `vox` binary
/// (not the legacy echo-stub proxy).
pub(crate) fn is_real_binary(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && m.len() >= MIN_REAL_BINARY_BYTES)
        .unwrap_or(false)
}

/// Back **both** `canonical` (`~/.vox/bin/vox`) and `secondary`
/// (`~/.cargo/bin/vox`) with a single real binary, by hard-linking the two
/// paths to one inode.
///
/// The real bytes are sourced from whichever location already holds a real
/// binary (preferring `canonical`); they are placed at `canonical`, then
/// `secondary` is hard-linked to it. If hard-linking fails (e.g. the two paths
/// live on different volumes) it falls back to a copy and warns — the two would
/// then be able to drift, which `vox doctor`'s binary-SSOT check surfaces.
pub(crate) fn establish_single_binary(canonical: &Path, secondary: &Path) -> Result<()> {
    let source = if is_real_binary(canonical) {
        canonical.to_path_buf()
    } else if is_real_binary(secondary) {
        secondary.to_path_buf()
    } else {
        anyhow::bail!(
            "no real `vox` binary found at {} or {} — install one first \
             (`cargo install --locked --path crates/vox-cli`), then re-run `voxup install`",
            canonical.display(),
            secondary.display()
        );
    };

    if let Some(parent) = canonical.parent() {
        fs::create_dir_all(parent).ok();
    }
    if let Some(parent) = secondary.parent() {
        fs::create_dir_all(parent).ok();
    }

    // Ensure the real bytes live at the canonical path.
    if source != *canonical {
        replace_file(&source, canonical).with_context(|| {
            format!(
                "seed canonical binary {} from {}",
                canonical.display(),
                source.display()
            )
        })?;
    }

    // Point the secondary path at the canonical inode: one real binary, two paths.
    link_or_copy(canonical, secondary)
        .with_context(|| format!("link {} -> {}", secondary.display(), canonical.display()))?;

    info!(
        "vox binary unified: {} and {} now share one inode",
        canonical.display(),
        secondary.display()
    );
    Ok(())
}

/// Overwrite `dst` with a byte-for-byte copy of `src` (removing any prior stub).
fn replace_file(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        #[cfg(windows)]
        {
            let old_path = dst.with_extension("exe.old");
            if old_path.exists() {
                let _ = fs::remove_file(&old_path);
            }
            if fs::rename(dst, &old_path).is_err() {
                fs::remove_file(dst).with_context(|| format!("remove {}", dst.display()))?;
            } else {
                let _ = fs::remove_file(&old_path); // try deleting non-blocking/best-effort
            }
        }
        #[cfg(not(windows))]
        {
            fs::remove_file(dst).with_context(|| format!("remove {}", dst.display()))?;
        }
    }
    fs::copy(src, dst).with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
    set_executable(dst);
    Ok(())
}

/// Hard-link `dst` to `src`; fall back to a copy (with a warning) if that fails.
fn link_or_copy(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_file(dst).with_context(|| format!("remove {}", dst.display()))?;
    }
    match fs::hard_link(src, dst) {
        Ok(()) => Ok(()),
        Err(e) => {
            warn!(
                "could not hard-link {} -> {} ({e}); copying instead — the two may drift",
                dst.display(),
                src.display()
            );
            fs::copy(src, dst)
                .with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
            set_executable(dst);
            Ok(())
        }
    }
}

/// Mark `path` executable on Unix; a no-op on Windows.
fn set_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(path, perms);
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

async fn provision_wasm_sysroots(toolchains_dir: &Path, rust_version: &str) -> Result<()> {
    let sysroot_dir = toolchains_dir.join(format!("wasm-sysroot-{}", rust_version));
    if !sysroot_dir.exists() {
        fs::create_dir_all(&sysroot_dir)?;
        info!(
            "Provisioned new WASM sysroot directory at {:?}",
            sysroot_dir
        );
    } else {
        info!("WASM sysroot for {} already exists.", rust_version);
    }
    Ok(())
}

// Removed run_proxy (now in proxy.rs)

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_fake_binary(path: &Path, byte: u8) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![byte; (MIN_REAL_BINARY_BYTES + 1) as usize]).unwrap();
    }

    #[test]
    fn staging_dir_keeps_full_dotted_version_and_does_not_collide() {
        let cache_dir = Path::new("/cache");
        let staging_070 = staging_dir_for(cache_dir, "0.7.0");
        let staging_075 = staging_dir_for(cache_dir, "0.7.5");
        assert_eq!(
            staging_070.file_name().unwrap().to_str().unwrap(),
            "vox-0.7.0.incoming"
        );
        assert_ne!(
            staging_070, staging_075,
            "distinct patch versions must not collide"
        );
    }

    #[test]
    fn is_real_binary_rejects_stub_accepts_large() {
        let dir = tempdir().unwrap();
        let stub = dir.path().join("vox-stub");
        fs::write(&stub, b"@echo off\r\necho Vox Proxy Wrapper\r\n").unwrap();
        assert!(!is_real_binary(&stub));

        let real = dir.path().join("vox-real");
        write_fake_binary(&real, 0xAB);
        assert!(is_real_binary(&real));

        assert!(!is_real_binary(&dir.path().join("does-not-exist")));
    }

    #[test]
    fn establish_unifies_both_paths_to_one_real_binary() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().join("vox-bin").join("vox");
        let cargo = dir.path().join("cargo-bin").join("vox");
        // Only the cargo location has a real binary to start.
        write_fake_binary(&cargo, 0x42);

        establish_single_binary(&canonical, &cargo).unwrap();

        // Both exist, are real, and hold identical bytes.
        assert!(is_real_binary(&canonical));
        assert!(is_real_binary(&cargo));
        assert_eq!(fs::read(&canonical).unwrap(), fs::read(&cargo).unwrap());

        // On Unix we can assert they share one inode (true hard link).
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                fs::metadata(&canonical).unwrap().ino(),
                fs::metadata(&cargo).unwrap().ino()
            );
        }
    }

    #[test]
    fn establish_overwrites_stub_at_canonical() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().join("vox-bin").join("vox");
        let cargo = dir.path().join("cargo-bin").join("vox");
        // Canonical holds a stub; cargo holds the real binary.
        fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        fs::write(&canonical, b"echo stub").unwrap();
        write_fake_binary(&cargo, 0x7E);

        establish_single_binary(&canonical, &cargo).unwrap();

        assert!(is_real_binary(&canonical));
        assert_eq!(fs::read(&canonical).unwrap(), fs::read(&cargo).unwrap());
    }

    #[test]
    fn establish_errors_when_no_real_binary_present() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().join("vox-bin").join("vox");
        let cargo = dir.path().join("cargo-bin").join("vox");
        // Both are stubs (too small).
        fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        fs::write(&canonical, b"stub").unwrap();
        let err = establish_single_binary(&canonical, &cargo).unwrap_err();
        assert!(err.to_string().contains("no real `vox` binary"));
    }

    #[test]
    fn place_binaries_installs_extracted_vox_not_running_voxup() {
        let dir = tempdir().unwrap();
        let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
        let voxup_exe = if cfg!(windows) { "voxup.exe" } else { "voxup" };

        // The extracted vox binary — byte 0xAA identifies it
        let tc_dir = dir.path().join("toolchains").join("vox-1.0.0");
        let extracted_vox = tc_dir.join(exe);
        write_fake_binary(&extracted_vox, 0xAA);

        // A fake "running voxup" — byte 0xBB identifies it
        let fake_voxup = dir.path().join(voxup_exe);
        write_fake_binary(&fake_voxup, 0xBB);

        let canonical = dir.path().join("bin").join(exe);
        let secondary = dir.path().join("cargo-bin").join(exe);
        let voxup_canonical = dir.path().join("bin").join(voxup_exe);

        place_binaries(
            &extracted_vox,
            &canonical,
            &secondary,
            &fake_voxup,
            &voxup_canonical,
        )
        .unwrap();

        // ~/.vox/bin/vox must contain the extracted vox bytes (0xAA), not voxup bytes (0xBB)
        let canonical_bytes = fs::read(&canonical).unwrap();
        assert!(
            canonical_bytes.iter().all(|&b| b == 0xAA),
            "~/.vox/bin/vox must be the extracted vox binary, not voxup"
        );

        // ~/.vox/bin/voxup must contain the voxup bytes (0xBB)
        let voxup_bytes = fs::read(&voxup_canonical).unwrap();
        assert!(
            voxup_bytes.iter().all(|&b| b == 0xBB),
            "~/.vox/bin/voxup must be the running voxup binary"
        );
    }
}
