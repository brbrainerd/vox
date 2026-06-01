//! Self-healing for the `mens-candle-cuda` runtime plugin.
//!
//! `vox mens train --device cuda` dispatches QLoRA training to a runtime-loaded
//! cdylib plugin (`mens-candle-cuda`). Three failure modes make a plain train
//! command fail out of the box:
//!
//! 1. **Missing** — the plugin was never installed.
//! 2. **Version/ABI mismatch** — the installed dll was built against an older
//!    workspace version (`plugin.load_failed error_kind="root_module"`).
//! 3. **Stale source** — the in-tree plugin source is newer than the installed dll.
//!
//! When auto-heal is enabled (default; opt out with `--no-auto-heal` or
//! `VOX_MENS_NO_AUTO_HEAL=1`), [`ensure_cuda_plugin`] rebuilds the cdylib from
//! the in-tree source with the correct toolchain environment and reinstalls it
//! before training proceeds. On Windows this means wrapping the `cargo build` in
//! a Visual Studio developer environment (so `nvcc` finds `cl.exe`) and pinning
//! `CUDA_PATH`/`CUDA_HOME` to the newest installed toolkit (so `cudarc` links a
//! driver library that has the symbols it references).
//!
//! This module is only compiled with the `gpu` feature, which is what makes
//! `vox-plugin-host` available.
#![cfg(feature = "gpu")]

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const PLUGIN_ID: &str = "mens-candle-cuda";
const PLUGIN_CRATE: &str = "vox-plugin-mens-candle-cuda";

#[cfg(windows)]
const ARTIFACT: &str = "vox_plugin_mens_candle_cuda.dll";
#[cfg(not(windows))]
const ARTIFACT: &str = "libvox_plugin_mens_candle_cuda.so";

/// Ensure the CUDA training plugin is installed and loadable.
///
/// Returns `Ok(())` when the plugin is already healthy or was successfully
/// healed. When `auto_heal` is false and the plugin is unusable, returns an
/// actionable error instead of rebuilding.
pub fn ensure_cuda_plugin(auto_heal: bool) -> Result<()> {
    // Env override always wins so operators can disable healing in CI.
    let auto_heal = auto_heal && std::env::var_os("VOX_MENS_NO_AUTO_HEAL").is_none();

    match probe_reason() {
        None => Ok(()),
        Some(reason) => {
            if !auto_heal {
                anyhow::bail!(
                    "The '{PLUGIN_ID}' plugin is not usable: {reason}\n\n\
                     Auto-heal is disabled. Fix it manually with:\n\n{}",
                    vox_plugin_host::format_install_hint(PLUGIN_ID, None)
                );
            }
            eprintln!(
                "⚠  '{PLUGIN_ID}' plugin unusable ({reason}); auto-healing (rebuild + reinstall)…"
            );
            rebuild_and_reinstall()
                .with_context(|| format!("auto-healing the '{PLUGIN_ID}' plugin"))?;
            // Confirm the heal actually fixed it rather than silently proceeding.
            match probe_reason() {
                None => {
                    eprintln!("✓  '{PLUGIN_ID}' plugin healed and loads cleanly.");
                    Ok(())
                }
                Some(still) => anyhow::bail!(
                    "Rebuilt + reinstalled '{PLUGIN_ID}' but it is still unusable: {still}"
                ),
            }
        }
    }
}

/// Try to load the plugin. Returns `None` when healthy, or `Some(reason)` when
/// the load fails (missing, ABI/version mismatch, init failure).
fn probe_reason() -> Option<String> {
    match vox_plugin_host::load_code_plugin_by_id(PLUGIN_ID) {
        Ok(_loaded) => None, // dropped immediately; the real dispatch reloads it.
        Err(e) => Some(e.to_string()),
    }
}

