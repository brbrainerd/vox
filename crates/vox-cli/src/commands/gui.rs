use anyhow::{Context, Result};
use std::env;

pub async fn run(args: crate::cli_args::GuiArgs) -> Result<()> {
    tracing::info!("Launching Vox Native GUI...");
    
    let mut cmd = if cfg!(debug_assertions) {
        let mut c = std::process::Command::new("cargo");
        c.args(["run", "-p", "vox-gui"]);
        c
    } else {
        let exe = env::current_exe()?;
        let parent = exe.parent().context("Failed to get executable directory")?;
        let gui_bin_name = if cfg!(windows) { "vox-gui.exe" } else { "vox-gui" };
        let mut c = std::process::Command::new(parent.join(gui_bin_name));
        c
    };
    
    if let Some(cmd_val) = args.command {
        cmd.arg("--command").arg(cmd_val);
    }

    let mut child = cmd.spawn()?;
    child.wait()?;
    Ok(())
}
