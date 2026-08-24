use std::path::{Path, PathBuf};

/// Check every `bundle.externalBin` entry in `tauri.conf.json` for the
/// Tauri-sidecar-suffixed binary (e.g. `vox-x86_64-pc-windows-msvc.exe`) before
/// handing off to `tauri_build`, which fails with a repo-unaware path-only
/// panic. Each fresh `git worktree add` gets its own separate `target/` (see
/// `.cargo/config.toml`'s `CARGO_TARGET_DIR` note — this is deliberate, not a
/// bug), so this is expected on the first vox-gui build in any new worktree.
///
/// Returns the list of `(sidecar_path, unsuffixed_bin_name)` pairs that are
/// still missing after an attempted self-heal (empty if everything is present
/// or the self-heal succeeded).
fn check_sidecar_binaries(gui_dir: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let conf_path = gui_dir.join("tauri.conf.json");
    let conf_raw = std::fs::read_to_string(&conf_path)
        .map_err(|e| format!("read {}: {e}", conf_path.display()))?;
    let conf: serde_json::Value = serde_json::from_str(&conf_raw)
        .map_err(|e| format!("parse {}: {e}", conf_path.display()))?;
    let Some(bins) = conf["bundle"]["externalBin"].as_array() else {
        return Ok(Vec::new()); // no externalBin declared — nothing to check
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
            let bin_name = Path::new(rel)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("vox")
                .to_string();
            missing.push((sidecar, bin_name));
        }
    }
    Ok(missing)
}

/// `cargo build -p vox-cli --release --bin <name>` then copy the unsuffixed
/// `target/release/<name>` to the triple-suffixed sidecar path Tauri expects.
/// Only handles the CLI binary itself — the frontend (`ui/dist`, built via
/// `vox run scripts/gui-build.vox`'s `pnpm build` step) is a separate, larger
/// dependency this can't self-heal, and is checked independently elsewhere.
fn autobuild_sidecar(sidecar: &Path, bin_name: &str) -> Result<(), String> {
    println!(
        "cargo:warning=vox-gui: sidecar binary {} missing, running `cargo build -p vox-cli --release --bin {bin_name}` (opt-in via VOX_GUI_AUTOBUILD_SIDECAR; this is a full release build of the heaviest crate in the workspace and takes tens of minutes)",
        sidecar.display()
    );
    let status = std::process::Command::new(env!("CARGO"))
        .args(["build", "-p", "vox-cli", "--release", "--bin", bin_name])
        .status()
        .map_err(|e| format!("spawn `cargo build -p vox-cli --release --bin {bin_name}`: {e}"))?;
    if !status.success() {
        return Err(format!(
            "`cargo build -p vox-cli --release --bin {bin_name}` exited with {status}"
        ));
    }
    let ext = if sidecar.extension().is_some() {
        format!(".{}", sidecar.extension().unwrap().to_string_lossy())
    } else {
        String::new()
    };
    let built = sidecar
        .parent()
        .expect("sidecar path has a parent dir")
        .join(format!("{bin_name}{ext}"));
    if !built.is_file() {
        return Err(format!(
            "cargo build succeeded but expected output {} is missing",
            built.display()
        ));
    }
    std::fs::copy(&built, sidecar)
        .map_err(|e| format!("copy {} -> {}: {e}", built.display(), sidecar.display()))?;
    println!(
        "cargo:warning=vox-gui: sidecar binary built and copied to {}",
        sidecar.display()
    );
    Ok(())
}

fn main() {
    vox_build_meta::emit();
    let gui_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let missing = check_sidecar_binaries(&gui_dir).unwrap_or_else(|e| panic!("{e}"));

    // Opt-in only: a nested `cargo build -p vox-cli --release` here is a full
    // release codegen of the heaviest crate in the workspace (917 deps). It ran
    // *inside* a plain `cargo check --workspace`, where it measured at 76 of the
    // 87 minutes of the whole run and serialized every dependent unit behind it.
    // Default is now a fast, actionable failure naming `scripts/gui-build.vox`.
    println!("cargo:rerun-if-env-changed=VOX_GUI_AUTOBUILD_SIDECAR");
    let autobuild = std::env::var("VOX_GUI_AUTOBUILD_SIDECAR").is_ok_and(|v| v != "0");
    let mut still_missing = Vec::new();
    for (sidecar, bin_name) in missing {
        if !autobuild {
            still_missing.push(sidecar);
            continue;
        }
        if let Err(e) = autobuild_sidecar(&sidecar, &bin_name) {
            println!("cargo:warning=vox-gui: sidecar autobuild failed: {e}");
            still_missing.push(sidecar);
        }
    }

    if !still_missing.is_empty() {
        let missing_list = still_missing
            .iter()
            .map(|p| format!("  - {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "vox-gui sidecar binary missing (autobuild {}):\n{missing_list}\n\n\
             Build it before building vox-gui:\n\n  \
             vox run scripts/gui-build.vox\n\n\
             or manually:\n\n  \
             cargo build -p vox-cli --release --bin vox\n  \
             # then copy target/release/vox<ext> to the path(s) listed above",
            if autobuild {
                "attempted and failed — see warnings above"
            } else {
                "off by default; set VOX_GUI_AUTOBUILD_SIDECAR=1 to build it here instead — \
                 it is a full release build of vox-cli and takes tens of minutes"
            }
        );
    }

    // Must not swallow errors: a missing Windows manifest yields STATUS_ENTRYPOINT_NOT_FOUND at runtime.
    if let Err(err) = tauri_build::try_build(tauri_build::Attributes::new()) {
        panic!("tauri build script failed: {err}");
    }
}
