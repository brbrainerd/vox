//! SSOT / matrix guard implementations for `vox ci`.

use anyhow::{Result, anyhow};
use std::process::Command;

use super::ai_fixtures_coverage;
use super::build_timings;
use super::canonical_docs;
use super::check_links;
use super::cmd_enums::{
    CiCmd, DocInventoryCmd, DocsRealityAuditCmd, EvalMatrixCmd, MensScorecardCmd,
    OperationsSyncTarget,
};
use super::command_compliance;
use super::command_sync;
use super::completion_quality;
use super::contracts_index;
use super::coverage_gates;
use super::dep_sprawl;
use super::determinism_audit;
use super::doctest_md;
use super::eval_matrix;
use super::exec_policy_contract;
use super::grammar_ssot_parity;
use super::line_endings;
use super::mens_scorecard;
use super::openclaw_contract;
use super::parse_status;
use super::release_build;
use super::scaling_audit;
use super::scientia_heuristics_parity;
use super::scientia_novelty_ledger_contract;
use super::scientia_worthiness_contract;
use super::{cargo_bin, repo_root};

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
    run_secrets_cutover_gates, run_secrets_parity, run_sql_surface_guard, run_ssot_audit,
    run_ssot_drift, run_toestub_scoped, run_toestub_self_apply, run_turso_import_guard,
};

use super::retired_symbol_check;

