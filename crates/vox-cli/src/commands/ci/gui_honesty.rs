//! `vox ci gui-honesty` — typed toasts + no placeholder/dead elements in surfaces.
//!
//! Runs two checks in `crates/vox-gui/ui`:
//!   1. `pnpm run typecheck`  — catches `ToastCause` / `ToastSeverity` type errors.
//!   2. `pnpm vitest run src/components/surfaces/__guards__/surfaceHonesty.guard.test.ts`
//!      — the honesty scanner (no placeholder text, no dead handlers in shipped surfaces).

use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};

/// Build a `Command` that runs `pnpm <args>` cross-platform.
///
/// On Windows, pnpm is a `.cmd` script and must be invoked explicitly with
/// the `.cmd` suffix so `CreateProcess` dispatches it correctly.  Pnpm itself
/// handles adding `node_modules/.bin` to the subprocess PATH, so `tsc` and
/// `vitest` are resolved from there.  On Unix, `pnpm` is a plain executable.
fn pnpm_cmd(ui_dir: &Path, args: &[&str]) -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("pnpm.cmd");
        cmd.current_dir(ui_dir);
        cmd.args(args);
        cmd
    } else {
        let mut cmd = Command::new("pnpm");
        cmd.current_dir(ui_dir);
        cmd.args(args);
        cmd
    }
}

pub fn run(root: &Path) -> Result<()> {
    let ui_dir = root.join("crates/vox-gui/ui");

    // ── 1. TypeScript type-check ──────────────────────────────────────────────
    let tsc = pnpm_cmd(&ui_dir, &["run", "typecheck"]).status()?;
    if !tsc.success() {
        return Err(anyhow!(
            "gui-honesty: `pnpm run typecheck` in crates/vox-gui/ui failed \
             (fix TypeScript errors — likely a ToastCause / ToastSeverity type mismatch)"
        ));
    }

    // ── 2. Vitest surface-honesty guard ───────────────────────────────────────
    let vitest = pnpm_cmd(
        &ui_dir,
        &[
            "vitest",
            "run",
            "src/components/surfaces/__guards__/surfaceHonesty.guard.test.ts",
        ],
    )
    .status()?;
    if !vitest.success() {
        return Err(anyhow!(
            "gui-honesty: `pnpm vitest run surfaceHonesty.guard.test.ts` failed \
             (placeholder text or dead handlers detected in shipped surfaces — \
             fix or add an allowlist entry in honestyScan.allowlist.ts)"
        ));
    }

    println!("gui-honesty: OK");
    Ok(())
}
