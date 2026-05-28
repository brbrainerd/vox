//! `vox workflow` — workflow introspection and operational commands (P1-T8, P2-T3).

pub mod drain;
pub mod ls;
pub mod preview;

#[derive(clap::Subcommand, Debug)]
pub enum WorkflowCmd {
    /// Mark a workflow content-hash as "no new starts"; in-flight runs continue.
    Drain(drain::DrainArgs),
    /// List known workflow content-hashes and their drain state.
    Ls(ls::LsArgs),
    /// Project the schedule of activities a workflow would dispatch without running it.
    Preview(preview::WorkflowPreviewArgs),
}

pub async fn run(cmd: WorkflowCmd) -> anyhow::Result<()> {
    match cmd {
        WorkflowCmd::Drain(args) => drain::run(&args).await,
        WorkflowCmd::Ls(args) => ls::run(&args).await,
        WorkflowCmd::Preview(args) => preview::run(&args).await,
    }
}
