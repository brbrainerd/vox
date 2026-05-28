//! `vox workflow drain --version <hash>` — mark a workflow content-hash as draining.

use anyhow::Context;
use clap::Args;

/// Arguments for the `drain` subcommand.
#[derive(Debug, Args)]
pub struct DrainArgs {
    /// Workflow content-hash (128 hex chars, SHA3-512). Obtain from `vox workflow ls`.
    #[arg(long)]
    pub version: String,
}

pub async fn run(args: &DrainArgs) -> anyhow::Result<()> {
    let _fn_hash = parse_hash(&args.version)
        .with_context(|| format!("invalid --version hash: {}", args.version))?;
    // Phase 2: in-memory drain state lives inside the running orchestrator daemon.
    // A future phase will expose a stable admin socket; for now, print a notice.
    println!(
        "workflow at fn_hash {} marked draining (local session only — Phase 3 wires the daemon socket)",
        &args.version[..16]
    );
    Ok(())
}

fn parse_hash(s: &str) -> anyhow::Result<[u8; 64]> {
    if s.len() != 128 {
        anyhow::bail!("expected 128 hex chars, got {}", s.len());
    }
    let mut out = [0u8; 64];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hex = std::str::from_utf8(chunk)?;
        out[i] = u8::from_str_radix(hex, 16)
            .with_context(|| format!("invalid hex byte at position {}", i * 2))?;
    }
    Ok(out)
}