/// Rebuild the cdylib from in-tree source and copy it into the install dir.
fn rebuild_and_reinstall() -> Result<()> {
    let source_dir = vox_plugin_host::workspace_local_plugin_source(PLUGIN_ID).ok_or_else(|| {
        anyhow::anyhow!(
            "cannot locate in-tree source for '{PLUGIN_ID}' (expected crates/{PLUGIN_CRATE}/Plugin.toml). \
             Auto-heal only works from a Vox workspace checkout; install the plugin manually otherwise."
        )
    })?;
    let workspace_root = find_workspace_root(&source_dir).ok_or_else(|| {
        anyhow::anyhow!("cannot find workspace root above {}", source_dir.display())
    })?;
    let version = read_plugin_version(&source_dir)
        .with_context(|| format!("reading plugin version from {}", source_dir.display()))?;

    eprintln!(
        "   building {PLUGIN_CRATE} (release, cuda) from {} …",
        workspace_root.display()
    );
    run_plugin_build(&workspace_root).context("cargo build for the cuda plugin")?;

    let artifact = workspace_root.join("target").join("release").join(ARTIFACT);
    if !artifact.is_file() {
        anyhow::bail!(
            "build reported success but artifact {} was not produced",
            artifact.display()
        );
    }

    let install_dir = vox_plugin_host::resolve_plugins_root()
        .join(PLUGIN_ID)
        .join(&version);
    std::fs::create_dir_all(&install_dir)
        .with_context(|| format!("creating install dir {}", install_dir.display()))?;

    // Copy the artifact plus the Plugin.toml/Cargo.toml metadata the host reads.
    let dest_artifact = install_dir.join(ARTIFACT);
    // Back up any prior artifact so a botched copy is recoverable.
    if dest_artifact.is_file() {
        let _ = std::fs::rename(&dest_artifact, install_dir.join(format!("{ARTIFACT}.prev")));
    }
    std::fs::copy(&artifact, &dest_artifact).with_context(|| {
        format!(
            "copying {} -> {}",
            artifact.display(),
            dest_artifact.display()
        )
    })?;
    for meta in ["Plugin.toml", "Cargo.toml"] {
        let from = source_dir.join(meta);
        if from.is_file() {
            let _ = std::fs::copy(&from, install_dir.join(meta));
        }
    }

    eprintln!(
        "   reinstalled '{PLUGIN_ID}' v{version} → {}",
        install_dir.display()
    );
    Ok(())
}

/// Run `cargo build -p <crate> --release --features cuda` with the right
/// toolchain environment for the host platform.
fn run_plugin_build(workspace_root: &Path) -> Result<()> {
    let status = if cfg!(windows) {
        run_plugin_build_windows(workspace_root)?
    } else {
        run_plugin_build_unix(workspace_root)?
    };
    if !status.success() {
        anyhow::bail!(
            "cargo build for '{PLUGIN_CRATE}' failed (exit {:?}). \
             On Windows ensure Visual Studio Build Tools + a CUDA toolkit are installed; \
             on Linux ensure nvcc and the CUDA libraries are on the toolchain path.",
            status.code()
        );
    }
    Ok(())
}

/// Windows: wrap cargo in a VS developer environment (so nvcc finds cl.exe) and
/// pin CUDA_PATH/CUDA_HOME to the newest toolkit (so cudarc links 13.x symbols).
#[cfg(windows)]
fn run_plugin_build_windows(workspace_root: &Path) -> Result<std::process::ExitStatus> {
    let vcvars = find_vcvars().ok_or_else(|| {
        anyhow::anyhow!(
            "could not locate vcvars64.bat (Visual Studio Build Tools). \
             Install the C++ build tools or run the build from a VS Developer shell."
        )
    })?;
    let cuda = find_newest_cuda().ok_or_else(|| {
        anyhow::anyhow!(
            "could not locate a CUDA toolkit with a driver import library (cuda.lib). \
             Install the CUDA Toolkit (>= 13.1 for cudarc 0.19)."
        )
    })?;

    // Single cmd line: call vcvars (sets cl.exe + INCLUDE/LIB), pin CUDA, then build.
    // `call` is required so the parent batch context survives vcvars.
    let cuda_s = cuda.display();
    let cmdline = format!(
        "call \"{}\" >nul 2>&1 && \
         set \"CUDA_PATH={cuda_s}\" && set \"CUDA_HOME={cuda_s}\" && set \"CUDA_ROOT={cuda_s}\" && \
         set \"PATH={cuda_s}\\bin;%PATH%\" && \
         cargo build -p {PLUGIN_CRATE} --release --features cuda",
        vcvars.display()
    );
    Command::new("cmd")
        .arg("/c")
        .arg(cmdline)
        .current_dir(workspace_root)
        .status()
        .context("spawning cmd for the windows plugin build")
}

/// Unix: pin CUDA env to the newest toolkit and build directly.
#[cfg(not(windows))]
fn run_plugin_build_unix(workspace_root: &Path) -> Result<std::process::ExitStatus> {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "build",
        "-p",
        PLUGIN_CRATE,
        "--release",
        "--features",
        "cuda",
    ])
    .current_dir(workspace_root);
    if let Some(cuda) = find_newest_cuda() {
        cmd.env("CUDA_PATH", &cuda)
            .env("CUDA_HOME", &cuda)
            .env("CUDA_ROOT", &cuda);
    }
    cmd.status()
        .context("spawning cargo for the unix plugin build")
}

