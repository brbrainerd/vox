use super::{PipelineProgress, PipelineStage};
use anyhow::Result;
use std::collections::HashSet;
use std::path::PathBuf;

/// Run the dogfood pipeline: corpus extract → validate → pairs → eval → optional native train.
pub async fn run(
    data_dir: PathBuf,
    output_dir: PathBuf,
    skip_train: bool,
    strict_gate: bool,
    device: Option<String>,
    model: Option<String>,
    epochs: Option<usize>,
    preset: Option<String>,
    stages: Option<String>,
    dry_run: bool,
    curriculum: bool,
) -> Result<()> {
    #[cfg(not(feature = "gpu"))]
    {
        let _ = (
            strict_gate,
            device.as_ref(),
            model.as_ref(),
            epochs,
            preset.as_ref(),
            curriculum,
        );
    }

    let run_id = vox_corpus::training::timestamp_string();

    let all_possible_stages = [
        PipelineStage::Generate,
        PipelineStage::ResearchGen,
        PipelineStage::Extract,
        PipelineStage::HealToDpo,
        PipelineStage::Replay,
        PipelineStage::ReviewIngest,
        PipelineStage::ReviewDatasetBuild,
        PipelineStage::ReviewEvalPackBuild,
        PipelineStage::Validate,
        PipelineStage::Pairs,
        PipelineStage::Eval,
        PipelineStage::Mix,
        PipelineStage::Train,
    ];

    let mut planned_stages = Vec::new();
    if let Some(s) = stages {
        let requested: HashSet<String> = s.split(',').map(|x| x.trim().to_lowercase()).collect();
        for stage in all_possible_stages {
            if requested.contains(stage.as_str()) {
                if stage == PipelineStage::Train && skip_train {
                    continue;
                }
                planned_stages.push(stage);
            }
        }
    } else {
        for stage in all_possible_stages {
            if stage == PipelineStage::Train && skip_train {
                continue;
            }
            planned_stages.push(stage);
        }
    }

    let total_stages = planned_stages.len();
    let validated = PathBuf::from("mens/data/validated.jsonl");
    let train_jsonl = data_dir.join("train.jsonl");
    let eval_out = output_dir.join("eval_results.json");

    tracing::info!(
        run_id = %run_id,
        stages = ?planned_stages,
        dry_run,
        "mens pipeline: start"
    );

    if !dry_run {
        if let Some(p) = validated.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(&output_dir)?;
        std::fs::create_dir_all("mens/data/mix_sources")?;
    }

    for (completed_stages, stage) in planned_stages.into_iter().enumerate() {
        let progress = PipelineProgress {
            run_id: run_id.clone(),
            current_stage: stage,
            total_stages,
            completed_stages,
            progress_pct: (completed_stages as f64 / total_stages as f64) * 100.0,
        };

        // Report progress to telemetry/logs
        tracing::info!(
            stage = stage.as_str(),
            progress = %format!("{:.0}%", progress.progress_pct),
            "--- Pipeline Stage: {} ---",
            stage.as_str().to_uppercase()
        );

        match stage {
            PipelineStage::Generate => {
                if !dry_run {
                    crate::commands::corpus::run(crate::commands::corpus::CorpusAction::Generate {
                        output: PathBuf::from("mens/data/synthetic.jsonl"),
                        force_regen: false,
                        dry_run: false,
                    })
                    .await?;
                }
            }
            PipelineStage::Extract => {
                if !dry_run {
                    // Extract from .vox examples
                    let examples_dir = PathBuf::from("examples");
                    if examples_dir.is_dir() {
                        crate::commands::corpus::run(
                            crate::commands::corpus::CorpusAction::Extract {
                                dir: examples_dir,
                                output: validated.clone(),
                            },
                        )
                        .await
                        .map_err(|e| anyhow::anyhow!("pipeline extract examples failed: {e}"))?;
                    }

                    // Extract from Rust source
                    let crates_dir = PathBuf::from("crates");
                    if crates_dir.is_dir() {
                        crate::commands::corpus::run(
                            crate::commands::corpus::CorpusAction::ExtractRs {
                                dir: crates_dir,
                                output: PathBuf::from("mens/data/mix_sources/rust_source.jsonl"),
                            },
                        )
                        .await
                        .map_err(|e| anyhow::anyhow!("pipeline extract rust failed: {e}"))?;
                    }

                    // Extract from documentation
                    let docs_dir = PathBuf::from("docs/src");
                    if docs_dir.is_dir() {
                        crate::commands::corpus::run(
                            crate::commands::corpus::CorpusAction::ExtractDocs {
                                dir: docs_dir,
                                output: PathBuf::from("mens/data/mix_sources/docs.jsonl"),
                            },
                        )
                        .await
                        .map_err(|e| anyhow::anyhow!("pipeline extract docs failed: {e}"))?;
                    }
                }
            }
            PipelineStage::Validate => {
                if !dry_run {
                    if !validated.is_file() {
                        anyhow::bail!("Validate stage: missing input file '{}'. Make sure Extract stage ran successfully.", validated.display());
                    }
                    crate::commands::corpus::run(crate::commands::corpus::CorpusAction::Validate {
                        input: validated.clone(),
                        output: Some(validated.clone()),
                        no_recheck: true,
                        quarantine: None,
                        report: None,
                        reward_hook: None,
                    })
                    .await?;
                }
            }
            PipelineStage::Replay => {
                if !dry_run {
                    crate::commands::corpus::run(crate::commands::corpus::CorpusAction::Replay {
                        chatml: true,
                        min_score: 4.0, // High quality only for auto-replay
                        output: PathBuf::from("mens/data/mix_sources/autofeedback.jsonl"),
                        limit: 1000,
                    })
                    .await?;
                }
            }
            PipelineStage::HealToDpo => {
                if !dry_run {
                    let input = dirs::home_dir()
                        .map(|h| h.join(".vox/corpus/heal_pairs.jsonl"))
                        .unwrap_or_else(|| PathBuf::from("heal_pairs.jsonl"));
                    crate::commands::corpus::run(
                        crate::commands::corpus::CorpusAction::HealToDpo {
                            input: Some(input),
                            output: PathBuf::from("target/dogfood/preference_pairs.jsonl"),
                        },
                    )
                    .await?;
                }
            }
            PipelineStage::ResearchGen => {
                if !dry_run {
                    crate::commands::corpus::run(
                        crate::commands::corpus::CorpusAction::ResearchGen {
                            output: PathBuf::from("mens/data/research-lane-sft.jsonl"),
                            count: 1000,
                        },
                    )
                    .await?;
                }
            }
            PipelineStage::ReviewIngest => {
                if !dry_run {
                    let repo_id_resolved =
                        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxReviewRepositoryId);
                    let repo_id = repo_id_resolved
                        .expose()
                        .unwrap_or("vox-foundation/vox")
                        .to_string();
                    crate::commands::corpus::run(
                        crate::commands::corpus::CorpusAction::ReviewExport {
                            repository_id: repo_id,
                            limit: 1000,
                            output: PathBuf::from("mens/data/mix_sources/review_findings.jsonl"),
                        },
                    )
                    .await?;
                }
            }
            PipelineStage::ReviewDatasetBuild => {
                if !dry_run {
                    crate::commands::corpus::run(
                        crate::commands::corpus::CorpusAction::ReviewValidate {
                            input: PathBuf::from("mens/data/mix_sources/review_findings.jsonl"),
                        },
                    )
                    .await?;
                }
            }
            PipelineStage::ReviewEvalPackBuild => {
                if !dry_run {
                    crate::commands::corpus::run(
                        crate::commands::corpus::CorpusAction::ReviewStats {
                            input: PathBuf::from("mens/data/mix_sources/review_findings.jsonl"),
                        },
                    )
                    .await?;
                }
            }
            PipelineStage::Pairs => {
                if !dry_run {
                    if !validated.is_file() {
                        anyhow::bail!("Pairs stage: missing input file '{}'. Make sure Extract/Validate stage ran successfully.", validated.display());
                    }
                    crate::commands::corpus::run(crate::commands::corpus::CorpusAction::Pairs {
                        input: validated.clone(),
                        output: train_jsonl.clone(),
                        docs: vec![PathBuf::from("docs/src")],
                    })
                    .await?;
                }
            }
            PipelineStage::Eval => {
                if !dry_run {
                    if !train_jsonl.is_file() {
                        anyhow::bail!("Eval stage: missing input file '{}'. Make sure Pairs stage ran successfully.", train_jsonl.display());
                    }
                    crate::commands::corpus::run(crate::commands::corpus::CorpusAction::Eval {
                        input: train_jsonl.clone(),
                        output: eval_out.clone(),
                        print_summary: false,
                    })
                    .await?;
                }
            }
            PipelineStage::Mix => {
                if !dry_run {
                    let ws = vox_corpus::training::contract::find_workspace_root();
                    let mix_config =
                        vox_corpus::training::mix_prepare::resolve_mix_config_path(ws.as_deref());
                    if mix_config.is_file() {
                        vox_corpus::training::mix_prepare::sync_mix_primary_with_train_jsonl(
                            ws.as_deref(),
                            &data_dir,
                            &mix_config,
                        )?;
                        crate::commands::corpus::run(crate::commands::corpus::CorpusAction::Mix {
                            config: mix_config,
                            allow_missing_sources: true,
                        })
                        .await?;
                    }
                }
            }
            PipelineStage::Train => {
                if !dry_run {
                    #[cfg(feature = "gpu")]
                    {
                        let device = device.clone().unwrap_or_else(|| "best".into());
                        let target_model = model.clone();
                        let target_preset = preset.clone().or_else(|| Some("prosumer_16g".into()));

                        // SAFETY: CLI process; no concurrent `getenv` readers rely on these during this block.
                        #[allow(unsafe_code)]
                        unsafe {
                            std::env::set_var("VOX_BENCHMARK", "1");
                            if strict_gate {
                                std::env::set_var("VOX_EVAL_STRICT", "1");
                                std::env::set_var("VOX_BENCHMARK_MIN_PASS_RATE", "0.80");
                            } else {
                                std::env::set_var("VOX_EVAL_STRICT", "0");
                                std::env::set_var("VOX_BENCHMARK_MIN_PASS_RATE", "0.0");
                            }
                        }

                        crate::commands::schola::train::run_train(
                            crate::commands::mens::PopuliTrainBackendCli::Qlora.into(),
                            target_model,
                            device,
                            data_dir.clone(),
                            output_dir.clone(),
                            None, // rank (auto from preset)
                            None, // alpha
                            None, // seq_len
                            None, // batch_size
                            None, // grad_accum
                            None, // resume
                            epochs,
                            None, // lr
                            None, // warmup
                            42,   // seed
                            None, // min_rating
                            target_preset,
                            vox_populi::mens::TrainingDeploymentTarget::Workstation,
                            "normal".into(),
                            None, // vram_limit_fraction
                            None, // adapter_tag
                            None, // context_filter
                            None, // validation_split_ratio (use default 5%)
                            crate::commands::mens::MensTokenizerCli::Hf.into(),
                            false, // qlora_no_double_quant
                            false, // qlora_require_full_proxy_stack
                            false, // qlora_allow_partial_proxy_stack
                            None,  // qlora_max_skip_rate
                            false, // qlora_lm_head_only
                            None,  // qlora_proxy_max_layers
                            64,    // qlora_ce_last_k
                            None,  // checkpoint_every
                            false, // force_restart
                            curriculum,
                            vox_populi::mens::OptimizerExperimentMode::Off,
                            true,               // require_gpu
                            false,              // allow_cpu_fallback
                            None,               // base_model_family
                            None,               // upstream_model_id
                            None,               // license_class
                            false,              // attribution_required
                            false,              // trajectory_weighting_enabled
                            1.1,                // trajectory_tool_trace_boost
                            1.15,               // trajectory_failure_category_boost
                            None,               // trajectory_quality_floor
                            1.05,               // trajectory_quality_boost
                            None,               // curriculum_schedule
                            Default::default(), // chatml_config
                            None,               // mix_config
                        )
                        .await?;

                        // W4-01: Flywheel signal telemetry
                        if curriculum {
                            tracing::info!(
                                "flywheel.signal: Curriculum training complete, pending human promotion gate."
                            );
                        }
                    }

                    #[cfg(not(feature = "gpu"))]
                    {
                        anyhow::bail!(
                            "mens pipeline: native train was requested but this `vox` binary was built without the `gpu` feature; pass `--skip-train` or rebuild with `--features gpu`"
                        );
                    }
                }
            }
        }
    }

    tracing::info!(
        run_id = %run_id,
        "mens pipeline: complete"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heal_pairs_path_has_no_tilde() {
        let input = dirs::home_dir()
            .map(|h| h.join(".vox/corpus/heal_pairs.jsonl"))
            .unwrap_or_else(|| PathBuf::from("heal_pairs.jsonl"));
        assert!(!input.to_string_lossy().starts_with('~'));
    }

    #[tokio::test]
    async fn test_empty_validated_fails_closed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let data_dir = temp_dir.path().join("data");
        let output_dir = temp_dir.path().join("output");

        let res = run(
            data_dir,
            output_dir,
            true, // skip_train
            false, // strict_gate
            None,
            None,
            None,
            None,
            Some("validate,pairs,eval".to_string()),
            false,
            false,
        )
        .await;

        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("missing input file") || err_msg.contains("produced no validated.jsonl"));
    }
}
