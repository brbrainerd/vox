//! `vox workflow ls` — list known workflow hashes and their drain state.

use clap::Args;

/// Arguments for the `ls` subcommand.
#[derive(Debug, Args)]
pub struct LsArgs {
    /// Show only currently-draining workflows.
    #[arg(long)]
    pub draining: bool,
}

pub async fn run(_args: &LsArgs) -> anyhow::Result<()> {
    // Phase 2: the drain state is in-memory inside the daemon process.
    // Phase 3 will expose this via the admin socket.
    println!("fn_hash (first 16 chars)  state   name");
    println!("(no running orchestrator daemon connected — Phase 3 wires the admin socket)");
    Ok(())
}
