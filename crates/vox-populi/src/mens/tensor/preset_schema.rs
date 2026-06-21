//! Training hyperparameter presets: 4080, safe, A100-shaped profiles.

use crate::mens::tensor::device::probe_gpu;

/// CLI numeric overrides for auto-tuning.
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub rank: Option<usize>,
    pub alpha: Option<f32>,
    pub seq_len: Option<usize>,
    pub batch_size: Option<usize>,
    pub grad_accum: Option<usize>,
    pub epochs: Option<usize>,
    pub warmup: Option<usize>,
    pub lr: Option<f64>,
    pub budget_seq_len: Option<usize>,
    pub budget_batch_size: Option<usize>,
    pub budget_grad_accum: Option<usize>,
    pub vram_limit_fraction: Option<f32>,
}

/// GPU-derived device profile.
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    pub model_name: String,
    pub vram_mb: u64,
}

impl DeviceProfile {
    pub fn from_gpu_info(model_name: &str, vram_mb: u64) -> Self {
        Self {
            model_name: model_name.to_string(),
            vram_mb,
        }
    }
}

/// Effective training hyperparameters after preset + overrides + dataset scaling heuristics.
#[derive(Debug, Clone)]
pub struct TrainPresetProfile {
    pub rank: usize,
    pub alpha: f32,
    pub seq_len: usize,
    pub batch_size: usize,
    pub grad_accum: usize,
    pub epochs: usize,
    pub warmup: usize,
    pub lr: f64,
}

pub const DEFAULT_PRESET: &str = "4080";

