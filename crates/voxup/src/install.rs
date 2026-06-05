use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Resolve the user home directory using env vars (cross-platform, no `dirs` dep).
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub async fn run_install(_profile: &str) -> Result<()> {
    let vox_dir = home_dir().join(".vox");
    let toolchains_dir = vox_dir.join("toolchains");
    let bin_dir = vox_dir.join("bin");

    if !toolchains_dir.exists() {
        fs::create_dir_all(&toolchains_dir)?;
        info!("Created ~/.vox/toolchains directory");
    }

    if !bin_dir.exists() {
        fs::create_dir_all(&bin_dir)?;
        info!("Created ~/.vox/bin directory");
    }

    info!("Parsing local manifest for stable channel...");
    let manifest_path = std::env::current_dir()?
        .join("contracts")
        .join("toolchain")
        .join("workspace-toolchain.v1.yaml");

    let mut expected_rust_version = String::from("1.92.0");

    if manifest_path.exists() {
        let content = fs::read_to_string(&manifest_path)?;
        let manifest = crate::manifest::WorkspaceToolchain::parse(&content)?;
        expected_rust_version = manifest
            .versions
            .get("rust")
            .unwrap_or(&expected_rust_version)
            .to_string();
        info!(
            "Successfully parsed toolchain manifest matching Rust version: {}",
            expected_rust_version
        );
    } else {
        warn!(
            "Could not locate workspace-toolchain.v1.yaml locally. \
             Falling back to default: {}",
            expected_rust_version
        );
    }

    info!("Linking a single vox binary across ~/.vox/bin and ~/.cargo/bin...");
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    let canonical = bin_dir.join(exe);
    let cargo_bin = home_dir().join(".cargo").join("bin").join(exe);
    establish_single_binary(&canonical, &cargo_bin)?;

    info!(
        "Provisioning isolated WASM sysroots targeting Rust {}...",
        expected_rust_version
    );
    provision_wasm_sysroots(&toolchains_dir, &expected_rust_version).await?;

    info!("Installation complete! Add ~/.vox/bin to your PATH.");

    Ok(())
}

/// A real `vox` executable is multi-megabyte; the legacy proxy stub this
/// replaced was a tiny shell/batch script. Anything below this size is treated
/// as "not a real binary" (a stub or placeholder to be overwritten).
const MIN_REAL_BINARY_BYTES: u64 = 64 * 1024;

/// True when `path` is an existing file large enough to be a real `vox` binary
/// (not the legacy echo-stub proxy).
fn is_real_binary(path: &Path) -> bool {
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
fn establish_single_binary(canonical: &Path, secondary: &Path) -> Result<()> {
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
        fs::remove_file(dst).with_context(|| format!("remove {}", dst.display()))?;
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

pub async fn run_proxy(args: &[String]) -> Result<()> {
    info!("Proxy execution intercept. Setting up hermetic environment...");
    let vox_dir = home_dir().join(".vox");

    let old_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!(
        "{}:{}",
        vox_dir.join("toolchains").join("bin").display(),
        old_path
    );
    // SAFETY: single-threaded at this point; proxy entrypoint runs before any thread is spawned.
    unsafe { std::env::set_var("PATH", new_path) };

    info!("Forwarding args to target: {:?}", args);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_fake_binary(path: &Path, byte: u8) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![byte; (MIN_REAL_BINARY_BYTES + 1) as usize]).unwrap();
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
}
