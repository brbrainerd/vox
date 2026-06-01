//! Mens — native Burn-based LoRA training and inference helpers.
//!
//! - **Preflight / prompts**: use [`vox_corpus::training`].
//! - **Tokenizer + JSONL**: re-exported from [`tensor::data`] (`vox-tensor`).
//!
//! Public surface is mostly CLI / training wiring; exhaustive per-field `///` is deferred (see
//! `docs/agents/doc-quality-verification.md`).

#![allow(missing_docs)]

pub mod hardware;
pub mod kernels;
pub mod tensor;

#[cfg(feature = "mens")]
pub mod healing;

#[cfg(feature = "mens-hf-hub")]
pub mod hub;

#[cfg(feature = "mens-cloud")]
pub mod cloud;

#[cfg(feature = "mesh-discovery-publish")]
pub mod discovery_publish;

/// Default HuggingFace model for Mens training and serving (VoxMens QLoRA SSOT).
///
/// Qwen2.5-Coder is the coding-focused dense family the candle QLoRA trainer
/// supports end-to-end (full-attention, no MoE/MTP/vision). The VRAM-aware ladder
/// retreats this 7B request to the largest variant that fits the card — 3B on a
/// 16 GiB consumer GPU (bf16), 7B on 24-32 GiB. The previous default, Qwen3.5-4B,
/// is a `*ForConditionalGeneration` vision-language checkpoint that the text
/// trainer cannot train (it is rejected up front by the vox-hf-layout VL guard).
pub const DEFAULT_MODEL_ID: &str = "Qwen/Qwen2.5-Coder-7B-Instruct";

pub use tensor::{
    DeviceKind, GpuInfo, apply_backend_env, detect_gpu_vendor, estimate_training_vram_mb,
    estimate_training_vram_mb_qlora, normalize_device, print_gpu_summary, print_gpu_summary_for,
    probe_gpu,
};

#[cfg(feature = "mens-train")]
pub use tensor::artifact_bridge::MERGE_QLORA_REJECTS_BURN_BIN;
#[cfg(feature = "mens-train")]
pub use tensor::operator_messages;
#[cfg(feature = "mens-train")]
pub use tensor::{
    CliOverrides, DEFAULT_PRESET, DatasetProfile, DeviceProfile, KNOWN_PRESETS, TrainPresetProfile,
    TrainPresetRegistry, load_registry, resolve_effective_profile,
};
#[cfg(feature = "mens-train")]
pub use tensor::{
    ExecutionKernel, FineTuneContract, LoraTrainingConfig, MensTokenizerMode,
    OptimizerExperimentMode, PopuliTrainBackend, TrainingDeploymentTarget, run_mens_training,
};
