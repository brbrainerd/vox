/// Top-level subcommand enum for `vox mens` (AI/ML surfaces).
#[derive(clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
#[command(
    name = "mens",
    about = "Vox AI/ML: train, serve, and corpus management",
    long_about = include_str!("populi_mens_long_about.txt")
)]
pub enum PopuliAction {
    /// Corpus extract/validate/pairs/eval and optional `vox mens train` (dogfood pipeline).
    #[cfg(feature = "mens-base")]
    Pipeline {
        /// Directory for `train.jsonl` (default matches legacy PS1 `target/dogfood`).
        #[arg(long, default_value = vox_corpus::training::CANONICAL_TRAIN_DATA_DIR)]
        data_dir: PathBuf,
        /// Run outputs (eval JSON, train checkpoints).
        #[arg(long, default_value = vox_scaling_policy::DEFAULT_MENS_RUNS_V1)]
        output_dir: PathBuf,
        /// Stop after corpus eval (no train stage).
        #[arg(long, default_value_t = false)]
        skip_train: bool,
        /// Stricter eval gate env for the train stage (`VOX_EVAL_STRICT`, min pass rate).
        #[arg(long, default_value_t = false)]
        strict_gate: bool,
        /// Passed to `vox mens train --device` when training runs (default `best` in the pipeline runner).
        #[arg(long)]
        device: Option<String>,
        /// HuggingFace model repo override (e.g. Qwen/Qwen3.5-4B).
        #[arg(long)]
        model: Option<String>,
        /// Number of training epochs (default: 3).
        #[arg(long)]
        epochs: Option<usize>,
        /// Configuration preset override (e.g. qwen_4080_16g).
        #[arg(long)]
        preset: Option<String>,
        /// Comma-separated list of stages to run (e.g. "extract,validate,pairs").
        #[arg(long)]
        stages: Option<String>,
        /// Dry-run mode: show what would happen without modifying files or starting training.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Enable curriculum learning: epoch-gated difficulty sampling.
        #[arg(long, default_value_t = false)]
        curriculum: bool,
    },

