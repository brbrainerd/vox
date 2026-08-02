//! `vox harness` — self-evaluation of the Vox agent harness (as opposed to
//! `vox model eval`, which scores *models*). See [`eval`].

use clap::{Parser, Subcommand};

pub mod eval;

/// Evaluate the harness itself against a small golden task set (`vox harness`).
#[derive(Parser)]
pub struct HarnessArgs {
    #[command(subcommand)]
    pub cmd: HarnessCmd,
}

#[derive(Subcommand)]
pub enum HarnessCmd {
    /// Run the golden task set `--samples` times each and gate on pass^k
    /// (all samples must pass, not just one — see `eval::run`).
    Eval(eval::EvalArgs),
}

pub async fn run(cmd: HarnessCmd) -> anyhow::Result<()> {
    match cmd {
        HarnessCmd::Eval(args) => eval::run(args).await,
    }
}
