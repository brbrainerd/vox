//! CAS-addressed SafeTensors bundles (`vox model cas`; Mn-T8).
//!
//! Listing/push/pull against `vox-package` artifact cache will land with full CAS indexing.

use std::path::PathBuf;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum CasCmd {
    /// List locally cached model bundles (stub — prints guidance until CAS indexer lands).
    Ls,
    /// Upload or register a bundle with the mesh CAS (not wired yet).
    Push {
        /// Directory or archive containing weights/tokenizer/config.
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// Fetch a bundle by lowercase SHA3-512 hex digest (not wired yet).
    Pull {
        #[arg(value_name = "SHA3_512_HEX")]
        digest_hex: String,
    },
}

pub async fn run(cmd: CasCmd) -> anyhow::Result<()> {
    match cmd {
        CasCmd::Ls => {
            // Keep `vox-inference` / `vox-mens-eval` in the CLI dependency graph for arch-check
            // until CAS listing and eval harness callsites are fully wired.
            let _ = vox_eval::mens::summarize_placeholder();
            let _ = std::sync::Arc::new(vox_populi::inference::CandleCpuStub);
            println!(
                "vox model cas ls: no bundle index yet — see Mn-T8 / vox-package model CAS helpers."
            );
            Ok(())
        }
        CasCmd::Push { path } => anyhow::bail!(
            "vox model cas push {:?}: not implemented — bundle indexer pending",
            path
        ),
        CasCmd::Pull { digest_hex } => anyhow::bail!(
            "vox model cas pull {}: not implemented — fetch-by-hash pending",
            digest_hex
        ),
    }
}