    /// Fine-tune: Burn LoRA (`--backend lora`) or Candle HF-embed adapter (`--backend qlora` + `--tokenizer hf`).
    #[cfg(feature = "gpu")]
    Train {
        /// HuggingFace model repo to fine-tune (e.g. Qwen/Qwen3.5-4B).
        /// When omitted on `--backend qlora`, defaults to VoxMens SSOT `DEFAULT_MODEL_ID`.
        /// When set, weights are downloaded natively via hf-hub before training.
        #[arg(long)]
        model: Option<String>,
        /// GPU backend: cpu, best, vulkan, dx12, or metal
        #[arg(long, default_value = "best")]
        device: String,
        /// Trainer: `qlora` = Candle qlora-rs NF4 (needs `--tokenizer hf`; `--model` optional—SSOT default when omitted; default backend); `lora` = Burn+wGPU + `--tokenizer vox` (deprecated).
        #[arg(long, value_enum, default_value_t = PopuliTrainBackendCli::Qlora)]
        backend: PopuliTrainBackendCli,
        /// Directory containing train.jsonl (produced by `vox mens corpus pairs`).
        /// Canonical path: target/dogfood (matches corpus merge output).
        #[arg(long, default_value = vox_corpus::training::CANONICAL_TRAIN_DATA_DIR)]
        data_dir: PathBuf,
        /// Where to save the trained adapter / checkpoint
        #[arg(long, default_value = "mens/runs/latest")]
        output_dir: PathBuf,
        /// LoRA rank (r). Higher = more expressiveness. Default: 16 (or auto-tuned)
        #[arg(long)]
        rank: Option<usize>,
        /// LoRA alpha scaling factor. Default: 32 (or r*2)
        #[arg(long)]
        alpha: Option<f32>,
        /// Maximum sequence length. When omitted, sized automatically from detected VRAM
        /// and model size (see `vox_populi::mens::tensor::memory_budget`) so it fits without
        /// OOM; pass an explicit value to override the VRAM-aware default.
        #[arg(long)]
        seq_len: Option<usize>,
        /// Batch size per step. Default: 4 (or auto-tuned)
        #[arg(long)]
        batch_size: Option<usize>,
        /// Gradient accumulation steps. Default: 4
        #[arg(long)]
        grad_accum: Option<usize>,
        /// Resume from a checkpoint directory
        #[arg(long)]
        resume: Option<PathBuf>,
        /// Number of epochs to train (default: 3)
        #[arg(long)]
        epochs: Option<usize>,
        /// Learning rate (default: 2e-4)
        #[arg(long)]
        lr: Option<f64>,
        /// Warmup steps (default: 100 or 10% of total)
        #[arg(long)]
        warmup: Option<usize>,
        /// Random seed for reproducibility (0 = random)
        #[arg(long, default_value = "42")]
        seed: u64,
        /// Minimum quality rating (1-5) to include. 0 = all. Default 3 matches mix.
        #[arg(long)]
        min_rating: Option<u8>,
        /// O029: Preset: tiny, safe, 4080 / qwen_4080_16g, a100, mobile_edge (implies `--deployment-target mobile_edge`), or auto.
        #[arg(long)]
        preset: Option<String>,
        /// Domain profile name to apply (loads from mens/config/domain-profiles.yaml)
        #[arg(long)]
        domain: Option<String>,
        /// Train for workstation (default) or mobile edge export (requires `--device cpu`; see mobile-edge-ai SSOT).
        #[arg(long, value_enum, default_value_t = TrainingDeploymentTargetCli::Workstation)]
        deployment_target: TrainingDeploymentTargetCli,
        /// Process priority: low (BELOW_NORMAL/nice 10) or normal. Default: normal. Use low when training while browsing.
        #[arg(long, default_value = "normal")]
        process_priority: String,
        /// Cap VRAM usage at this fraction (0.0–1.0). E.g. 0.8 = 80%. Unset = adaptive 85%. Use with --process-priority low for background training.
        #[arg(long)]
        vram_limit_fraction: Option<f32>,
        /// Convenience: implies --process-priority low and --vram-limit-fraction 0.8 for training while browsing.
        #[arg(long)]
        background: bool,
        /// Run training in background and write all output to this directory (e.g. mens/runs/logs or a temp path).
        /// Creates `train_<timestamp>.log`. Use to avoid IDE timeouts; tail the log to monitor progress.
        #[arg(long)]
        log_dir: Option<PathBuf>,
        /// Dual-mode adapter tag. Sets the output directory suffix (e.g. "target" → runs/lora-target/).
        /// Use "target" for Vox app-code mode, "meta" for codebase reasoning mode.
        /// When set, automatically applies a matching context filter unless --context-filter is also specified.
        #[arg(long)]
        adapter_tag: Option<String>,
        /// Filter training records by context field. One of: target, meta, both.
        /// Records tagged "both" always pass any filter. Defaults to the adapter_tag if set.
        #[arg(long)]
        context_filter: Option<String>,
        /// Tokenizer mode: `hf` for Hugging Face `tokenizer.json` (default; native qlora preflight). `vox` for corpus JSONL (Burn LoRA).
        #[arg(long, value_enum, default_value_t = MensTokenizerCli::Hf)]
        tokenizer: MensTokenizerCli,
        /// Disable qlora-rs double quantization of NF4 scales (default: double quant **on** for smaller VRAM).
        #[arg(long)]
        qlora_no_double_quant: bool,
        /// Candle QLoRA: strict preflight for full-graph training (requires expected projection/block tensors in safetensors).
        /// On `--device cuda`, this defaults to **on** unless `--qlora-allow-partial-proxy-stack` is set.
        #[arg(long)]
        qlora_require_full_proxy_stack: bool,
        /// Candle QLoRA: opt out of the CUDA default full proxy-stack preflight (partial / experimental graphs only).
        #[arg(long, default_value_t = false)]
        qlora_allow_partial_proxy_stack: bool,
        /// Candle QLoRA: reserved/deferred flag for LM-head-only mode; currently rejected by trainer (full graph only).
        #[arg(long)]
        qlora_lm_head_only: bool,
        /// Candle QLoRA: abort an epoch if skipped pairs / pair visits exceed this rate (0.0–1.0).
        #[arg(long)]
        qlora_max_skip_rate: Option<f32>,
        /// Candle QLoRA: reserved/deferred partial-depth flag; values below model depth are currently rejected by trainer.
        #[arg(long)]
        qlora_proxy_max_layers: Option<usize>,
        /// Candle QLoRA: next-token CE on the last **K** positions per JSONL row (default 64). Capped by effective `--seq-len` and 64.
        #[arg(long, default_value_t = 64)]
        qlora_ce_last_k: usize,
        /// Steps between mid-epoch checkpoints. Saves adapter and resume state to `--output-dir/checkpoint_state.json`.
        #[arg(long)]
        checkpoint_every: Option<usize>,
        /// Fraction (0.0–1.0) of training pairs held out for mid-epoch validation. Default 0.05.
        #[arg(long, default_value_t = 0.05)]
        validation_split_ratio: f64,
        /// Ignore existing resume state and force a fresh run from step 0.
        #[arg(long)]
        force_restart: bool,
        /// Activation/gradient checkpointing: recompute each layer-segment's forward during
        /// backward so only ~1 segment's activations are retained, bounding the single-backward
        /// VRAM peak. Trades ~1 extra forward for a much lower peak — required for 3B QLoRA on
        /// a 16GB GPU. Default off (1.5B fits without it). Segment count via `VOX_MENS_GC_SEGMENTS`.
        #[arg(long, default_value_t = false)]
        gradient_checkpointing: bool,
        /// Disable automatic rebuild/reinstall of the `mens-candle-cuda` plugin when it is
        /// missing, stale, or ABI-mismatched. By default `--device cuda` self-heals the plugin
        /// before training (also disableable via `VOX_MENS_NO_AUTO_HEAL=1`).
        #[cfg(feature = "gpu")]
        #[arg(long, default_value_t = false)]
        no_auto_heal: bool,
        /// Require accelerator execution. Fails if selected runtime resolves to CPU.
        #[arg(long, default_value_t = false)]
        require_gpu: bool,
        /// Allow CPU fallback when `--device best` cannot initialize CUDA/Metal.
        #[arg(long, default_value_t = true)]
        allow_cpu_fallback: bool,
        /// Enable curriculum learning: epoch-gated difficulty sampling (Candle QLoRA).
        #[arg(long, default_value_t = false)]
        curriculum: bool,
        /// Experimental optimizer lane (guarded by VOX_MENS_EXPERIMENTAL_OPTIMIZER when non-off).
        #[arg(long, value_enum, default_value_t = OptimizerExperimentModeCli::Off)]
        optimizer_experiment_mode: OptimizerExperimentModeCli,
        /// How to refresh training inputs before the trainer runs (`strict` = abort on refresh failures; reproducible full corpus path). Use `auto-refresh` for lenient legacy behavior.
        #[cfg(feature = "gpu")]
        #[arg(long, value_enum, default_value_t = TrainDataModeCli::Strict)]
        data_mode: TrainDataModeCli,
        /// Use existing `train.jsonl` in `--data-dir` only; skip mix-driven refresh (`VOX_TRAIN_SKIP_CORPUS_MIX`). For dev/tests — default off for a full corpus pipeline when stale.
        #[cfg(feature = "gpu")]
        #[arg(long, default_value_t = false)]
        fast_corpus: bool,
        /// Provenance: coarse family label for upstream lineage (e.g. `kimi-k2.5`, `qwen2.5`).
        #[arg(long)]
        base_model_family: Option<String>,
        /// Provenance: explicit upstream model id used as initialization source.
        #[arg(long)]
        upstream_model_id: Option<String>,
        /// Provenance: license class for downstream attribution/compliance policy.
        #[arg(long)]
        license_class: Option<String>,
        /// Provenance: mark downstream artifact publication as attribution-required.
        #[arg(long, default_value_t = false)]
        attribution_required: bool,
        /// Enable trajectory-aware weighting for tool/failure rows.
        #[arg(long, default_value_t = false)]
        trajectory_weighting_enabled: bool,
        /// Multiplier for tool-trace/trajectory categories.
        #[arg(long, default_value_t = 1.1)]
        trajectory_tool_trace_boost: f32,
        /// Multiplier for failure/error categories.
        #[arg(long, default_value_t = 1.15)]
        trajectory_failure_category_boost: f32,
        /// Minimum rating (1-5) to apply quality boost.
        #[arg(long)]
        trajectory_quality_floor: Option<u8>,
        /// Multiplier for rows that meet `--trajectory-quality-floor`.
        #[arg(long, default_value_t = 1.05)]
        trajectory_quality_boost: f32,

        /// Cloud provider (auto, vast, runpod, local)
        #[arg(long, default_value = "local")]
        cloud: String,
        /// Max budget USD
        #[arg(long)]
        max_budget: Option<f64>,
        /// Training data on HF Hub
        #[arg(long)]
        train_data_hf: Option<String>,
        /// HF Hub repo to upload the trained adapter
        #[arg(long)]
        adapter_upload_hf: Option<String>,
        /// Absolute hard cap for runtime in seconds
        #[arg(long)]
        max_runtime_secs: Option<u64>,
        /// Transition cloud deployments to long-lived mesh nodes after training.
        #[arg(long, default_value_t = false)]
        persistent: bool,
    },

