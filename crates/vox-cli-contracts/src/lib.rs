//! Trait seam that inverts vox-cli's CI coupling: vox-cli implements these traits
//! (`VoxCliProviders`) and vox-cli-ci consumes them, so the CI subsystem can move out
//! without depending back on vox-cli. Also home to the shared check-manifest types.
//!
//! Keep this crate's *code* sync — it uses no async runtime directly, so it stays a
//! cheap leaf to compile. (The `cargo tree -i tokio` graph is not empty because the
//! workspace-hack feature-unification crate pulls tokio into every member's tree; that
//! is a graph artifact, not a real dependency of this crate's own compilation.)

use std::path::Path;

use serde::Deserialize;

/// Finding-candidate / novelty-bundle schema validation (`vox scientia` + the ci
/// scientia-ledger gate). Lives here because it is shared by a non-ci command and ci.
pub mod scientia_ledger_contract;

// ---------------------------------------------------------------------------
// Shared manifest types (moved from vox-cli `commands::audit`)
// ---------------------------------------------------------------------------

/// Top-level structure of `contracts/ci/check-targets.v1.yaml`.
#[derive(Debug, Deserialize)]
pub struct CheckManifest {
    pub schema_version: u32,
    pub checks: Vec<CheckEntry>,
}

/// One quality-gate entry in the manifest.
#[derive(Debug, Deserialize, Clone)]
pub struct CheckEntry {
    pub id: String,
    pub description: String,
    pub category: String,
    pub blocking: bool,
    pub runs_on: Vec<String>,
    #[serde(default)]
    pub rust_only: bool,
    pub command: Vec<String>,
    /// When `true` this check is skipped by `--quick`.
    #[serde(default)]
    pub quick_skip: bool,
}

// ---------------------------------------------------------------------------
// Provider traits (vox-cli impls; vox-cli-ci consumes)
// ---------------------------------------------------------------------------

/// Audit seam: enumerate `contracts/ci/check-targets.v1.yaml` into policy entries.
/// Replaces ci's direct `crate::commands::audit::CheckManifest` deserialization.
pub trait CheckProvider {
    fn load_check_targets(&self, repo_root: &Path) -> anyhow::Result<Vec<vox_config::PolicyEntry>>;
}

/// Policy seam: write gate-run status (VCS identity + results). `ran_at` stays
/// caller-supplied — the single non-deterministic input, kept out of the impl.
pub trait GateStatusWriter {
    fn current_branch(&self, repo_root: &Path) -> String;
    fn head_commit(&self, repo_root: &Path) -> String;
    fn write_results(
        &self,
        repo_root: &Path,
        branch: &str,
        commit: &str,
        ran_at: &str,
        results: Vec<vox_config::PolicyResult>,
    ) -> anyhow::Result<()>;
}

/// Runtime seam: terminal-policy validation for `vox ci`. The `check_terminal`
/// primitives stay in vox-cli behind this impl; the orchestration moves to vox-cli-ci.
pub trait TerminalPolicyValidator {
    fn default_policy_rel(&self) -> &'static str;
    fn validate_policy_file(&self, repo_root: &Path, policy: &Path) -> anyhow::Result<()>;
    fn run_check_for_ci(&self, payload: &str, policy: Option<&Path>) -> anyhow::Result<()>;
}
