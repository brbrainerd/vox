//! `vox-lsp` binary — Language Server Protocol frontend for Vox sources.

#[tokio::main]
async fn main() {
    vox_lsp::server::run().await;
}
