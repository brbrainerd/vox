//! `vox memory` — hybrid retrieval CLI (`memory search`).

use clap::Subcommand;

pub mod search;

#[derive(Subcommand)]
pub enum MemoryCmd {
    /// Run planner-backed hybrid retrieval (memory logs, VoxDb chunks, repo inventory).
    Search {
        /// Query tokens (joined with spaces).
        #[arg(required = true)]
        query: Vec<String>,
        /// Approximate max hits per corpus lane.
        #[arg(long, default_value_t = 16)]
        limit: usize,
    },
}

pub async fn run(cmd: MemoryCmd) -> anyhow::Result<()> {
    match cmd {
        MemoryCmd::Search { query, limit } => search::run(query, limit).await,
    }
}