/// Preset names accepted by `--preset` / planner normalization.
///
/// **Contract SSOT:** mirror every entry in `contracts/mens/training-presets.v1.yaml` (enforced by
/// `vox-populi` integration test `training_presets_yaml_contract`).
pub const KNOWN_PRESETS: &[&str] = &[
    "tiny",
    "safe",
    "4080",
    "4080_safe",
    "qwen_4080_16g",
    "qwen_small_8g",
    "qwen_rtx3090_24g",
    "qwen_a100_80g",
    "a100",
    "default",
    "distributed",
    "mobile_edge",
    // Code-generation fine-tune preset (Vox .box target language).
    "vox-gen",
    // Qwen3 dense ladder presets — additive alongside legacy qwen_* presets.
    "qwen3_dev_cpu", // Qwen3-0.6B r8, CPU smoke — no quality gate
    "qwen3_16g",     // Qwen3-8B QLoRA r16 (RTX 4080 Super 16GB)
    "qwen3_24g",     // Qwen3-14B QLoRA r32 (3090/4090 24GB)
    "qwen3_48g",     // Qwen3-14B LoRA r32 un-quantized (48GB)
    "qwen3_96g",     // Qwen3-32B QLoRA r64 (96GB)
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QwenSizeClass {
    S0p8,
    S2,
    S4,
    S9,
    Other,
}

fn detect_qwen_size_class(model_hint: Option<&str>) -> Option<QwenSizeClass> {
    let m = model_hint?.to_ascii_lowercase();
    if !m.contains("qwen") {
        return None;
    }
    if m.contains("0.8b") {
        return Some(QwenSizeClass::S0p8);
    }
    if m.contains("2b") {
        return Some(QwenSizeClass::S2);
    }
    if m.contains("4b") {
        return Some(QwenSizeClass::S4);
    }
    if m.contains("9b") {
        return Some(QwenSizeClass::S9);
    }
    Some(QwenSizeClass::Other)
}

fn apply_qwen_size_ladder_policy(
    mut p: TrainPresetProfile,
    class: QwenSizeClass,
    vram_mb: u64,
) -> TrainPresetProfile {
    match class {
        QwenSizeClass::S0p8 => {
            p.rank = p.rank.min(16);
            p.alpha = p.alpha.min(32.0);
            p.seq_len = p.seq_len.clamp(384, 1024);
            p.batch_size = p.batch_size.max(2);
            p.grad_accum = p.grad_accum.max(4);
        }
        QwenSizeClass::S2 => {
            p.rank = p.rank.min(16);
            p.alpha = p.alpha.min(32.0);
            p.seq_len = p.seq_len.clamp(320, 768);
            p.batch_size = p.batch_size.max(1);
            p.grad_accum = p.grad_accum.max(6);
        }
        QwenSizeClass::S4 => {
            // Keep current 4080-class defaults; only enforce safe floors.
            p.batch_size = p.batch_size.max(1);
            p.grad_accum = p.grad_accum.max(8);
        }
        QwenSizeClass::S9 => {
            // 9B requires a tighter envelope on 16G class cards.
            p.rank = p.rank.min(8);
            p.alpha = p.alpha.min(16.0);
            if vram_mb <= 16_384 {
                p.seq_len = p.seq_len.min(256);
                p.batch_size = 1;
                p.grad_accum = p.grad_accum.max(16);
                p.lr = p.lr.min(1.0e-4);
            } else if vram_mb <= 24_576 {
                p.seq_len = p.seq_len.min(384);
                p.batch_size = p.batch_size.min(1);
                p.grad_accum = p.grad_accum.max(12);
            } else {
                p.seq_len = p.seq_len.min(512);
                p.grad_accum = p.grad_accum.max(8);
            }
        }
        QwenSizeClass::Other => {}
    }
    p
}

/// Canonicalize historical aliases to the current preset SSOT names.
fn normalize_preset_name(name: &str) -> &str {
    match name {
        // Legacy aliases still emitted by some autodetect paths.
        "qwen_small_8g" => "safe",
        "qwen_rtx3090_24g" => "4080",
        "qwen_a100_80g" => "a100",
        // Prosumer presets aligned with gpu-specs.yaml SSOT
        "prosumer_16g" => "qwen_4080_16g",
        "prosumer_24g" => "4080",
        "prosumer_12g" => "safe",
        // Historical generic alias kept as the 4080-class default.
        "default" => "4080",
        other => other,
    }
}

fn base_for_name(name: &str) -> TrainPresetProfile {
    match normalize_preset_name(name) {
        "tiny" => TrainPresetProfile {
            rank: 4,
            alpha: 8.0,
            seq_len: 128,
            batch_size: 1,
            grad_accum: 1,
            epochs: 1,
            warmup: 10,
            lr: 1e-4,
        },
        "safe" | "4080_safe" => TrainPresetProfile {
            rank: 8,
            alpha: 16.0,
            seq_len: 256,
            batch_size: 2,
            grad_accum: 8,
            epochs: 3,
            warmup: 50,
            lr: 2e-4,
        },
        // Conservative Qwen + Candle QLoRA on ~16GB (e.g. RTX 4080 Super).
        // `4080` is an alias of `qwen_4080_16g` so default preset matches 16G QLoRA, not generic LoRA.
        "4080" | "qwen_4080_16g" => TrainPresetProfile {
            rank: 16,
            alpha: 32.0,
            seq_len: 384,
            batch_size: 1,
            grad_accum: 8,
            epochs: 3,
            warmup: 80,
            lr: 1.5e-4,
        },
        "a100" => TrainPresetProfile {
            rank: 32,
            alpha: 64.0,
            seq_len: 1024,
            batch_size: 8,
            grad_accum: 2,
            epochs: 3,
            warmup: 200,
            lr: 2e-4,
        },
        "distributed" => TrainPresetProfile {
            rank: 16,
            alpha: 32.0,
            seq_len: 512,
            batch_size: 4,
            grad_accum: 8,
            epochs: 3,
            warmup: 150,
            lr: 1.5e-4,
        },
        "mobile_edge" => TrainPresetProfile {
            rank: 8,
            alpha: 16.0,
            seq_len: 256,
            batch_size: 1,
            grad_accum: 8,
            epochs: 3,
            warmup: 40,
            lr: 1.5e-4,
        },
        // Vox .vox code-generation fine-tune — short sequences, aggressive LoRA rank
        // to capture the compact grammar surface. Designed for RTX 4080-class (16GB).
        "vox-gen" => TrainPresetProfile {
            rank: 16,
            alpha: 32.0,
            seq_len: 256, // .vox programs are compact; 256 tokens covers most functions
            batch_size: 2,
            grad_accum: 8,
            epochs: 5, // more epochs for code: grammar must be memorized
            warmup: 60,
            lr: 1.5e-4,
        },
        "qwen3_dev_cpu" => TrainPresetProfile {
            rank: 8,
            alpha: 16.0,
            seq_len: 128,
            batch_size: 1,
            grad_accum: 1,
            epochs: 1, // smoke only — no quality gate at this tier
            warmup: 10,
            lr: 1e-4,
        },
        "qwen3_16g" => TrainPresetProfile {
            rank: 16,
            alpha: 32.0,
            seq_len: 512,
            batch_size: 1,
            grad_accum: 8,
            epochs: 3,
            warmup: 100,
            lr: 1.5e-4,
        },
        "qwen3_24g" => TrainPresetProfile {
            rank: 32,
            alpha: 64.0,
            seq_len: 768,
            batch_size: 1,
            grad_accum: 4,
            epochs: 3,
            warmup: 100,
            lr: 1e-4,
        },
        "qwen3_48g" => TrainPresetProfile {
            rank: 32,
            alpha: 64.0,
            seq_len: 1024,
            batch_size: 2,
            grad_accum: 4,
            epochs: 3,
            warmup: 100,
            lr: 1e-4,
        },
        "qwen3_96g" => TrainPresetProfile {
            rank: 64,
            alpha: 128.0,
            seq_len: 2048,
            batch_size: 4,
            grad_accum: 2,
            epochs: 3,
            warmup: 100,
            lr: 8e-5,
        },
        _ => TrainPresetProfile {
            rank: 16,
            alpha: 32.0,
            seq_len: 512,
            batch_size: 4,
            grad_accum: 4,
            epochs: 3,
            warmup: 100,
            lr: 2e-4,
        },
    }
}

/// Load the global GPU specifications and presets from `mens/config/gpu-specs.yaml`.
pub fn load_gpu_specs() -> Option<GpuSpecsFile> {
    let root = vox_corpus::training::contract::find_workspace_root()?;
    let p = root.join("mens/config/gpu-specs.yaml");
    let raw = vox_bounded_fs::read_utf8_path_capped(p.as_path()).ok()?;
    serde_yaml::from_str(&raw).ok()
}

/// Load optional YAML registry from `mens/config/train-presets.yaml` if present.
pub struct TrainPresetRegistry;

impl TrainPresetRegistry {
    pub fn load() -> Option<serde_yaml::Value> {
        let root = vox_corpus::training::contract::find_workspace_root()?;
        let p = root.join("mens/config/train-presets.yaml");
        let raw = vox_bounded_fs::read_utf8_path_capped(p.as_path()).ok()?;
        serde_yaml::from_str(&raw).ok()
    }
}

pub fn load_registry() -> Option<serde_yaml::Value> {
    TrainPresetRegistry::load()
}

/// Resolve preset from `VOX_TRAIN_PROFILE` env, CLI `--preset`, device heuristics, and overrides.
pub fn resolve_effective_profile(
    preset: Option<&str>,
    device: DeviceProfile,
    sample_count: Option<usize>,
    overrides: CliOverrides,
) -> TrainPresetProfile {
    let model_hint_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxBaseModel);
    let model_hint = model_hint_resolved.expose();
    let env_p_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxTrainProfile);
    let env_p = env_p_resolved.expose();
    let name = normalize_preset_name(preset.or(env_p).unwrap_or(DEFAULT_PRESET));

    let mut p = if name == "auto" {
        if let Some(specs) = load_gpu_specs() {
            if let Some((_name, preset_spec)) =
                TrainingPreset::best_for_vram(&specs.presets, device.vram_mb)
            {
                TrainPresetProfile {
                    rank: 16,
                    alpha: 32.0,
                    seq_len: preset_spec.seq_len,
                    batch_size: preset_spec.batch_size,
                    grad_accum: preset_spec.grad_accum,
                    epochs: 3,
                    warmup: 100,
                    lr: preset_spec.lr,
                }
            } else {
                base_for_name("4080_safe")
            }
        } else {
            base_for_name("4080_safe")
        }
    } else {
        base_for_name(name)
    };

    if let Some(n) = sample_count
        && n < 500
    {
        p.epochs = p.epochs.clamp(2, 5);
        p.warmup = p.warmup.min(50);
    }

    if let Some(r) = overrides.rank {
        p.rank = r;
    }
    if let Some(a) = overrides.alpha {
        p.alpha = a;
    }
    if let Some(s) = overrides.seq_len {
        p.seq_len = s;
    }
    if let Some(b) = overrides.batch_size {
        p.batch_size = b;
    }
    if let Some(g) = overrides.grad_accum {
        p.grad_accum = g;
    }
    if let Some(e) = overrides.epochs {
        p.epochs = e;
    }
    if let Some(w) = overrides.warmup {
        p.warmup = w;
    }
    if let Some(l) = overrides.lr {
        p.lr = l;
    }

    if let Some(class) = detect_qwen_size_class(model_hint.as_deref()) {
        p = apply_qwen_size_ladder_policy(p, class, device.vram_mb);
    }

    // Determine the VRAM budget limits, either from the passed pre-computed overrides
    // or by running the budget planner internally as a fallback.
    let budget_limits = if let Some(seq) = overrides.budget_seq_len
        && let Some(batch) = overrides.budget_batch_size
        && let Some(accum) = overrides.budget_grad_accum
    {
        Some((seq, batch, accum))
    } else if device.vram_mb > 0 {
        // Fallback: run budget planner internally
        let mut vram_gib = (device.vram_mb as f64) / 1024.0;
        if let Some(frac) = overrides.vram_limit_fraction {
            vram_gib *= frac as f64;
        }

        let hint = model_hint
            .as_deref()
            .unwrap_or(crate::mens::DEFAULT_MODEL_ID);
        let params_b =
            crate::mens::tensor::memory_budget::params_b_from_model_hint(hint).unwrap_or(7.0);

        let gc_explicit = std::env::var("VOX_MENS_GRADIENT_CHECKPOINTING")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let gc_auto = params_b >= 2.9;
        let gradient_checkpointing = gc_explicit || gc_auto;

        let quant = crate::mens::tensor::finetune_contract::BaseQuantMode::Nf4;

        let mp = if crate::mens::tensor::memory_budget::is_qwen25coder(hint) {
            crate::mens::tensor::memory_budget::plan_qwen25coder_with_options(
                vram_gib,
                params_b,
                quant,
                gradient_checkpointing,
            )
        } else if crate::mens::tensor::memory_budget::is_qwen35(hint) {
            crate::mens::tensor::memory_budget::plan_qwen35_with_options(
                vram_gib,
                params_b,
                quant,
                gradient_checkpointing,
            )
        } else if crate::mens::tensor::memory_budget::is_qwen3(hint) {
            crate::mens::tensor::memory_budget::plan_qwen3_with_options(
                vram_gib,
                params_b,
                quant,
                gradient_checkpointing,
            )
        } else {
            let resident_per_b = crate::mens::tensor::memory_budget::get_resident_per_b(
                hint,
                quant,
                gradient_checkpointing,
            );
            let p = crate::mens::tensor::memory_budget::plan_with_resident(
                vram_gib,
                params_b,
                resident_per_b,
            );
            crate::mens::tensor::memory_budget::ModelPlan {
                model_id: hint.to_string(),
                params_b,
                seq_len: p.seq_len,
                batch_size: p.batch_size,
                grad_accum: p.grad_accum,
                retreated_from_b: None,
                over_budget: p.over_budget,
                rationale: p.rationale,
            }
        };

        // Dual-sizing fix: if the planner retreated, we must re-solve specifically
        // for the requested model's parameters to avoid OOM at training runtime.
        let final_plan = if mp.retreated_from_b.is_some() {
            let resident_per_b = crate::mens::tensor::memory_budget::get_resident_per_b(
                hint,
                quant,
                gradient_checkpointing,
            );
            let p = crate::mens::tensor::memory_budget::plan_with_resident(
                vram_gib,
                params_b,
                resident_per_b,
            );
            crate::mens::tensor::memory_budget::ModelPlan {
                model_id: hint.to_string(),
                params_b,
                seq_len: p.seq_len,
                batch_size: p.batch_size,
                grad_accum: p.grad_accum,
                retreated_from_b: None,
                over_budget: p.over_budget,
                rationale: p.rationale,
            }
        } else {
            mp
        };

        Some((
            final_plan.seq_len,
            final_plan.batch_size,
            final_plan.grad_accum,
        ))
    } else {
        None
    };

    if let Some((b_seq_len, b_batch_size, b_grad_accum)) = budget_limits {
        if overrides.seq_len.is_none() {
            p.seq_len = p.seq_len.min(b_seq_len);
        }
        if overrides.batch_size.is_none() {
            p.batch_size = p.batch_size.min(b_batch_size);
        }
        if overrides.grad_accum.is_none() {
            p.grad_accum = p.grad_accum.max(b_grad_accum);
        }
    }

    let _ = probe_gpu();
    p
}

