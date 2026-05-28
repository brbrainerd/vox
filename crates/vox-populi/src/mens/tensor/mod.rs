//! Mens tensor surface: LoRA transformer, device helpers, training loop, manifests.

#![allow(clippy::module_inception)]

pub mod data;
pub mod device;
pub mod hf_keymap;
pub mod hf_load;
pub mod manifest;
pub mod model_card;
pub mod telemetry;
pub mod telemetry_schema;

// SP3 Unit 1: candle_model_qwen deleted; canonical copy in vox-plugin-mens-candle-cuda/src/model.rs
// SP3 Unit 3: candle_inference_serve deleted; canonical copy in vox-plugin-mens-candle-cuda/src/inference.rs
// vox-ml-cli eval-local is rewired to use Option<()> stub until plugin-host dispatch is plumbed.
pub use crate::distributed_training::mesh_env::{MeshTrainConfig, get_mesh_rank, is_mesh_mode};
pub mod train_log;
#[cfg(feature = "mens-train")]
pub mod training_text;

pub mod vram_autodetect;

pub use device::{
    DeviceKind, GpuInfo, TrainProfile, apply_backend_env, detect_gpu_vendor,
    estimate_training_vram_mb, estimate_training_vram_mb_qlora, normalize_device, oom_guidance,
    print_gpu_summary, print_gpu_summary_for, probe_gpu, recommend_config,
    recommend_config_for_profile, sample_vram_used_mb,
};

// adapter_schema_v3 deleted: vox-ml-cli/merge_qlora.rs holds inline serde types; plugin owns merge impl.
#[cfg(feature = "mens-train")]
pub mod artifact_bridge;
#[cfg(feature = "mens-train")]
pub mod backend;

#[cfg(feature = "mens-train")]
mod backend_candle_qlora;
#[cfg(feature = "mens-train")]
pub mod checkpoint_state;
// SP3-D: candle_qlora_train, candle_qlora_weights, qlora_preflight, candle_qlora_graph extracted
// to vox-plugin-mens-candle-cuda.
// candle_model_qwen + candle_inference_serve deleted (SP3 Units 1+3).
//
// candle_qlora_merge deleted: plugin owns merge impl; vox-ml-cli/merge_qlora.rs dispatches via plugin.
#[cfg(feature = "mens-train")]
pub mod domain_profiles;
pub mod domain_router;
#[cfg(feature = "mens-train")]
pub mod execution_planner;
#[cfg(feature = "mens-train")]
pub mod external_serving_handoff;
#[cfg(feature = "mens-train")]
pub mod finetune_contract;
#[cfg(feature = "mens-train")]
pub mod finetune_registry;
#[cfg(feature = "mens-train")]
pub mod lora_train;
#[cfg(feature = "mens-train")]
pub mod operator_messages;
#[cfg(feature = "mens-train")]
pub mod preflight_train;
#[cfg(any(feature = "mens-train", feature = "mens-cloud"))]
pub mod preset_schema;
#[cfg(feature = "mens-train")]
pub mod train_backend;
#[cfg(feature = "mens-train")]
pub mod train_jsonl_preflight;
#[cfg(feature = "mens-train")]
pub mod training_config;

// Private backend dispatch; anchor for unwired-module scans.
#[cfg(feature = "mens-train")]
#[allow(unused_imports)]
use self::backend_candle_qlora as _;

#[cfg(feature = "mens-train")]
pub use execution_planner::{ExecutionPlan, ExecutionPlanner};
#[cfg(feature = "mens-train")]
pub use finetune_contract::FineTuneContract;
#[cfg(feature = "mens-train")]
pub use lora_train::run_mens_training;
#[cfg(feature = "mens-train")]
pub use preflight_train::{
    TrainingPreflightRecord, preflight_for_contract, write_training_preflight_json,
};
#[cfg(feature = "mens-train")]
pub use preset_schema::{
    CliOverrides, DEFAULT_PRESET, DatasetProfile, DeviceProfile, KNOWN_PRESETS, TrainPresetProfile,
    TrainPresetRegistry, load_registry, resolve_effective_profile,
};
#[cfg(feature = "mens-train")]
pub use train_backend::{ExecutionKernel, PopuliTrainBackend};
#[cfg(feature = "mens-train")]
pub use training_config::{
    LoraTrainingConfig, MensTokenizerMode, OptimizerExperimentMode, TrainingDeploymentTarget,
};
