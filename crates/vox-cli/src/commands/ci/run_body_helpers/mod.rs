//! Helpers for `run_body` (`vox ci` SSOT / matrix guards), split from `include!` shards.

mod contracts;
mod corpus_decl_coverage;
mod cuda;
mod cuda_release_build;
mod data_ssot_guards;
mod docs;
mod grammar;
mod guards;
mod hash;
mod matrix;
pub(crate) mod mens;
mod orchestration_audit;
mod syntax_k;
mod timings;

pub(crate) use contracts::run_secrets_contracts;
pub(crate) use corpus_decl_coverage::run_corpus_decl_coverage;
pub(crate) use cuda::run_cuda_features;
pub(crate) use cuda_release_build::run_cuda_release_build;
pub(crate) use data_ssot_guards::run_data_ssot_guards;
pub(crate) use docs::{check_codex_ssot, check_docs_ssot, run_manifest, run_ssot_drift};
pub(crate) use grammar::{run_grammar_drift, run_grammar_export_check};
pub(crate) use guards::{
    TURSO_BUILTIN_CRATES, run_operator_env_guard, run_query_all_guard, run_repo_guards,
    run_secret_env_guard, run_secrets_cutover_audit, run_secrets_cutover_gates, run_secrets_parity,
    run_sql_surface_guard, run_turso_import_guard,
};
pub(crate) use matrix::{
    MensGateOpts, check_no_vox_dei, check_workflow_scripts, run_feature_matrix, run_mens_gate,
    run_script_hygiene, run_toestub_scoped_roots, run_toestub_self_apply,
};
pub(crate) use mens::{
    run_collateral_damage_gate, run_constrained_gen_smoke, run_grpo_reward_baseline,
    run_mens_corpus_health,
};
pub(crate) use orchestration_audit::run_ssot_audit;
pub(crate) use syntax_k::run_k_complexity_budget;
pub(crate) use timings::run_build_timings;