// Keep the opposite-platform builder referenced so dead_code lints stay quiet
// when only one is used.
#[cfg(windows)]
#[allow(dead_code)]
fn run_plugin_build_unix(_workspace_root: &Path) -> Result<std::process::ExitStatus> {
    unreachable!("unix builder is not used on windows")
}
#[cfg(not(windows))]
#[allow(dead_code)]
fn run_plugin_build_windows(_workspace_root: &Path) -> Result<std::process::ExitStatus> {
    unreachable!("windows builder is not used on unix")
}

/// Walk up from the plugin source dir to the directory whose Cargo.toml declares
/// a `[workspace]`.
fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start;
    loop {
        let cargo = dir.join("Cargo.toml");
        if cargo.is_file() {
            if let Ok(text) = std::fs::read_to_string(&cargo) {
                if text.contains("[workspace]") {
                    return Some(dir.to_path_buf());
                }
            }
        }
        dir = dir.parent()?;
    }
}

/// Parse the `version = "x.y.z"` line from the plugin's Plugin.toml `[plugin]` table.
fn read_plugin_version(source_dir: &Path) -> Result<String> {
    let path = source_dir.join("Plugin.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    // Minimal parse: first `version = "..."` under the file (the [plugin] table
    // is first). Avoids pulling a TOML dep into the hot path.
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("version") {
            if let Some(eq) = rest.trim_start().strip_prefix('=') {
                let v = eq.trim().trim_matches('"').trim_matches('\'');
                if !v.is_empty() {
                    return Ok(v.to_string());
                }
            }
        }
    }
    anyhow::bail!("no `version = \"...\"` found in {}", path.display())
}

/// Locate `vcvars64.bat` via `vswhere`, then the well-known fixed path.
#[cfg(windows)]
fn find_vcvars() -> Option<PathBuf> {
    // 1. vswhere (handles non-default install locations + editions).
    let pf86 =
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:/Program Files (x86)".to_string());
    let vswhere = PathBuf::from(&pf86)
        .join("Microsoft Visual Studio")
        .join("Installer")
        .join("vswhere.exe");
    if vswhere.is_file() {
        if let Ok(out) = Command::new(&vswhere)
            .args([
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property",
                "installationPath",
            ])
            .output()
        {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    let vcvars = PathBuf::from(path)
                        .join("VC")
                        .join("Auxiliary")
                        .join("Build")
                        .join("vcvars64.bat");
                    if vcvars.is_file() {
                        return Some(vcvars);
                    }
                }
            }
        }
    }

    // 2. Fixed fallbacks for common editions.
    for edition in ["BuildTools", "Community", "Professional", "Enterprise"] {
        let cand = PathBuf::from(&pf86)
            .join("Microsoft Visual Studio")
            .join("2022")
            .join(edition)
            .join("VC")
            .join("Auxiliary")
            .join("Build")
            .join("vcvars64.bat");
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// Find the newest CUDA toolkit that ships a driver import library.
///
/// On Windows the driver import library is `lib/x64/cuda.lib`; cudarc links
/// against it, so a toolkit without it (or one too old to define the symbols
/// cudarc references) is unusable. We pick the lexicographically-newest version
/// directory that has it, which orders `v12.x < v13.x` correctly.
fn find_newest_cuda() -> Option<PathBuf> {
    // Honor an explicit override first.
    for var in ["CUDA_PATH", "CUDA_HOME", "CUDA_ROOT"] {
        if let Some(p) = std::env::var_os(var) {
            let p = PathBuf::from(p);
            if cuda_is_usable(&p) {
                return Some(p);
            }
        }
    }

    let base: PathBuf = if cfg!(windows) {
        PathBuf::from("C:/Program Files/NVIDIA GPU Computing Toolkit/CUDA")
    } else {
        PathBuf::from("/usr/local")
    };
    let entries = std::fs::read_dir(&base).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            // On unix CUDA dirs look like /usr/local/cuda-13.2; on windows v13.2.
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            (name.starts_with('v') || name.starts_with("cuda")) && cuda_is_usable(p)
        })
        .collect();
    candidates.sort();
    candidates.pop()
}

/// True when `root` looks like a usable CUDA toolkit (has the driver import lib).
fn cuda_is_usable(root: &Path) -> bool {
    if cfg!(windows) {
        root.join("lib").join("x64").join("cuda.lib").is_file()
    } else {
        // libcuda is provided by the driver; the toolkit ships nvcc + stubs.
        root.join("bin").join("nvcc").is_file()
            || root
                .join("lib64")
                .join("stubs")
                .join("libcuda.so")
                .is_file()
    }
}
