//! Shared training run configuration for all Mens native trainers (`--backend`).
//!
//! Copied verbatim from `vox-populi/src/mens/tensor/training_config.rs` (SP3 sub-batch A).
//! This is pure-data (serde structs, no vox-populi internal type references).
//! Sub-batches B-D will wire this into the JSON deserialization path for
//! `run_full_training`.

/// Where trained artifacts are intended to run (planner gates + manifest hints).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum TrainingDeploymentTarget {
    /// Workstation / server Mens stack (default).
    #[default]
    Workstation,
    /// Export-oriented profile for phone / edge inference (train off-device).
    MobileEdge,
}

impl TrainingDeploymentTarget {
    /// Stable wire / manifest label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Workstation => "workstation",
            Self::MobileEdge => "mobile_edge",
        }
    }
}

/// Tokenization strategy for training pairs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MensTokenizerMode {
    /// [`vox_tensor::data::VoxTokenizer`] (Burn LoRA default; corpus-native).
    #[default]
    Vox,
    /// Hugging Face `tokenizer.json` (`--tokenizer hf`; required for `--backend qlora`).
    Hf,
}

/// Non-default optimizer lane reserved for explicit experiments.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum OptimizerExperimentMode {
    /// Stable default behavior.
    #[default]
    Off,
    /// Reserved experimental lane for MuonClip-style optimizer studies.
    MuonClipLike,
}

/// ContextFilter used to filter training pairs based on categories, difficulty, and ratings.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ContextFilter {
    pub categories: Option<Vec<String>>,
    pub difficulty_min: Option<u8>,
    pub difficulty_max: Option<u8>,
    pub rating_min: Option<u8>,
}

/// Dynamic curriculum schedule for difficult-gated training.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct CurriculumSchedule {
    pub epoch_1_max_difficulty: Option<u8>,
    pub epoch_2_max_difficulty: Option<u8>,
    pub epoch_3_max_difficulty: Option<u8>,
    /// Sequential phase labels for documentation/telemetry (e.g. ["syntax", "logic"])
    pub curriculum_phases: Option<Vec<String>>,
}

/// Dynamic ChatML separator configuration (registry-driven).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatmlConfig {
    pub im_start: String,
    pub im_end: String,
    pub role_system: String,
    pub role_user: String,
    pub role_assistant: String,
}

impl Default for ChatmlConfig {
    fn default() -> Self {
        Self {
            im_start: "<|im_start|>".to_string(),
            im_end: "<|im_end|>".to_string(),
            role_system: "system".to_string(),
            role_user: "user".to_string(),
            role_assistant: "assistant".to_string(),
        }
    }
}