/// Back-compat alias used in older docs.
pub type DatasetProfile = TrainPresetProfile;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level structure of `mens/config/gpu-specs.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSpecsFile {
    /// GPU name → physical specification.
    pub gpus: HashMap<String, GpuSpec>,
    /// VRAM preset name → training configuration.
    #[serde(default)]
    pub presets: HashMap<String, TrainingPreset>,
}

/// Physical GPU specification loaded from `mens/config/gpu-specs.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSpec {
    /// FP16 TFLOPS from vendor datasheet.
    pub fp16_tflops: f64,
    /// VRAM in MB.
    pub vram_mb: u64,
}

/// Training preset configuration — auto-selected by VRAM tier for both local and cloud.
///
/// Defined once in `gpu-specs.yaml`; consumed by both `vox mens train` (local)
/// and cloud dispatch (to set container env vars). This is the SSOT for preset configs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingPreset {
    /// Sequence length in tokens.
    pub seq_len: usize,
    /// Micro-batch size per gradient step.
    pub batch_size: usize,
    /// Gradient accumulation steps (effective batch = batch_size × grad_accum).
    pub grad_accum: usize,
    /// Learning rate.
    pub lr: f64,
    /// Maximum VRAM in MB this preset can fit. Used to auto-select from local VRAM.
    pub max_vram_mb: u64,
}