    #[cfg(not(feature = "gpu"))]
    #[command(name = "train")]
    TrainStub {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        _args: Vec<String>,
    },

    /// Dogfood training alias. A zero-config command to execute the canonical Qwen QLoRA training pipeline.
    #[cfg(feature = "gpu")]
    Dogfood {
        /// Where to save checkpoints
        #[arg(long, default_value = vox_scaling_policy::DEFAULT_MENS_RUNS_V1)]
        output_dir: PathBuf,
        /// Steps between mid-epoch checkpoints. Saves adapter and resume state to `--output-dir/checkpoint_state.json`.
        #[arg(long, default_value_t = 500)]
        checkpoint_every: usize,
        /// Ignore existing resume state and force a fresh run from step 0.
        #[arg(long)]
        force_restart: bool,
    },

    #[cfg(not(feature = "gpu"))]
    #[command(name = "dogfood")]
    DogfoodStub {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        _args: Vec<String>,
    },

    /// Retired: UV/Python quantized training path removed; dispatch prints migration guidance.
    /// Use native **`vox mens train`** (QLoRA) or follow docs for your GPU setup.
    TrainUv {
        /// HuggingFace model ID (e.g. Qwen/Qwen3.5-4B)
        #[arg(long)]
        model: Option<String>,
        /// Directory containing train.jsonl
        #[arg(long, default_value = vox_corpus::training::CANONICAL_TRAIN_DATA_DIR)]
        data_dir: PathBuf,
        /// Where to write run_manifest.json, metrics.jsonl
        #[arg(long, default_value = vox_scaling_policy::DEFAULT_MENS_RUNS_UV_OUTPUT)]
        output_dir: PathBuf,
        /// LoRA rank (default 16)
        #[arg(long)]
        rank: Option<usize>,
        /// LoRA alpha (default 32)
        #[arg(long)]
        alpha: Option<f32>,
        /// Number of epochs (default 3)
        #[arg(long)]
        epochs: Option<usize>,
    },

