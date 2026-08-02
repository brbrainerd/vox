//! `vox harness` — self-evaluation of the Vox agent harness (as opposed to
//! `vox model eval`, which scores *models*). See [`eval`].

use clap::{Parser, Subcommand};

pub mod eval;
pub mod live_eval;
pub mod publish;
pub mod report;

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
    /// List recent persisted harness eval runs (`vox harness history`).
    History(report::HistoryArgs),
    /// Compare the two most recent runs (or since a given run_id) and flag regressions
    /// (`vox harness report`).
    Report(report::ReportArgs),
}

pub async fn run(cmd: HarnessCmd) -> anyhow::Result<()> {
    match cmd {
        HarnessCmd::Eval(args) => eval::run(args).await,
        HarnessCmd::Publish(args) => publish::run(args).await,
        HarnessCmd::History(args) => report::run_history(args).await,
        HarnessCmd::Report(args) => report::run_report(args).await,
    }
}
