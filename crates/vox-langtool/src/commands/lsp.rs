//! `vox-langtool lsp` command runner.

use anyhow::Result;

/// Start the Language Server Protocol runner over stdio.
pub fn run() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            vox_lsp::server::run().await;
        });
    Ok(())
}
