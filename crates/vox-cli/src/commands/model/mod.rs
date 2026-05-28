//! Subcommands for managing models, discovery, and orchestration health (`vox model`).

use clap::{Parser, Subcommand};

pub mod cas;
pub mod classify;
pub mod costs;
pub mod council_report;
pub mod discover;
pub mod explain;
pub mod list;
pub mod preferences;
pub mod pricing;
pub mod rollup;
pub mod scoreboard;
pub mod show;

/// Manage models: discovery, scoreboard, and explainability.
#[derive(Parser)]
pub struct ModelArgs {
    #[command(subcommand)]
    pub cmd: ModelCmd,
}

#[derive(Subcommand)]
pub enum ModelCmd {
    /// Mesh CAS model bundles (SafeTensors; Mn-T8).
    Cas {
        #[command(subcommand)]
        cmd: cas::CasCmd,
    },
    /// Refresh the model catalog from all sources (discovery).
    Discover(discover::DiscoverArgs),
    /// Perform batch aggregation of telemetry into the scoreboard.
    Rollup(rollup::RollupArgs),
    /// Show the model scoreboard (latency, success rate, cost).
    Scoreboard(scoreboard::ScoreboardArgs),
    /// List model ids from the local catalog cache.
    List(list::ListArgs),
    /// Show JSON for one model from the cache.
    Show(show::ShowArgs),
    /// Set or reset Clavis capability routing preferences.
    Preferences(preferences::PreferencesArgs),
    /// Explain model selection for a given task description.
    Explain(explain::ExplainArgs),
    /// Show detailed cost reporting.
    Costs(costs::CostsArgs),
    /// Manage and view observed pricing SSOT.
    Pricing(pricing::PricingArgs),
    /// L2 — Build/inspect the classifier prompt for a model id (autonomic system).
    Classify(classify::ClassifyArgs),
    /// L3 — Render the quarterly model-council report (autonomic system).
    CouncilReport(council_report::CouncilReportArgs),
}

pub async fn run(cmd: ModelCmd) -> anyhow::Result<()> {
    match cmd {
        ModelCmd::Cas { cmd } => cas::run(cmd).await,
        ModelCmd::Discover(args) => discover::run(args).await,
        ModelCmd::Rollup(args) => rollup::run(args).await,
        ModelCmd::Scoreboard(args) => scoreboard::run(args).await,
        ModelCmd::List(args) => list::run(args).await,
        ModelCmd::Show(args) => show::run(args).await,
        ModelCmd::Preferences(args) => preferences::run(args).await,
        ModelCmd::Explain(args) => explain::run(args).await,
        ModelCmd::Costs(args) => costs::run(args).await,
        ModelCmd::Pricing(args) => pricing::run(args).await,
        ModelCmd::Classify(args) => classify::run(args).await,
        ModelCmd::CouncilReport(args) => council_report::run(args).await,
    }
}
