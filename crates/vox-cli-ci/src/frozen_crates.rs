//! `vox ci check-frozen` — legacy stub; frozen-core policy lives in SSOT contracts.

use anyhow::Result;
use std::path::Path;

pub fn check_frozen_crates(_root: &Path) -> Result<()> {
    // crates/_frozen.md was deleted (superseded by layers.toml and
    // contracts/db/data-storage-policy.v1.yaml frozen_core_crates list).
    // The canonical frozen-core set is in data-storage-policy.v1.yaml.
    println!(
        "check-frozen: crates/_frozen.md was superseded by layers.toml and contracts/db/data-storage-policy.v1.yaml. Nothing to enforce."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn check_frozen_is_noop_ok() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root");
        check_frozen_crates(root).expect("noop");
    }
}
