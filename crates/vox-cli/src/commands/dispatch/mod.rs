//! `vox dispatch` — dispatch-time routing tools (P2-T6).

pub mod preview;

#[derive(clap::Subcommand, Debug)]
pub enum DispatchCmd {
    /// Project the routing decision tree for a workflow without dispatching.
    Preview(preview::PreviewArgs),
}

pub async fn run(cmd: DispatchCmd) -> anyhow::Result<()> {
    match cmd {
        DispatchCmd::Preview(args) => preview::run(args).await,
    }
}
