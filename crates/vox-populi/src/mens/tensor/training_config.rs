//! Shared training run configuration for all Mens native trainers (`--backend`).

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
    /// Set by [`crate::mens::tensor::lora_train::run_mens_training`] from the execution plan.
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
    /// Process argv captured at training start — written to the manifest so a failed
    /// run can be relaunched with the exact same flags without operator memory.
    /// Defaulted to empty for backward serde compatibility with older configs.
    #[serde(default)]
    pub launch_argv: Vec<String>,
    /// Activation/gradient checkpointing: segment the transformer stack and
    /// recompute each segment's forward during backward so only ~1 segment's
    /// activations are retained at once — bounds the single-backward VRAM peak so
    /// 3B QLoRA fits on a 16GB GPU. Default off (1.5B fits without it). Serialized
    /// to the candle plugin's matching config field; segment count via
    /// `VOX_MENS_GC_SEGMENTS`.
    #[serde(default)]
    pub gradient_checkpointing: bool,
}

#[cfg(test)]
mod semcov_wave26_tests {
    use super::*;

    // ── TrainingDeploymentTarget ─────────────────────────────────────────────

    #[test]
    fn deployment_target_as_str_roundtrip() {
        // Catches: as_str() returning wrong variant label, breaking manifest writes.
        assert_eq!(
            TrainingDeploymentTarget::Workstation.as_str(),
            "workstation"
        );
        assert_eq!(TrainingDeploymentTarget::MobileEdge.as_str(), "mobile_edge");
    }

    #[test]
    fn deployment_target_default_is_workstation() {
        // Catches: Default impl returning MobileEdge instead of Workstation,
        // causing unintended mobile-optimized export for all default training runs.
        assert_eq!(
            TrainingDeploymentTarget::default(),
            TrainingDeploymentTarget::Workstation
        );
    }

