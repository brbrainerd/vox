//! `vox ci scientia-novelty-ledger-contracts` — validate example ledger JSON against v1 schemas.

use std::path::Path;

use anyhow::Result;

use vox_cli_contracts::scientia_ledger_contract::{
    example_finding_candidate_path, example_novelty_bundle_path, validate_finding_candidate_file,
    validate_novelty_bundle_file,
};

pub fn run(repo_root: &Path) -> Result<()> {
    let fc = example_finding_candidate_path(repo_root);
    let nb = example_novelty_bundle_path(repo_root);
    validate_finding_candidate_file(repo_root, &fc)?;
    validate_novelty_bundle_file(repo_root, &nb)?;
    println!("scientia-novelty-ledger-contracts OK (examples + schemas)");
    Ok(())
}
