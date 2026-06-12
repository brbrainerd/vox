//! Scaling policy loaded from the repo SSOT [`contracts/scaling/policy.yaml`](../../contracts/scaling/policy.yaml).
//!
//! Use [`ScalingPolicy::embedded`] for stable defaults without I/O. For local overrides in tests,
//! use [`ScalingPolicy::from_yaml_str`].

use serde::Deserialize;

pub mod cost_defense;
mod policy_types;

pub use cost_defense::{
    CostCircuitBreaker, CostDefenseConfig, CostDefenseRejection, CostDefenseState,
};
pub use policy_types::{PathLiterals, PerCrateOverride, Thresholds};

/// Parse, edit, and pretty-print mesh `donations.vox` policy files (formerly `vox-mesh-policy`).
pub mod donations_vox;
/// Alias for [`donations_vox`] (former `vox-mesh-policy` crate surface).
pub mod mesh_policy {
    pub use super::donations_vox::*;
}
pub use donations_vox::{ParseError as DonationsVoxParseError, load_policy, pretty_print};

const EMBEDDED_YAML: &str = include_str!("../../../contracts/scaling/policy.yaml");

/// Repo-root-relative path to the scaling policy YAML SSOT (for docs, CLI messages, and tooling).
pub const SCALING_POLICY_YAML_REPO_PATH: &str = "contracts/scaling/policy.yaml";

/// Full policy document from SSOT YAML.
#[derive(Debug, Clone, Deserialize)]
pub struct ScalingPolicy {
    #[serde(default)]
    pub schema_version: u32,
    /// Human-readable baseline id (e.g. git tag or date).
    #[serde(default)]
    pub baseline_id: String,
    #[serde(default)]
    pub thresholds: Thresholds,
    #[serde(default)]
    pub path_literals: PathLiterals,
    #[serde(default)]
    pub magic_numeric_hints: Vec<u64>,
    #[serde(default)]
    pub per_crate_overrides: Vec<PerCrateOverride>,
}

impl ScalingPolicy {
    /// Policy embedded at build time from `contracts/scaling/policy.yaml`.
    pub fn embedded() -> Self {
        Self::from_yaml_str(EMBEDDED_YAML).expect("embedded scaling policy YAML must parse")
    }

    /// Parse policy from YAML (for tests or tooling).
    pub fn from_yaml_str(s: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(s)
    }
}

// ---------------------------------------------------------------------------
// Stable path constants (SSOT-backed; keep literals out of call sites)
// ---------------------------------------------------------------------------

/// Canonical default for Mens/Populi run artifact root (repo-relative).
pub const DEFAULT_MENS_RUNS_ROOT: &str = "mens/runs";

/// Latest symlink / pointer directory under [`DEFAULT_MENS_RUNS_ROOT`].
pub const DEFAULT_MENS_RUNS_LATEST: &str = "mens/runs/latest";

/// Default training run layout revision under [`DEFAULT_MENS_RUNS_ROOT`].
pub const DEFAULT_MENS_RUNS_V1: &str = "mens/runs/v1";

/// Default UV / tooling output subdirectory under runs.
pub const DEFAULT_MENS_RUNS_UV_OUTPUT: &str = "mens/runs/uv_output";

/// Default QLoRA run directory basename (repo-relative path prefix).
pub const DEFAULT_MENS_RUNS_QWEN_QLORA: &str = "mens/runs/qwen35_qlora";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_parses() {
        let p = ScalingPolicy::embedded();
        assert!(p.schema_version >= 1);
        assert!(!p.path_literals.mens_runs_variants.is_empty());
    }
}
