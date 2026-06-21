//! `vox-discover` — on-demand local discovery + dedup. Advisory only; never
//! installs, executes, or publishes.

use std::path::PathBuf;

use clap::Parser;
use vox_plugin_types::skill_manifest::SkillManifest;
use vox_skill_discovery::{
    Candidate, DiscoverOptions, dedup_skills, mine_repeated_code, render_json, render_terminal,
    validate_ssot,
};

#[derive(Parser, Debug)]
#[command(
    name = "vox-discover",
    about = "Local skill/code discovery + dedup (advisory)"
)]
struct Args {
    /// Repository root to scan for repeated `.vox` code blocks.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Comma-separated sources: code,installed
    #[arg(long, default_value = "code")]
    source: String,

    /// Path to a JSON file containing `[SkillManifest, ...]` for installed-source checks.
    #[arg(long)]
    manifests: Option<PathBuf>,

    /// Output format: terminal | json
    #[arg(long, default_value = "terminal")]
    format: String,

    /// Minimum token count for a code block.
    #[arg(long, default_value_t = 40)]
    min_tokens: usize,

    /// Minimum occurrences for a code cluster.
    #[arg(long, default_value_t = 3)]
    min_occurrences: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let opts = DiscoverOptions {
        min_tokens: args.min_tokens,
        min_occurrences: args.min_occurrences,
        ..DiscoverOptions::default()
    };
    let sources: Vec<&str> = args.source.split(',').map(|s| s.trim()).collect();

    let mut candidates: Vec<Candidate> = Vec::new();

    if sources.contains(&"code") {
        candidates.extend(mine_repeated_code(&args.root, &opts));
    }

    if sources.contains(&"installed") {
        let manifests: Vec<SkillManifest> = match &args.manifests {
            Some(path) => {
                let raw = std::fs::read_to_string(path)?;
                serde_json::from_str(&raw)?
            }
            None => Vec::new(),
        };
        candidates.extend(dedup_skills(&manifests, &opts));
        candidates.extend(validate_ssot(&manifests));
    }

    let rendered = match args.format.as_str() {
        "json" => render_json(&candidates)?,
        _ => render_terminal(&candidates),
    };
    println!("{rendered}");
    Ok(())
}
