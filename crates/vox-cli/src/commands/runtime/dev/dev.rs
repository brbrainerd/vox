//! `vox dev` — compilerd-backed watch mode (same as [`crate::commands::dev`]).
//!
//! This module is for the extended `commands/runtime` tree; the shipping binary uses
//! [`crate::commands::dev`] directly.

use anyhow::Result;
use std::path::Path;

/// Start the dev server for `file`, binding to `port` (default 3000).
pub async fn run(
    file: &Path,
    out_dir: &Path,
    port: Option<u16>,
    open: bool,
    build_target: Option<crate::cli_args::BuildTargetArg>,
) -> Result<()> {
    crate::commands::dev::run(file, out_dir, port, open, build_target).await
}
