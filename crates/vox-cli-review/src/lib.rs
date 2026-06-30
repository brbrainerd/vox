//! `vox review coderabbit …` — GitHub CodeRabbit batch-PR review (semantic PR batches,
//! ingest, tasks), extracted from vox-cli. The whole crate is the `coderabbit` surface;
//! vox-cli depends on it optionally behind its own `coderabbit` feature.

pub mod coderabbit;

/// Dispatch `vox review …` (CodeRabbit).
pub async fn run(cli: ReviewCli) -> anyhow::Result<()> {
    match cli {
        ReviewCli::Coderabbit { action } => coderabbit::run(action).await,
    }
}

/// Top-level `vox review …` CLI.
#[derive(clap::Subcommand, Debug)]
pub enum ReviewCli {
    /// CodeRabbit: semantic PR batches, ingest, tasks (`vox review coderabbit …`).
    Coderabbit {
        #[command(subcommand)]
        action: coderabbit::CodeRabbitAction,
    },
}
