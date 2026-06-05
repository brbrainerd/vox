use anyhow::{Context, Result};
use std::env;
use std::path::Path;

pub async fn run(args: crate::cli_args::GuiArgs) -> Result<()> {
    tracing::info!("Launching Vox Native GUI...");

    let mut cmd = if cfg!(debug_assertions) {
        let mut c = std::process::Command::new("cargo");
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
        let gui_path = parent.join(gui_bin_name);
        if !gui_path.exists() {
            // The GUI is an optional catalog *component*, not part of the CLI build,
            // so CLI-only users never compile it. Resolve the component and give an
            // actionable install instruction rather than spawning a missing binary.
            ensure_gui_available(&gui_path)?;
        }
        std::process::Command::new(gui_path)
    };

    if let Some(cmd_val) = args.command {
        cmd.arg("--command").arg(cmd_val);
    }

    let mut child = cmd.spawn()?;
    child.wait()?;
    Ok(())
}

/// Verify the optional GUI component can run on this platform and, if its binary
/// is missing, return an actionable install instruction.
///
/// The GUI ships as an optional `[[component]]` in the plugin catalog (see
/// `vox-plugin-catalog`) rather than as part of the CLI build. A turnkey fetch
/// (prebuilt release asset or `cargo build -p vox-gui`) is a follow-up gated on
/// the release pipeline producing GUI assets; until then this gives a clear
/// manual path instead of pretending to install.
fn ensure_gui_available(expected_path: &Path) -> Result<()> {
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

    anyhow::bail!(
        "the Vox GUI is an optional component and is not installed at {}.\n\
         Build it with `cargo build -p vox-gui` (the binary lands next to your `vox` \
         executable), or install a prebuilt release asset once GUI assets ship.\n\
         Catalog source: {}",
        expected_path.display(),
        component.default_source,
    );
}
