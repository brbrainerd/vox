//! `vox ci scientia-heuristics-parity` — fail if the SCIENTIA dynamics seed cannot be parsed.

use std::path::Path;

use anyhow::Result;

/// Validate `contracts/scientia/impact-readership-projection.seed.v1.yaml` parses into heuristics.
pub fn run(root: &Path) -> Result<()> {
    let _ = vox_publisher::scientia_heuristics::ScientiaHeuristics::try_load_from_repo_root(root)?;
    println!("scientia-heuristics-parity OK");
    Ok(())
}