    /// Serve a trained Mens checkpoint via HTTP (OpenAI-compatible API)
    #[cfg(feature = "gpu")]
    Serve {
        /// Path to model checkpoint (.safetensors from `vox mens train`). Required for local.
        #[arg(long)]
        model: Option<PathBuf>,
        /// HTTP port to listen on
        #[arg(long, default_value_t = crate::commands::ai::inference_defaults::DEFAULT_INFERENCE_PORT)]
        port: u16,
        /// Host to bind (use 0.0.0.0 for network access)
        #[arg(long, default_value = crate::commands::ai::inference_defaults::DEFAULT_INFERENCE_HOST)]
        host: String,
        /// Maximum tokens to generate per request
        #[arg(
            long,
            default_value_t = crate::commands::ai::inference_defaults::DEFAULT_INFERENCE_MAX_TOKENS
        )]
        max_tokens: usize,
        /// Sampling temperature (0.0 = greedy, 1.0 = random)
        #[arg(
            long,
            default_value_t = crate::commands::ai::inference_defaults::DEFAULT_INFERENCE_TEMPERATURE
        )]
        temperature: f32,

        /// Cloud provider (auto, vast, runpod, local)
        #[arg(long, default_value = "local")]
        cloud: String,
        /// Max budget USD
        #[arg(long)]
        max_budget: Option<f64>,
        /// HF Hub model to pull and serve (required for cloud)
        #[arg(long)]
        model_hf: Option<String>,
        /// Absolute hard cap for runtime in seconds
        #[arg(long)]
        max_runtime_secs: Option<u64>,
        /// Transition cloud deployments to long-lived mesh nodes.
        #[arg(long, default_value_t = false)]
        persistent: bool,
    },

    #[cfg(not(feature = "gpu"))]
    #[command(name = "serve")]
    ServeStub {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        _args: Vec<String>,
    },

    /// Training data pipeline: extract, validate, mix, eval, audit…
    #[command(subcommand)]
    Corpus(CorpusAction),

    /// Detect GPU capabilities and print recommended LoRA training configuration
    Probe {
        /// Show detailed hardware metadata and recommended hyperparameter presets.
        #[arg(long, short = 'v')]
        verbose: bool,
    },

    /// Show training run status or BYOK quota usage
    Status {
        /// Path to telemetry JSONL (default: mens/runs/latest/telemetry.jsonl)
        #[arg(long)]
        run_dir: Option<PathBuf>,
        /// Show BYOK quota usage vs limits (Wave 2)
        #[arg(long, default_value = "false")]
        quotas: bool,
        /// Show current agent/orchestrator inference configuration (Wave 7)
        #[arg(long, default_value = "false")]
        config: bool,
        /// Show cloud GPU dispatch summary and accrued cost.
        #[arg(long, default_value = "false")]
        cloud: bool,
        /// Query MENS intelligence metrics from the Arca database.
        #[arg(long, default_value = "false")]
        db: bool,
    },

    /// Tail `train.err.log` + training JSONL telemetry (periodic summary; default poll 3s).
    #[command(name = "watch-telemetry", visible_alias = "watch")]
    WatchTelemetry {
        /// Training telemetry JSONL path (`"event":"train"` lines).
        #[arg(long, default_value = "target/dogfood/telemetry.jsonl")]
        telemetry: std::path::PathBuf,
        /// Engine stderr log path (build + runtime banners).
        #[arg(long, default_value = "target/dogfood/train.err.log")]
        err_log: std::path::PathBuf,
        /// Poll interval in milliseconds.
        #[arg(long, default_value_t = 3000)]
        interval_ms: u64,
    },

    #[cfg(feature = "gpu")]
    /// List all trained models in the local registry
    Models,

    /// Merge Candle QLoRA adapter v2 (`candle_qlora_adapter*.safetensors`) into base f32 weights (writes merged keys only).
    /// Canonical: `merge-qlora`. Alias: `merge-adapter` (same flags).
    #[cfg(feature = "gpu")]
    #[command(name = "merge-qlora", visible_alias = "merge-adapter")]
    MergeQlora {
        /// Base model safetensors shard path (repeat `--base-shard` for each file).
        #[arg(long = "base-shard", required = true)]
        base_shard: Vec<PathBuf>,
        /// `candle_qlora_adapter.safetensors` from a qlora-rs training run.
        #[arg(long, required = true)]
        adapter: PathBuf,
        /// `candle_qlora_adapter_meta.json` (format v2).
        #[arg(long, required = true)]
        meta: PathBuf,
        /// Output safetensors path (subset of merged keys).
        #[arg(long, required = true)]
        output: PathBuf,
        /// After merging, recombine over base and quantize to this mixture
        /// (q4_k_m|q5_k_m|q6_k|q8_0). Writes a quantized artifact next to --output.
        #[arg(long)]
        quantize: Option<String>,
    },

    /// Export merged safetensors weights to GGUF (not yet implemented).
    ///
    /// Prerequisite: run `vox mens merge-qlora` to produce merged `.safetensors` first.
    #[command(name = "export-gguf")]
    ExportGguf {
        /// Merged safetensors path (output of `merge-qlora`).
        #[arg(long, required = true)]
        input: PathBuf,
        /// Destination `.gguf` path.
        #[arg(long, required = true)]
        output: PathBuf,
    },

    /// AI-powered code generation from a natural language prompt
    #[cfg(feature = "mens-dei")]
    Generate {
        /// Natural language prompt
        prompt: String,
        /// Path to write the generated code
        #[arg(short = 'o', long)]
        output: Option<std::path::PathBuf>,
        /// Skip syntactic validation of the output
        #[arg(long, default_value = "false")]
        no_validate: bool,
        /// Custom inference server URL
        #[arg(long)]
        server_url: Option<String>,
        /// Max retries on network failure
        #[arg(long)]
        max_retries: Option<u32>,
        /// P015: Output mode (strict_json, jsonl_records, tool_args_json) for constrained decoding
        #[arg(long)]
        output_mode: Option<String>,
        /// P016: JSON schema path for post-generation validation (requires output_mode)
        #[arg(long)]
        schema: Option<std::path::PathBuf>,
        /// Context assembly mode: minimal | repo-aware | schema-only | graph-aware | full
        #[arg(long, default_value = "minimal")]
        context_mode: String,
        /// Conversation ID for graph-aware context (pulls versions/edges from Codex)
        #[arg(long)]
        conversation_id: Option<i64>,
        /// Track job in Codex task_jobs (queue-aware, sync execution). Fallback to direct when Codex unavailable.
        #[arg(long, default_value = "false")]
        queue: bool,
        /// Execution mode: efficient | fast | verbose | precision
        #[arg(long)]
        mode: Option<String>,
    },

    /// AI-powered code review for one or more paths
    #[cfg(feature = "mens-dei")]
    Review {
        /// Path(s) to files or directories to review
        #[arg(default_value = ".")]
        targets: Vec<std::path::PathBuf>,
        /// LLM model override
        #[arg(long)]
        model: Option<String>,
        /// Output format (text, json, markdown)
        #[arg(long)]
        format: Option<String>,
        /// Minimum severity to report
        #[arg(long)]
        severity: Option<String>,
        /// Restrict to free providers
        #[arg(long, default_value = "false")]
        free_only: bool,
        /// Review the current git diff only
        #[arg(long, default_value = "false")]
        diff: bool,
        /// CI mode: exit non-zero on issues
        #[arg(long, default_value = "false")]
        ci: bool,
        /// Add review as PR comments (GitHub)
        #[arg(long, default_value = "false")]
        pr_comment: bool,
        /// Git base ref for diff-based review
        #[arg(long)]
        diff_base: Option<String>,
        /// Execution mode profile
        #[arg(long)]
        mode: Option<String>,
    },

    /// Inspect and run Vox workflow definitions
    #[cfg(feature = "mens-dei")]
    #[command(subcommand)]
    Workflow(vox_cli_core::cli_actions::WorkflowAction),

    /// AI-powered code check for potential bugs or anti-patterns (alias: verify)
    #[cfg(feature = "mens-dei")]
    #[command(alias = "verify")]
    Check {
        /// File to check
        #[arg(required = true)]
        file: std::path::PathBuf,
    },

    /// Attempt to automatically fix compiler errors using AI
    #[cfg(feature = "mens-dei")]
    Fix {
        /// File to fix
        #[arg(required = true)]
        file: std::path::PathBuf,
        /// Optional: specific compiler errors to fix (omit to re-run check)
        #[arg(long)]
        errors: Option<String>,
    },

    /// Evaluate a trained model checkpoint against the heldout benchmark set.
    ///
    /// Runs inference on each sample in the benchmark directory and reports
    /// per-sample completion quality and pass rates by category.
    #[cfg(feature = "gpu")]
    #[command(name = "eval-local")]
    EvalLocal {
        /// Path to model checkpoint (.safetensors from `vox mens train`)
        #[arg(long, required = true)]
        model: PathBuf,
        /// Path to heldout benchmark directory (default: mens/data/heldout_bench)
        #[arg(long, default_value = "mens/data/heldout_bench")]
        bench: PathBuf,
        /// Maximum tokens to generate per sample
        #[arg(long, default_value = "128")]
        max_tokens: usize,
        /// Sampling temperature (0.0 = greedy)
        #[arg(long, default_value = "0.0")]
        temperature: f32,
        /// Number of samples per benchmark item for pass@k (k)
        #[arg(long, default_value = "1")]
        samples: usize,
        /// Base RNG seed for sampled generations (used when temperature > 0)
        #[arg(long, default_value = "1337")]
        seed_base: u64,
        /// Output JSON results to this path
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
    },

    #[command(flatten)]
    MensTail(super::mens_tail_subcommands::PopuliMensTail),
}
