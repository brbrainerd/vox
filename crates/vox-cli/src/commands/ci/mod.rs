//! `vox ci` — repository guard checks (SSOT, manifests, feature matrix) without shell/Python.

pub mod build_timings;
mod capability_sync;
mod command_compliance;
mod command_sync;
mod eval_matrix;
mod exec_policy_contract;
mod gui_catalog_parity;
mod gui_surface_coverage;
pub mod gui_surface_registry;
mod operations_catalog;
mod pipeline_parity;
mod policy_allowlist_parity;
mod policy_registry;
mod pre_push;
mod profile_parity;
mod providers;
mod release_build;
mod runner_scale;
pub mod workspace_artifacts;

pub(crate) mod run_body;

use anyhow::Result;

pub use vox_cli_ci::cmd_enums::{
    CiCmd, CoolifyEvalCmd, CoverageGateMode, DocInventoryCmd, DocsRealityAuditCmd, EvalMatrixCmd,
    GovernanceGateMode, GrammarDriftEmit, MensScorecardCmd, OperationsSyncTarget, ScalingAuditCmd,
};
// Shared ci helpers + constants moved to vox-cli-ci; re-exported so the guards still
// living here keep using `super::repo_root` / `super::constants` etc.
pub use vox_cli_ci::constants;
pub use vox_cli_ci::{cargo_bin, nvcc_available, nvcc_version_command, repo_root};

/// Run `vox ci` subcommand.
pub async fn run(cmd: CiCmd) -> Result<()> {
    run_body::run(cmd).await
}
