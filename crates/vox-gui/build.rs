use std::path::{Path, PathBuf};

/// Check every `bundle.externalBin` entry in `tauri.conf.json` for the
/// Tauri-sidecar-suffixed binary (e.g. `vox-x86_64-pc-windows-msvc.exe`) before
/// handing off to `tauri_build`, which fails with a repo-unaware path-only
/// panic. Each fresh `git worktree add` gets its own separate `target/` (see
/// `.cargo/config.toml`'s `CARGO_TARGET_DIR` note — this is deliberate, not a
/// bug), so this is expected to fail the first time any worktree builds
/// vox-gui, not just a one-off misconfiguration.
fn check_sidecar_binaries(gui_dir: &Path) -> Result<(), String> {
    let conf_path = gui_dir.join("tauri.conf.json");
    let conf_raw = std::fs::read_to_string(&conf_path)
        .map_err(|e| format!("read {}: {e}", conf_path.display()))?;
    let conf: serde_json::Value = serde_json::from_str(&conf_raw)
        .map_err(|e| format!("parse {}: {e}", conf_path.display()))?;
    let Some(bins) = conf["bundle"]["externalBin"].as_array() else {
        return Ok(()); // no externalBin declared — nothing to check
    };

    let triple = std::env::var("TARGET").unwrap_or_default();
    let ext = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };

    let mut missing = Vec::new();
    for entry in bins {
        let Some(rel) = entry.as_str() else { continue };
        let sidecar = gui_dir.join(format!("{rel}-{triple}{ext}"));
        if !sidecar.is_file() {
            missing.push(sidecar);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }

    let missing_list = missing
        .iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(format!(
        "vox-gui sidecar binary missing:\n{missing_list}\n\n\
         Every fresh `git worktree add` gets its own `target/` (per-worktree by\n\
         design — see `.cargo/config.toml`), so a new worktree has no release\n\
         `vox` binary yet. Build it before building vox-gui:\n\n  \
         vox run scripts/gui-build.vox\n\n\
         or manually:\n\n  \
         cargo build -p vox-cli --release --bin vox\n  \
         # then copy target/release/vox{ext} to the path(s) listed above"
    ))
}

fn main() {
    vox_build_meta::emit();
    let gui_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Err(msg) = check_sidecar_binaries(&gui_dir) {
        panic!("{msg}");
    }
    // Must not swallow errors: a missing Windows manifest yields STATUS_ENTRYPOINT_NOT_FOUND at runtime.
    if let Err(err) = tauri_build::try_build(tauri_build::Attributes::new()) {
        panic!("tauri build script failed: {err}");
    }
}
