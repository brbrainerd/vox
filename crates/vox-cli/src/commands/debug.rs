//! `vox debug` — launch a DAP debugging session for a Vox source file.
//!
//! In **stdio mode** (default, used by editors):
//!   Launches `vox-dap` as a subprocess. The editor's DAP client connects via
//!   the process's stdin/stdout — this is the standard DAP "inline server" model
//!   used by VS Code's `debuggers` contribution and nvim-dap.
//!
//! In **port mode** (`--port N`):
//!   Launches `vox-dap --listen N` for editors that prefer TCP connections.
//!
//! When called from a terminal (not an editor), the command runs a quick
//! script-mode session using the built-in `HirInterp` directly (no DAP framing)
//! so developers get a readable trace without editor setup.

use anyhow::{Context, Result};
use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped;

/// Entry point for `vox debug`.
pub async fn run(
    file: &Path,
    mode: &str,
    stop_on_entry: bool,
    port: Option<u16>,
) -> Result<()> {
    let abs = file
        .canonicalize()
        .with_context(|| format!("cannot resolve path: {}", file.display()))?;

    // Detect whether we're being called by an editor (stdin is a pipe) or a terminal.
    let is_editor_session = !std::io::IsTerminal::is_terminal(&std::io::stdin()) || port.is_some();

    if is_editor_session {
        launch_dap_server(&abs, mode, stop_on_entry, port).await
    } else {
        run_direct_script(&abs, mode, stop_on_entry).await
    }
}

/// Launch `vox-dap` as a subprocess. The calling process becomes a transparent
/// pipe bridge so the editor's DAP client talks directly to `vox-dap`.
async fn launch_dap_server(
    abs: &Path,
    mode: &str,
    _stop_on_entry: bool,
    port: Option<u16>,
) -> Result<()> {
    // Locate the vox-dap binary next to the current executable.
    let dap_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("vox-dap")))
        .filter(|p| p.exists())
        .or_else(which_vox_dap)
        .ok_or_else(|| anyhow::anyhow!(
            "vox-dap not found. Run `cargo build -p vox-dap` to build it."
        ))?;

    let mut cmd = tokio::process::Command::new(&dap_bin);

    if let Some(p) = port {
        cmd.arg("--listen").arg(p.to_string());
    }

    // Pass launch hint env vars so vox-dap can auto-launch without a `launch` request.
    cmd.env("VOX_DAP_PROGRAM", abs.to_string_lossy().as_ref());
    cmd.env("VOX_DAP_MODE", mode);

    // Inherit stdin/stdout/stderr so the editor talks directly to vox-dap.
    cmd.stdin(std::process::Stdio::inherit())
       .stdout(std::process::Stdio::inherit())
       .stderr(std::process::Stdio::inherit());

    let mut child = cmd.spawn()
        .with_context(|| format!("failed to spawn {}", dap_bin.display()))?;

    let status = child.wait().await.context("vox-dap process error")?;
    if !status.success() {
        anyhow::bail!("vox-dap exited with {}", status);
    }
    Ok(())
}

/// Direct script-mode interpreter run (no DAP framing) — for terminal use.
///
/// Runs the HIR interpreter with a `NullDapChannel` (no stepping) and prints
/// a readable execution trace so developers can debug without an editor.
async fn run_direct_script(abs: &Path, _mode: &str, _stop_on_entry: bool) -> Result<()> {
    let source = read_utf8_path_capped(abs)
        .with_context(|| format!("cannot read {}", abs.display()))?;
    let frontend = crate::pipeline::run_frontend_str(&source, abs, false)?;
    crate::pipeline::print_diagnostics(&frontend, abs, false);
    if frontend.has_errors() {
        anyhow::bail!(
            "debug frontend failed with {} error(s) and {} warning(s)",
            frontend.error_count(),
            frontend.warning_count()
        );
    }
    let hir = frontend.hir;

    let path_str = abs.to_string_lossy().to_string();
    let mut interp = vox_machina::interp::HirInterp::new(
        &source,
        &path_str,
        Some(&vox_machina::NullDapChannel),
    );

    let val = interp.run(&hir)
        .with_context(|| format!("runtime error in {}", abs.display()))?;

    if !matches!(val, vox_machina::interp::Val::Nil) {
        println!("→ {val}");
    }
    Ok(())
}

/// Try to find `vox-dap` on PATH.
fn which_vox_dap() -> Option<std::path::PathBuf> {
    which::which("vox-dap").ok()
}
