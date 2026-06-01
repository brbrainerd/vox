//! `PopuliAction::Train` implementation (corpus preflight + `schola::train`).

use std::path::{Path, PathBuf};

use super::action::{
    MensTokenizerCli, PopuliTrainBackendCli, TrainDataModeCli, TrainingDeploymentTargetCli,
};
use crate::commands::schola::train;

#[allow(clippy::too_many_arguments)]
pub async fn run_train(
    model: Option<String>,
    device: String,
    backend: PopuliTrainBackendCli,
    data_dir: PathBuf,
    output_dir: PathBuf,
    rank: Option<usize>,
    alpha: Option<f32>,
    seq_len: Option<usize>,
    batch_size: Option<usize>,
    grad_accum: Option<usize>,
    resume: Option<PathBuf>,
    epochs: Option<usize>,
    lr: Option<f64>,
    warmup: Option<usize>,
    seed: u64,
    min_rating: Option<u8>,
    preset: Option<String>,
    domain: Option<String>,
    deployment_target: TrainingDeploymentTargetCli,
    process_priority: String,
    vram_limit_fraction: Option<f32>,
    background: bool,
    log_dir: Option<PathBuf>,
    adapter_tag: Option<String>,
    context_filter: Option<String>,
    tokenizer: MensTokenizerCli,
    qlora_no_double_quant: bool,
    qlora_require_full_proxy_stack: bool,
    qlora_allow_partial_proxy_stack: bool,
    qlora_lm_head_only: bool,
    qlora_max_skip_rate: Option<f32>,
    qlora_proxy_max_layers: Option<usize>,
    qlora_ce_last_k: usize,
    checkpoint_every: Option<usize>,
    force_restart: bool,
    require_gpu: bool,
    allow_cpu_fallback: bool,
    base_model_family: Option<String>,
    upstream_model_id: Option<String>,
    license_class: Option<String>,
    attribution_required: bool,
    trajectory_weighting_enabled: bool,
    trajectory_tool_trace_boost: f32,
    trajectory_failure_category_boost: f32,
    trajectory_quality_floor: Option<u8>,
    trajectory_quality_boost: f32,
    cloud: String,
    _max_budget: Option<f64>,
    _train_data_hf: Option<String>,
    _adapter_upload_hf: Option<String>,
    _max_runtime_secs: Option<u64>,
    validation_split_ratio: f64,
    curriculum: bool,
    optimizer_experiment_mode: vox_populi::mens::OptimizerExperimentMode,
    data_mode: TrainDataModeCli,
    fast_corpus: bool,
    persistent: bool,
) -> anyhow::Result<()> {
    if cloud != "local" {
        #[cfg(feature = "cloud")]
        {
            use vox_populi::mens::cloud::{CloudJobSpec, CloudResolver};
            let config = vox_populi::mens::cloud::CloudProviderConfig::default();
            let mut spec = CloudJobSpec::new_train(&config);
            spec.model_id = model.unwrap_or_else(|| vox_populi::mens::DEFAULT_MODEL_ID.to_string());
            spec.train_data_hf = _train_data_hf;
            spec.adapter_upload_hf = _adapter_upload_hf;
            spec.max_budget_usd = _max_budget;
            spec.max_runtime_secs = _max_runtime_secs;
            spec.preset = preset.clone().unwrap_or_else(|| "auto".to_string());
            spec.seq_len = seq_len.unwrap_or(512);
            spec.batch_size = batch_size.unwrap_or(4);
            spec.epochs = epochs.unwrap_or(3);
            spec.num_samples = 5000;
            spec.persistent = persistent;

            let resolver = CloudResolver::new_from_env().await?;
            return resolver.dispatch(spec, &cloud).await;
        }
        #[cfg(not(feature = "cloud"))]
        {
            anyhow::bail!(
                "Cloud dispatch requires the 'cloud' feature. Rebuild with: cargo build -p vox-cli --features cloud"
            );
        }
    }
    let process_priority = if background {
        "low".to_string()
    } else {
        process_priority
    };
    let vram_limit_fraction = if background {
        vram_limit_fraction.or(Some(0.8))
    } else {
        vram_limit_fraction
    };

    let workspace_root = vox_corpus::training::contract::find_workspace_root();
    let data_dir = vox_corpus::training::contract::normalize_workspace_relative_path(
        data_dir,
        workspace_root.as_deref(),
    );
    let output_dir = vox_corpus::training::contract::normalize_workspace_relative_path(
        output_dir,
        workspace_root.as_deref(),
    );
    let resume = resume.map(|r| {
        vox_corpus::training::contract::normalize_training_resume_path(r, workspace_root.as_deref())
    });

    #[allow(unsafe_code)]
    unsafe {
        if fast_corpus {
            std::env::set_var("VOX_TRAIN_SKIP_CORPUS_MIX", "1");
        } else {
            std::env::remove_var("VOX_TRAIN_SKIP_CORPUS_MIX");
        }
    }

    // Preflight: stale corpus fingerprint → same refresh path for both data modes (synthetic + pipeline w/o train + mix).
    // `strict`: refresh failures abort. `auto-refresh`: log warnings and continue (legacy).
    if let Some(ref root) = workspace_root {
        use owo_colors::OwoColorize;
        let current_fp = vox_corpus::corpus::preflight::compute_corpus_fingerprint(root);

        let fingerprint_fresh = if let Ok(db) = vox_db::VoxDb::connect_default().await {
            db.is_corpus_fresh(&current_fp).await.unwrap_or(false)
        } else {
            let fp_file = vox_corpus::corpus::preflight::fingerprint_cache_path(root);
            vox_corpus::corpus::preflight::corpus_is_fresh(root, &fp_file)
        };

        // Version gate: even when the input fingerprint is unchanged, a corpus
        // produced by a different compiler version may use a stale pair-encoding
        // schema. Force a refresh when `metadata.json`'s `compiler_version` does
        // not match this build, so a version bump alone triggers regeneration.
        let version_mismatch = corpus_compiler_version_mismatch(&data_dir);
        let is_fresh = fingerprint_fresh && version_mismatch.is_none();

        let skip_regen = vox_corpus::training::mix_prepare::corpus_mix_skip_from_env();
        if !is_fresh && !skip_regen {
            let strict = matches!(data_mode, TrainDataModeCli::Strict);
            let reason = match &version_mismatch {
                Some((found, current)) => format!(
                    "compiler version changed ({found} → {current}); fingerprint: {current_fp}"
                ),
                None => format!("fingerprint: {current_fp}"),
            };
            eprintln!(
                "  {} Stale corpus detected ({}). {}",
                "🔄".cyan(),
                reason,
                if strict {
                    "Running blocking refresh before train..."
                } else {
                    "Regenerating..."
                }
            );
            let res =
                refresh_stale_training_corpus(root, &data_dir, &output_dir, &current_fp, strict)
                    .await;
            if strict {
                res?;
            } else {
                let _ = res;
            }
        }
    }

    let mut effective_min_rating = min_rating;
    let mut effective_ce_last_k = qlora_ce_last_k;
    // Resolution order for memory-sizing knobs: explicit CLI > domain profile >
    // VRAM-aware budget > preset fallback (applied in gpu.rs). `None` means
    // "not yet set"; each stage fills only what an earlier stage left unset.
    let mut effective_seq_len: Option<usize> = seq_len;
    let mut effective_batch_size: Option<usize> = batch_size;
    let mut effective_grad_accum: Option<usize> = grad_accum;
    // May be retreated to a smaller Qwen3.5 variant by the VRAM budget below.
    let mut effective_model = model;
    let mut effective_validation_split_ratio = validation_split_ratio;
    let mut _effective_max_grad_norm = None; // pass down if needed
    let mut effective_curriculum = curriculum;
    let mut effective_trajectory_weighting_enabled = trajectory_weighting_enabled;
    let mut effective_trajectory_tool_trace_boost = trajectory_tool_trace_boost;
    let mut effective_context_filter = None;
    let mut effective_adapter_tag = adapter_tag.clone();
    let mut effective_curriculum_schedule = None;
    let mut effective_chatml = vox_populi::mens::tensor::training_config::ChatmlConfig::default();
    let mut effective_mix_config = None;

    if let Some(domain_name) = &domain {
        match vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile::load_domain_profile(
            domain_name,
            workspace_root.as_deref(),
        ) {
            Ok(profile) => {
                use owo_colors::OwoColorize;
                eprintln!(
                    "  {} Applied domain profile: {}",
                    "✓".green(),
                    domain_name.cyan()
                );

                if let Some(desc) = &profile.description {
                    eprintln!("    Description: {}", desc.dimmed());
                }

                effective_min_rating = profile.min_rating.or(min_rating);
                effective_ce_last_k = profile.ce_last_k.unwrap_or(qlora_ce_last_k);
                // Domain seq_len fills the slot only when the user did not pass --seq-len.
                if effective_seq_len.is_none() {
                    effective_seq_len = profile.seq_len;
                }
                effective_validation_split_ratio = profile
                    .validation_split_ratio
                    .unwrap_or(validation_split_ratio);
                _effective_max_grad_norm = profile.max_grad_norm;
                effective_curriculum = profile.curriculum.unwrap_or(curriculum);
                effective_trajectory_weighting_enabled = profile
                    .trajectory_weighting
                    .unwrap_or(trajectory_weighting_enabled);
                if let Some(boost) = profile.trajectory_tool_trace_boost {
                    effective_trajectory_tool_trace_boost = boost;
                }
                effective_context_filter = profile.context_filter.clone();
                if effective_adapter_tag.is_none() {
                    effective_adapter_tag = Some(domain_name.clone());
                }
                effective_curriculum_schedule = profile.curriculum_schedule.clone();
                effective_chatml = profile.chatml.clone();

                if let Some(ref mix_path) = profile.mix_config {
                    effective_mix_config = Some(mix_path.clone());
                    // Update env var to point mix to this one if `vox mens corpus mix` called?
                    // Actually simply inform.
                    eprintln!("    Mix config: {}", mix_path.display());
                }
            }
            Err(e) => {
                anyhow::bail!("Failed to load domain profile '{}': {}", domain_name, e);
            }
        }
    }

    // ── VRAM-aware memory budgeting ──────────────────────────────────────────
    // When seq_len / batch_size / grad_accum were not pinned by the user or a
    // domain profile, size them from the detected VRAM and model size so the run
    // fits without OOM and still maximizes utilization. Only applies on CUDA.
    {
        use owo_colors::OwoColorize;
        let device_is_cuda = vox_populi::mens::normalize_device(&device)
            .map(|d| matches!(d, vox_populi::mens::DeviceKind::Cuda))
            .unwrap_or(false);
        if device_is_cuda {
            use vox_populi::mens::tensor::memory_budget;
            let model_hint = effective_model
                .as_deref()
                .unwrap_or(vox_populi::mens::DEFAULT_MODEL_ID);
            let requested_b = memory_budget::params_b_from_model_hint(model_hint).unwrap_or(4.0);
            if let Some(vram_gb) = vox_populi::mens::tensor::vram_autodetect::get_system_vram_gb() {
                let vram = vram_gb as f64;
                // For Qwen3.5 models (including the default), let the ladder pick the
                // largest variant that fits this card — retreating 4B → 2B → 0.8B as
                // needed, or scaling the sequence/batch up on roomier GPUs.
                if memory_budget::is_qwen35(model_hint) {
                    let mp = memory_budget::plan_qwen35(vram, requested_b);
                    eprintln!("  {} VRAM budget: {}", "⚙".cyan(), mp.rationale);
                    if let Some(from_b) = mp.retreated_from_b {
                        // Only switch the model when the user did not explicitly pin one.
                        if effective_model.is_none() {
                            eprintln!(
                                "  {} Auto-selected {} for {:.0} GiB VRAM (requested ≈{:.1}B would OOM).",
                                "↓".yellow(),
                                mp.model_id,
                                vram,
                                from_b
                            );
                            effective_model = Some(mp.model_id.clone());
                        } else {
                            eprintln!(
                                "  {} {} is pinned but does not fit {:.0} GiB — it may OOM. \
                                 Omit --model to auto-retreat to {}.",
                                "⚠".yellow(),
                                model_hint,
                                vram,
                                mp.model_id
                            );
                        }
                    }
                    if effective_seq_len.is_none() {
                        effective_seq_len = Some(mp.seq_len);
                    }
                    if effective_batch_size.is_none() {
                        effective_batch_size = Some(mp.batch_size);
                    }
                    if effective_grad_accum.is_none() {
                        effective_grad_accum = Some(mp.grad_accum);
                    }
                } else {
                    // Non-Qwen3.5: size the given model in place (no model swap).
                    let plan = memory_budget::plan(vram, requested_b);
                    eprintln!("  {} VRAM budget: {}", "⚙".cyan(), plan.rationale);
                    if plan.over_budget {
                        eprintln!(
                            "  {} {:.1}B params is tight for {:.0} GiB VRAM — using the smallest \
                             safe config. If it still OOMs, train a smaller base model.",
                            "⚠".yellow(),
                            requested_b,
                            vram
                        );
                    }
                    if effective_seq_len.is_none() {
                        effective_seq_len = Some(plan.seq_len);
                    }
                    if effective_batch_size.is_none() {
                        effective_batch_size = Some(plan.batch_size);
                    }
                    if effective_grad_accum.is_none() {
                        effective_grad_accum = Some(plan.grad_accum);
                    }
                }
            } else {
                eprintln!(
                    "  {} Could not detect VRAM; using preset defaults. Set VOX_VRAM_OVERRIDE_GB \
                     to enable VRAM-aware sizing.",
                    "⚙".cyan()
                );
            }
        }
    }

    let parsed_filter = if let Some(cf) = effective_context_filter {
        Some(cf)
    } else {
        context_filter
            .or_else(|| effective_adapter_tag.clone())
            .map(
                |s| vox_populi::mens::tensor::training_config::ContextFilter {
                    categories: Some(vec![s]),
                    ..Default::default()
                },
            )
    };

    let spawn_log_dir = if background {
        Some(log_dir.clone().unwrap_or_else(|| {
            workspace_root
                .as_ref()
                .map(|r| r.join("mens/runs/logs"))
                .unwrap_or_else(|| PathBuf::from("mens/runs/logs"))
        }))
    } else {
        log_dir.clone()
    };
    if let Some(ref log_dir) = spawn_log_dir {
        return crate::commands::schola::train::spawn_train_with_log(log_dir.clone());
    }
    let deployment_target = if preset.as_deref() == Some("mobile_edge") {
        vox_populi::mens::TrainingDeploymentTarget::MobileEdge
    } else {
        deployment_target.into()
    };
    let train_res = train::run_train(
        backend.into(),
        effective_model,
        device,
        data_dir,
        output_dir,
        rank,
        alpha,
        effective_seq_len,
        effective_batch_size,
        effective_grad_accum,
        resume,
        epochs,
        lr,
        warmup,
        seed,
        effective_min_rating,
        preset,
        deployment_target,
        process_priority,
        vram_limit_fraction,
        effective_adapter_tag,
        parsed_filter,
        Some(effective_validation_split_ratio),
        tokenizer.into(),
        qlora_no_double_quant,
        qlora_require_full_proxy_stack,
        qlora_allow_partial_proxy_stack,
        qlora_max_skip_rate,
        qlora_lm_head_only,
        qlora_proxy_max_layers,
        effective_ce_last_k,
        checkpoint_every,
        force_restart,
        effective_curriculum,
        optimizer_experiment_mode,
        require_gpu,
        allow_cpu_fallback,
        base_model_family,
        upstream_model_id,
        license_class,
        attribution_required,
        effective_trajectory_weighting_enabled,
        effective_trajectory_tool_trace_boost,
        trajectory_failure_category_boost,
        trajectory_quality_floor,
        trajectory_quality_boost,
        effective_curriculum_schedule,
        effective_chatml,
        effective_mix_config,
    )
    .await;

    if train_res.is_ok() {
        if let Some(ref r) = workspace_root {
            let mixed = r.join("target/dogfood/train_mixed.jsonl");
            let backup = r.join("mens/data/train_full_backup.jsonl");
            if mixed.exists() {
                if let Some(parent) = backup.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::copy(&mixed, &backup) {
                    use owo_colors::OwoColorize;
                    eprintln!(
                        "  {} Failed to copy train_mixed.jsonl to backup: {e}",
                        "⚠️".yellow()
                    );
                } else {
                    use owo_colors::OwoColorize;
                    eprintln!(
                        "  {} Backed up running corpus to {}",
                        "✓".green(),
                        backup.display()
                    );
                }
            }
        }
    }

    train_res
}

