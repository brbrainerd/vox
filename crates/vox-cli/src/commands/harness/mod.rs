//! `vox harness` — self-evaluation of the Vox agent harness (as opposed to
//! `vox model eval`, which scores *models*). See [`eval`].

use clap::{Parser, Subcommand};

pub mod eval;
pub mod live_eval;
pub mod publish;

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
    /// Export new local harness_eval_* rows to a git-committed JSONL history file (and ingest
    /// any rows already published by others), so `git pull` is enough to sync results — see
    /// `publish` module docs.
    Publish(publish::PublishArgs),
}

pub async fn run(cmd: HarnessCmd) -> anyhow::Result<()> {
    match cmd {
        HarnessCmd::Eval(args) => eval::run(args).await,
        HarnessCmd::Publish(args) => publish::run(args).await,
    }
}
