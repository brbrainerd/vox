//! `vox research mesh-intake` — promote SCIENTIA mesh intake files into the JSONL ledger.

use clap::Subcommand;

#[derive(Subcommand)]
pub enum MeshIntakeCmd {
    /// Validate pending intake JSON, append to `research-mesh-promoted/events.v1.jsonl`, move to `processed/`.
    Consume {
        /// Repository root (default: discover from cwd).
        #[arg(long)]
        repo: Option<std::path::PathBuf>,
    },
}

pub fn run(cmd: MeshIntakeCmd) -> anyhow::Result<()> {
    match cmd {
        MeshIntakeCmd::Consume { repo } => {
            let root = if let Some(p) = repo {
                p
            } else {
                let cwd = std::env::current_dir()?;
                vox_repository::discover_repository_or_fallback(&cwd).root
            };
            let summary = vox_publisher::research_mesh::consume_pending_intake(&root)?;
            println!("promoted: {}", summary.promoted);
            if !summary.errors.is_empty() {
                for err in &summary.errors {
                    eprintln!("error: {err}");
                }
            }
            Ok(())
        }
    }
}
