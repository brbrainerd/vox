//! `vox ci` — repository guard checks (SSOT, manifests, feature matrix) without shell/Python.

mod build_bench;
pub mod build_timings;
mod capability_sync;
mod command_compliance;
mod command_sync;
mod compile_matrix;
pub mod completion_quality;
pub mod config_aggregate;
pub mod config_gui_codegen;
pub mod config_hygiene;
pub mod config_registry_parity;
mod coolify_eval;
mod crate_build_map_parity;
pub mod data_storage_guard;
mod db_schema_coverage;
mod dep_cycles;
mod detect_rules_bench;
mod determinism_audit;
mod dev_loop_audit;
mod docs_reality_audit;
pub(super) mod doctor_build_cache;
mod eval_matrix;
mod exec_policy_contract;
mod generate_plugin_catalog_docs;
mod grammar_ssot_parity;
mod gui_catalog_parity;
mod gui_honesty;
mod gui_smoke;
mod gui_surface_coverage;
pub mod gui_surface_registry;
mod gui_version_sync;
mod job_timings;
mod mcp_vox_surface_parity;
mod mens_scorecard;
mod operations_catalog;
mod pipeline_parity;
mod plugin_abi_parity;
mod plugin_catalog_parity;
mod plugin_catalog_sync;
mod plugin_skill_parity;
mod plugin_surface;
mod policy_allowlist_parity;
mod policy_registry;
mod pre_push;
mod profile_parity;
mod providers;
mod release_build;
pub(crate) mod retired_symbol_check;
mod runner_scale;
mod scaling_audit;
mod scientia_heuristics_parity;
mod scientia_novelty_ledger_contract;
mod scientia_worthiness_contract;
mod speech_runtime_suite;
mod test_governance;
pub mod test_runtime_report;
pub mod watch_run;
pub mod workspace_artifacts;

mod constants;
mod coverage_gates;
pub(crate) mod run_body;

use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::Result;

pub use vox_cli_ci::cmd_enums::{
    CiCmd, CoolifyEvalCmd, CoverageGateMode, DocInventoryCmd, DocsRealityAuditCmd, EvalMatrixCmd,
    GovernanceGateMode, GrammarDriftEmit, MensScorecardCmd, OperationsSyncTarget, ScalingAuditCmd,
};

/// Resolve repository root: `VOX_REPO_ROOT`, else walk up from CWD for `AGENTS.md` + `Cargo.toml`.
pub fn repo_root() -> PathBuf {
    vox_repository::resolve_repo_root_for_ci()
}

pub(super) fn cargo_bin() -> PathBuf {
    if let Ok(h) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let win = PathBuf::from(&h).join(".cargo/bin/cargo.exe");
        if win.is_file() {
            return win;
        }
    }
    PathBuf::from("cargo")
}

/// `nvcc --version` using `CUDA_PATH`/`CUDA_HOME` when set (agent shells often lack full `PATH`).
fn nvcc_version_command() -> Command {
    let try_cuda_bin = |base: &str| -> Option<PathBuf> {
        let root = PathBuf::from(base);
        let exe = if cfg!(windows) {
            root.join("bin").join("nvcc.exe")
        } else {
            root.join("bin").join("nvcc")
        };
        exe.is_file().then_some(exe)
    };
    if let Ok(p) = std::env::var("CUDA_PATH").or_else(|_| std::env::var("CUDA_HOME")) {
        if let Some(exe) = try_cuda_bin(&p) {
            return Command::new(exe);
        }
    }
    Command::new("nvcc")
}

pub(super) fn nvcc_available() -> bool {
    nvcc_version_command()
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run `vox ci` subcommand.
pub async fn run(cmd: CiCmd) -> Result<()> {
    run_body::run(cmd).await
}
