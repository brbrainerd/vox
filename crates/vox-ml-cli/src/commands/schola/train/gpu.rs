//! GPU-enabled training path (`vox mens train` with `gpu` feature).

use anyhow::Result;
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_gpu_training(
    train_backend: vox_populi::mens::PopuliTrainBackend,
    model: Option<String>,
    device: String,
    data_dir: PathBuf,
    output_dir: PathBuf,
    resume: Option<PathBuf>,
    preset: Option<String>,
    device_profile: vox_populi::mens::DeviceProfile,
    cli_overrides: vox_populi::mens::CliOverrides,
    gpu_info: vox_populi::mens::GpuInfo,
    device_kind: vox_populi::mens::DeviceKind,
    min_rating: Option<u8>,
    deployment_target: vox_populi::mens::TrainingDeploymentTarget,
    tokenizer_mode: vox_populi::mens::MensTokenizerMode,
    qlora_no_double_quant: bool,
    qlora_require_full_proxy_stack: bool,
    qlora_max_skip_rate: Option<f32>,
    qlora_lm_head_only: bool,
    qlora_proxy_max_layers: Option<usize>,
    qlora_ce_last_k: usize,
    checkpoint_every: Option<usize>,
    force_restart: bool,
    curriculum: bool,
    optimizer_experiment_mode: vox_populi::mens::OptimizerExperimentMode,
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
    vram_limit_fraction: Option<f32>,
    adapter_tag: Option<String>,
    context_filter: Option<vox_populi::mens::tensor::training_config::ContextFilter>,
    validation_split_ratio: Option<f64>,
    seed: u64,
    curriculum_schedule: Option<vox_populi::mens::tensor::training_config::CurriculumSchedule>,
    chatml: vox_populi::mens::tensor::training_config::ChatmlConfig,
    mix_config: Option<PathBuf>,
) -> Result<()> {
    use owo_colors::OwoColorize;

    let workspace_root = vox_corpus::training::contract::find_workspace_root();
    let skip_mix = vox_corpus::training::mix_prepare::corpus_mix_skip_from_env();
    if skip_mix {
        eprintln!(
            "  {} Skipping corpus mix (`VOX_TRAIN_SKIP_CORPUS_MIX`); using train file under data-dir",
            "⏭".cyan()
        );
    }
    let mix_path = mix_config.unwrap_or_else(|| {
        vox_corpus::training::mix_prepare::resolve_mix_config_path(workspace_root.as_deref())
    });
    if !skip_mix && mix_path.is_file() {
        eprintln!(
            "  {} Running corpus mix to refresh training data...",
            "🔄".cyan()
        );
    }
    let contract_override =
        vox_corpus::training::mix_prepare::refresh_train_contract_override_from_mix(
            workspace_root.as_deref(),
            &data_dir,
            skip_mix,
            true,
            Some(&mix_path),
        )?;

    let resolved = vox_corpus::training::preflight::validate_train_preflight(
        &data_dir,
        contract_override.as_deref(),
        workspace_root.as_deref(),
    )?;
    tracing::debug!(path = %resolved.path.display(), source = ?resolved.source, "Preflight resolved train input");

    let mut final_preset = preset.clone();
    if final_preset.is_none() && device.to_lowercase() == "cuda" {
        eprintln!(
            "  {} {}",
            "⚙".cyan(),
            vox_populi::mens::tensor::vram_autodetect::vram_summary(true)
        );
        let auto_preset = vox_populi::mens::tensor::vram_autodetect::auto_preset(
            true,
            vox_populi::mens::tensor::vram_autodetect::get_system_vram_gb(),
        );
        if let Some(ap) = auto_preset {
            eprintln!(
                "  {} Auto-detected 16 GB VRAM → using preset '{}'",
                "⚙".cyan(),
                ap
            );
            final_preset = Some(ap.to_string());
        }
    }

    let profile = vox_populi::mens::resolve_effective_profile(
        final_preset.as_deref(),
        device_profile,
        resolved.sample_count,
        cli_overrides,
    );
    let rank = profile.rank;
    let alpha = profile.alpha;
    let seq_len = profile.seq_len;
    if matches!(
        train_backend,
        vox_populi::mens::PopuliTrainBackend::CandleQlora
    ) {
        let k = qlora_ce_last_k;
        if k > 0 && k > 64 {
            anyhow::bail!("--qlora-ce-last-k must be at most 64 (got {k})");
        }
        if k > seq_len {
            anyhow::bail!(
                "--qlora-ce-last-k ({k}) cannot exceed effective sequence length ({seq_len})"
            );
        }
    }
    let batch_size = profile.batch_size;
    let grad_accum = profile.grad_accum;
    let epochs = profile.epochs;
    let warmup = profile.warmup;
    let lr = profile.lr;
    if !trajectory_tool_trace_boost.is_finite() || trajectory_tool_trace_boost < 0.0 {
        anyhow::bail!(
            "--trajectory-tool-trace-boost must be finite and non-negative (got {trajectory_tool_trace_boost})"
        );
    }
    if !trajectory_failure_category_boost.is_finite() || trajectory_failure_category_boost < 0.0 {
        anyhow::bail!(
            "--trajectory-failure-category-boost must be finite and non-negative (got {trajectory_failure_category_boost})"
        );
    }
    if !trajectory_quality_boost.is_finite() || trajectory_quality_boost < 0.0 {
        anyhow::bail!(
            "--trajectory-quality-boost must be finite and non-negative (got {trajectory_quality_boost})"
        );
    }
    if let Some(q) = trajectory_quality_floor
        && !(1..=5).contains(&q)
    {
        anyhow::bail!("--trajectory-quality-floor must be between 1 and 5 (got {q})");
    }

    let mut base_model_paths = None::<(Vec<std::path::PathBuf>, std::path::PathBuf)>;
    let mut tokenizer_path = None::<std::path::PathBuf>;
    if let Some(ref repo_id) = model {
        eprintln!(
            "  {} Downloading base model from Hugging Face: {}",
            "📥".cyan(),
            repo_id
        );
        let files = vox_populi::mens::hub::download_model_blocking(repo_id).map_err(|e| {
            anyhow::anyhow!(
                "HF download failed for `{repo_id}` ({e}). \
                 Set HF token env vars if this is a gated repo and retry."
            )
        })?;
        if !files.is_safetensors() {
            anyhow::bail!(
                "HF model `{repo_id}` has no safetensors; QLoRA requires safetensors base weights."
            );
        }
        base_model_paths = Some((files.weights.clone(), files.config.clone()));
        tokenizer_path = files.tokenizer.clone();
        eprintln!("  {} Cached at {}", "✓".green(), files.cache_dir.display());

        if let Ok(arch) = vox_populi::mens::tensor::hf_load::detect_hf_architecture(&files.config) {
            eprintln!("  {} Architecture: {:?}", "📐".cyan(), arch);
            let cfg = vox_populi::mens::tensor::hf_load::config_dims_for_architecture(
                &files.config,
                arch,
            )
            .map_err(|e| anyhow::anyhow!("HF config: {}", e))?;
            let tokenizer_src = tokenizer_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Vox (built-in)".to_string());
            eprintln!("  {} Tokenizer: {}", "🔤".cyan(), tokenizer_src);
            let est_mb = if matches!(
                train_backend,
                vox_populi::mens::PopuliTrainBackend::CandleQlora
            ) {
                vox_populi::mens::estimate_training_vram_mb_qlora(
                    cfg.n_embd,
                    cfg.n_head,
                    cfg.n_layer,
                    cfg.vocab_size,
                    profile.batch_size,
                    profile.seq_len,
                )
            } else {
                vox_populi::mens::estimate_training_vram_mb(
                    cfg.n_embd,
                    cfg.n_head,
                    cfg.n_layer,
                    cfg.vocab_size,
                    profile.batch_size,
                    profile.seq_len,
                )
            };
            if gpu_info.vram_mb > 0 && est_mb as f64 > gpu_info.vram_mb as f64 * 0.85 {
                eprintln!(
                    "  {} VRAM risk: est. {} MB > 85% of {} MB. Try --batch-size 2 --seq-len 256 or VOX_TRAIN_PROFILE=safe",
                    "⚠".yellow(),
                    est_mb,
                    gpu_info.vram_mb
                );
            } else if gpu_info.vram_mb > 0 {
                eprintln!(
                    "  {} VRAM: est. ~{} MB / {} MB available",
                    "✓".green(),
                    est_mb,
                    gpu_info.vram_mb
                );
            }
        }
    }

    let train_file_path =
        vox_corpus::training::mix_prepare::recover_train_input_path_after_prefetch(
            workspace_root.as_deref(),
            &data_dir,
            &mix_path,
            skip_mix,
            &resolved.path,
        )?;

    let run_id = vox_corpus::training::timestamp_string();
    let git_sha = option_env!("VOX_GIT_HASH").unwrap_or("unknown").to_string();
    let device_profile_str = if device_kind == vox_populi::mens::DeviceKind::Cpu {
        "cpu".to_string()
    } else {
        gpu_info.model_name.clone()
    };
    let config = vox_populi::mens::LoraTrainingConfig {
        base_model: model,
        base_model_family,
        upstream_model_id,
        license_class,
        attribution_required,
        base_model_paths,
        tokenizer_path,
        train_file: Some(train_file_path),
        rank,
        alpha,
        seq_len,
        batch_size,
        grad_accum,
        resume_from: resume,
        epochs,
        learning_rate: lr,
        warmup_steps: warmup,
        seed,
        min_rating: min_rating.unwrap_or(3),
        run_id: Some(run_id),
        git_sha: Some(git_sha),
        device_profile: Some(device_profile_str.clone()),
        max_vram_fraction: vram_limit_fraction,
        adapter_tag,
        context_filter,
        validation_split_ratio,
        tokenizer_mode,
        qlora_double_quant: !qlora_no_double_quant,
        finetune_contract_digest: None,
        qlora_require_full_proxy_stack,
        qlora_max_skip_rate,
        qlora_lm_head_only,
        qlora_proxy_max_layers,
        qlora_ce_last_k,
        checkpoint_every,
        force_restart,
        deployment_target,
        curriculum,
        optimizer_experiment_mode,
        trajectory_weighting_enabled,
        trajectory_tool_trace_boost,
        trajectory_failure_category_boost,
        trajectory_quality_floor,
        trajectory_quality_boost,
        require_gpu,
        allow_cpu_fallback,
        curriculum_schedule,
        chatml,
        reward_hook: None,
        launch_argv: std::env::args().collect(),
    };
    let model_name_for_stats = config
        .base_model
        .clone()
        .unwrap_or_else(|| "scratch".to_string());
    let preset_for_stats = preset.clone().unwrap_or_else(|| "unknown".to_string());

    let system_prompt = vox_corpus::training::generate_training_system_prompt();

    let summary = vox_populi::mens::run_mens_training(
        train_backend,
        &data_dir,
        Some(&output_dir),
        &config,
        device_kind,
        &system_prompt,
    )?;

    if let Ok(db) = vox_db::VoxDb::connect_default().await {
        let _ = db
            .local_log_train_run(
                &device_profile_str,
                &model_name_for_stats,
                &preset_for_stats,
                summary.wall_secs,
                summary.total_steps as i64,
                summary.total_tokens as i64,
                Some(summary.ms_per_step),
            )
            .await;
    }

    Ok(())
}