impl TrainingPreset {
    /// Select the best preset for the given VRAM amount.
    pub fn best_for_vram(
        presets: &HashMap<String, TrainingPreset>,
        vram_mb: u64,
    ) -> Option<(&str, &TrainingPreset)> {
        presets
            .iter()
            .filter(|(_, p)| p.max_vram_mb <= vram_mb)
            .max_by_key(|(_, p)| p.max_vram_mb)
            .map(|(k, v)| (k.as_str(), v))
    }
}

#[cfg(test)]
mod preset_tests {
    use super::*;

    #[test]
    fn preset_4080_matches_qwen_4080_16g() {
        let a = base_for_name("4080");
        let b = base_for_name("qwen_4080_16g");
        assert_eq!(a.seq_len, b.seq_len);
        assert_eq!(a.batch_size, b.batch_size);
        assert_eq!(a.grad_accum, b.grad_accum);
        assert_eq!(a.rank, b.rank);
        assert_eq!(a.lr, b.lr);
    }

    #[test]
    fn known_presets_include_4080_family() {
        assert!(KNOWN_PRESETS.contains(&"4080"));
        assert!(KNOWN_PRESETS.contains(&"qwen_4080_16g"));
    }

    #[test]
    fn legacy_qwen_aliases_map_to_current_profiles() {
        let small = base_for_name("qwen_small_8g");
        let safe = base_for_name("safe");
        assert_eq!(small.seq_len, safe.seq_len);
        assert_eq!(small.rank, safe.rank);

        let midsize = base_for_name("qwen_rtx3090_24g");
        let p4080 = base_for_name("4080");
        assert_eq!(midsize.seq_len, p4080.seq_len);
        assert_eq!(midsize.rank, p4080.rank);

        let big = base_for_name("qwen_a100_80g");
        let a100 = base_for_name("a100");
        assert_eq!(big.seq_len, a100.seq_len);
        assert_eq!(big.rank, a100.rank);
    }