/// The version this build stamps into freshly generated corpora.
fn current_corpus_compiler_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Inspect `<data_dir>/metadata.json` and return `Some((found, current))` when its
/// recorded `compiler_version` differs from this build's version (i.e. the corpus
/// was generated by a different compiler and should be regenerated). Returns `None`
/// when the versions match, when there is no metadata yet (a fresh build will create
/// it), or when the field is absent/unparseable (don't force a refresh on noise).
fn corpus_compiler_version_mismatch(data_dir: &Path) -> Option<(String, String)> {
    let meta_path = data_dir.join("metadata.json");
    let raw = std::fs::read_to_string(&meta_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let found = value.get("compiler_version")?.as_str()?.trim().to_string();
    if found.is_empty() {
        return None;
    }
    let current = current_corpus_compiler_version();
    if found != current {
        Some((found, current.to_string()))
    } else {
        None
    }
}

/// Regenerate synthetic data, run `vox mens pipeline` with `skip_train`, optionally copy mix → `train.jsonl`,
/// then record fingerprint. See [`TrainDataModeCli`](super::action::TrainDataModeCli).
async fn refresh_stale_training_corpus(
    root: &Path,
    data_dir: &PathBuf,
    output_dir: &PathBuf,
    current_fp: &str,
    strict: bool,
) -> anyhow::Result<()> {
    use anyhow::Context;
    use owo_colors::OwoColorize;

    if strict {
        vox_corpus::corpus::preflight::clean_corpus_targets(root)
            .map_err(|e| anyhow::anyhow!("clean_corpus_targets: {e}"))?;
    } else {
        let _ = vox_corpus::corpus::preflight::clean_corpus_targets(root);
    }

    let cfg = vox_corpus::synthetic_gen::SyntheticGenConfig::default();
    let out_path = root.join("mens/data/synthetic.jsonl");
    let mut pairs: i64 = 0;
    match vox_corpus::synthetic_gen::generate_all(&cfg, &out_path) {
        Ok(count) => {
            eprintln!("  {} Regenerated {} synthetic pairs", "✓".green(), count);
            pairs = count as i64;
        }
        Err(e) => {
            if strict {
                return Err(anyhow::anyhow!("synthetic corpus regen: {e}"));
            }
            eprintln!("  {} Synthetic regen failed: {}", "⚠️".yellow(), e);
        }
    }

    eprintln!("  {} Running corpus extraction pipeline...", "🔄".cyan());
    match crate::commands::mens::pipeline::run(
        data_dir.clone(),
        output_dir.clone(),
        true,
        false,
        None,
        None,
        None,
        None,
        None,
        false,
        false,
    )
    .await
    {
        Ok(()) => eprintln!("  {} Corpus extraction pipeline completed.", "✓".green()),
        Err(e) => {
            if strict {
                return Err(e).context("corpus extraction pipeline (stale-fingerprint refresh)");
            }
            eprintln!("  {} Pipeline error: {}", "⚠️".yellow(), e);
        }
    }

    let mix_yaml = root.join(vox_corpus::training::mix_prepare::MIX_CONFIG_REL);
    if mix_yaml.is_file() {
        match vox_corpus::training::mix_prepare::copy_mix_output_to_train_jsonl(
            root, data_dir, &mix_yaml,
        ) {
            Ok(true) => {
                eprintln!(
                    "  {} Mixed data ready at: {}",
                    "✓".green(),
                    data_dir.join("train.jsonl").display()
                );
                #[allow(unsafe_code)]
                unsafe {
                    std::env::set_var("VOX_TRAIN_SKIP_CORPUS_MIX", "1");
                }
            }
            Ok(false) => {
                if strict {
                    anyhow::bail!(
                        "mix output not found after pipeline; check {}",
                        mix_yaml.display()
                    );
                }
                eprintln!(
                    "  {} Mix output not found after pipeline; check {}",
                    "⚠️".yellow(),
                    mix_yaml.display()
                );
            }
            Err(e) => {
                if strict {
                    return Err(e).context(format!(
                        "copy mixed corpus to {}",
                        data_dir.join("train.jsonl").display()
                    ));
                }
                eprintln!(
                    "  {} Failed to copy mixed corpus to train.jsonl: {}",
                    "⚠️".yellow(),
                    e
                );
            }
        }
    }

    if let Ok(db) = vox_db::VoxDb::connect_default().await {
        if strict {
            db.record_corpus_snapshot(current_fp, env!("CARGO_PKG_VERSION"), pairs, None)
                .await
                .map_err(|e| anyhow::anyhow!("record_corpus_snapshot: {e}"))?;
        } else {
            let _ = db
                .record_corpus_snapshot(current_fp, env!("CARGO_PKG_VERSION"), pairs, None)
                .await;
        }
    } else {
        let fp_file = vox_corpus::corpus::preflight::fingerprint_cache_path(root);
        if strict {
            vox_corpus::corpus::preflight::write_fingerprint_snapshot(root, &fp_file)
                .map_err(|e| anyhow::anyhow!("write fingerprint snapshot: {e}"))?;
        } else {
            let _ = vox_corpus::corpus::preflight::write_fingerprint_snapshot(root, &fp_file);
        }
    }

    Ok(())
}
