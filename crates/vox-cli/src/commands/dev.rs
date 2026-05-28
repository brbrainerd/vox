//! `vox dev` — watch mode backed by **`vox-compilerd`** (JSON-RPC over stdio).
//!
//! Spawns the daemon (sibling to `vox` or on `PATH`), sends a `dev` request, and streams
//! logs/progress until EOF or Ctrl+C. For an in-repo daemon, build `vox-compilerd` and ensure
//! it is next to `vox` or on `PATH`.

use anyhow::Result;
use std::path::Path;

/// Start dev/watch for `file`, writing artifacts under `out_dir`, binding `port` (default 3000).
///
/// Long-lived: runs until the daemon exits or the user presses Ctrl+C (forwards interrupt to child).
///
/// `--target=server` skips opening a browser by default (API-only loop).
pub async fn run(
    file: &Path,
    out_dir: &Path,
    port: Option<u16>,
    open: bool,
    build_target: Option<crate::cli_args::BuildTargetArg>,
) -> Result<()> {
    let open_browser =
        open && !matches!(build_target, Some(crate::cli_args::BuildTargetArg::Server));
    let mut params = serde_json::json!({
        "file": file.display().to_string(),
        "out_dir": out_dir.display().to_string(),
        "port": port.unwrap_or_else(crate::config::default_port),
        "open": open_browser,
    });
    if let Some(t) = build_target {
        params
            .as_object_mut()
            .expect("dev params object")
            .insert("target".into(), serde_json::to_value(t)?);
    }
    crate::dispatch::call_daemon_streaming("vox-compilerd", "dev", params, open_browser).await
}
