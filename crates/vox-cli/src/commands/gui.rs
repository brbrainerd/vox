use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub async fn run(args: crate::cli_args::GuiArgs) -> Result<()> {
    tracing::info!("Launching Vox Axis (Axis) — the native Vox GUI…");

    let mut cmd = if cfg!(debug_assertions) {
        let mut c = Command::new("cargo");
        c.args(["run", "-p", "vox-gui"]);
        c
    } else {
        let exe = env::current_exe()?;
        let parent = exe.parent().context("Failed to get executable directory")?;
        let gui_bin_name = if cfg!(windows) {
            "vox-gui.exe"
        } else {
            "vox-gui"
        };
        let installed = parent.join(gui_bin_name);
        let launch_path = if installed.exists() {
            installed
        } else {
            // The GUI is an optional catalog *component*, not part of the CLI build,
            // so CLI-only users never compile it. Resolve + build/install on demand.
            resolve_or_build_gui(&installed, gui_bin_name)?
        };
        Command::new(launch_path)
    };

    if let Some(cmd_val) = args.command {
        cmd.arg("--command").arg(cmd_val);
    }

    let mut child = cmd.spawn()?;
    child.wait()?;
    Ok(())
}

/// Resolve a runnable GUI binary when it isn't installed next to the `vox`
/// executable. The GUI ships as an optional `[[component]]` in the plugin catalog
/// (see `vox-plugin-catalog`) rather than as part of the CLI build.
///
/// Resolution order:
///   1. Verify the component exists in the catalog and targets this platform.
///   2. If we're inside a Vox source checkout, build it with
///      `cargo build -p vox-gui --release` and install the binary next to `vox`.
///   3. Otherwise return an actionable error (clone+build, or install a prebuilt
///      release asset once the release pipeline ships them — a follow-up, since
///      no GUI release assets are produced yet).
fn resolve_or_build_gui(installed: &Path, gui_bin_name: &str) -> Result<PathBuf> {
    let component = vox_plugin_catalog::all_components()
        .iter()
        .find(|c| c.id == "gui")
        .context("no 'gui' component declared in the plugin catalog")?;

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let os_ok = component.requires.os.is_empty() || component.requires.os.iter().any(|o| o == os);
    let arch_ok =
        component.requires.arch.is_empty() || component.requires.arch.iter().any(|a| a == arch);
    if !os_ok || !arch_ok {
        anyhow::bail!("the Vox GUI component is not available for this platform ({os}/{arch}).");
    }

    let Some(workspace_root) = locate_workspace_root() else {
        anyhow::bail!(gui_missing_no_checkout_message(
            installed,
            &component.default_source,
        ));
    };

    tracing::info!(
        "Vox GUI not installed; building from source (cargo build -p vox-gui --release)…"
    );
    let status = Command::new("cargo")
        .args(["build", "-p", "vox-gui", "--release"])
        .current_dir(&workspace_root)
        .status()
        .context("failed to invoke `cargo build -p vox-gui`")?;
    if !status.success() {
        anyhow::bail!(
            "`cargo build -p vox-gui --release` failed. The GUI is a Tauri app and needs its \
             frontend toolchain (Node + the platform webview deps) and a built `ui/dist`; see \
             crates/vox-gui for setup."
        );
    }

    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root.join("target"));
    let built = target_dir.join("release").join(gui_bin_name);
    if !built.exists() {
        anyhow::bail!(
            "GUI build reported success but no binary was found at {}.",
            built.display()
        );
    }
    // Best-effort: install next to the `vox` exe so future launches skip the build.
    if let Some(parent) = installed.parent() {
        let _ = std::fs::create_dir_all(parent);
        if std::fs::copy(&built, installed).is_ok() {
            tracing::info!("Installed Vox GUI to {}", installed.display());
            return Ok(installed.to_path_buf());
        }
    }
    Ok(built)
}

/// Path to the workspace root (parent of the workspace `Cargo.toml`) when invoked
/// from inside a Cargo workspace, else `None`.
fn locate_workspace_root() -> Option<PathBuf> {
    crate::contributor_mode::locate_workspace_root()
}

/// Message for the "no GUI installed, and no checkout to build one from"
/// branch — reached precisely when [`locate_workspace_root`] has already
/// confirmed the caller has no Vox workspace above them (spec §9.1's
/// non-contributor persona, by construction of this branch).
///
/// Leads with the actual status for this user (not installed, no prebuilt
/// asset, no checkout here) and reports the checkout requirement as a
/// precondition being described, not as an instruction to someone who by
/// construction cannot follow it. The source-build route is named only as
/// "the contributor path" for context, never as this user's remedy.
fn gui_missing_no_checkout_message(installed: &Path, catalog_source: &str) -> String {
    format!(
        "the Vox GUI is an optional component and is not installed at {}.\n\
         Prebuilt GUI release assets don't ship yet, and this isn't a Vox source \
         checkout — so there's no way to obtain the GUI here. (Building it from \
         source with `cargo build -p vox-gui` is the contributor path, from inside \
         a checkout.)\n\
         Catalog source: {catalog_source}",
        installed.display(),
    )
}

#[cfg(test)]
mod gui_missing_message_tests {
    use super::*;

    #[test]
    fn leads_with_actual_status_not_installed_instruction() {
        let msg = gui_missing_no_checkout_message(
            Path::new("/opt/vox/bin/vox-gui"),
            "https://example.invalid/vox-gui",
        );
        // Reports status honestly: not installed, no prebuilt asset yet, no
        // checkout here — this is a description of fact, not a command.
        assert!(msg.contains("is not installed at"));
        assert!(msg.contains("don't ship yet"));
        assert!(msg.contains("this isn't a Vox source checkout"));
        // The checkout route is framed as context ("the contributor path"),
        // never as an instruction ("Clone the repo and run ...").
        assert!(msg.contains("the contributor path"));
        assert!(!msg.contains("Clone the repo"));
        assert!(msg.contains("https://example.invalid/vox-gui"));
    }
}