/// Run `vox ci` subcommand.
pub async fn run(cmd: CiCmd) -> Result<()> {
    let root = repo_root();
    match cmd {
        CiCmd::Manifest => run_manifest(&root),
        CiCmd::CheckDocsSsot => check_docs_ssot(&root),
        CiCmd::CheckFrozen => super::frozen_crates::check_frozen_crates(&root),
        CiCmd::GuiCatalogParity => super::gui_catalog_parity::run(&root),
        CiCmd::GuiVersionSync { write } => super::gui_version_sync::run(&root, write),
        CiCmd::GuiSurfaceCoverage { write } => super::gui_surface_coverage::run(&root, write),
        CiCmd::ModelRoutingCheck => super::model_routing_check::run(&root),
        CiCmd::CheckCodexSsot => check_codex_ssot(&root),
        CiCmd::ContractsIndex => contracts_index::run(&root),
        CiCmd::AiFixturesCoverage => ai_fixtures_coverage::run(&root),
        CiCmd::ExecPolicyContract => exec_policy_contract::run(&root),
        CiCmd::OpenClawContract => openclaw_contract::run(&root),
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
        } => super::speech_runtime_suite::run(
            &root,
            super::speech_runtime_suite::SpeechRuntimeSuiteOpts {
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
            },
        ),
        CiCmd::TierBudgetCheck { junit, profile } => {
            super::tier_budget_check::run(&root, &junit, &profile)
        }
        CiCmd::DevLoopAudit { json } => super::dev_loop_audit::run(&root, json),
        CiCmd::SsotAudit => run_ssot_audit(&root).await,
        CiCmd::DataSsotGuards => run_data_ssot_guards(&root),
        CiCmd::DataStorageGuard(opts) => {
            let report = crate::commands::ci::data_storage_guard::run(&opts)?;
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
        CiCmd::CompileMatrix => super::compile_matrix::run(&root),
        CiCmd::RetirementAudit => super::retirement_audit::run(&root),
        CiCmd::NoDeiImport => check_no_vox_dei(&root),
        CiCmd::AttentionEventLedgerParity => super::attention_ledger_parity::run(&root),
        CiCmd::CheckSummaryDrift => {
            let cargo = cargo_bin();
            let st = Command::new(&cargo)
                .current_dir(&root)
                .args(["run", "-p", "vox-doc-pipeline", "--", "--check"])
                .status()?;
            if !st.success() {
                return Err(anyhow!(
                    "SUMMARY.md is out of sync with docs/src. Run 'cargo run -p vox-doc-pipeline' to fix."
                ));
            }
            println!("SUMMARY.md is up to date.");
            Ok(())
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
            DocsRealityAuditCmd::Verify => super::docs_reality_audit::run_verify(&root),
            DocsRealityAuditCmd::Metrics { write } => {
                super::docs_reality_audit::run_metrics(&root, write)
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
        CiCmd::FmtCheck => super::pre_push::check_fmt(&root),
        CiCmd::LineEndings { all, base, autofix } => line_endings::run(&root, all, base, autofix),
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
        } => super::detect_rules_bench::run(&rules, &fixtures_root, min_f1, json),
        CiCmd::ToestubBudget => super::toestub_budget::run(),
        CiCmd::JsonParseCheck { globs } => super::parse_check::run_json(&globs),
        CiCmd::YamlParseCheck { globs } => super::parse_check::run_yaml(&globs),
        CiCmd::ToestubSelfApply => run_toestub_self_apply(&root),
        CiCmd::ToestubScoped {
            root: scan_root,
            mode,
        } => run_toestub_scoped(&root, &scan_root, mode),
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
                build_timings::bench_build_run(persist.unwrap_or(true), name, Some(profile))
                    .await?;
                Ok(())
            } else {
                run_build_timings(&root, json, crates)
            }
        }
        CiCmd::GrammarDrift { emit } => run_grammar_drift(&root, emit),
        CiCmd::GrammarSsotParity => grammar_ssot_parity::run().await,
        CiCmd::KComplexityBudget {
            tolerance_percent,
            update,
        } => run_k_complexity_budget(&root, tolerance_percent, update),
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
        CiCmd::DbSchemaCoverage => super::db_schema_coverage::run(&root),
        CiCmd::PolicyAllowlistParity => super::policy_allowlist_parity::run(&root),
        CiCmd::RowSerdeLint => super::row_serde_lint::run(&root),
        CiCmd::StringIdLint => super::string_id_lint::run(&root, false),
        CiCmd::SecretsContracts => run_secrets_contracts(&root),
        CiCmd::SecretsParity => run_secrets_parity(&root),
        CiCmd::SecretsCutoverGates => run_secrets_cutover_gates(&root),
        CiCmd::SecretsCutoverAudit { all } => run_secrets_cutover_audit(&root, all),
        CiCmd::CapabilitySync { write } => super::capability_sync::run(&root, write),
        CiCmd::CapabilitySnapshot => super::capability_snapshot::run(&root),
        CiCmd::AttentionConfigParity => super::attention_parity::run(&root),
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
                return Err(anyhow!(
                    "rust ecosystem policy parity failed; run `cargo test -p vox-compiler --test rust_ecosystem_support_parity`"
                ));
            }
            println!("rust-ecosystem-policy OK");
            Ok(())
        }
        CiCmd::PolicySmoke => {
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
        }
        CiCmd::BackendTests => {
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
        }
        CiCmd::GuiSmoke => super::gui_smoke::run(&root),
        CiCmd::CoverageGates {
            summary_json,
            mode,
            config,
        } => coverage_gates::run(summary_json, mode, config),
        CiCmd::CommandSync { write } => command_sync::run(&root, write),
        CiCmd::PmProvenance {
            strict,
            root: provenance_root,
        } => super::pm_provenance::run(&root, &provenance_root, strict),
        CiCmd::CheckLinks { target } => check_links::run(&root, target.as_deref()),
        CiCmd::CanonicalMapVerify => canonical_docs::run(&root),
        CiCmd::ReleaseBuild {
            target,
            version,
            out_dir,
            package,
        } => release_build::run(&root, &target, version.as_deref(), &out_dir, package),
        CiCmd::ArtifactAudit { json } => super::workspace_artifacts::run_audit(&root, json),
        CiCmd::ArtifactPrune {
            dry_run,
            apply,
            policy,
        } => super::workspace_artifacts::run_prune(&root, dry_run, apply, policy.as_deref()),
        CiCmd::NomenclatureGuard { json } => super::nomenclature_guard::run(&root, json),
        CiCmd::RetiredSymbolCheck => retired_symbol_check::run(&root),
        CiCmd::SyncIgnoreFiles { verify } => super::sync_ignore_files::run(&root, verify),
        CiCmd::KillStuckTests { what_if } => super::kill_stuck_tests::run(&root, what_if),
        CiCmd::InstallHooks => super::install_hooks::run(&root),
        CiCmd::ScriptHygiene { retired_check } => run_script_hygiene(&root, retired_check),
        CiCmd::DeterminismAudit => determinism_audit::run(&root),
        CiCmd::DepSprawl { cap } => dep_sprawl::run(&root, cap),
        CiCmd::DoctestMd { paths, strict } => doctest_md::run(paths, strict).await,
        CiCmd::TestInventory {
            json,
            output,
            markdown,
            check,
        } => super::test_inventory::run(
            &root,
            super::test_inventory::TestInventoryOpts {
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
        } => super::safety_inventory::run(
            &root,
            super::safety_inventory::SafetyInventoryOpts {
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
        } => super::test_runtime_report::run(
            &root,
            super::test_runtime_report::TestRuntimeReportOpts {
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
        } => super::test_governance::run_ignored_test_age(&root, mode, inventory, json),
        CiCmd::FlakeBudget {
            mode,
            report_json,
            junit,
            top,
            max_candidates,
            json,
        } => super::test_governance::run_flake_budget(
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
        } => super::test_governance::run_runtime_regress(
            mode,
            current,
            baseline,
            percent,
            absolute_ms,
            json,
        ),
        CiCmd::DeployStatus { write_to } => super::deploy_status::run(write_to).await,
        CiCmd::GeneratePluginCatalogDocs {
            catalog_out,
            bundles_out,
            check,
        } => super::generate_plugin_catalog_docs::run(catalog_out, bundles_out, check),
        CiCmd::PluginCatalogParity => super::plugin_catalog_parity::run(),
        CiCmd::NoTauriInCore => super::no_tauri_in_core::check(&root),
        CiCmd::NoPluginCdylibAsCompileDep => super::no_plugin_cdylib_as_compile_dep::check(&root),
        CiCmd::PluginAbiParity => super::plugin_abi_parity::run(),
        CiCmd::PluginSkillParity => super::plugin_skill_parity::run(),
        CiCmd::AgentSkillsCompliance => super::agentskills_compliance::run(),
        CiCmd::CoolifyEval { cmd } => super::coolify_eval::run(cmd).await,
        CiCmd::WatchRun {
            sha,
            timeout_secs,
            advisory,
            failures_only,
        } => {
            super::watch_run::run(super::watch_run::WatchRunArgs {
                sha,
                timeout_secs,
                advisory,
                failures_only,
            })
            .await
        }
    }
}
