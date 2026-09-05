//! SSOT / matrix guard implementations for `vox ci`.

use anyhow::{Result, anyhow};
use std::process::Command;

use super::command_compliance;
use super::command_sync;
use super::eval_matrix;
use super::exec_policy_contract;
use super::release_build;
use super::{cargo_bin, repo_root};
use vox_cli_ci::canonical_docs;
use vox_cli_ci::check_links;
use vox_cli_ci::cmd_enums::{
    CiCmd, DocInventoryCmd, DocsRealityAuditCmd, EvalMatrixCmd, MensScorecardCmd,
    OperationsSyncTarget,
};
use vox_cli_ci::completion_quality;
use vox_cli_ci::contracts_index;
use vox_cli_ci::coverage_gates;
use vox_cli_ci::determinism_audit;
use vox_cli_ci::doctest_md;
use vox_cli_ci::grammar_ssot_parity;
use vox_cli_ci::mens_scorecard;
use vox_cli_ci::parse_status;
use vox_cli_ci::scaling_audit;
use vox_cli_ci::scientia_heuristics_parity;
use vox_cli_ci::scientia_novelty_ledger_contract;
use vox_cli_ci::scientia_worthiness_contract;

/// Helpers live in `ci/run_body_helpers/`; `#[path]` keeps them out of `ci/run_body/` (submodule rule).
#[path = "run_body_helpers/mod.rs"]
pub(crate) mod run_body_helpers;

use run_body_helpers::{
    MensGateOpts, check_codex_ssot, check_docs_ssot, check_no_vox_dei, check_workflow_scripts,
    run_build_timings, run_collateral_damage_gate, run_constrained_gen_smoke,
    run_corpus_decl_coverage, run_cuda_features, run_cuda_release_build, run_data_ssot_guards,
    run_feature_matrix, run_grammar_drift, run_grammar_export_check, run_grpo_reward_baseline,
    run_k_complexity_budget, run_manifest, run_mens_corpus_health, run_mens_gate,
    run_operator_env_guard, run_query_all_guard, run_repo_guards, run_script_hygiene,
    run_secret_env_guard, run_secrets_contracts, run_secrets_cutover_audit,
    run_secrets_cutover_gates, run_secrets_parity, run_source_token_budget, run_spoke_check,
    run_sql_surface_guard, run_ssot_audit, run_ssot_drift, run_toestub_scoped_roots,
    run_toestub_self_apply, run_turso_import_guard,
};

use vox_cli_ci::retired_symbol_check;

/// Whether a `vox ci` subcommand must pass the stale-binary freshness guard.
///
/// `false` for infra reconcile/read commands (runner autoscaler/preflight/status):
/// they carry no correctness verdict and must keep the CI fleet alive even when the
/// installed binary lags a fast-moving source tree.
fn should_enforce_freshness(cmd: &CiCmd) -> bool {
    !matches!(
        cmd,
        CiCmd::RunnerScale { .. } | CiCmd::RunnerPreflight | CiCmd::RunnerStatus
    )
}

