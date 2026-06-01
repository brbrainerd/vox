//! `vox mens` — the unified AI/ML command surface for Vox.
//!
//! All model training, serving, and corpus management lives here.
//! This is the canonical entry point; the deprecated top-level `vox train` remains
//! for Together / legacy native paths (see registry + `commands::ai::train`).
//!
//! ## Subcommands
//!
//! ```text
//! vox mens train       — Fine-tune: Candle QLoRA (default) or Burn LoRA (`--backend lora` deprecated)
//! vox mens serve      — Local: delegates to `vox-schola serve` (QLoRA run dir). Burn HTTP needs `execution-api`.
//! vox mens corpus     — Training data pipeline (extract, validate, mix, eval…)
//! vox mens probe      — Detect GPU capabilities and print recommended LoRA training config
//! vox mens status     — Show training run status from the latest telemetry log
//! vox mens watch-telemetry — Periodic tail of train.err.log + JSONL (alias: `watch`)
//! vox mens eval-local — Evaluate a trained model against the heldout benchmark set
//! Oratio speech-to-text lives at **`vox oratio`** (top-level), not under `mens`.
//! ```

/// Latency and throughput benchmarking for completions.
pub mod bench_completion;
#[cfg(feature = "gpu")]
pub mod eval_collateral;
pub(crate) mod eval_gate;
#[cfg(feature = "gpu")]
mod eval_local;
mod eval_local_prompt;
#[cfg(feature = "gpu")]
pub mod models;
#[cfg(feature = "mens-base")]
mod pipeline;
/// Self-healing for the `mens-candle-cuda` runtime plugin (rebuild + reinstall
/// on a stale/missing/ABI-mismatched plugin before CUDA training).
#[cfg(feature = "gpu")]
pub mod plugin_heal;

mod probe;
mod status;
mod system_prompt_template;
mod watch_telemetry;

mod populi;

#[cfg(feature = "gpu")]
pub use populi::{MensTokenizerCli, PopuliTrainBackendCli, TrainingDeploymentTargetCli};
pub use populi::{PipelineProgress, PipelineStage, PopuliAction, run};

#[cfg(all(test, feature = "gpu"))]
mod tests {
    include!("populi/gpu_tests_body.rs");
}
