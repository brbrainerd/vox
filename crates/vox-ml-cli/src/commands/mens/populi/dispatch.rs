//! Dispatch for [`PopuliAction`](crate::commands::mens::PopuliAction).

use anyhow::Result;

use super::PopuliAction;

#[cfg(feature = "gpu")]
use super::{
    MensTokenizerCli, OptimizerExperimentModeCli, PopuliTrainBackendCli,
    TrainingDeploymentTargetCli,
};

use crate::commands::mens::bench_completion;
use crate::commands::mens::eval_gate;
use crate::commands::mens::pipeline;
use crate::commands::mens::status;

#[cfg(feature = "gpu")]
use crate::commands::mens::eval_local;
use crate::commands::mens::probe;
#[cfg(feature = "gpu")]
use crate::commands::schola::merge_qlora;

#[cfg(feature = "gpu")]
use std::path::PathBuf;

/// Dispatch `vox mens` subcommands to their feature-gated implementations.
pub async fn run(action: PopuliAction, _global_json: bool, _global_verbose: bool) -> Result<()> {
    match action {
        #[cfg(feature = "mens-base")]
        PopuliAction::Pipeline {
            data_dir,
            output_dir,
            skip_train,
            strict_gate,
            device,
            model,
            epochs,
            preset,
            stages,
            dry_run,
            curriculum,
        } => {
            pipeline::run(
                data_dir,
                output_dir,
                skip_train,
                strict_gate,
                device,
                model,
                epochs,
                preset,
                stages,
                dry_run,
                curriculum,
            )
            .await
        }
        PopuliAction::TrainUv {
            model: _,
            data_dir: _,
            output_dir: _,
            rank: _,
            alpha: _,
            epochs: _,
        } => {
            anyhow::bail!(
                "`vox mens train-uv` is retired: `quantized_train.py` is not shipped in this repository.\n\
                 Use **`vox mens train --backend qlora --tokenizer hf`** (see docs/src/architecture/mens-training-ssot.md)."
            );
        }
        #[cfg(feature = "gpu")]
        PopuliAction::Dogfood {
            output_dir,
            checkpoint_every,
            force_restart,
        } => {
            let data_dir = PathBuf::from(vox_corpus::training::CANONICAL_TRAIN_DATA_DIR);

            crate::commands::schola::train::run_train(
                PopuliTrainBackendCli::Qlora.into(),
                None,
                "cuda".into(),
                data_dir,
                output_dir,
                None,                         // rank
                None,                         // alpha
                None,                         // seq_len
                None,                         // batch_size
                None,                         // grad_accum
                None,                         // resume
                None,                         // epochs
                None,                         // lr
                None,                         // warmup
                42,                           // seed
                None,                         // min_rating
                Some("qwen_4080_16g".into()), // preset
                TrainingDeploymentTargetCli::Workstation.into(),
                "normal".into(),                   // process_priority
                None,                              // vram_limit_fraction
                Some("vox_dogfood_gpu_v1".into()), // adapter_tag
                Some(vox_populi::mens::tensor::training_config::ContextFilter {
                    categories: Some(vec!["vox".to_string()]),
                    ..Default::default()
                }), // context_filter
                Some(0.05),                        // validation_split_ratio
                MensTokenizerCli::Hf.into(),
                false, // qlora_no_double_quant
                true,  // qlora_require_full_proxy_stack
                false, // qlora_allow_partial_proxy_stack
                None,  // qlora_max_skip_rate
                false, // qlora_lm_head_only
                None,  // qlora_proxy_max_layers
                64,    // qlora_ce_last_k
                Some(checkpoint_every),
                force_restart,
                false, // curriculum (dogfood default: off)
                OptimizerExperimentModeCli::Off.into(),
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
            Ok(())
        }
        #[cfg(feature = "gpu")]
        PopuliAction::Train {
            model,
            device,
            backend,
            data_dir,
            output_dir,
            rank,
            alpha,
            seq_len,
            batch_size,
            grad_accum,
            resume,
            epochs,
            lr,
            warmup,
            seed,
            min_rating,
            preset,
            domain,
            deployment_target,
            process_priority,
            vram_limit_fraction,
            background,
            log_dir,
            adapter_tag,
            context_filter,
            tokenizer,
            qlora_no_double_quant,
            qlora_require_full_proxy_stack,
            qlora_allow_partial_proxy_stack,
            qlora_lm_head_only,
            qlora_max_skip_rate,
            qlora_proxy_max_layers,
            qlora_ce_last_k,
            checkpoint_every,
            force_restart,
            require_gpu,
            allow_cpu_fallback,
            base_model_family,
            upstream_model_id,
            license_class,
            attribution_required,
            trajectory_weighting_enabled,
            trajectory_tool_trace_boost,
            trajectory_failure_category_boost,
            trajectory_quality_floor,
            trajectory_quality_boost,
            cloud,
            max_budget,
            train_data_hf,
            adapter_upload_hf,
            max_runtime_secs,
            validation_split_ratio,
            curriculum,
            optimizer_experiment_mode,
            data_mode,
            fast_corpus,
            persistent,
        } => {
            super::train_arm::run_train(
                model,
                device,
                backend,
                data_dir,
                output_dir,
                rank,
                alpha,
                seq_len,
                batch_size,
                grad_accum,
                resume,
                epochs,
                lr,
                warmup,
                seed,
                min_rating,
                preset,
                domain,
                deployment_target,
                process_priority,
                vram_limit_fraction,
                background,
                log_dir,
                adapter_tag,
                context_filter,
                tokenizer,
                qlora_no_double_quant,
                qlora_require_full_proxy_stack,
                qlora_allow_partial_proxy_stack,
                qlora_lm_head_only,
                qlora_max_skip_rate,
                qlora_proxy_max_layers,
                qlora_ce_last_k,
                checkpoint_every,
                force_restart,
                require_gpu,
                allow_cpu_fallback,
                base_model_family,
                upstream_model_id,
                license_class,
                attribution_required,
                trajectory_weighting_enabled,
                trajectory_tool_trace_boost,
                trajectory_failure_category_boost,
                trajectory_quality_floor,
                trajectory_quality_boost,
                cloud,
                max_budget,
                train_data_hf,
                adapter_upload_hf,
                max_runtime_secs,
                validation_split_ratio,
                curriculum,
                optimizer_experiment_mode.into(),
                data_mode,
                fast_corpus,
                persistent,
            )
            .await
        }

        #[cfg(not(feature = "gpu"))]
        PopuliAction::TrainStub { .. }
        | PopuliAction::DogfoodStub { .. }
        | PopuliAction::ServeStub { .. } => {
            // This `gpu` is a vox-ml-cli **cargo feature**, not a runtime
            // plugin. Old wording told users to `vox plugin install
            // tensor-burn-wgpu`, which doesn't fix anything — the binary
            // itself was compiled without the gpu feature, so the dispatch
            // arm above never gets reached. The real fix is to rebuild.
            //
            // Auto-installing a runtime plugin can't help here either: the
            // training code path is `#[cfg]`-gated out of this binary.
            anyhow::bail!(
                "vox mens {{train, dogfood, serve}} requires the `gpu` cargo feature, \
                 which was not enabled when this binary was built.\n\n\
                 To enable, rebuild from the workspace root with:\n\n  \
                 cargo build -p vox-ml-cli --release --features gpu,mens-candle-cuda\n\n\
                 Then re-install the runtime CUDA plugin (catalog or workspace):\n\n  \
                 vox plugin install mens-candle-cuda\n\
                 or, from this workspace:\n  \
                 vox plugin install --path crates/vox-plugin-mens-candle-cuda --yes\n\n\
                 See: docs/src/reference/plugins.md and \
                 docs/src/reference/mens-training.md"
            );
        }

        #[cfg(feature = "gpu")]
        PopuliAction::Serve {
            model,
            port,
            host,
            max_tokens,
            temperature,
            cloud,
            max_budget: _max_budget,
            model_hf: _model_hf,
            max_runtime_secs: _max_runtime_secs,
            persistent: _persistent,
        } => {
            if cloud != "local" {
                #[cfg(feature = "cloud")]
                {
                    use vox_populi::mens::cloud::CloudJobSpec;
                    let config = vox_populi::mens::cloud::CloudProviderConfig::default();
                    let rt = _max_runtime_secs.ok_or_else(|| {
                        anyhow::anyhow!("--max-runtime-secs is REQUIRED for cloud serve")
                    })?;
                    let mut spec = CloudJobSpec::new_serve(&config, rt);
                    spec.model_id =
                        _model_hf.unwrap_or_else(|| vox_populi::mens::DEFAULT_MODEL_ID.to_string());
                    spec.max_budget_usd = _max_budget;
                    spec.serve_port = port;
                    spec.persistent = _persistent;

                    let resolver = vox_populi::mens::cloud::CloudResolver::new_from_env().await?;
                    return resolver.dispatch(spec, &cloud).await;
                }
                #[cfg(not(feature = "cloud"))]
                {
                    anyhow::bail!(
                        "Cloud dispatch requires the 'cloud' feature. Rebuild with: cargo build -p vox-cli --features cloud"
                    );
                }
            }

            let model = model
                .ok_or_else(|| anyhow::anyhow!("--model <path> is required for local serve"))?;

            // Gate: Ensure collateral damage eval has run and passed if we're serving an adapter
            let manifest_path = model.join("training_manifest.json");
            let adapter_path = model.join("candle_qlora_adapter.safetensors");
            if manifest_path.exists() || adapter_path.exists() {
                let report_path = model.join("collateral_damage_report.json");
                if !report_path.exists() {
                    anyhow::bail!(
                        "eval_collateral_damage check not found! Run `vox mens eval collateral-damage --pre-score <baseline.json> --post <adapter>` before serving this adapter."
                    );
                }

                let report_raw = std::fs::read_to_string(&report_path)?;
                let report_json: serde_json::Value = serde_json::from_str(&report_raw)?;
                let status = report_json.get("status").and_then(|s| s.as_str());
                if status != Some("pass") {
                    anyhow::bail!(
                        "eval_collateral_damage check FAILED. The adapter degraded performance beyond acceptable thresholds and cannot be served."
                    );
                }
            }

            // Serve delegates directly to the lightweight vox-schola binary inference mode
            println!("Delegating to vox-schola serve...");

            let mut cmd = std::process::Command::new("vox-schola");
            cmd.arg("serve");
            cmd.arg("--model").arg(model);
            cmd.arg("--port").arg(port.to_string());
            cmd.arg("--host").arg(host);
            cmd.arg("--max-tokens").arg(max_tokens.to_string());
            cmd.arg("--temperature").arg(temperature.to_string());

            let status = cmd
                .status()
                .map_err(|e| anyhow::anyhow!("Failed to spawn vox-schola: {}", e))?;
            if !status.success() {
                anyhow::bail!("vox-schola serve exited with status: {}", status);
            }
            Ok(())
        }

        PopuliAction::Corpus(action) => crate::commands::corpus::run(action).await,

        #[cfg(feature = "gpu")]
        PopuliAction::Models => crate::commands::mens::models::run_models(_global_verbose),

        PopuliAction::Probe { verbose } => {
            let v = verbose || _global_verbose;
            probe::run_probe(v).await
        }

        PopuliAction::WatchTelemetry {
            telemetry,
            err_log,
            interval_ms,
        } => crate::commands::mens::watch_telemetry::run(telemetry, err_log, interval_ms),
        PopuliAction::Status {
            run_dir,
            quotas,
            config,
            cloud,
            db,
        } => {
            if cloud {
                #[cfg(feature = "codex")]
                {
                    use owo_colors::OwoColorize;
                    let db = vox_db::VoxDb::connect_default().await?;
                    let summary = db.cloud_cost_summary().await?;

                    println!("\n  {}", "Cloud GPU Dispatch Summary".bold().cyan());
                    println!(
                        "  Jobs:      {}",
                        summary.running_jobs + summary.completed_jobs
                    );
                    println!("  Spent:     ${:.2}", summary.total_spent_usd);
                    println!("  Accruing:  ${:.2}", summary.accrued_usd);
                    println!(
                        "  Efficiency: {:.0} tokens/$",
                        summary.avg_tokens_per_dollar
                    );
                    return Ok(());
                }
                #[cfg(not(feature = "codex"))]
                {
                    anyhow::bail!("Cloud status requires the 'codex' feature (VoxDb access).");
                }
            }
            let _ = _global_json;
            status::run_status(run_dir, _global_json, quotas, config, db).await
        }

        #[cfg(feature = "gpu")]
        PopuliAction::MergeQlora {
            base_shard,
            adapter,
            meta,
            output,
        } => merge_qlora::run_merge_qlora(base_shard, adapter, meta, output),

        #[cfg(feature = "mens-dei")]
        PopuliAction::Generate {
            prompt,
            output,
            no_validate,
            server_url,
            max_retries,
            output_mode,
            schema,
            context_mode,
            conversation_id,
            queue,
            mode,
        } => {
            if let Some(ref m) = mode {
                // SAFETY: isolated env var for this process; no other threads read it during this block
                #[allow(unsafe_code)]
                unsafe {
                    std::env::set_var("VOX_DEI_MODE_PROFILE", m);
                }
            }
            // Run generate in a dedicated thread with its own runtime to avoid
            // "Cannot drop a runtime in a context where blocking is not allowed" during shutdown.
            let prompt = prompt.clone();
            let output = output.clone();
            let server_url = server_url.clone();
            let output_mode = output_mode.as_deref();
            let schema = schema.as_deref();
            let context_mode = context_mode.clone();
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Runtime::new().expect("create runtime for generate");
                rt.block_on(crate::commands::ai::generate::run(
                    &prompt,
                    output,
                    no_validate,
                    server_url.as_deref(),
                    max_retries,
                    output_mode,
                    schema,
                    Some(&context_mode),
                    conversation_id,
                    queue,
                ))
            })
        }

        #[cfg(feature = "mens-dei")]
        PopuliAction::Review {
            targets,
            model,
            format,
            severity,
            free_only,
            diff,
            ci,
            pr_comment,
            diff_base,
            mode,
        } => {
            if let Some(ref m) = mode {
                // SAFETY: main-thread env set before spawning review; no concurrent readers
                #[allow(unsafe_code)]
                unsafe {
                    std::env::set_var("VOX_DEI_MODE_PROFILE", m);
                }
            }
            let mut cmd = tokio::process::Command::new("vox");
            cmd.arg("review");
            for t in &targets {
                cmd.arg(t);
            }
            if let Some(m) = model {
                cmd.arg("--model").arg(m);
            }
            if let Some(f) = format {
                cmd.arg("--format").arg(f);
            }
            if let Some(s) = severity {
                cmd.arg("--severity").arg(s);
            }
            if free_only {
                cmd.arg("--free-only");
            }
            if diff {
                cmd.arg("--diff");
            }
            if ci {
                cmd.arg("--ci");
            }
            if pr_comment {
                cmd.arg("--pr-comment");
            }
            if let Some(db) = diff_base {
                cmd.arg("--diff-base").arg(db);
            }

            let status = cmd.status().await?;
            if !status.success() {
                anyhow::bail!("vox review via subprocess failed");
            }
            Ok(())
        }

        #[cfg(feature = "mens-dei")]
        PopuliAction::Workflow(action) => crate::commands::ai::workflow::run(action).await,

        #[cfg(feature = "mens-dei")]
        PopuliAction::Check { file } => {
            crate::dei_daemon::call(
                crate::dei_daemon::method::AI_CHECK,
                serde_json::json!({
                    "file": file,
                }),
                false,
            )
            .await?;
            Ok(())
        }

        #[cfg(feature = "mens-dei")]
        PopuliAction::Fix { file, errors } => {
            let code = vox_bounded_fs::read_utf8_path_capped(&file)?;
            let errors_val = if let Some(e) = errors {
                e
            } else {
                "".to_string()
            };
            crate::dei_daemon::call(
                crate::dei_daemon::method::AI_FIX,
                serde_json::json!({
                    "code": code,
                    "errors": errors_val,
                }),
                false,
            )
            .await?;
            Ok(())
        }

        #[cfg(feature = "gpu")]
        PopuliAction::EvalLocal {
            model,
            bench,
            max_tokens,
            temperature,
            samples,
            seed_base,
            output,
        } => eval_local::run_eval_local(
            model,
            bench,
            max_tokens,
            temperature,
            samples,
            seed_base,
            output,
        ),

        PopuliAction::MensTail(tail) => match tail {
            super::mens_tail_subcommands::PopuliMensTail::EvalGate { run_dir, policy } => {
                let code = eval_gate::run_eval_gate(run_dir, policy)?;
                std::process::exit(code);
            }

            super::mens_tail_subcommands::PopuliMensTail::EvalCollateralDamage {
                pre_score,
                post_adapter,
            } => {
                #[cfg(not(feature = "gpu"))]
                let _ = (pre_score, post_adapter);

                #[cfg(feature = "gpu")]
                crate::commands::mens::eval_collateral::run_collateral_damage(
                    pre_score,
                    post_adapter,
                )?;
                Ok(())
            }

            super::mens_tail_subcommands::PopuliMensTail::BenchCompletion {
                url,
                count,
                warmup,
            } => bench_completion::run_bench(&url, count, warmup).await,

            super::mens_tail_subcommands::PopuliMensTail::SystemPromptTemplate {
                output,
                format,
            } => crate::commands::mens::system_prompt_template::run(output, &format).await,
        },
    }
}