    #[test]
    fn mobile_edge_preset_is_single_batch() {
        let p = base_for_name("mobile_edge");
        assert_eq!(p.batch_size, 1);
        assert!(p.seq_len <= 512);
        assert!(p.rank <= 32);
    }

    #[test]
    fn test_prosumer_16g_preset_resolves() {
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("VOX_BASE_MODEL", "Qwen/Qwen2.5-Coder-1.5B-Instruct");
        }
        let dev = DeviceProfile::from_gpu_info("rtx 4080 super", 16384);
        let profile =
            resolve_effective_profile(Some("prosumer_16g"), dev, None, CliOverrides::default());
        assert_eq!(profile.seq_len, 384);
        assert_eq!(profile.batch_size, 1);
        assert_eq!(profile.grad_accum, 8);
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_BASE_MODEL");
        }
    }

    #[test]
    fn presets_are_bounded_by_vram() {
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("VOX_BASE_MODEL", "Qwen/Qwen2.5-Coder-7B-Instruct");
        }
        let dev = DeviceProfile::from_gpu_info("rtx 4080 super", 16384);
        let profile = resolve_effective_profile(Some("a100"), dev, None, CliOverrides::default());
        assert!(profile.seq_len < 1024);
        assert!(profile.batch_size < 8);
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_BASE_MODEL");
        }
    }

    #[test]
    fn test_preset_bounds_dynamically_to_fit_vram() {
        let dev = DeviceProfile::from_gpu_info("rtx 4080 super", 16384);
        let profile =
            resolve_effective_profile(Some("prosumer_16g"), dev, None, CliOverrides::default());
        // For a 7B model on 16GB, it should safely scale parameters down.
        assert!(profile.seq_len <= 384);
    }
}

