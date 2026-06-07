//! Narrow codegen entrypoints (`vox emit …`) without running a full multi-target build.

use anyhow::Result;
use clap::Subcommand;

mod client;
mod openapi;

/// Subcommands under `vox emit`.
#[derive(Subcommand)]
pub enum EmitCmd {
    /// Emit Library-shaped TypeScript (`vox-client.ts`, types, schemas, `openapi.json`).
    Client(crate::cli_args::EmitClientArgs),
    /// Emit a standalone OpenAPI 3.1 JSON spec from a Vox source file.
    ///
    /// Produces a canonical `openapi.json` (or `--out` path) from the `@query`/`@mutation`/
    /// `@server` endpoints declared in the source. Wire-format follows the Vox
    /// wire-format-v1 SSOT: Decimal→string, DateTime→date-time, `Option<T>`→absent,
    /// sum types→oneOf+discriminator.
    Openapi(crate::cli_args::EmitOpenapiArgs),
}

pub async fn run(cmd: EmitCmd) -> Result<()> {
    match cmd {
        EmitCmd::Client(args) => client::run(&args).await,
        EmitCmd::Openapi(args) => openapi::run(&args).await,
    }
}
