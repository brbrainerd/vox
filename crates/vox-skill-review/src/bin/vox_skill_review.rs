//! `vox-skill-review` — local advisory pre-publish review of a candidate SKILL.md.

use std::path::PathBuf;

use clap::Parser;
use vox_plugin_types::skill_manifest::SkillManifest;
use vox_skill_review::{review_skill, ReviewReport};

#[derive(Parser, Debug)]
#[command(
    name = "vox-skill-review",
    about = "Local advisory pre-publish skill review (deterministic)"
)]
struct Args {
    /// Path to the candidate SKILL.md.
    #[arg(long)]
    skill: PathBuf,
    /// Optional JSON file: [SkillManifest, ...] of installed skills (for dedup).
    #[arg(long)]
    installed: Option<PathBuf>,
    /// Output: terminal | json
    #[arg(long, default_value = "terminal")]
    format: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let skill_md = std::fs::read_to_string(&args.skill)?;
    let installed: Vec<SkillManifest> = match &args.installed {
        Some(p) => serde_json::from_str(&std::fs::read_to_string(p)?)?,
        None => Vec::new(),
    };
    let report = review_skill(&skill_md, &installed);
    match args.format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
        _ => print_terminal(&report),
    }
    // Advisory: exit 0 always (gate-before-listing is the caller's policy decision).
    Ok(())
}

fn print_terminal(r: &ReviewReport) {
    println!("skill: {}  verdict: {:?}", r.skill_id, r.verdict);
    if !r.suggested_tags.is_empty() {
        println!("suggested tags: {}", r.suggested_tags.join(", "));
    }
    if r.items.is_empty() {
        println!("no findings.");
    }
    for it in &r.items {
        println!("  [{:?}] {} — {}", it.severity, it.rule, it.message);
    }
}