/// Full configuration for one LoRA / QLoRA training run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LoraTrainingConfig {
    pub base_model: Option<String>,
    /// Provenance: coarse family label for the upstream/base model lineage.
    pub base_model_family: Option<String>,
    /// Provenance: explicit upstream model id used as initialization source.
    pub upstream_model_id: Option<String>,
    /// Provenance: license class label (e.g. `apache-2.0`, `modified-mit`).
    pub license_class: Option<String>,
    /// Provenance: whether downstream artifact publication requires attribution.
    pub attribution_required: bool,
    pub base_model_paths: Option<(Vec<std::path::PathBuf>, std::path::PathBuf)>,
    pub tokenizer_path: Option<std::path::PathBuf>,
    pub train_file: Option<std::path::PathBuf>,
    pub rank: usize,
    pub alpha: f32,
    pub seq_len: usize,
    pub batch_size: usize,
    pub grad_accum: usize,
    pub resume_from: Option<std::path::PathBuf>,
    pub epochs: usize,
    pub learning_rate: f64,
    pub warmup_steps: usize,
    pub seed: u64,
    pub min_rating: u8,
    pub run_id: Option<String>,
    pub git_sha: Option<String>,
    pub device_profile: Option<String>,
    pub max_vram_fraction: Option<f32>,
    pub adapter_tag: Option<String>,
    pub context_filter: Option<ContextFilter>,
    pub validation_split_ratio: Option<f64>,
    pub tokenizer_mode: MensTokenizerMode,
    /// When false, sets qlora-rs `QuantizationConfig.double_quant` off (debug / ablation). Default: true.
    pub qlora_double_quant: bool,
    /// Set by `vox_populi::mens::tensor::lora_train::run_mens_training` from the execution plan.
    pub finetune_contract_digest: Option<String>,
    /// Candle QLoRA: fail preflight when middle projection keys are incomplete (`--qlora-require-full-proxy-stack`).
    pub qlora_require_full_proxy_stack: bool,
    /// Candle QLoRA: abort training when skip rate (skipped pairs / pair visits) exceeds this value in an epoch.
    pub qlora_max_skip_rate: Option<f32>,
    /// Candle QLoRA: reserved/deferred LM-head-only mode; current trainer rejects this and runs full graph only.
    pub qlora_lm_head_only: bool,
    /// Candle QLoRA: reserved/deferred partial-depth cap; current trainer rejects values below model depth.
    pub qlora_proxy_max_layers: Option<usize>,
    /// Candle QLoRA: next-token CE over the last **K** positions per JSONL row (default 64).
    pub qlora_ce_last_k: usize,
    /// Steps between mid-epoch checkpoints. None means only epoch-boundary checkpoints.
    pub checkpoint_every: Option<usize>,
    /// Ignore existing checkpoints and force a fresh run.
    pub force_restart: bool,
    /// Intended deployment surface for trained artifacts (planner gates + manifest).
    pub deployment_target: TrainingDeploymentTarget,
    /// Whether to use curriculum learning (epoch-gated difficulty sampling).
    pub curriculum: bool,
    /// Explicit schedule for curriculum difficulty ramp-up per epoch.
    pub curriculum_schedule: Option<CurriculumSchedule>,
    /// Experimental optimizer lane. Must stay `off` unless explicitly requested.
    pub optimizer_experiment_mode: OptimizerExperimentMode,
    /// Enable trajectory-aware sample weighting for agentic/tool traces.
    pub trajectory_weighting_enabled: bool,
    /// Multiplier for rows tagged as tool traces / trajectories.
    pub trajectory_tool_trace_boost: f32,
    /// Multiplier for rows tagged as failure/error trajectories.
    pub trajectory_failure_category_boost: f32,
    /// Optional minimum quality rating to apply quality boost.
    pub trajectory_quality_floor: Option<u8>,
    /// Multiplier for rows meeting `trajectory_quality_floor`.
    pub trajectory_quality_boost: f32,
    /// Require a real GPU execution path; fail if device selection falls back to CPU.
    pub require_gpu: bool,
    /// Allow automatic CPU fallback when `--device best` cannot initialize an accelerator.
    pub allow_cpu_fallback: bool,
    /// Dynamic ChatML separator configuration (registry-driven).
    pub chatml: ChatmlConfig,
    /// Optional dynamic hook for running code evaluations (e.g. "cargo_build")
    pub reward_hook: Option<String>,
}

impl Default for LoraTrainingConfig {
    fn default() -> Self {
        Self {
            base_model: None,
            base_model_family: None,
            upstream_model_id: None,
            license_class: None,
            attribution_required: false,
            base_model_paths: None,
            tokenizer_path: None,
            train_file: None,
            rank: 16,
            alpha: 32.0,
            seq_len: 256,
            batch_size: 4,
            grad_accum: 4,
            resume_from: None,
            epochs: 3,
            learning_rate: 2e-4,
            warmup_steps: 100,
            seed: 42,
            min_rating: 3,
            run_id: None,
            git_sha: None,
            device_profile: None,
            max_vram_fraction: None,
            adapter_tag: None,
            context_filter: None,
            validation_split_ratio: Some(0.05),
            tokenizer_mode: MensTokenizerMode::Hf,
            qlora_double_quant: true,
            finetune_contract_digest: None,
            qlora_require_full_proxy_stack: false,
            qlora_max_skip_rate: None,
            qlora_lm_head_only: false,
            qlora_proxy_max_layers: None,
            qlora_ce_last_k: 64,
            checkpoint_every: Some(500),
            force_restart: false,
            deployment_target: TrainingDeploymentTarget::default(),
            curriculum: false,
            curriculum_schedule: None,
            optimizer_experiment_mode: OptimizerExperimentMode::Off,
            trajectory_weighting_enabled: false,
            trajectory_tool_trace_boost: 1.1,
            trajectory_failure_category_boost: 1.15,
            trajectory_quality_floor: None,
            trajectory_quality_boost: 1.05,
            require_gpu: false,
            allow_cpu_fallback: true,
            chatml: ChatmlConfig::default(),
            reward_hook: None,
        }
    }
}
