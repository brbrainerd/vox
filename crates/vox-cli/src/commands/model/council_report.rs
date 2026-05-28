//! `vox model council-report` — L3 of the model-autonomic system.
//!
//! Render the auto-generated council report. Reads the current registry
//! snapshot; future iterations will also query the telemetry sink to fill
//! in the cost/usage/promotion sections.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use vox_orchestrator::models::ModelRegistry;
use vox_orchestrator::models::autonomic::render_council_report;

#[derive(Args, Debug)]
pub struct CouncilReportArgs {
    /// Write the report to this file instead of stdout.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

pub async fn run(args: CouncilReportArgs) -> Result<()> {
    let registry = ModelRegistry::from_cache();
    let report = render_council_report(&registry);
    match args.out {
        Some(path) => {
            std::fs::write(&path, &report)?;
            eprintln!("Wrote council report to {}", path.display());
        }
        None => {
            print!("{report}");
        }
    }
    Ok(())
}
