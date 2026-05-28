//! Wire-format SSOT drift detector.
//!
//! Checks that `docs/src/architecture/wire-format-v1-ssot.md` has not been
//! modified without a corresponding update to the Contract IR implementation
//! (`vox_compiler::contract_ir`).
//!
//! **How the lock works:**
//! - [`EXPECTED_SSOT_HASH`] contains the blake3 hex digest of the SSOT doc as
//!   it stood when the Contract IR was last updated.
//! - [`check_ssot_drift`] re-hashes the live file and compares.
//! - If they differ, the SSOT changed without an IR update (or vice versa if
//!   someone updated the IR and forgot to run `--update`).
//!
//! **Updating the lock after a legitimate SSOT change:**
//! ```pwsh
//! cargo run -p vox-wire-format-validator -- --update
//! ```
//! Commit the resulting change to `src/expected_hash.rs`.

pub mod expected_hash;

use std::path::Path;

pub use expected_hash::EXPECTED_SSOT_HASH;

/// The repo-relative path to the wire-format SSOT document.
pub const SSOT_DOC_PATH: &str = "docs/src/architecture/wire-format-v1-ssot.md";

/// Diagnostic ID emitted on drift (stable, append-only per the diagnostic catalog).
pub const DRIFT_DIAGNOSTIC_ID: &str = "vox/wire-format/spec-drift";

/// Error returned when the SSOT has drifted from the expected hash.
#[derive(Debug)]
pub struct SpecDriftError {
    pub expected: String,
    pub actual: String,
    pub ssot_path: std::path::PathBuf,
}

impl std::fmt::Display for SpecDriftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] Wire-format SSOT has drifted.\n\
             File: {}\n\
             Expected blake3: {}\n\
             Actual blake3:   {}\n\
             \n\
             Either the SSOT was edited without updating the Contract IR, or the\n\
             Contract IR was updated without recording the new SSOT hash.\n\
             \n\
             To update the lock after a legitimate SSOT change:\n\
             \n  cargo run -p vox-wire-format-validator -- --update",
            DRIFT_DIAGNOSTIC_ID,
            self.ssot_path.display(),
            self.expected,
            self.actual,
        )
    }
}

impl std::error::Error for SpecDriftError {}

/// Check whether [`SSOT_DOC_PATH`] (resolved from `repo_root`) matches the
/// stored hash in [`EXPECTED_SSOT_HASH`].
///
/// Returns `Ok(())` if the hashes match, `Err(SpecDriftError)` if they diverge.
pub fn check_ssot_drift(repo_root: &Path) -> Result<(), SpecDriftError> {
    let ssot_path = repo_root.join(SSOT_DOC_PATH);
    let content = std::fs::read(&ssot_path).unwrap_or_else(|e| {
        panic!(
            "vox-wire-format-validator: cannot read SSOT doc at {}: {e}",
            ssot_path.display()
        )
    });
    let actual = blake3::hash(&content).to_hex().to_string();
    if actual != EXPECTED_SSOT_HASH {
        return Err(SpecDriftError {
            expected: EXPECTED_SSOT_HASH.to_string(),
            actual,
            ssot_path,
        });
    }
    Ok(())
}

/// Compute and return the blake3 hex digest of the SSOT doc at `repo_root`.
///
/// Used by `--update` mode and tests.
pub fn compute_ssot_hash(repo_root: &Path) -> anyhow::Result<String> {
    let ssot_path = repo_root.join(SSOT_DOC_PATH);
    let content = std::fs::read(&ssot_path)?;
    Ok(blake3::hash(&content).to_hex().to_string())
}