/// Run `vox ci` subcommand.
pub async fn run(cmd: CiCmd) -> Result<()> {
    let root = repo_root();
    // A stale `vox` binary runs outdated guard logic/allowlists, so its `vox ci`
    // verdict would not reflect the current source. Refuse rather than mislead.
    //
    // EXCEPTION: the runner autoscaler/preflight/status are infra reconcile + read
    // commands, not guard verdicts — they spawn/inspect Docker CI runners and produce
    // no correctness judgement. Gating them on freshness lets a fast-moving source tree
    // (multiple agents racing ahead of the installed binary) starve the CI fleet to zero
    // every tick. They must run regardless of binary staleness.
    if should_enforce_freshness(&cmd) {
        crate::freshness::enforce_for_ci(&root)?;
    }

    // Per-gate status capture (Phase 1c). Only registry-backed gates are tracked;
    // others record nothing (honest grey). Disabled via VOX_NO_POLICY_STATUS=1.
    let gate_id = cmd.gate_policy_id();
    let started = std::time::Instant::now();

    let result: Result<()> = match cmd {
        CiCmd::BuildCacheDoctor => vox_cli_ci::doctor_build_cache::run(),
        CiCmd::Manifest => run_manifest(&root),
        CiCmd::PolicyRegistry { write } => {
            super::policy_registry::run_generate(&root, write).map_err(|e| anyhow!(e))
        }
        CiCmd::PolicyRegistryParity => {
            super::policy_registry::run_parity(&root).map_err(|e| anyhow!(e))
        }
        CiCmd::ConfigHygiene {
            update_baseline,
            write,
        } => {
            if write {
                vox_cli_ci::config_hygiene::write_registry(
                    vox_cli_ci::config_hygiene::WriteRegistryOpts { root: root.clone() },
                )
            } else {
                vox_cli_ci::config_hygiene::run(update_baseline)
            }
        }
        CiCmd::ConfigRegistryParity { update_baseline } => {
            vox_cli_ci::config_registry_parity::run(update_baseline)
        }
        CiCmd::ConfigGuiCodegen { check, fields } => {
            if fields {
                vox_cli_ci::config_gui_codegen::run_fields(check)
            } else {
                vox_cli_ci::config_gui_codegen::run(check)
            }
        }
        CiCmd::CheckDocsSsot => check_docs_ssot(&root),
        CiCmd::CheckFrozen => vox_cli_ci::frozen_crates::check_frozen_crates(&root),
        CiCmd::GuiCatalogParity => super::gui_catalog_parity::run(&root),
        CiCmd::GuiVersionSync { write } => vox_cli_ci::gui_version_sync::run(&root, write),
        CiCmd::GuiSurfaceCoverage { write } => super::gui_surface_coverage::run(&root, write),
        CiCmd::GuiSurfaceRegistry { write } => super::gui_surface_registry::run(&root, write),
        CiCmd::GuiHonesty => vox_cli_ci::gui_honesty::run(&root),
        CiCmd::HarnessTrustGuard => {
            // Check 1 (args.get("user_approval")) lives in the always-on
            // retired-symbol scan (T0.1); run it first so a single `vox ci
            // harness-trust-guard` invocation covers the full T2.4
            // checklist without vox-cli-ci duplicating that pattern (see
            // crates/vox-cli-ci/src/harness_trust_guard.rs's module doc).
            retired_symbol_check::run(&root)?;
            vox_cli_ci::harness_trust_guard::run(&root)
        }
        CiCmd::ModelRoutingCheck => vox_cli_ci::model_routing_check::run(&root),
        CiCmd::CheckCodexSsot => check_codex_ssot(&root),
        CiCmd::ContractsIndex => contracts_index::run(&root),
        CiCmd::AiFixturesCoverage => vox_cli_ci::ai_fixtures_coverage::run(&root),
        CiCmd::ExecPolicyContract => exec_policy_contract::run(&root),
        CiCmd::OpenClawContract => vox_cli_ci::openclaw_contract::run(&root),
        CiCmd::OperationsVerify => super::operations_catalog::verify(&root),
        CiCmd::OperationsSync { target, write } => {
            let target = match target {
                OperationsSyncTarget::Catalog => "catalog",
                OperationsSyncTarget::Mcp => "mcp",
                OperationsSyncTarget::Cli => "cli",
                OperationsSyncTarget::Capability => "capability",
                OperationsSyncTarget::All => "all",
            };
            super::operations_catalog::sync(&root, target, write)
        }
        CiCmd::ScientiaWorthinessContract => scientia_worthiness_contract::run(&root),
        CiCmd::ScientiaHeuristicsParity => scientia_heuristics_parity::run(&root),
        CiCmd::ScientiaNoveltyLedgerContracts => scientia_novelty_ledger_contract::run(&root),
        CiCmd::SpeechRuntimeSuite {
            run_id,
            limit,
            eval_manifest,
            plugins_dir,
            skip_runtime,
        } => vox_cli_ci::speech_runtime_suite::run(
            &root,
            vox_cli_ci::speech_runtime_suite::SpeechRuntimeSuiteOpts {
                run_id,
                limit,
                eval_manifest,
                plugins_dir,
                skip_runtime,
            },
        ),
        CiCmd::SsotDrift => run_ssot_drift(&root),
        CiCmd::PrePush {
            quick,
            complete,
            full,
            dry_run,
            act,
            report_json,
            include_slow,
            with_coverage,
            since,
            enforce_budgets,
            skip_complete,
        } => super::pre_push::run(
            &root,
            super::pre_push::PrePushOpts {
                quick,
                complete,
                full,
                dry_run,
                act,
                report_json,
                include_slow,
                with_coverage,
                since,
                enforce_budgets,
                skip_complete,
            },
        ),
        CiCmd::TierBudgetCheck { junit, profile } => {
            vox_cli_ci::tier_budget_check::run(&root, &junit, &profile)
        }
        CiCmd::DevLoopAudit { json } => vox_cli_ci::dev_loop_audit::run(&root, json),
        CiCmd::SsotAudit => run_ssot_audit(&root).await,
        CiCmd::DataSsotGuards => run_data_ssot_guards(&root),
        CiCmd::DataStorageGuard(opts) => {
            let report = vox_cli_ci::data_storage_guard::run(&opts)?;
            if opts.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            if !report.violations.is_empty() {
                anyhow::bail!(
                    "DataStorageGuard failed with {} violations",
                    report.violations.len()
                );
            }
            Ok(())
        }
        CiCmd::FeatureMatrix => run_feature_matrix(&root),
        CiCmd::CompileMatrix => vox_cli_ci::compile_matrix::run(&root),
        CiCmd::RetirementAudit => vox_cli_ci::retirement_audit::run(&root),
        CiCmd::NoDeiImport => check_no_vox_dei(&root),
        CiCmd::AttentionEventLedgerParity => vox_cli_ci::attention_ledger_parity::run(&root),
        CiCmd::CheckSummaryDrift => {
            let cargo = cargo_bin();
            let st = Command::new(&cargo)
                .current_dir(&root)
                .args(["run", "-p", "vox-doc-pipeline", "--", "--check"])
                .status()?;
            if !st.success() {
                // Must EVALUATE to Err (not early-return) so the per-gate status
                // wrapper below records Fail; a bare `return` would leave a stale Pass.
                Err(anyhow!(
                    "SUMMARY.md is out of sync with docs/src. Run 'cargo run -p vox-doc-pipeline' to fix."
                ))
            } else {
                println!("SUMMARY.md is up to date.");
                Ok(())
            }
        }
        CiCmd::BuildDocs => {
            let cargo = cargo_bin();
            // 1. Generate SUMMARY.md
            let st = Command::new(&cargo)
                .current_dir(&root)
                .args(["run", "-p", "vox-doc-pipeline"])
                .status()?;
            if !st.success() {
                return Err(anyhow!("failed to generate SUMMARY.md"));
            }
            // 2. Run Astro build
            let docs_dir = root.join("docs-astro");
            let pnpm = crate::frontend::pnpm_executable();

            let st = Command::new(pnpm)
                .current_dir(&docs_dir)
                .args(["install", "--frozen-lockfile"])
                .status()?;
            if !st.success() {
                return Err(anyhow!("Astro pnpm install failed"));
            }

            let st = Command::new(pnpm)
                .current_dir(&docs_dir)
                .args(["run", "build"])
                .status()?;
            if !st.success() {
                return Err(anyhow!("Astro build docs failed"));
            }
            println!("Documentation built successfully.");
            Ok(())
        }
        CiCmd::DocInventory { cmd: sub } => match sub {
            DocInventoryCmd::Generate { output } => {
                let out =
                    output.unwrap_or_else(|| root.join(vox_doc_inventory::DEFAULT_INVENTORY_PATH));
                vox_doc_inventory::generate(&root, &out)?;
                println!("Wrote {}", out.display());
                Ok(())
            }
            DocInventoryCmd::Verify => {
                let committed = root.join(vox_doc_inventory::DEFAULT_INVENTORY_PATH);
                vox_doc_inventory::verify_fresh(&root, &committed)?;
                println!("doc-inventory.json matches generator output (excluding generated_at)");
                Ok(())
            }
        },
        CiCmd::DocsRealityAudit { cmd: sub } => match sub {
            DocsRealityAuditCmd::Verify => vox_cli_ci::docs_reality_audit::run_verify(&root),
            DocsRealityAuditCmd::Metrics { write } => {
                vox_cli_ci::docs_reality_audit::run_metrics(&root, write)
            }
        },
        CiCmd::EvalMatrix { cmd: sub } => match sub {
            EvalMatrixCmd::Verify => eval_matrix::run_verify(&root),
            EvalMatrixCmd::Run { milestone } => {
                eval_matrix::run_executions(&root, milestone.as_deref())
            }
        },
        CiCmd::MensScorecard { cmd: sub } => match sub {
            MensScorecardCmd::Verify { spec } => mens_scorecard::run_verify(&root, &spec),
            MensScorecardCmd::Run { spec, out_dir } => {
                mens_scorecard::run_execute(&root, &spec, out_dir.as_deref()).await
            }
            MensScorecardCmd::Decide { summaries, json } => {
                mens_scorecard::run_decide(&root, &summaries, json)
            }
            MensScorecardCmd::BurnRnd {
                qlora_summary,
                burn_summary,
                json,
            } => mens_scorecard::run_burn_rnd(&root, &qlora_summary, burn_summary.as_deref(), json),
            MensScorecardCmd::IngestTrust { summary } => {
                mens_scorecard::run_ingest_trust(&root, &summary).await
            }
        },
        CiCmd::WorkflowScripts { allowlist } => check_workflow_scripts(&root, &allowlist),
        CiCmd::CommitLint { base } => {
            let violations = vox_cli_ci::commit_lint::run(&root, &base)?;
            if !violations.is_empty() {
                for v in &violations {
                    eprintln!(
                        "ERROR: Commit {} violates policy!\nSummary: {}\nReason: {}\n",
                        v.commit, v.summary, v.reason
                    );
                }
                anyhow::bail!("commit-lint failed with {} violation(s)", violations.len());
            }
            println!("commit-lint passed.");
            Ok(())
        }
        CiCmd::FmtCheck => super::pre_push::check_fmt(&root),
        CiCmd::RunnerPolicyCheck { strict } => vox_cli_ci::runner_policy_check::run(&root, strict),
        CiCmd::WorkflowConcurrencyGuard { strict } => {
            vox_cli_ci::workflow_concurrency_guard::run(&root, strict)
        }
        CiCmd::ReleaseDraftGuard => vox_cli_ci::release_draft_guard::run(&root),
        CiCmd::GuiVisualReview { no_ai } => vox_cli_ci::gui_visual_review::run(&root, no_ai),
        CiCmd::LineEndings { all, base, autofix } => {
            vox_cli_ci::line_endings::run(&root, all, base, autofix)
        }
        CiCmd::BomCheck => vox_cli_ci::line_endings::check_bom(&root),
        CiCmd::SpokeCheck => run_spoke_check(&root),
        CiCmd::FreeBinary { target, apply } => vox_cli_ci::free_binary::run(&root, target, apply),
        CiCmd::ParseStatus { write } => parse_status::run(&root, write),
        CiCmd::MeshGate {
            profile,
            isolated_runner,
            windows_isolated_runner,
            gate_build_target_dir,
            gate_log_file,
        } => run_mens_gate(
            &root,
            &profile,
            &MensGateOpts {
                isolated_runner: isolated_runner || windows_isolated_runner,
                gate_build_target_dir,
                gate_log_file,
            },
        ),
        CiCmd::CudaReleaseBuild { log_dir } => run_cuda_release_build(&root, log_dir),
        CiCmd::DetectRulesBench {
            rules,
            fixtures_root,
            min_f1,
            json,
        } => vox_cli_ci::detect_rules_bench::run(&rules, &fixtures_root, min_f1, json),
        CiCmd::ToestubBudget => vox_cli_ci::toestub_budget::run(),
        CiCmd::JsonParseCheck { globs } => vox_cli_ci::parse_check::run_json(&globs),
        CiCmd::YamlParseCheck { globs } => vox_cli_ci::parse_check::run_yaml(&globs),
        CiCmd::VoxParseCheck { globs } => vox_cli_ci::parse_check::run_vox(&globs),
        CiCmd::ToestubSelfApply => run_toestub_self_apply(&root),
        CiCmd::ToestubScoped { roots, mode } => run_toestub_scoped_roots(&root, &roots, mode),
        CiCmd::ScalingAudit { cmd } => scaling_audit::run(&root, cmd),
        CiCmd::CudaFeatures => run_cuda_features(),
        CiCmd::BuildTimings {
            json,
            crates,
            deep,
            persist,
            name,
            profile,
        } => {
            if deep {
                vox_cli_ci::build_timings::bench_build_run(
                    persist.unwrap_or(true),
                    name,
                    Some(profile),
                )
                .await?;
                Ok(())
            } else {
                run_build_timings(&root, json, crates)
            }
        }
        CiCmd::GrammarDrift { emit } => run_grammar_drift(&root, emit),
        CiCmd::GrammarSsotParity => grammar_ssot_parity::run().await,
        CiCmd::PipelineParity => super::pipeline_parity::run(&root).await,
        CiCmd::KComplexityBudget {
            tolerance_percent,
            update,
        } => run_k_complexity_budget(&root, tolerance_percent, update),
        CiCmd::SourceTokenBudget {
            tolerance_percent,
            update,
        } => run_source_token_budget(&root, tolerance_percent, update),
        CiCmd::GrammarExportCheck => run_grammar_export_check(&root),
        CiCmd::CorpusDeclCoverage => run_corpus_decl_coverage(&root),
        CiCmd::RepoGuards => run_repo_guards(&root),
        CiCmd::SecretEnvGuard { all } => run_secret_env_guard(&root, all),
        CiCmd::OperatorEnvGuard { all } => run_operator_env_guard(&root, all),
        CiCmd::MensCorpusHealth {
            min_pairs,
            min_human_ratio,
        } => run_mens_corpus_health(&root, min_pairs, min_human_ratio).await,
        CiCmd::GrpoRewardBaseline => run_grpo_reward_baseline(&root).await,
        CiCmd::CollateralDamageGate { max_damage_rate } => {
            run_collateral_damage_gate(&root, max_damage_rate).await
        }
        CiCmd::ConstrainedGenSmoke { n_samples } => {
            run_constrained_gen_smoke(&root, n_samples).await
        }
        CiCmd::SqlSurfaceGuard { all } => run_sql_surface_guard(&root, all),
        CiCmd::QueryAllGuard { all } => run_query_all_guard(&root, all),
        CiCmd::TursoImportGuard { all } => run_turso_import_guard(&root, all),
        CiCmd::DbSchemaCoverage => vox_cli_ci::db_schema_coverage::run(&root),
        CiCmd::PolicyAllowlistParity => super::policy_allowlist_parity::run(&root),
        CiCmd::RowSerdeLint => vox_cli_ci::row_serde_lint::run(&root),
        CiCmd::StringIdLint => vox_cli_ci::string_id_lint::run(&root, false),
        CiCmd::SecretsContracts => run_secrets_contracts(&root),
        CiCmd::SecretsParity => run_secrets_parity(&root),
        CiCmd::SecretsCutoverGates => run_secrets_cutover_gates(&root),
        CiCmd::SecretsCutoverAudit { all } => run_secrets_cutover_audit(&root, all),
        CiCmd::CapabilitySync { write } => super::capability_sync::run(&root, write),
        CiCmd::CapabilitySnapshot => vox_cli_ci::capability_snapshot::run(&root),
        CiCmd::AttentionConfigParity => vox_cli_ci::attention_parity::run(&root),
        CiCmd::CommandCompliance => command_compliance::run(&root),
        CiCmd::CompletionAudit { scan_extra } => completion_quality::run_audit(&root, &scan_extra),
        CiCmd::CompletionGates { mode } => completion_quality::run_gates(&root, mode),
        CiCmd::CompletionIngest {
            report,
            workflow,
            run_kind,
        } => completion_quality::run_ingest(&root, report, &workflow, &run_kind).await,
        CiCmd::RustEcosystemPolicy => {
            let cargo = cargo_bin();
            let st = Command::new(&cargo)
                .current_dir(&root)
                .args([
                    "test",
                    "-p",
                    "vox-compiler",
                    "--test",
                    "rust_ecosystem_support_parity",
                ])
                .status()?;
            if !st.success() {
                // EVALUATE to Err so the wrapper records Fail (no stale Pass).
                Err(anyhow!(
                    "rust ecosystem policy parity failed; run `cargo test -p vox-compiler --test rust_ecosystem_support_parity`"
                ))
            } else {
                println!("rust-ecosystem-policy OK");
                Ok(())
            }
        }
        CiCmd::PolicySmoke => {
            // Closure so every failure path EVALUATES to Err (flows through `result`),
            // letting the per-gate wrapper record Fail instead of a stale Pass.
            (|| -> Result<()> {
                let cargo = cargo_bin();

                let st = Command::new(&cargo)
                    .current_dir(&root)
                    .args(["check", "-p", "vox-orchestrator"])
                    .status()?;
                if !st.success() {
                    return Err(anyhow!(
                        "policy-smoke failed: `cargo check -p vox-orchestrator` returned non-zero"
                    ));
                }

                command_compliance::run(&root)?;

                let st = Command::new(&cargo)
                    .current_dir(&root)
                    .args([
                        "test",
                        "-p",
                        "vox-compiler",
                        "--test",
                        "rust_ecosystem_support_parity",
                    ])
                    .status()?;
                if !st.success() {
                    return Err(anyhow!(
                        "policy-smoke failed: `cargo test -p vox-compiler --test rust_ecosystem_support_parity` returned non-zero"
                    ));
                }

                println!("policy-smoke OK");
                Ok(())
            })()
        }
        CiCmd::BackendTests => {
            // Closure so the loop's failure path EVALUATES to Err (flows through
            // `result`), letting the wrapper record Fail instead of a stale Pass.
            (|| -> Result<()> {
                let cargo = cargo_bin();
                let suites: &[(&[&str], &str)] = &[
                    (&["test", "-p", "vox-actor-runtime"], "vox-actor-runtime"),
                    (
                        &["test", "-p", "vox-orchestrator", "model_route_policy"],
                        "vox-orchestrator model_route_policy",
                    ),
                    (
                        &["test", "-p", "vox-db", "research_metrics_contract"],
                        "vox-db research_metrics_contract",
                    ),
                    (
                        &["test", "-p", "vox-sql", "--test", "p2_conformance"],
                        "vox-sql p2_conformance",
                    ),
                    (
                        &["test", "-p", "vox-sql", "--test", "p3_introspect_smoke"],
                        "vox-sql p3_introspect_smoke",
                    ),
                    (
                        &["test", "-p", "vox-sql", "--test", "p5_ddl_conformance"],
                        "vox-sql p5_ddl_conformance",
                    ),
                    (
                        &["test", "-p", "vox-sql", "--test", "p5_migrate_smoke"],
                        "vox-sql p5_migrate_smoke",
                    ),
                ];
                for (args, label) in suites {
                    let st = Command::new(&cargo)
                        .current_dir(&root)
                        .args(*args)
                        .status()?;
                    if !st.success() {
                        return Err(anyhow!(
                            "backend-tests failed ({label}); rerun: cargo {}",
                            args.join(" ")
                        ));
                    }
                }
                println!("backend-tests OK");
                Ok(())
            })()
        }
        CiCmd::GuiSmoke => vox_cli_ci::gui_smoke::run(&root),
        CiCmd::CoverageGates {
            summary_json,
            mode,
            config,
        } => coverage_gates::run(summary_json, mode, config),
        CiCmd::CommandSync { write } => command_sync::run(&root, write),
        CiCmd::PmProvenance {
            strict,
            root: provenance_root,
        } => vox_cli_ci::pm_provenance::run(&root, &provenance_root, strict),
        CiCmd::CheckLinks { target } => check_links::run(&root, target.as_deref()),
        CiCmd::CanonicalMapVerify => canonical_docs::run(&root),
        CiCmd::ReleaseBuild {
            target,
            version,
            out_dir,
            package,
        } => release_build::run(&root, &target, version.as_deref(), &out_dir, package),
        CiCmd::ArtifactAudit {
            json,
            include_worktrees,
        } => super::workspace_artifacts::run_audit(&root, json, include_worktrees),
        CiCmd::ArtifactPrune {
            dry_run,
            apply,
            policy,
            include_worktrees,
            remove_stale_worktrees,
            include_dirty_targets,
            incremental_only,
            max_age_days,
        } => super::workspace_artifacts::run_prune(
            &root,
            dry_run,
            apply,
            policy.as_deref(),
            super::workspace_artifacts::WorktreeGcOpts {
                include_worktrees,
                remove_stale_worktrees,
                include_dirty_targets,
                incremental_only,
                max_age_days,
            },
        ),
        CiCmd::RunnerScale { apply } => super::runner_scale::run_scale(apply),
        CiCmd::BuildBench {
            label,
            write,
            compare,
            repeat,
            ingest,
        } => vox_cli_ci::build_bench::run_build_bench(&root, label, write, compare, repeat, ingest),
        CiCmd::CrateBudget { exit_zero } => {
            vox_cli_ci::crate_budget::run_crate_budget(&root, exit_zero)
        }
        CiCmd::CrateBuildMapParity => {
            vox_cli_ci::crate_build_map_parity::run_crate_build_map_parity(&root)
        }
        CiCmd::FanInBudget { exit_zero } => {
            vox_cli_ci::fan_in_budget::run_fan_in_budget(&root, exit_zero)
        }
        CiCmd::CrateEdges { tighten } => vox_cli_ci::crate_edges::run(&root, tighten),
        CiCmd::DepCycles {
            deny_new,
            allowlist,
        } => vox_cli_ci::dep_cycles::run_dep_cycles(&root, deny_new, allowlist.as_deref()),
        CiCmd::AffectedCrates {
            changed,
            graph,
            regen,
            out,
            check,
            github_output,
        } => {
            let mut args: Vec<String> = vec![];
            if regen {
                args.push("--regen".into());
            }
            if check {
                args.push("--check".into());
            }
            if let Some(p) = changed {
                args.push("--changed".into());
                args.push(p);
            }
            if let Some(p) = graph {
                args.push("--graph".into());
                args.push(p);
            }
            if let Some(p) = out {
                args.push("--out".into());
                args.push(p);
            }
            if let Some(p) = github_output {
                args.push("--github-output".into());
                args.push(p);
            }
            let code = vox_cli_ci::affected_cmd::run_affected_cmd(&args);
            if code == 0 {
                Ok(())
            } else {
                Err(anyhow!("affected-crates exited with code {code}"))
            }
        }
        CiCmd::RunnerPreflight => super::runner_scale::run_preflight(),
        CiCmd::RunnerStatus => super::runner_scale::run_status(),
        CiCmd::Queue {
            json,
            brief,
            from_snapshot,
            clear,
            dry_run,
            ttl_mins,
            hook_guard,
        } => super::queue::run(super::queue::QueueArgs {
            json,
            brief,
            from_snapshot,
            clear,
            dry_run,
            ttl_mins,
            hook_guard,
        }),
        CiCmd::JobTimings {
            run_id,
            threshold_mins,
            limit,
            json,
            annotate,
            strict,
        } => vox_cli_ci::job_timings::run(run_id, threshold_mins, limit, json, annotate, strict),
        CiCmd::NomenclatureGuard { json } => vox_cli_ci::nomenclature_guard::run(&root, json),
        CiCmd::RetiredSymbolCheck => retired_symbol_check::run(&root),
        CiCmd::SyncIgnoreFiles { verify } => vox_cli_ci::sync_ignore_files::run(&root, verify),
        CiCmd::KillStuckTests { what_if } => vox_cli_ci::kill_stuck_tests::run(&root, what_if),
        CiCmd::InstallHooks => vox_cli_ci::install_hooks::run(&root),
        CiCmd::ScriptHygiene { retired_check } => run_script_hygiene(&root, retired_check),
        CiCmd::DeterminismAudit => determinism_audit::run(&root),
        CiCmd::DepSprawl { cap } => vox_cli_ci::dep_sprawl::run(&root, cap),
        CiCmd::DoctestMd { paths, strict } => doctest_md::run(paths, strict),
        CiCmd::TestInventory {
            json,
            output,
            markdown,
            check,
        } => vox_cli_ci::test_inventory::run(
            &root,
            vox_cli_ci::test_inventory::TestInventoryOpts {
                json_stdout: json,
                output,
                markdown,
                check,
            },
        ),
        CiCmd::SafetyInventory {
            json,
            output,
            check,
        } => vox_cli_ci::safety_inventory::run(
            &root,
            vox_cli_ci::safety_inventory::SafetyInventoryOpts {
                json_stdout: json,
                output,
                check,
            },
        ),
        CiCmd::TestRuntimeReport {
            junit,
            json,
            markdown,
            top,
            fail_over_ms,
            fail_retry_count,
        } => vox_cli_ci::test_runtime_report::run(
            &root,
            vox_cli_ci::test_runtime_report::TestRuntimeReportOpts {
                junit,
                json,
                markdown,
                top,
                fail_over_ms,
                fail_retry_candidates: fail_retry_count,
            },
        ),
        CiCmd::IgnoredTestAge {
            mode,
            inventory,
            json,
        } => vox_cli_ci::test_governance::run_ignored_test_age(&root, mode, inventory, json),
        CiCmd::FlakeBudget {
            mode,
            report_json,
            junit,
            top,
            max_candidates,
            json,
        } => vox_cli_ci::test_governance::run_flake_budget(
            &root,
            mode,
            report_json,
            junit,
            top,
            max_candidates,
            json,
        ),
        CiCmd::RuntimeRegress {
            mode,
            current,
            baseline,
            percent,
            absolute_ms,
            json,
        } => vox_cli_ci::test_governance::run_runtime_regress(
            mode,
            current,
            baseline,
            percent,
            absolute_ms,
            json,
        ),
        CiCmd::DeployStatus { write_to } => vox_cli_ci::deploy_status::run(write_to).await,
        CiCmd::GeneratePluginCatalogDocs {
            catalog_out,
            bundles_out,
            check,
        } => vox_cli_ci::generate_plugin_catalog_docs::run(catalog_out, bundles_out, check),
        CiCmd::PluginCatalogParity => vox_cli_ci::plugin_catalog_parity::run(),
        CiCmd::NoTauriInCore => vox_cli_ci::no_tauri_in_core::run(&root),
        CiCmd::NoPluginCdylibAsCompileDep => {
            vox_cli_ci::no_plugin_cdylib_as_compile_dep::run(&root)
        }
        CiCmd::PluginDepBoundary => vox_cli_ci::plugin_dep_boundary::run(&root),
        CiCmd::PluginAbiParity { build } => vox_cli_ci::plugin_abi_parity::run(build),
        CiCmd::ProfileParity => super::profile_parity::run(),
        CiCmd::PluginSurfaceSync { write } => vox_cli_ci::plugin_surface::run(&root, write),
        CiCmd::PluginCatalogSync { write } => vox_cli_ci::plugin_catalog_sync::run(&root, write),
        CiCmd::PluginSkillParity { write } => vox_cli_ci::plugin_skill_parity::run(write),
        CiCmd::AgentSkillsCompliance => vox_cli_ci::agentskills_compliance::run(),
        CiCmd::McpVoxSurfaceParity => vox_cli_ci::mcp_vox_surface_parity::run(),
        CiCmd::CoolifyEval { cmd } => vox_cli_ci::coolify_eval::run(cmd).await,
        CiCmd::WatchRun {
            sha,
            timeout_secs,
            advisory,
            failures_only,
        } => {
            vox_cli_ci::watch_run::run(vox_cli_ci::watch_run::WatchRunArgs {
                sha,
                timeout_secs,
                advisory,
                failures_only,
            })
            .await
        }
    };

    // Record the gate's pass/fail into the per-branch status store (best-effort:
    // a status-write failure must never fail the gate). `ran_at` is stamped here
    // (the single non-deterministic seam) so the writer/merge stay pure.
    if let Some(id) = gate_id {
        if std::env::var("VOX_NO_POLICY_STATUS").is_err() {
            use vox_cli_contracts::GateStatusWriter;
            let providers = super::providers::VoxCliProviders;
            let duration_ms = started.elapsed().as_millis() as u64;
            let policy_result = gate_status_result(id, result.is_ok(), duration_ms);
            let branch = providers.current_branch(&root);
            let commit = providers.head_commit(&root);
            let ran_at = chrono::Utc::now().to_rfc3339();
            let _ = providers.write_results(&root, &branch, &commit, &ran_at, vec![policy_result]);
        }
    }

    result
}