    #[test]
    fn deployment_target_serde_roundtrip() {
        // Catches: serde rename_all mismatch where snake_case serialization
        // produces "mobile_edge" but deserialization expects "mobileEdge".
        let v = TrainingDeploymentTarget::MobileEdge;
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"mobile_edge\"");
        let back: TrainingDeploymentTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(back, TrainingDeploymentTarget::MobileEdge);
    }

    // ── MensTokenizerMode ────────────────────────────────────────────────────

    #[test]
    fn tokenizer_mode_default_is_vox() {
        // Catches: Default impl returning Hf, which would break Burn LoRA
        // training runs that expect corpus-native VoxTokenizer.
        assert_eq!(MensTokenizerMode::default(), MensTokenizerMode::Vox);
    }

    #[test]
    fn tokenizer_mode_hf_serde_roundtrip() {
        // Catches: snake_case rename producing "h_f" instead of "hf".
        let v = MensTokenizerMode::Hf;
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"hf\"");
        let back: MensTokenizerMode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, MensTokenizerMode::Hf);
    }

    // ── OptimizerExperimentMode ──────────────────────────────────────────────

    #[test]
    fn optimizer_experiment_mode_default_is_off() {
        // Catches: Default returning MuonClipLike, silently enabling
        // experimental optimizer in all default training runs.
        assert_eq!(
            OptimizerExperimentMode::default(),
            OptimizerExperimentMode::Off
        );
    }

    #[test]
    fn optimizer_experiment_serde_roundtrip() {
        // Catches: serde rename producing "muon_clip_like" vs "muonclip_like"
        // divergence breaking config reload after a run with this mode.
        let v = OptimizerExperimentMode::MuonClipLike;
        let json = serde_json::to_string(&v).unwrap();
        let back: OptimizerExperimentMode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, OptimizerExperimentMode::MuonClipLike);
    }

    // ── ContextFilter ────────────────────────────────────────────────────────

    #[test]
    fn context_filter_default_is_all_none() {
        // Catches: Default leaving difficulty_min=Some(0) which would silently
        // filter out training rows with unset difficulty fields.
        let f = ContextFilter::default();
        assert!(f.categories.is_none());
        assert!(f.difficulty_min.is_none());
        assert!(f.difficulty_max.is_none());
        assert!(f.rating_min.is_none());
    }

    #[test]
    fn context_filter_boundary_values_serde_roundtrip() {
        // Catches: u8 overflow when serializing difficulty_min=0 or rating_min=255.
        let f = ContextFilter {
            categories: Some(vec!["rust".to_string()]),
            difficulty_min: Some(0),
            difficulty_max: Some(255),
            rating_min: Some(255),
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: ContextFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(back.difficulty_min, Some(0));
        assert_eq!(back.difficulty_max, Some(255));
        assert_eq!(back.rating_min, Some(255));
    }

    // ── ChatmlConfig ─────────────────────────────────────────────────────────

    #[test]
    fn chatml_config_default_tokens() {
        // Catches: default producing empty strings or wrong tokens, causing
        // chatml_supervised_text to emit malformed training text with no markers.
        let cfg = ChatmlConfig::default();
        assert_eq!(cfg.im_start, "<|im_start|>");
        assert_eq!(cfg.im_end, "<|im_end|>");
        assert_eq!(cfg.role_system, "system");
        assert_eq!(cfg.role_user, "user");
        assert_eq!(cfg.role_assistant, "assistant");
    }

    #[test]
    fn chatml_config_custom_tokens_serde_roundtrip() {
        // Catches: partial serialization omitting one role field, leading to
        // deserialized ChatmlConfig with Default-filled role on reload.
        let cfg = ChatmlConfig {
            im_start: "<s>".to_string(),
            im_end: "</s>".to_string(),
            role_system: "sys".to_string(),
            role_user: "human".to_string(),
            role_assistant: "gpt".to_string(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: ChatmlConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role_user, "human");
        assert_eq!(back.role_assistant, "gpt");
        assert_eq!(back.im_start, "<s>");
    }

    // ── LoraTrainingConfig defaults ──────────────────────────────────────────

    #[test]
    fn lora_training_config_default_rank_and_alpha() {
        // Catches: rank=0 or alpha=0.0 default that would make all LoRA
        // updates zero-scaled, producing a model that never learns.
        let cfg = LoraTrainingConfig::default();
        assert!(cfg.rank > 0, "rank must be positive, got {}", cfg.rank);
        assert!(cfg.alpha > 0.0, "alpha must be positive, got {}", cfg.alpha);
    }

    #[test]
    fn lora_training_config_default_grad_accum_positive() {
        // Catches: grad_accum=0 default that causes a divide-by-zero in the
        // effective-batch-size computation downstream.
        let cfg = LoraTrainingConfig::default();
        assert!(
            cfg.grad_accum > 0,
            "grad_accum=0 would cause divide-by-zero"
        );
    }

    #[test]
    fn lora_training_config_default_validation_split_in_range() {
        // Catches: validation_split_ratio=Some(1.05) or negative default that
        // causes an empty training set or panic in split logic.
        let cfg = LoraTrainingConfig::default();
        if let Some(ratio) = cfg.validation_split_ratio {
            assert!(
                ratio > 0.0 && ratio < 1.0,
                "validation_split_ratio must be in (0, 1), got {ratio}"
            );
        }
    }

    #[test]
    fn lora_training_config_default_trajectory_boosts_at_least_one() {
        // Catches: boost defaults < 1.0 that would penalize (down-weight) tool
        // trace rows instead of boosting them, inverting the intended weighting.
        let cfg = LoraTrainingConfig::default();
        assert!(
            cfg.trajectory_tool_trace_boost >= 1.0,
            "tool trace boost should not penalize rows"
        );
        assert!(
            cfg.trajectory_failure_category_boost >= 1.0,
            "failure boost should not penalize rows"
        );
        assert!(
            cfg.trajectory_quality_boost >= 1.0,
            "quality boost should not penalize rows"
        );
    }

    #[test]
    fn lora_training_config_default_allow_cpu_fallback_is_true() {
        // Catches: allow_cpu_fallback=false default that would make every
        // non-GPU machine fail at training startup with no clear message.
        let cfg = LoraTrainingConfig::default();
        assert!(
            cfg.allow_cpu_fallback,
            "default must allow cpu fallback for portability"
        );
    }

    #[test]
    fn lora_training_config_qlora_ce_last_k_default_nonzero() {
        // Catches: qlora_ce_last_k=0 default that would compute CE loss over
        // zero positions per row, producing NaN loss on the first step.
        let cfg = LoraTrainingConfig::default();
        assert!(
            cfg.qlora_ce_last_k > 0,
            "qlora_ce_last_k=0 produces NaN loss"
        );
    }

    #[test]
    fn lora_training_config_serde_roundtrip_preserves_chatml() {
        // Catches: serde skip or flatten bug that drops the chatml sub-config,
        // causing reload to silently reset to Default ChatmlConfig.
        let mut cfg = LoraTrainingConfig::default();
        cfg.chatml.role_user = "human".to_string();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: LoraTrainingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.chatml.role_user, "human",
            "chatml must survive serde roundtrip"
        );
    }

    // ── CurriculumSchedule ───────────────────────────────────────────────────

    #[test]
    fn curriculum_schedule_default_all_none() {
        // Catches: Default setting epoch_1_max_difficulty=Some(0) which would
        // exclude ALL rows in epoch 1 (difficulty 0 passes but nothing else).
        let cs = CurriculumSchedule::default();
        assert!(cs.epoch_1_max_difficulty.is_none());
        assert!(cs.epoch_2_max_difficulty.is_none());
        assert!(cs.epoch_3_max_difficulty.is_none());
        assert!(cs.curriculum_phases.is_none());
    }
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
            launch_argv: Vec::new(),
            gradient_checkpointing: false,
        }
    }
}
