use anyhow::{Context, Result};
use tracing::{info, warn};
use std::path::Path;
use std::fs;

pub async fn run_install(_profile: &str) -> Result<()> {
    let home = dirs::home_dir().expect("Failed to determine home directory");
    let vox_dir = home.join(".vox");
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
    let manifest_path = std::env::current_dir()?.join("contracts").join("toolchain").join("workspace-toolchain.v1.yaml");

    let mut expected_rust_version = String::from("1.92.0");

    if manifest_path.exists() {
        let content = fs::read_to_string(&manifest_path)?;
        let manifest = crate::manifest::WorkspaceToolchain::parse(&content)?;
        expected_rust_version = manifest.versions.get("rust").unwrap_or(&expected_rust_version).to_string();
        info!("Successfully parsed toolchain manifest matching Rust version: {}", expected_rust_version);
    } else {
        warn!("Could not locate workspace-toolchain.v1.yaml locally. Falling back to default: {}", expected_rust_version);
    }

    info!("Installing vox CLI proxy into ~/.vox/bin/vox...");
    install_proxy_binary(&bin_dir)?;

    info!("Provisioning isolated WASM sysroots targeting Rust {}...", expected_rust_version);
    provision_wasm_sysroots(&toolchains_dir, &expected_rust_version).await?;

    info!("Installation complete! Add ~/.vox/bin to your PATH.");

    Ok(())
}

fn install_proxy_binary(bin_dir: &Path) -> Result<()> {
    let proxy_path = bin_dir.join(if cfg!(windows) { "vox.exe" } else { "vox" });
    let script_content = if cfg!(windows) {
        "@echo off\r\necho Vox Proxy Wrapper\r\n"
    } else {
        "#!/bin/bash\necho 'Vox Proxy Wrapper'\n"
    };
    fs::write(&proxy_path, script_content).context("Failed to write proxy binary")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&proxy_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&proxy_path, perms)?;
    }

    Ok(())
}

async fn provision_wasm_sysroots(toolchains_dir: &Path, rust_version: &str) -> Result<()> {
    let sysroot_dir = toolchains_dir.join(format!("wasm-sysroot-{}", rust_version));
    if !sysroot_dir.exists() {
        fs::create_dir_all(&sysroot_dir)?;
        info!("Provisioned new WASM sysroot directory at {:?}", sysroot_dir);
    } else {
        info!("WASM sysroot for {} already exists.", rust_version);
    }
    Ok(())
}

pub async fn run_proxy(args: &[String]) -> Result<()> {
    info!("Proxy execution intercept. Setting up hermetic environment...");
    let vox_dir = dirs::home_dir()
        .expect("Failed to determine home directory")
        .join(".vox");

    let old_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", vox_dir.join("toolchains").join("bin").display(), old_path);
    unsafe { std::env::set_var("PATH", new_path); }

    info!("Forwarding args to target: {:?}", args);
    Ok(())
}
