//! Checks that the Tauri `externalBin` sidecar for `vox-gui` exists under the
//! *current* `target/` (each `git worktree add` gets its own — see
//! `.cargo/config.toml`), so building `vox-gui` doesn't fail deep inside
//! `tauri-build` with a path-only panic. See `crates/vox-gui/build.rs`.

use std::path::PathBuf;

use super::super::common::Check;

pub fn run(checks: &mut Vec<Check>) {
    let gui_dir = PathBuf::from("crates/vox-gui");
    let conf_path = gui_dir.join("tauri.conf.json");

    let raw = match std::fs::read_to_string(&conf_path) {
        Ok(s) => s,
        Err(_) => return, // not running from a vox-gui-having checkout; skip silently
    };
    let Ok(conf) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };
    let Some(bins) = conf["bundle"]["externalBin"].as_array() else {
        return;
    };

    let triple = current_triple();
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
            missing.push(sidecar.display().to_string());
        }
    }

    if missing.is_empty() {
        checks.push(Check::pass(
            "vox-gui sidecar",
            format!("present for {triple}"),
        ));
    } else {
        checks.push(Check::fail(
            "vox-gui sidecar",
            format!(
                "missing {} — run: vox run scripts/gui-build.vox (or `cargo build -p vox-cli --release` then copy target/release/vox{ext} to the path(s) above; each git worktree needs its own copy)",
                missing.join(", ")
            ),
        ));
    }
}

fn current_triple() -> String {
    // Doctor runs the already-built `vox` binary, so `TARGET` (a build-script-only
    // env var) isn't available here — shell out to rustc like compile_target.rs.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    std::process::Command::new(rustc)
        .arg("-vV")
        .output()
        .ok()
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .find_map(|l| l.strip_prefix("host: ").map(str::to_string))
        })
        .unwrap_or_else(|| "x86_64-pc-windows-msvc".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_triple_returns_a_real_looking_triple() {
        let t = current_triple();
        assert!(t.contains('-'), "expected a target triple, got {t:?}");
    }
}
