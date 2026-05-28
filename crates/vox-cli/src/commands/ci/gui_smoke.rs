//! `vox ci gui-smoke` — deterministic WebIR lowering tests plus opt-in Vite / Playwright integration lanes.

use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};

use super::cargo_bin;

/// Stable ignored-only WebIR smoke: matches compiler-gates `web_ir_lower_emit_test` TanStack/router guard.
const WEB_IR_LOWER_EMIT_SMOKE_FILTER: &str =
    "test(codegen_output_never_includes_vox_tanstack_router_or_server_fns)";

/// Run the GUI smoke bundle (web-ir-smoke) from repo `root`.
pub fn run(root: &Path) -> Result<()> {
    let cargo = cargo_bin();

    let st = Command::new(&cargo)
        .current_dir(root)
        .args([
            "nextest",
            "run",
            "-p",
            "vox-compiler",
            "--test",
            "web_ir_lower_emit_test",
            "--run-ignored",
            "ignored-only",
            "-E",
            WEB_IR_LOWER_EMIT_SMOKE_FILTER,
            "--no-capture",
        ])
        .status()?;
    if !st.success() {
        return Err(anyhow!(
            "gui-smoke: `cargo nextest run -p vox-compiler --test web_ir_lower_emit_test --run-ignored ignored-only -E '{WEB_IR_LOWER_EMIT_SMOKE_FILTER}'` failed"
        ));
    }
    println!("gui-smoke: web_ir_lower_emit_test (ignored TanStack/router guard) OK");

    if std::env::var("VOX_GUI_PNPM_BUILD").ok().as_deref() == Some("1") {
        let st = Command::new(if cfg!(windows) { "pnpm.cmd" } else { "pnpm" })
            .current_dir(root.join("crates/vox-gui/ui"))
            .args(["run", "build"])
            .status()?;
        if !st.success() {
            return Err(anyhow!(
                "gui-smoke: `pnpm run build` in crates/vox-gui/ui failed"
            ));
        }
        println!("gui-smoke: pnpm run build OK");
    } else {
        println!("gui-smoke: skip pnpm build lane (set VOX_GUI_PNPM_BUILD=1)");
    }

    if std::env::var("VOX_WEB_VITE_SMOKE").ok().as_deref() == Some("1") {
        let st = Command::new(&cargo)
            .current_dir(root)
            .env("VOX_WEB_VITE_SMOKE", "1")
            .args([
                "nextest",
                "run",
                "-p",
                "vox-integration-tests",
                "--test",
                "web_vite_smoke_test",
                "--run-ignored",
                "ignored-only",
                "--no-capture",
            ])
            .status()?;
        if !st.success() {
            return Err(anyhow!(
                "gui-smoke: Vite `web_vite_smoke_test` failed (see `VOX_WEB_VITE_SMOKE`)"
            ));
        }
        println!("gui-smoke: Vite web_vite_smoke_test OK");
    } else {
        println!(
            "gui-smoke: skip Vite lane (set VOX_WEB_VITE_SMOKE=1 to run `web_vite_smoke_test`)"
        );
    }

    if std::env::var("VOX_GUI_PLAYWRIGHT").ok().as_deref() == Some("1") {
        let st = Command::new(&cargo)
            .current_dir(root)
            .env("VOX_GUI_PLAYWRIGHT", "1")
            .args([
                "nextest",
                "run",
                "-p",
                "vox-integration-tests",
                "--test",
                "playwright_golden_route_test",
                "--run-ignored",
                "ignored-only",
                "--no-capture",
            ])
            .status()?;
        if !st.success() {
            return Err(anyhow!(
                "gui-smoke: Playwright `playwright_golden_route_test` failed (set browsers + `pnpm install` under `crates/vox-integration-tests`)"
            ));
        }
        println!("gui-smoke: Playwright golden_route OK");
    } else {
        println!(
            "gui-smoke: skip Playwright lane (set VOX_GUI_PLAYWRIGHT=1 on browser-capable runners)"
        );
    }

    Ok(())
}
