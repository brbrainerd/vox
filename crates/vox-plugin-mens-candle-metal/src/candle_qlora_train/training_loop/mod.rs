//! Main epoch loop for QLoRA training.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use candle_core::Device;
use owo_colors::OwoColorize;
use std::sync::atomic::Ordering;
use tokenizers::Tokenizer;
use vox_tensor::data::TrainingPair;

use qlora_rs::training::QLoraTrainer;
use rand::SeedableRng;

use super::{
    PAUSE_FLAG, QLORA_ETA_EMA_ALPHA, TrainingDbEvent, TrainingLoopStats, compute_cosine_lr,
};
use crate::{
    config::LoraTrainingConfig, manifest, qlora_preflight::QloraEmbedBundle, telemetry,
    telemetry_schema, train_log, training_summary::TrainingSummary,
};

pub mod checkpoint;
pub mod curriculum;
pub mod encoding;
pub mod forward;
pub mod logic;
pub mod telem_helpers;
pub mod types;
pub mod validation;

pub use self::checkpoint::apply_checkpoint_resume;
pub use self::validation::preflight_masked_ce_finite;

use self::types::*;

#[allow(clippy::too_many_arguments)]
pub fn run_training_loop(
    trainer: &mut QLoraTrainer,
    model: super::TrainGraphModel,
    bundle: &QloraEmbedBundle,
    resume: QloraTrainingResume,
    out: &Path,
    config: &LoraTrainingConfig,
    pairs: Vec<TrainingPair>,
    eval_pairs: Vec<TrainingPair>,
    tokenizer: &Tokenizer,
    device: &Device,
    db_tx: &tokio::sync::mpsc::UnboundedSender<TrainingDbEvent>,
    run_id: &str,
    device_label: &str,
    train_path: &Path,
    system_prompt: &str,
    adapter_layer_order: &[String],
    base_key_map: &HashMap<String, String>,
    total_steps_planned: u32,
    total_optimizer_steps_planned: u32,
    warmup_steps: usize,
    contamination_score: Option<f32>,
) -> Result<TrainingSummary> {
    if !config.qlora_double_quant {
        train_log::info(
            "qlora_double_quant=false: Candle QLoRA uses qlora-rs preset quantization; flag is recorded for contract/manifest parity only.",
        );
    }
    if config.qlora_max_skip_rate.is_some() {
        train_log::info(
            "qlora_max_skip_rate is reserved for future skip accounting; value is stored in config/manifest for observability only.",
        );
    }

    let proxy_stack_complete = match crate::qlora_weights::tensor_keys_union(&bundle.weight_paths) {
        Ok(present) => {
            let cov = crate::qlora_weights::middle_projection_coverage(&bundle.layout, &present);
            cov.expected == 0 || cov.complete
        }
        Err(err) => {
            train_log::warn(&format!(
                "Could not recompute middle projection coverage for manifest: {err}"
            ));
            false
        }
    };

    let QloraTrainingResume {
        start_epoch,
        mut global_step,
        resume_pair_offset,
        resume_shuffled_indices,
    } = resume;

    manifest::write_training_manifest(
        out,
        manifest::initial_training_manifest(
            manifest::ArchParams {
                vocab_size: bundle.vocab,
                d_model: bundle.d_model,
                n_heads: bundle.layout.num_attention_heads,
                n_layers: bundle.layout.num_hidden_layers,
            },
            train_path.display().to_string(),
            {
                let mut run = manifest::InitialManifestRun::from_lora_config(config);
                run.contamination_score = contamination_score;
                run
            },
            Some(bundle.tokenizer_path.display().to_string()),
            manifest::InitialTrainingKernel::CandleQlora {
                proxy_stack_complete,
                middle_layers_active: bundle.layout.num_hidden_layers,
                ce_last_k: config.qlora_ce_last_k,
                architecture: match bundle.layout.architecture {
                    crate::hf_layout::HfArchitecture::Qwen35 => "qwen3_5".to_string(),
                    crate::hf_layout::HfArchitecture::Gpt2 => "gpt2".to_string(),
                },
                linear_layers: Some(
                    bundle
                        .layout
                        .layer_types
                        .iter()
                        .filter(|t| t.as_str() == "linear_attention")
                        .count(),
                ),
                full_layers: Some(
                    bundle
                        .layout
                        .layer_types
                        .iter()
                        .filter(|t| t.as_str() == "full_attention")
                        .count(),
                ),
            },
        ),
    )?;

    telemetry::append(
        out,
        telemetry_schema::events::TRAIN_START,
        serde_json::json!({
            telemetry_schema::keys::TRAIN_FILE: train_path.display().to_string(),
            telemetry_schema::keys::OUTPUT_DIR: out.display().to_string(),
            telemetry_schema::keys::SEED: config.seed,
            telemetry_schema::keys::GRAD_ACCUM: config.grad_accum.max(1),
            telemetry_schema::keys::EPOCHS: config.epochs,
            telemetry_schema::keys::PLANNED_STEPS_TOTAL: total_steps_planned,
            "planned_optimizer_steps_total": total_optimizer_steps_planned,
            "compute_device": device_label,
            "warmup_steps": warmup_steps,
            "n_heads": bundle.layout.num_attention_heads,
            "n_kv_heads": bundle.layout.num_key_value_heads,
            "run_id": run_id,
        }),
    )?;

    let mut rng = rand::rngs::StdRng::seed_from_u64(config.seed);
    let mut last_progress = Instant::now();
    let progress_every = Duration::from_secs(5);
    let mut ema_steps_per_sec: Option<f64> = None;
    let mut optimizer_step_count: u32 = global_step / config.grad_accum.max(1) as u32;
    let mut progress_anchor_step = optimizer_step_count;
    let mut progress_anchor_time = Instant::now();
    let mut last_loss_val: f32 = 0.0;
    let mut ema_loss_val: Option<f64> = None;
    let mut total_loss_sum = 0.0f64;
    let mut total_step_count: u32 = 0;
    let mut total_tokens: usize = 0;
    let mut last_avg_val_loss: Option<f64> = None;
    let grad_accum = config.grad_accum.max(1) as u32;
    let mut skip_no_supervised_positions: u64 = 0;
    let mut skip_short_seq: u64 = 0;
    let mut skip_curriculum: u64 = 0;
    let mut skip_token_id_oob: u64 = 0;
    let mut token_oob_warned = false;
    let mut trajectory_weighted_pairs: u64 = 0;
    let mut trajectory_clamped_pairs: u64 = 0;
    let mut total_valid_tokens: u64 = 0;
    let mut total_theoretical_tokens: u64 = 0;
    let mut total_syntax_weight: f64 = 0.0;

    let run_start_inst = Instant::now();
    for epoch in start_epoch..=config.epochs {
        let shuffled_indices = checkpoint::build_epoch_shuffled_indices(
            epoch,
            start_epoch,
            &pairs,
            &resume_shuffled_indices,
            &mut rng,
            config.curriculum,
        );

        trainer.start_epoch();

        if config.curriculum {
            let max_difficulty = curriculum::max_difficulty_for_epoch(epoch, config);
            let phase_label = config
                .curriculum_schedule
                .as_ref()
                .and_then(|s| s.curriculum_phases.as_ref())
                .and_then(|p| p.get(epoch - 1))
                .cloned()
                .unwrap_or_else(|| "auto".to_string());

            train_log::info(&format!(
                "Epoch {}/{} [phase: {}] curriculum threshold: diff <= {}",
                epoch,
                config.epochs,
                phase_label.cyan(),
                max_difficulty
            ));
        }

        let mut epoch_loss_sum = 0.0f64;
        let mut epoch_steps = 0u32;
        let pair_start = if epoch == start_epoch {
            resume_pair_offset.min(shuffled_indices.len())
        } else {
            0
        };

        let max_difficulty = curriculum::max_difficulty_for_epoch(epoch, config);

        for (pair_loop_idx, &pair_real_idx) in shuffled_indices.iter().enumerate().skip(pair_start)
        {
            let pair = &pairs[pair_real_idx];
            let (sample_weight, was_clamped) = logic::trajectory_weight_for_pair(pair, config);
            if config.trajectory_weighting_enabled && (sample_weight - 1.0_f64).abs() > f64::EPSILON
            {
                trajectory_weighted_pairs += 1;
            }
            if was_clamped {
                trajectory_clamped_pairs += 1;
            }

            let enc = match encoding::try_encode_training_step(
                pair,
                system_prompt,
                tokenizer,
                config,
                max_difficulty,
            )? {
                TryEncodeOutcome::SkipCurriculum => {
                    skip_curriculum += 1;
                    continue;
                }
                TryEncodeOutcome::SkipShortSeq => {
                    skip_short_seq += 1;
                    continue;
                }
                TryEncodeOutcome::Encoded(enc) => enc,
            };

            total_tokens += enc.raw_token_len;

            if !encoding::token_ids_in_model_vocab(&enc.ids, bundle.vocab) {
                skip_token_id_oob += 1;
                if !token_oob_warned {
                    token_oob_warned = true;
                    let max_id = enc.ids.iter().copied().max().unwrap_or(0);
                    train_log::warn(&format!(
                        "token id out of model vocab range (max_id={max_id}, vocab_size={}, pair_real_idx={pair_real_idx}); skipping pairs with OOV ids",
                        bundle.vocab
                    ));
                }
                continue;
            }

            let mut lr_applied_this_step = 0.0_f64;
            let loss_result = forward::forward_masked_ce(
                &model,
                &enc.ids,
                enc.prefix_len,
                enc.trunc_offset,
                enc.sample_weight,
                enc.token_weights.as_deref(),
                config,
                device,
            )?;

            let loss_val_micro = match loss_result {
                MaskedCeForward::NoSupervision => {
                    skip_no_supervised_positions += 1;
                    None
                }
                MaskedCeForward::NonFinite { kind, mask_sum } => {
                    train_log::warn(&format!(
                        "Non-finite loss ({kind}) before backward at epoch {epoch} micro_step {global_step} (skip update); mask_sum={mask_sum:.3}",
                    ));
                    None
                }
                MaskedCeForward::Finite {
                    loss,
                    loss_scalar,
                    supervised_tokens,
                    theoretical_tokens,
                    syntax_weight_sum,
                } => {
                    trainer
                        .backward_step(&loss)
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    total_valid_tokens = total_valid_tokens.saturating_add(supervised_tokens);
                    total_theoretical_tokens =
                        total_theoretical_tokens.saturating_add(theoretical_tokens);
                    total_syntax_weight += syntax_weight_sum as f64;

                    lr_applied_this_step = trainer.current_lr();

                    let lr_next = compute_cosine_lr(
                        optimizer_step_count,
                        warmup_steps,
                        total_optimizer_steps_planned,
                        config.learning_rate,
                    );
                    let micro_step_after_backward = global_step + 1;
                    if micro_step_after_backward.is_multiple_of(grad_accum) {
                        optimizer_step_count += 1;
                        trainer.config.adapter_config.learning_rate = lr_next;
                        trainer.update_lr();
                    }
                    Some(loss_scalar)
                }
            };

            let Some(loss_val) = loss_val_micro else {
                continue;
            };

            global_step += 1;
            epoch_steps += 1;
            epoch_loss_sum += loss_val as f64;
            total_loss_sum += loss_val as f64;
            total_step_count += 1;
            last_loss_val = loss_val;

            ema_loss_val = Some(match ema_loss_val {
                None => loss_val as f64,
                Some(prev) => 0.1 * (loss_val as f64) + 0.9 * prev,
            });

            let mut computed_reward = pair.rating.unwrap_or(0) as f32;
            match config.reward_hook.as_deref() {
                Some("cargo_build") | Some("cargo_test") => {
                    let mut snippet_opt = pair.response.as_deref();
                    if snippet_opt.is_none()
                        && let Some(turns) = &pair.messages
                        && let Some(last) = turns.last()
                    {
                        snippet_opt = Some(last.content.as_str());
                    }
                    if let Some(resp) = snippet_opt {
                        let code =
                            vox_eval::extract_vox_code(resp).unwrap_or_else(|| resp.to_string());
                        let ast_report = vox_compiler::ast_eval(&code);
                        let r_syntax = if ast_report.parse_success { 1.0 } else { 0.0 };
                        let r_test = vox_eval::cargo_build_reward(resp);
                        let r_coverage = ast_report.coverage_score();
                        let w1 = 1.0;
                        let w2 = 1.0;
                        let r = r_syntax * (w1 * r_test + w2 * r_coverage);
                        if r_syntax == 0.0 {
                            computed_reward = -1.0;
                        } else {
                            computed_reward = r as f32;
                        }
                    }
                }
                _ => {}
            }

            let _ = db_tx.send(TrainingDbEvent::GrpoStep {
                run_id: run_id.to_string(),
                step: global_step,
                mean_reward: computed_reward,
                policy_loss: loss_val,
                clip_fraction: 0.0,
                parse_rate: 1.0,
            });

            let elapsed_since_progress = last_progress.elapsed();
            if elapsed_since_progress >= progress_every {
                let now = Instant::now();
                let dt = now
                    .duration_since(progress_anchor_time)
                    .as_secs_f64()
                    .max(1e-3);
                let ds = (optimizer_step_count - progress_anchor_step) as f64;
                let sps = ds / dt;
                ema_steps_per_sec = Some(match ema_steps_per_sec {
                    None => sps,
                    Some(prev) => QLORA_ETA_EMA_ALPHA * sps + (1.0 - QLORA_ETA_EMA_ALPHA) * prev,
                });
                let pct = if total_optimizer_steps_planned > 0 {
                    100.0 * optimizer_step_count as f64 / total_optimizer_steps_planned as f64
                } else {
                    0.0
                };
                const ETA_CALIBRATION_MIN_STEPS: u32 = 8;
                let eta_s_telem: Option<u64> = if optimizer_step_count >= ETA_CALIBRATION_MIN_STEPS
                {
                    ema_steps_per_sec.and_then(|s| {
                        if s > 1e-6 {
                            Some(
                                (total_optimizer_steps_planned.saturating_sub(optimizer_step_count)
                                    as f64
                                    / s) as u64,
                            )
                        } else {
                            None
                        }
                    })
                } else {
                    None
                };
                let eta_str = if optimizer_step_count < ETA_CALIBRATION_MIN_STEPS {
                    "calibrating...".to_string()
                } else {
                    eta_s_telem.map_or("eta ?".into(), |s| {
                        if s >= 3600 {
                            format!("eta ~{}h {:02}m {:02}s", s / 3600, (s % 3600) / 60, s % 60)
                        } else {
                            format!("eta ~{:02}m {:02}s", s / 60, s % 60)
                        }
                    })
                };
                let eff_batch = config.batch_size.max(1) * config.grad_accum.max(1);
                let ema_str = ema_loss_val
                    .map(|v| format!("{:.4}", v))
                    .unwrap_or_else(|| "----".to_string());
                train_log::info(&format!(
                    "E{:02}/{} step={} opt_step={} loss={:.4} (ema={}) lr={:.2e} eff_batch={} {:.1}% {} skips(no_sup={},short={},curric={},oob={}) traj(weighted_pairs={},clamped_pairs={})",
                    epoch,
                    config.epochs,
                    global_step,
                    optimizer_step_count,
                    loss_val,
                    ema_str,
                    lr_applied_this_step,
                    eff_batch,
                    pct,
                    eta_str,
                    skip_no_supervised_positions,
                    skip_short_seq,
                    skip_curriculum,
                    skip_token_id_oob,
                    trajectory_weighted_pairs,
                    trajectory_clamped_pairs
                ));
                let step_payload = telem_helpers::build_train_step_payload(
                    epoch,
                    global_step,
                    optimizer_step_count,
                    loss_val,
                    lr_applied_this_step,
                    eta_s_telem,
                    total_optimizer_steps_planned,
                    skip_no_supervised_positions,
                    skip_short_seq,
                    skip_curriculum,
                    skip_token_id_oob,
                    trajectory_weighted_pairs,
                    trajectory_clamped_pairs,
                    ema_steps_per_sec,
                    total_valid_tokens,
                    total_theoretical_tokens,
                    config.batch_size.max(1) as u64,
                    config.seq_len as u64,
                    total_syntax_weight,
                );
                telemetry::append(out, telemetry_schema::events::TRAIN_STEP, step_payload)?;
                progress_anchor_step = optimizer_step_count;
                progress_anchor_time = now;
                last_progress = now;
            }

            if PAUSE_FLAG.load(Ordering::SeqCst) {
                let ckpt_path = out.join(format!("pause_step_{global_step}.safetensors"));
                trainer
                    .save_adapter(&ckpt_path)
                    .context("save pause adapter")?;
                let state = crate::checkpoint_state::CheckpointState {
                    schema: crate::checkpoint_state::CHECKPOINT_SCHEMA.to_string(),
                    run_id: run_id.to_string(),
                    epoch: epoch as u32,
                    global_step,
                    pair_offset: pair_loop_idx + 1,
                    shuffled_indices: shuffled_indices.clone(),
                    rng_seed: config.seed,
                    adapter_path: ckpt_path.display().to_string(),
                    last_loss: last_loss_val,
                    wall_seconds_elapsed: run_start_inst.elapsed().as_secs_f64(),
                    saved_at_utc: crate::checkpoint_state::CheckpointState::now_utc(),
                };
                state.save(out).context("save CheckpointState on pause")?;
                let wall_secs = run_start_inst.elapsed().as_secs_f64();
                let ms_per_step = if global_step > 0 {
                    (wall_secs * 1000.0) / global_step as f64
                } else {
                    0.0
                };
                train_log::warn(&format!(
                    "Training paused at step {global_step}. Resume with 'vox mens train --resume {}'",
                    out.display()
                ));
                return Ok(TrainingSummary {
                    wall_secs,
                    total_steps: global_step as usize,
                    total_tokens,
                    ms_per_step,
                });
            }

            super::checkpoint_mid::maybe_save_mid_epoch_checkpoint(
                trainer,
                out,
                config,
                db_tx,
                run_id,
                epoch,
                global_step,
                pair_loop_idx,
                &shuffled_indices,
                last_loss_val,
                run_start_inst,
            )?;
        }

        let (val_loss_sum, val_steps) = validation::run_validation_pass(
            &eval_pairs,
            tokenizer,
            device,
            &model,
            system_prompt,
            config,
        );
        if val_steps > 0 {
            last_avg_val_loss = Some(val_loss_sum / val_steps as f64);
        }

        super::epoch_boundary::finish_epoch(
            trainer,
            out,
            config,
            db_tx,
            run_id,
            epoch,
            global_step,
            epoch_steps,
            epoch_loss_sum,
            val_loss_sum,
            val_steps,
            last_loss_val,
            progress_anchor_time,
        )?;
    }

    super::finalize::finalize_training_run(
        trainer,
        bundle,
        out,
        config,
        db_tx,
        run_id,
        device_label,
        adapter_layer_order,
        base_key_map,
        global_step,
        optimizer_step_count,
        total_tokens,
        total_step_count,
        total_loss_sum,
        last_avg_val_loss,
        TrainingLoopStats {
            skip_no_supervised_positions,
            skip_short_seq,
            skip_curriculum,
            skip_token_id_oob,
        },
        run_start_inst,
    )
}