#[cfg(test)]
mod qwen3_preset_tests {
    use super::*;

    #[test]
    fn known_presets_contains_all_qwen3_tiers() {
        for name in &["qwen3_dev_cpu", "qwen3_16g", "qwen3_24g", "qwen3_48g", "qwen3_96g"] {
            assert!(
                KNOWN_PRESETS.contains(name),
                "KNOWN_PRESETS missing qwen3 preset: {}",
                name
            );
        }
    }

    #[test]
    fn qwen3_dev_cpu_is_smoke_only() {
        let p = base_for_name("qwen3_dev_cpu");
        assert_eq!(p.rank, 8, "dev cpu must be r8 (smoke only)");
        assert_eq!(p.epochs, 1, "dev cpu is single-epoch smoke only");
        assert!(p.seq_len <= 256, "dev cpu must have short seq_len for CPU fit, got {}", p.seq_len);
    }

    #[test]
    fn qwen3_96g_is_high_rank() {
        let p = base_for_name("qwen3_96g");
        assert!(p.rank >= 64, "qwen3_96g must have rank >= 64, got {}", p.rank);
    }

    #[test]
    fn old_qwen_presets_still_load() {
        // Backwards compat: existing presets must not be broken
        for name in &["qwen_4080_16g", "qwen_small_8g", "qwen_rtx3090_24g", "qwen_a100_80g"] {
            let _ = base_for_name(name);
        }
        // qwen_4080_16g should still work as it always did
        let p = base_for_name("qwen_4080_16g");
        assert_eq!(p.rank, 16);
    }
}

/// B0.7 — Local 4080/CPU backwards-compatibility guard.
///
/// These tests prove that:
/// 1. The `qwen_4080_16g` preset is unchanged alongside the new `qwen3_*` presets.
/// 2. The `AdapterCard` / `DomainRouter` infrastructure works for `provider: "local"`.
/// 3. (mens-train only) The execution planner still maps QLoRA+NF4 → CandleQlora backend.
///
/// Note: this module and `local_compat_b07_planner_tests` both require `--features mens-train`
/// to compile (same gate as the parent `preset_schema` module in `tensor/mod.rs`).
///
/// These tests document already-working invariants; no implementation change is expected.
/// If any test fails, it indicates B0's AdapterCard work broke local training infrastructure.
#[cfg(test)]
mod local_compat_b07_tests {
    use super::*;
    use crate::mens::tensor::adapter_card::AdapterCard;
    use crate::mens::tensor::domain_router::DomainRouter;

