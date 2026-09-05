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
/// Revision is pinned to a real HF commit SHA. The fail-closed placeholder guard
/// ([`tensor::spoke_base_resolver::ensure_not_placeholder`]) still rejects any
/// id whose revision text contains "PLACEHOLDER", so a money run cannot proceed
/// against an unpinned base — dry-run / planning paths still print the plan and exit 0.
///
/// Local backwards-compat (USER DECISION): the `CandleQlora` path and the
/// `qwen_4080_16g` preset remain unchanged; `qwen3_*` rungs are additive.
pub const DEFAULT_MODEL_ID: &str = "Qwen/Qwen3-8B@b968826d9c46dd6066d109eabc6255188de91218";

/// Approximate on-disk download size of [`DEFAULT_MODEL_ID`]'s weights, in
/// bytes, so a user opting into the default can see the cost before it lands.
///
/// **Arithmetic (checkable, not folklore):** Qwen3-8B has ~8.19 billion
/// parameters (per the upstream model card); the pinned revision is served as
/// bf16 safetensors, i.e. 2 bytes per parameter:
///
/// ```text
/// 8_190_000_000 params × 2 bytes/param = 16_380_000_000 bytes ≈ 16.4 GB
/// ```
///
/// This is **approximate**, not a verified byte count: it ignores the small
/// tokenizer/config files bundled alongside the weights, and a network call
/// would be required to read the repo's exact reported size — which resolution
/// code must not make just to print a number (see `vox-speech`'s
/// `sherpa_model_config` for the same discipline applied to speech models).
/// Treat this as an order-of-magnitude warning, not an exact byte count.
pub const DEFAULT_MODEL_APPROX_BYTES: u64 = 8_190_000_000 * 2;

/// Human-readable rendering of [`DEFAULT_MODEL_APPROX_BYTES`], for display to
/// a user or CI log *before* a training run starts pulling the default base
/// model — see [`DEFAULT_MODEL_APPROX_BYTES`] for the underlying arithmetic
/// and its approximation caveats.
pub fn default_model_approx_size_human() -> String {
    let gb = DEFAULT_MODEL_APPROX_BYTES as f64 / 1_000_000_000.0;
    format!("~{gb:.1} GB (approximate — see DEFAULT_MODEL_APPROX_BYTES doc)")
}

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

    #[test]
    fn default_model_approx_bytes_matches_documented_arithmetic() {
        // 8.19B params x 2 bytes/param (bf16) — keep this in lockstep with the
        // arithmetic spelled out in the doc comment above the constant.
        assert_eq!(DEFAULT_MODEL_APPROX_BYTES, 8_190_000_000 * 2);
    }

    #[test]
    fn default_model_approx_size_human_reports_gb() {
        let human = default_model_approx_size_human();
        assert!(
            human.contains("GB"),
            "expected a GB-scale human size, got: {human}"
        );
        assert!(
            human.contains("16.4"),
            "expected ~16.4 GB for Qwen3-8B at bf16, got: {human}"
        );
    }
}
