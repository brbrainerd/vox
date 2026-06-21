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

#[cfg(any(feature = "mens-train", feature = "mens-cloud"))]
pub mod cohort;

#[cfg(feature = "mens")]
pub mod healing;

#[cfg(feature = "mens-hf-hub")]
pub mod hub;

#[cfg(feature = "mens-cloud")]
pub mod cloud;

#[cfg(feature = "mesh-discovery-publish")]
pub mod discovery_publish;

pub mod serving;

/// Default HuggingFace model for Mens training and serving (VoxMens QLoRA SSOT).
///
/// **USER DECISION (2026-06-21): Qwen3 everywhere.** The bare no-domain default
/// resolves to the Qwen3 `agentic_default` 16 GB-tier rung (Qwen3-8B), matching
/// the spoke ladders in `gpu-specs.yaml` so a 16 GB box defaults to Qwen3-8B
/// instead of the legacy Qwen2.5-Coder-7B.
///
/// The `@PLACEHOLDER-*` revision is deliberate: every Qwen3 rung is unpinned
/// until a real HF commit SHA is recorded. The fail-closed placeholder guard
/// ([`tensor::spoke_base_resolver::ensure_not_placeholder`]) rejects this id on
/// any real train/dispatch path, so a money run cannot proceed against an
/// unpinned base — dry-run / planning paths still print the plan and exit 0.
///
/// Local backwards-compat (USER DECISION): the `CandleQlora` path and the
/// `qwen_4080_16g` preset remain unchanged; `qwen3_*` rungs are additive.
pub const DEFAULT_MODEL_ID: &str = "Qwen/Qwen3-8B@PLACEHOLDER-a7b3d091";

/// Resolve the default training/inference base model id from a raw env override,
/// falling back to [`DEFAULT_MODEL_ID`]. Blank/whitespace overrides fall back.
pub fn resolve_default_model_id(raw: Option<&str>) -> String {
    match raw.map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => DEFAULT_MODEL_ID.to_string(),
    }
}

/// Convenience: resolve from the `VOX_MENS_DEFAULT_MODEL` process env.
pub fn default_model_id() -> String {
    resolve_default_model_id(std::env::var("VOX_MENS_DEFAULT_MODEL").ok().as_deref())
}

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

#[cfg(test)]
mod default_model_tests {
    use super::*;

    #[test]
    fn falls_back_to_const() {
        assert_eq!(resolve_default_model_id(None), DEFAULT_MODEL_ID);
    }

    #[test]
    fn env_override_wins() {
        assert_eq!(
            resolve_default_model_id(Some("org/My-Model")),
            "org/My-Model"
        );
    }

    #[test]
    fn blank_env_falls_back() {
        assert_eq!(resolve_default_model_id(Some("   ")), DEFAULT_MODEL_ID);
    }

    #[test]
    fn empty_string_keeps_default() {
        assert_eq!(resolve_default_model_id(Some("")), DEFAULT_MODEL_ID);
    }
}