/// Pure mapping from a tracked gate's `Ok`/`Err` outcome to the `PolicyResult`
/// recorded in the per-branch status store. Extracted so the honesty-critical
/// "failure ⇒ Fail" contract is unit-testable without invoking a real gate:
/// `ok == false` MUST yield `RunStatus::Fail` so a failing gate overwrites a
/// stale `Pass` rather than silently staying green.
fn gate_status_result(id: &str, ok: bool, duration_ms: u64) -> vox_config::PolicyResult {
    let status = if ok {
        vox_config::RunStatus::Pass
    } else {
        vox_config::RunStatus::Fail
    };
    vox_config::PolicyResult {
        id: id.to_string(),
        status,
        hits: vec![],
        duration_ms,
    }
}

#[cfg(test)]
mod gate_status_tests {
    use super::{gate_status_result, should_enforce_freshness};
    use vox_cli_ci::cmd_enums::CiCmd;
    use vox_config::RunStatus;

    #[test]
    fn err_outcome_records_fail_not_stale_pass() {
        let r = gate_status_result("ci-gate/ci.check-summary-drift", false, 5);
        assert_eq!(r.status, RunStatus::Fail);
        assert_eq!(r.id, "ci-gate/ci.check-summary-drift");
    }

    #[test]
    fn ok_outcome_records_pass() {
        let r = gate_status_result("ci-gate/ci.backend-tests", true, 5);
        assert_eq!(r.status, RunStatus::Pass);
    }