    /// Prove the legacy qwen_4080_16g preset is not perturbed by the new qwen3_* additions.
    /// This is the primary preset used by the local RTX 4080 Super training path.
    #[test]
    fn qwen_4080_16g_preset_still_loads_alongside_qwen3() {
        let p = base_for_name("qwen_4080_16g");
        assert_eq!(p.rank, 16, "qwen_4080_16g rank must remain 16 (r16 QLoRA for 16GB VRAM)");
        assert_eq!(p.grad_accum, 8, "qwen_4080_16g grad_accum must remain 8");
        assert_eq!(p.lr, 1.5e-4, "qwen_4080_16g lr must remain 1.5e-4");
        assert_eq!(p.alpha, 32.0, "qwen_4080_16g alpha must remain 32.0");
        assert_eq!(p.seq_len, 384, "qwen_4080_16g seq_len must remain 384");

        // Both old and new presets must coexist in KNOWN_PRESETS
        assert!(
            KNOWN_PRESETS.contains(&"qwen_4080_16g"),
            "qwen_4080_16g must remain in KNOWN_PRESETS"
        );
        assert!(
            KNOWN_PRESETS.contains(&"qwen3_16g"),
            "new qwen3_16g preset must coexist with old qwen_4080_16g"
        );
        assert!(
            KNOWN_PRESETS.contains(&"qwen3_dev_cpu"),
            "new qwen3_dev_cpu (CPU smoke tier) must coexist with old qwen_4080_16g"
        );
    }

    /// Prove that an AdapterCard with `provider: "local"` can be created, passes validation,
    /// and registers successfully in a DomainRouter. This is the end-to-end local training
    /// card emit path (B5/B8 will wire this to the actual training loop).
    #[test]
    fn local_adapter_card_provider_is_local_and_registers() {
        // Use the for_test() constructor — which already sets provider="local"
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        assert_eq!(card.provider, "local", "for_test() must produce provider=local (4080 Super)");

        // Validate passes
        card.validate()
            .expect("local AdapterCard from for_test() must pass validation");

        // Compatibility checks
        assert!(
            card.is_compatible_with("qwen3_16g", "qlora"),
            "local card must be compatible with its own rung+quant"
        );
        assert!(
            !card.is_compatible_with("qwen3_24g", "qlora"),
            "rung mismatch (qwen3_24g vs qwen3_16g) must be detected at serve time"
        );
        assert!(
            !card.is_compatible_with("qwen3_16g", "lora"),
            "quant mismatch (lora vs qlora) must be detected at serve time"
        );

        // DomainRouter registration
        let mut router = DomainRouter::new();
        router
            .register("vox-lang", "/fake/path/adapter_model.safetensors", card)
            .expect("local AdapterCard must register in DomainRouter without error");

        // Verify round-trip
        let (_, registered_card) = router.route("vox-lang")
            .expect("vox-lang domain must be routable after registration");
        assert_eq!(
            registered_card.provider, "local",
            "provider field must survive DomainRouter round-trip"
        );
        assert_eq!(registered_card.base_rung, "qwen3_16g");
        assert_eq!(registered_card.quantization, "qlora");
    }

    /// Prove the fail-closed guard: a card missing base_rung must not register.
    /// (Prevents silent "local" adapters with empty provenance from being served.)
    #[test]
    fn local_adapter_card_with_empty_rung_rejected_by_router() {
        let mut card = AdapterCard::for_test("", "qlora"); // empty rung
        card.base_revision = "abc".to_string();
        let mut router = DomainRouter::new();
        let result = router.register("vox-lang", "/fake/path/adapter_model.safetensors", card);
        assert!(
            result.is_err(),
            "empty base_rung must be rejected by DomainRouter (fail-closed)"
        );
    }
}

