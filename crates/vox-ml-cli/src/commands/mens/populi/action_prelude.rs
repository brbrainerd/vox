use std::path::PathBuf;

use crate::commands::corpus::CorpusAction;
/// CLI mapping for `vox mens train --backend` → [`vox_populi::mens::PopuliTrainBackend`].
#[cfg(feature = "gpu")]
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum PopuliTrainBackendCli {
    /// Burn + wgpu LoRA on VoxTokenizer JSONL (deprecated).
    Lora,
    /// Candle + qlora-rs NF4 on HF safetensors (`--tokenizer hf`, `--model`, CUDA/Metal optional; default).
    #[default]
    Qlora,
}

#[cfg(feature = "gpu")]
impl From<PopuliTrainBackendCli> for vox_populi::mens::PopuliTrainBackend {
    fn from(value: PopuliTrainBackendCli) -> Self {
        match value {
            PopuliTrainBackendCli::Lora => Self::BurnLora,
            PopuliTrainBackendCli::Qlora => Self::CandleQlora,
        }
    }
}

/// CLI mapping for `vox mens train --tokenizer` → [`vox_populi::mens::MensTokenizerMode`].
#[cfg(feature = "gpu")]
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum MensTokenizerCli {
    /// Corpus VoxTokenizer JSONL (Burn LoRA).
    Vox,
    /// Hugging Face `tokenizer.json` (required for native `--backend qlora` preflight; default).
    #[default]
    Hf,
}

#[cfg(feature = "gpu")]
impl From<MensTokenizerCli> for vox_populi::mens::MensTokenizerMode {
    fn from(value: MensTokenizerCli) -> Self {
        match value {
            MensTokenizerCli::Vox => Self::Vox,
            MensTokenizerCli::Hf => Self::Hf,
        }
    }
}

/// CLI mapping for `vox mens train --deployment-target` → [`vox_populi::mens::TrainingDeploymentTarget`].
#[cfg(feature = "gpu")]
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum TrainingDeploymentTargetCli {
    /// Default workstation / server Mens path.
    #[default]
    Workstation,
    /// Mobile edge export profile (`--device cpu` required; planner gates).
    MobileEdge,
}

#[cfg(feature = "gpu")]
impl From<TrainingDeploymentTargetCli> for vox_populi::mens::TrainingDeploymentTarget {
    fn from(value: TrainingDeploymentTargetCli) -> Self {
        match value {
            TrainingDeploymentTargetCli::Workstation => Self::Workstation,
            TrainingDeploymentTargetCli::MobileEdge => Self::MobileEdge,
        }
    }
}

/// CLI mapping for optimizer experiment mode.
#[cfg(feature = "gpu")]
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum OptimizerExperimentModeCli {
    /// Stable default behavior.
    #[default]
    Off,
    /// Reserved experimental lane for MuonClip-style studies.
    MuonClipLike,
}

#[cfg(feature = "gpu")]
impl From<OptimizerExperimentModeCli> for vox_populi::mens::OptimizerExperimentMode {
    fn from(value: OptimizerExperimentModeCli) -> Self {
        match value {
            OptimizerExperimentModeCli::Off => Self::Off,
            OptimizerExperimentModeCli::MuonClipLike => Self::MuonClipLike,
        }
    }
}

/// Training data preparation policy before the native trainer runs.
#[cfg(feature = "gpu")]
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum, PartialEq, Eq)]
pub enum TrainDataModeCli {
    /// When the workspace corpus fingerprint is stale, run the same refresh path as `auto-refresh`
    /// (synthetic regen + `vox mens pipeline` without train + mix copy). **Refresh failures abort.**
    /// Prefer this for CI, release gates, and reproducible local runs.
    Strict,
    /// When stale, refresh corpus artifacts; non-fatal step failures are logged and training may still proceed.
    #[default]
    AutoRefresh,
}

/// Structured stages for the dogfood pipeline (`vox mens pipeline`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    /// Synthetic data generation (`vox mens corpus generate`).
    Generate,
    /// Extracting training pairs from source files (`vox mens corpus extract`).
    Extract,
    /// Validating and deduplicating the corpus (`vox mens corpus validate`).
    Validate,
    /// Generating instruction-response pairs (`vox mens corpus pairs`).
    Pairs,
    /// Evaluating training data quality metrics (`vox mens corpus eval`).
    Eval,
    /// Merging corpus sources per `mix.yaml` (`vox mens corpus mix`).
    Mix,
    /// Converting heuristic failures into preference data (`vox mens corpus heal-to-dpo`).
    HealToDpo,
    /// Generating synthetic research multi-hop chains (`vox mens corpus research-gen`).
    ResearchGen,
    /// Replaying Arca telemetry into training pairs (`vox mens corpus replay`).
    Replay,
    /// Ingesting external review findings (`vox review coderabbit ingest` / backfill).
    ReviewIngest,
    /// Building review-derived dataset artifacts.
    ReviewDatasetBuild,
    /// Building review-derived evaluation challenge pack.
    ReviewEvalPackBuild,
    /// Native model training (`vox mens train`).
    Train,
    /// Export accepted/rejected KB entries as training/DPO signals.
    KbSignals,
}

impl PipelineStage {
    /// Human-readable label for the stage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generate => "generate",
            Self::Extract => "extract",
            Self::Validate => "validate",
            Self::Pairs => "pairs",
            Self::Eval => "eval",
            Self::Mix => "mix",
            Self::HealToDpo => "heal_to_dpo",
            Self::ResearchGen => "research_gen",
            Self::Replay => "replay",
            Self::ReviewIngest => "review_ingest",
            Self::ReviewDatasetBuild => "review_dataset_build",
            Self::ReviewEvalPackBuild => "review_eval_pack_build",
            Self::Train => "train",
            Self::KbSignals => "kb_signals",
        }
    }
}

/// Progress snapshot for a pipeline run, used for telemetry and dashboard reporting.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PipelineProgress {
    /// Unique run ID (timestamp-based).
    pub run_id: String,
    /// Current active stage.
    pub current_stage: PipelineStage,
    /// Total number of stages planned.
    pub total_stages: usize,
    /// Number of stages completed so far.
    pub completed_stages: usize,
    /// Percentage complete (0.0 - 100.0).
    pub progress_pct: f64,
}