    /// End-to-end via the real writer: a tracked gate that was previously `Pass`
    /// must show `Fail` after a failing run (merge-by-id must not keep it green).
    #[test]
    fn failing_gate_overwrites_prior_pass_in_store() {
        use crate::commands::policy::status_writer::write_results;
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let branch = "sad-euler-31d645";
        let id = "ci-gate/ci.check-summary-drift";

        // Seed a stale Pass.
        write_results(
            root,
            branch,
            "deadbeef",
            "2026-06-06T00:00:00Z",
            vec![gate_status_result(id, true, 1)],
        )
        .unwrap();
        let prior = vox_config::load_status(root, branch).unwrap().unwrap();
        assert_eq!(
            prior.results.iter().find(|r| r.id == id).unwrap().status,
            RunStatus::Pass
        );

        // Now the gate fails.
        write_results(
            root,
            branch,
            "deadbeef",
            "2026-06-06T01:00:00Z",
            vec![gate_status_result(id, false, 1)],
        )
        .unwrap();
        let after = vox_config::load_status(root, branch).unwrap().unwrap();
        assert_eq!(
            after.results.iter().find(|r| r.id == id).unwrap().status,
            RunStatus::Fail,
            "failing tracked gate must overwrite stale Pass with Fail"
        );
    }

    #[test]
    fn freshness_exempts_runner_infra_but_guards_enforce() {
        // Infra reconcile/read commands must run even with a stale binary (keep the fleet alive).
        assert!(!should_enforce_freshness(&CiCmd::RunnerScale {
            apply: false
        }));
        assert!(!should_enforce_freshness(&CiCmd::RunnerScale {
            apply: true
        }));
        assert!(!should_enforce_freshness(&CiCmd::RunnerPreflight));
        assert!(!should_enforce_freshness(&CiCmd::RunnerStatus));
        // Real guard verdicts still require freshness.
        assert!(should_enforce_freshness(&CiCmd::SsotDrift));
        assert!(should_enforce_freshness(&CiCmd::RepoGuards));
    }
}