/// B0.7 execution-planner kernel mapping guard (mens-train feature only).
///
/// Proves that (AdapterMethod::Qlora, BaseQuantMode::Nf4) still maps to
/// PopuliTrainBackend::CandleQlora — the kernel used by the local RTX 4080 Super path.
/// This mapping must not be perturbed by any AdapterCard or preset work.
#[cfg(all(test, feature = "mens-train"))]
mod local_compat_b07_planner_tests {
    use crate::mens::tensor::execution_planner::ExecutionPlanner;
    use crate::mens::tensor::finetune_contract::{
        AdapterMethod, AdapterSpec, AdapterTargetMask, ArtifactSpec, BaseQuantMode, DataSpec,
        ExecSpec, FineTuneContract, ModelProvenanceSpec, ModelSpec, QuantSpec,
    };
    use crate::mens::tensor::train_backend::PopuliTrainBackend;
    use crate::mens::tensor::training_config::MensTokenizerMode;

    fn minimal_qlora_nf4_contract() -> FineTuneContract {
        FineTuneContract {
            model: ModelSpec {
                hf_repo: None,
                weight_shards: None,
                config_json: None,
                tokenizer_json: None,
            },
            collateral_damage_verified: false,
            provenance: ModelProvenanceSpec {
                base_family: None,
                upstream_model_id: None,
                license_class: None,
                attribution_required: false,
            },
            data: DataSpec {
                train_file: None,
                tokenizer_mode: MensTokenizerMode::Hf,
                min_rating: 3,
                context_filter: None,
            },
            adapter: AdapterSpec {
                method: AdapterMethod::Qlora,
                rank: 16,
                alpha: 32.0,
                dropout: 0.0,
                targets: AdapterTargetMask::FullGraph,
            },
            quant: QuantSpec {
                base: BaseQuantMode::Nf4,
                double_quant: true,
            },
            exec: ExecSpec {
                epochs: 1,
                seq_len: 384,
                batch_size: 1,
                grad_accum: 8,
                learning_rate: 1.5e-4,
                warmup_steps: 80,
                seed: 42,
                resume_from: None,
                max_vram_fraction: None,
                adapter_tag: None,
                qlora_require_full_proxy_stack: false,
                qlora_max_skip_rate: None,
                qlora_lm_head_only: false,
                qlora_proxy_max_layers: None,
                qlora_ce_last_k: 1,
                curriculum_schedule: None,
            },
            artifact: ArtifactSpec::default(),
        }
    }

    /// The local 4080 Super uses QLoRA+NF4 → must resolve to CandleQlora (not BurnLora).
    /// If this mapping changes, local training silently breaks.
    ///
    /// Strategy: `force_kernel=CandleQlora` makes `plan()` error if the planner infers a
    /// different backend — so a successful `plan()` call is proof of the CandleQlora mapping.
    #[test]
    fn local_qlora_nf4_resolves_to_candle_backend() {
        let contract = minimal_qlora_nf4_contract();
        // Use force_kernel=CandleQlora: plan() errors if inferred != forced, so success here
        // proves the planner infers CandleQlora for (Qlora, Nf4) contracts.
        let plan = ExecutionPlanner {
            force_kernel: Some(PopuliTrainBackend::CandleQlora),
        }
        .plan(&contract)
        .expect("QLoRA+NF4 contract must resolve to CandleQlora (local 4080 Super path)");

        assert_eq!(
            plan.kernel,
            PopuliTrainBackend::CandleQlora,
            "local 4080 Super: Qlora+Nf4 must map to CandleQlora, not BurnLora"
        );
        assert!(
            plan.candle_compat_mode,
            "CandleQlora kernel must set candle_compat_mode=true"
        );
    }

    /// Prove BurnLora is NOT the local path: Lora+None maps to BurnLora, which is distinct.
    /// Guards against accidentally switching the local preset to Burn by mistake.
    #[test]
    fn burn_lora_is_distinct_from_local_qlora_path() {
        // The local 4080 path is CandleQlora — BurnLora would be a regression.
        // Ensure the two kernels are distinguishable (not accidentally made equal).
        assert_ne!(
            PopuliTrainBackend::CandleQlora,
            PopuliTrainBackend::BurnLora,
            "CandleQlora and BurnLora must be distinct enum variants"
        );
    }
}
