//! Repository CI guard checks extracted from `vox-cli` (`vox ci *` implementation wedge).

/// Seam for the Tier-2 `vox ci` guards that stay in vox-cli because they reach into
/// its internals (`command_catalog`, `command_registry_model`, `utils`, `fs_utils`,
/// `commands::runtime`). The moved dispatcher (`run::run`) calls `dispatch_heavy(&cmd)`
/// first; vox-cli implements it on `VoxCliProviders`. Extends `GateStatusWriter` so the
/// dispatcher can also record per-gate status through the same host object.
pub trait HeavyGuardHost: vox_cli_contracts::GateStatusWriter {
    /// Handle `cmd` if it is a Tier-2 guard: `Some(result)`. `None` ⇒ the dispatcher
    /// runs it as a Tier-1 (moved) guard.
    fn dispatch_heavy(
        &self,
        cmd: &cmd_enums::CiCmd,
        root: &std::path::Path,
    ) -> Option<anyhow::Result<()>>;
}

pub mod affected;
pub mod benchmark_telemetry;
pub mod build_bench;
pub mod build_timings;
pub mod completion_quality;
pub mod constants;
pub mod coolify_eval;
pub mod corpus_integrity;
pub mod coverage_gates;
pub mod crate_edges;
pub mod data_storage_guard;
pub mod dep_cycles;
pub mod determinism_audit;
pub mod grammar_ssot_parity;
pub mod gui_smoke;
pub mod helpers;
pub mod job_timings;
pub mod mcp_vox_surface_parity;
pub mod mens_scorecard;
pub mod retired_symbol_check;
pub mod scaling_audit;
pub mod scientia_novelty_ledger_contract;
pub mod speech_runtime_suite;
pub mod version_ssot;
pub mod watch_run;
pub use helpers::{cargo_bin, nvcc_available, nvcc_version_command, repo_root};
pub mod affected_cmd;
pub mod agentskills_compliance;
pub mod ai_fixtures_coverage;
pub mod attention_ledger_parity;
pub mod attention_parity;
pub mod canonical_docs;
pub mod capability_snapshot;
pub mod check_links;
pub mod cmd_enums;
pub mod commit_lint;
pub mod compile_matrix;
pub mod config_aggregate;
pub mod config_gui_codegen;
pub mod config_hygiene;
pub mod config_registry_parity;
pub mod contracts_index;
pub mod crate_budget;
pub mod crate_build_map_parity;
pub mod db_schema_coverage;
pub mod dep_sprawl;
pub mod deploy_status;
pub mod detect_rules_bench;
pub mod dev_loop_audit;
pub mod docs_deprecated_command_guard;
pub mod docs_reality_audit;
pub mod doctest_md;
pub mod doctor_build_cache;
pub mod fan_in_budget;
pub mod free_binary;
pub mod frozen_crates;
pub mod generate_plugin_catalog_docs;
pub mod gui_honesty;
pub mod gui_version_sync;
pub mod gui_visual_review;
pub mod harness_trust_guard;
pub mod install_hooks;
pub mod kill_stuck_tests;
pub mod line_endings;
pub mod model_routing_check;
pub mod no_plugin_cdylib_as_compile_dep;
pub mod no_tauri_in_core;
pub mod nomenclature_guard;
pub mod openclaw_contract;
pub mod package_manifests;
pub mod parse_check;
pub mod parse_status;
pub mod plugin_abi_parity;
pub mod plugin_catalog_parity;
pub mod plugin_catalog_sync;
pub mod plugin_dep_boundary;
pub mod plugin_skill_parity;
pub mod plugin_surface;
pub mod pm_provenance;
pub mod profile_parity;
pub mod release_draft_guard;
pub mod retirement_audit;
pub mod row_serde_lint;
pub mod runner_policy_check;
pub mod safety_inventory;
pub mod scientia_heuristics_parity;
pub mod scientia_worthiness_contract;
pub mod string_id_lint;
pub mod sync_ignore_files;
pub mod test_governance;
pub mod test_inventory;
pub mod test_runtime_report;
pub mod tier_budget_check;
pub mod toestub_budget;
pub mod toolchain_ssot;
pub mod workflow_concurrency_guard;
pub mod workflow_permissions_guard;
