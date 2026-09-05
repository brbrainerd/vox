use anyhow::Result;
use std::path::PathBuf;

#[allow(
    clippy::too_many_arguments,
    unused_variables,
    unused_assignments,
    unsafe_code
)]
pub async fn run_train(
    train_backend: vox_populi::mens::PopuliTrainBackend,
    model: Option<String>,
    device: String,
    data_dir: PathBuf,
    output_dir: PathBuf,
    rank: Option<usize>,
    alpha: Option<f32>,
    seq_len: Option<usize>,
    batch_size: Option<usize>,
    grad_accum: Option<usize>,
    budget_seq_len: Option<usize>,
    budget_batch_size: Option<usize>,
    budget_grad_accum: Option<usize>,
    resume: Option<PathBuf>,
    epochs: Option<usize>,
    lr: Option<f64>,
    warmup: Option<usize>,
    seed: u64,
    min_rating: Option<u8>,
    preset: Option<String>,
    deployment_target: vox_populi::mens::TrainingDeploymentTarget,
    process_priority: String,
    vram_limit_fraction: Option<f32>,
    adapter_tag: Option<String>,
    context_filter: Option<vox_populi::mens::tensor::training_config::ContextFilter>,
    validation_split_ratio: Option<f64>,
    tokenizer_mode: vox_populi::mens::MensTokenizerMode,
    qlora_no_double_quant: bool,
    qlora_require_full_proxy_stack: bool,
    qlora_allow_partial_proxy_stack: bool,
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
    curriculum_schedule: Option<vox_populi::mens::tensor::training_config::CurriculumSchedule>,
    chatml: vox_populi::mens::tensor::training_config::ChatmlConfig,
    mix_config: Option<PathBuf>,
) -> Result<()> {
    use owo_colors::OwoColorize;

    super::super::process_priority::apply(&process_priority);

    let device_kind =
        vox_populi::mens::normalize_device(&device).map_err(|e| anyhow::anyhow!("{}", e))?;
    vox_populi::mens::apply_backend_env(device_kind);

    if matches!(
        deployment_target,
        vox_populi::mens::TrainingDeploymentTarget::MobileEdge
    ) && !matches!(device_kind, vox_populi::mens::DeviceKind::Cpu)
    {
        anyhow::bail!(vox_populi::mens::operator_messages::MOBILE_EDGE_REQUIRES_CPU_DEVICE);
    }

    if matches!(
        train_backend,
        vox_populi::mens::PopuliTrainBackend::CandleQlora
    ) && tokenizer_mode != vox_populi::mens::MensTokenizerMode::Hf
    {
        anyhow::bail!(vox_populi::mens::operator_messages::QLORA_REQUIRES_HF_TOKENIZER);
    }
    if qlora_allow_partial_proxy_stack && qlora_require_full_proxy_stack {
        anyhow::bail!(
            "`--qlora-allow-partial-proxy-stack` cannot be combined with `--qlora-require-full-proxy-stack`."
        );
    }
    if let Some(r) = qlora_max_skip_rate {
        if !r.is_finite() || !(0.0..=1.0).contains(&r) {
            anyhow::bail!("--qlora-max-skip-rate must be between 0.0 and 1.0 (got {r})");
        }
    }

    let mut model = model;
    if matches!(
        train_backend,
        vox_populi::mens::PopuliTrainBackend::CandleQlora
    ) && model.is_none()
    {
        let resolved = vox_populi::mens::resolve_default_model_id(
            std::env::var("VOX_MENS_DEFAULT_MODEL").ok().as_deref(),
        );
        tracing::info!(
            model = %resolved,
            "Using default HF model for Candle QLoRA (`--model` omitted; see contracts/mens/training-presets.v1.yaml)."
        );
        model = Some(resolved);
    }

    let effective_qlora_require_full_proxy_stack = !qlora_allow_partial_proxy_stack
        && (qlora_require_full_proxy_stack
            || (matches!(
                train_backend,
                vox_populi::mens::PopuliTrainBackend::CandleQlora
            ) && matches!(device_kind, vox_populi::mens::DeviceKind::Cuda)));

    if effective_qlora_require_full_proxy_stack && qlora_lm_head_only {
        anyhow::bail!(
            "Full proxy stack (CUDA default or `--qlora-require-full-proxy-stack`) conflicts with `--qlora-lm-head-only`; pick one."
        );
    }
    if qlora_lm_head_only && qlora_proxy_max_layers.is_some_and(|m| m > 0) {
        anyhow::bail!(
            "--qlora-lm-head-only conflicts with --qlora-proxy-max-layers > 0; omit the cap or use `--qlora-proxy-max-layers 0`."
        );
    }

    if matches!(
        train_backend,
        vox_populi::mens::PopuliTrainBackend::CandleQlora
    ) {
        if matches!(device_kind, vox_populi::mens::DeviceKind::Cuda) {
            #[cfg(not(feature = "mens-candle-cuda"))]
            anyhow::bail!(
                "`--device cuda` for Candle QLoRA requires a CUDA-enabled build.\n\
                 Rebuild: `cargo build -p vox-ml-cli --features gpu,mens-candle-cuda` (or `cargo vox-cuda-release`).\n\
                 On Windows use a VS Developer shell so `nvcc` can find MSVC."
            );
            // Preflight: make sure the runtime cuda plugin is installed + loadable,
            // self-healing (rebuild + reinstall) on a stale/missing/ABI-mismatched
            // plugin so a plain `vox mens train --device cuda` works out of the box.
            // Opt out with `--no-auto-heal` (sets VOX_MENS_NO_AUTO_HEAL).
            // Gated on `mens-candle-cuda`: that is when the real CUDA dispatch is
            // compiled (and it implies `gpu`, so plugin_heal is available).
            #[cfg(feature = "mens-candle-cuda")]
            crate::commands::mens::plugin_heal::ensure_cuda_plugin(true)?;
        }
        #[cfg(target_os = "macos")]
        if matches!(device_kind, vox_populi::mens::DeviceKind::Metal) {
            // DEAD GATE REMOVED: this used to be `#[cfg(not(feature =
            // "mens-candle-metal"))]`, but no such Cargo feature exists on
            // vox-ml-cli (or any crate) — `mens-candle-metal` is a *runtime
            // plugin* id (crates/vox-plugin-mens-candle-metal, see
            // vox-plugin-catalog/catalog.toml), not a build feature. The
            // negation was therefore always true, so the bail always fired
            // while advertising an unbuildable remediation.
            //
            // The gate is now unconditional, which is behaviourally identical
            // and honest: Metal QLoRA training genuinely has no backend today.
            // vox-populi exposes `mens-candle-qlora-cuda` but no Metal twin
            // (no `candle-core/metal` wiring), and the plugin's
            // `run_train_step`/`run_eval_step` still return unimplemented
            // pending the SP3-D host protocol
            // (crates/vox-plugin-mens-candle-metal/src/training.rs). Declaring
            // a `mens-candle-metal` feature here would only move the dead end.
            // Re-gate this once vox-populi grows a Metal Candle backend.
            anyhow::bail!(
                "`--device metal` for Candle QLoRA is not supported yet: there is no \
                 Metal-enabled Candle training backend in this build.\n\
                 Use `--device cpu` on Apple Silicon, or `--device cuda` on an \
                 NVIDIA host built with `cargo build -p vox-ml-cli --features gpu,mens-candle-cuda`.\n\
                 (Tracking: vox-populi needs a Metal twin of `mens-candle-qlora-cuda`, and \
                 the `mens-candle-metal` plugin needs the SP3-D training host protocol.)"
            );
        }
    }

    tracing::debug!(
        ?train_backend,
        ?tokenizer_mode,
        model = ?model.as_deref(),
        "vox mens train entry (backend + tokenizer SSOT)"
    );
    tracing::debug!(
        model = ?model.as_deref(),
        device = %device,
        ?preset,
        ?rank,
        ?alpha,
        ?seq_len,
        ?batch_size,
        ?grad_accum,
        ?epochs,
        ?warmup,
        ?lr,
        seed,
        qlora_no_double_quant,
        qlora_require_full_proxy_stack,
        qlora_allow_partial_proxy_stack,
        effective_qlora_require_full_proxy_stack,
        qlora_lm_head_only,
        ?qlora_max_skip_rate,
        ?qlora_proxy_max_layers,
        qlora_ce_last_k,
        checkpoint_every,
        force_restart,
        curriculum,
        ?optimizer_experiment_mode,
        require_gpu,
        allow_cpu_fallback,
        ?base_model_family,
        ?upstream_model_id,
        ?license_class,
        attribution_required,
        trajectory_weighting_enabled,
        trajectory_tool_trace_boost,
        ?trajectory_failure_category_boost,
        ?trajectory_quality_floor,
        trajectory_quality_boost,
        ?context_filter,
        "Training parser payload"
    );

    unsafe {
        if let Some(ref m) = model {
            std::env::set_var("VOX_BASE_MODEL", m);
        }
        std::env::set_var("VK_LOADER_DEBUG", "none");
        std::env::set_var("VK_LOADER_LOG_LEVEL", "none");
        std::env::set_var("WGPU_LOG_LEVEL", "error");
        std::env::set_var("WGPU_VALIDATION", "0");
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "info");
        }
    }

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

    let gpu_info = vox_populi::mens::probe_gpu();
    let device_profile =
        vox_populi::mens::DeviceProfile::from_gpu_info(&gpu_info.model_name, gpu_info.vram_mb);
    let cli_overrides = vox_populi::mens::CliOverrides {
        rank,
        alpha,
        seq_len,
        batch_size,
        grad_accum,
        epochs,
        warmup,
        lr,
        budget_seq_len,
        budget_batch_size,
        budget_grad_accum,
        vram_limit_fraction,
    };
    let preview_profile = vox_populi::mens::resolve_effective_profile(
        preset.as_deref(),
        device_profile.clone(),
        None,
        cli_overrides.clone(),
    );

    tracing::debug!(
        model = ?model,
        device = %device,
        rank = preview_profile.rank,
        alpha = preview_profile.alpha,
        "Dispatching training payload to native orchestra"
    );

    // ── P2-009: Corpus threshold gate (research: CL minimum corpus) ────────
    // Refuse to start training if the corpus has fewer than the minimum viable
    // pairs. Current corpus is ~340 (Gap G-11); research proves <500 pairs
    // guarantees catastrophic overfitting.
    // Reference: docs/src/architecture/research-cl-qlora-minimum-corpus-2026.md
    const MIN_CORPUS_PAIRS: usize = 100;
    {
        let train_jsonl = data_dir.join("train.jsonl");
        if train_jsonl.exists() {
            let pair_count = std::fs::read_to_string(&train_jsonl)
                .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
                .unwrap_or(0);
            if pair_count < MIN_CORPUS_PAIRS {
                anyhow::bail!(
                    "Corpus has {} validated pairs (minimum: {}). \
                     Fine-tuning with fewer than {} pairs risks catastrophic overfitting \
                     (see research-cl-qlora-minimum-corpus-2026.md).\n\
                     Use `vox mens serve --rag` for in-context learning until the corpus \
                     reaches the threshold, or generate more pairs with \
                     `vox mens corpus extract` + `vox mens corpus pairs`.",
                    pair_count,
                    MIN_CORPUS_PAIRS,
                    MIN_CORPUS_PAIRS
                );
            }
        }
    }

    eprintln!("{}", "╔══════════════════════════════════════════╗".cyan());
    eprintln!("{}", "║   VoxMens — native fine-tuning (QLoRA)  ║".cyan());
    eprintln!("{}", "╚══════════════════════════════════════════╝".cyan());
    eprintln!();
    eprintln!(
        "  Model:       {}",
        model.as_deref().unwrap_or("(scratch init)").cyan()
    );
    eprintln!("  Device:      {}", device.cyan());
    eprintln!("  Data:        {}", data_dir.display().cyan());
    eprintln!("  Output:      {}", output_dir.display().cyan());
    eprintln!(
        "  Rank/Alpha:  {}/{}",
        preview_profile.rank, preview_profile.alpha
    );
    eprintln!(
        "  Batch/Accum: {}/{} (effective={})",
        preview_profile.batch_size,
        preview_profile.grad_accum,
        preview_profile.batch_size * preview_profile.grad_accum
    );
    eprintln!("  Backend:     {}", train_backend);
    eprintln!("  Tokenizer:   {:?}", tokenizer_mode);
    eprintln!("  Seq len:     {}", preview_profile.seq_len);
    if let Some(ref r) = resume {
        eprintln!("  Resume:      {}", r.display().cyan());
    }
    eprintln!();

    #[cfg(feature = "gpu")]
    {
        return super::gpu::run_gpu_training(
            train_backend,
            model,
            device,
            data_dir,
            output_dir,
            resume,
            preset,
            device_profile,
            cli_overrides,
            gpu_info,
            device_kind,
            min_rating,
            deployment_target,
            tokenizer_mode,
            qlora_no_double_quant,
            effective_qlora_require_full_proxy_stack,
            qlora_max_skip_rate,
            qlora_lm_head_only,
            qlora_proxy_max_layers,
            qlora_ce_last_k,
            checkpoint_every,
            force_restart,
            curriculum,
            optimizer_experiment_mode,
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
            vram_limit_fraction,
            adapter_tag,
            context_filter,
            validation_split_ratio,
            seed,
            curriculum_schedule,
            chatml,
            mix_config,
        )
        .await;
    }

    #[cfg(not(feature = "gpu"))]
    {
        let _ = (
            train_backend,
            model,
            device,
            data_dir,
            output_dir,
            rank,
            alpha,
            seq_len,
            batch_size,
            grad_accum,
            resume,
            epochs,
            warmup,
            lr,
            seed,
            min_rating,
            preset,
            deployment_target,
            process_priority,
            vram_limit_fraction,
            adapter_tag,
            context_filter,
            tokenizer_mode,
            qlora_no_double_quant,
            qlora_require_full_proxy_stack,
            qlora_allow_partial_proxy_stack,
            qlora_max_skip_rate,
            qlora_lm_head_only,
            qlora_proxy_max_layers,
            qlora_ce_last_k,
            checkpoint_every,
            force_restart,
            curriculum,
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
            optimizer_experiment_mode,
        );
        eprintln!(
            "  {} LoRA training requires the 'gpu' feature.",
            "⚠".yellow()
        );
        eprintln!("  Rebuild with: cargo build --features gpu");
        eprintln!();
        eprintln!("  Canonical QLoRA (when `gpu` is enabled): `vox mens train --backend qlora …`");
        eprintln!("  See docs/src/reference/mens-training.md");
        Ok(())
    }
}
