//! CAS-addressed SafeTensors bundles (`vox model cas`; Mn-T8).
//!
//! Hidden until listing/push/pull against `vox-package` artifact cache lands.

use std::path::PathBuf;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum CasCmd {
    /// List locally cached model bundles.
    Ls,
    /// Upload or register a bundle with the mesh CAS.
    Push {
        /// Directory or archive containing weights/tokenizer/config.
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// Fetch a bundle by lowercase SHA3-512 hex digest.
    Pull {
        #[arg(value_name = "SHA3_512_HEX")]
        digest_hex: String,
    },
}

const NOT_WIRED: &str = "vox model cas is not implemented yet (Mn-T8). Use vox-package cache paths and vox mens train/merge-qlora for model artifacts today.";

pub async fn run(cmd: CasCmd) -> anyhow::Result<()> {
    let _ = cmd;
    anyhow::bail!("{NOT_WIRED}")
}
