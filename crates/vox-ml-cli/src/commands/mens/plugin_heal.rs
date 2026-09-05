//! Self-healing for the `mens-candle-cuda` runtime plugin.
//!
//! `vox mens train --device cuda` dispatches QLoRA training to a runtime-loaded
//! cdylib plugin (`mens-candle-cuda`). Two failure modes make a plain train
//! command fail out of the box:
//!
//! 1. **Missing** — the plugin was never installed.
//! 2. **Version/ABI mismatch** — the installed dll was built against an older
//!    workspace version (`plugin.load_failed error_kind="root_module"`).
//!
//! When auto-heal is enabled (default; opt out with `--no-auto-heal` or
//! `VOX_MENS_NO_AUTO_HEAL=1`), [`ensure_cuda_plugin`] re-fetches a verified
//! release artifact for the plugin and reinstalls it before training proceeds.
//!
//! This does **not** compile anything at runtime. An earlier revision of this
//! module rebuilt the cdylib from in-tree source with `cargo build` — on
//! Windows wrapped in a Visual Studio developer environment so `nvcc` could
//! find `cl.exe`, with `CUDA_PATH`/`CUDA_HOME` pinned to the newest installed
//! toolkit. That assumed a Rust toolchain, a CUDA toolkit, and a repo
//! checkout — none of which an installed end user has (spec §4.1 P6: no
//! runtime cargo). Auto-heal now shells out to the already-built `vox`
//! binary's own `plugin install` subcommand instead; see
//! [`reinstall_via_vox_plugin_install`] for why that's a subprocess call
//! rather than a library dependency, and why it's found by name on `PATH`
//! rather than via `std::env::current_exe()`.
//!
//! This module is only compiled with the `gpu` feature, which is what makes
//! `vox-plugin-host` available.
#![cfg(feature = "gpu")]

use anyhow::{Context, Result};
use std::process::Command;

const PLUGIN_ID: &str = "mens-candle-cuda";

/// Ensure the CUDA training plugin is installed and loadable.
///
/// Returns `Ok(())` when the plugin is already healthy or was successfully
/// healed. When `auto_heal` is false and the plugin is unusable, returns an
/// actionable error instead of fetching a replacement.
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
                "⚠  '{PLUGIN_ID}' plugin unusable ({reason}); auto-healing (fetching verified artifact)…"
            );
            reinstall_via_vox_plugin_install(PLUGIN_ID)
                .with_context(|| format!("auto-healing the '{PLUGIN_ID}' plugin"))?;
            // Confirm the heal actually fixed it rather than silently proceeding.
            match probe_reason() {
                None => {
                    eprintln!("✓  '{PLUGIN_ID}' plugin healed and loads cleanly.");
                    Ok(())
                }
                Some(still) => {
                    anyhow::bail!("Reinstalled '{PLUGIN_ID}' but it is still unusable: {still}")
                }
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

/// Reinstall the plugin by invoking the `vox` binary's own `plugin install`
/// subcommand — the same checksum-gated, tag-aware artifact-download path a
/// fresh `vox plugin install <id>` uses (see
/// `vox-cli/src/commands/plugin/install.rs::install_from_catalog`).
///
/// # Why a subprocess call, not a library dependency
/// `vox-plugin-host` (this crate's plugin-loading dependency) has no
/// download/HTTP capability of its own — its `Cargo.toml` carries no
/// `reqwest`/http-client dependency. The verified download-and-checksum logic
/// lives in `vox-cli`, a downstream binary crate. Depending on it directly
/// from here would add a `vox-ml-cli` → `vox-cli` crate-graph edge, which is
/// architecturally backwards (a library-ish crate depending on a downstream
/// binary's command internals) and would need a user-authorized entry in
/// `contracts/ci/crate-edges.allow.v1.json` that this code cannot grant
/// itself. So instead this shells out to the already-built `vox` binary as a
/// subprocess — turning the exact command
/// [`vox_plugin_host::format_install_hint`] already prints to the user into a
/// programmatic auto-heal call.
///
/// # Why a PATH lookup, not `std::env::current_exe()`
/// `vox-ml-cli` ships as a SEPARATE binary from `vox` (see
/// `contracts/distribution/profiles.v1.yaml`'s `full` tier: `[vox,
/// vox-ml-cli, voxup]`), so `std::env::current_exe()` here would resolve to
/// `vox-ml-cli`'s own path, never `vox`'s — there is no reliable on-disk
/// relationship between the two binaries to exploit. A plain `"vox"` PATH
/// lookup matches `format_install_hint`'s own assumption that `vox` is on
/// PATH, and needs no packaging-layout knowledge.
///
/// Does not fall back to compiling anything: if `vox` is not on `PATH`, this
/// fails with an actionable message rather than silently rebuilding from
/// source — that fallback is exactly the runtime-cargo dependency this
/// module removes.
/// Resolve the `vox` binary, preferring the one sitting beside THIS binary
/// over whatever `vox` a PATH lookup happens to find first.
///
/// `Command::new("vox")` alone is a hijack vector: any directory earlier on
/// PATH containing an executable named `vox` gets run instead, with this
/// process's privileges, from inside the auto-heal path of a security
/// hardening feature. Installers put `vox` and `vox-ml-cli` in the same
/// directory, so the sibling lookup succeeds in the normal case and PATH stays
/// as the fallback for development checkouts where they are built separately.
///
/// `current_exe()` cannot be used directly for this -- it resolves to
/// vox-ml-cli's own path, never vox's -- but its PARENT is exactly the
/// directory a co-installed `vox` lives in.
fn resolve_vox_binary() -> std::ffi::OsString {
    let exe_name = if cfg!(windows) { "vox.exe" } else { "vox" };
    if let Ok(me) = std::env::current_exe()
        && let Some(dir) = me.parent()
    {
        let sibling = dir.join(exe_name);
        if sibling.is_file() {
            return sibling.into_os_string();
        }
    }
    exe_name.into()
}

fn reinstall_via_vox_plugin_install(plugin_id: &str) -> Result<()> {
    let status = Command::new(resolve_vox_binary())
        .args(["plugin", "install", plugin_id, "--yes"])
        .status()
        .with_context(|| {
            format!(
                "spawning `vox plugin install {plugin_id}` for auto-heal — is `vox` on PATH?\n\n{}",
                vox_plugin_host::format_install_hint(plugin_id, None)
            )
        })?;
    anyhow::ensure!(
        status.success(),
        "`vox plugin install {plugin_id} --yes` failed (exit {:?})",
        status.code()
    );
    Ok(())
}
