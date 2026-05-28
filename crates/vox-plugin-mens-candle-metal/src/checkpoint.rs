//! Checkpoint save/load for the CandleModel.
//!
//! # SP3 stub
//!
//! The real implementation calls `QLoraTrainer::save_adapter(path)` from `qlora-rs`.
//! The `QLoraTrainer` is constructed inside `vox-populi`'s `run_candle_qlora_train`
//! and holds the VarMap with trainable LoRA parameters. The trainer is not owned by
//! `CandleModel` in the current extraction because:
//!
//! 1. `CandleModel::load_from_path` is stubbed (see model.rs) — no model is actually
//!    loaded yet from within this plugin.
//! 2. `QLoraTrainer` has a non-trivial lifecycle: it is constructed after
//!    `QLoraConfig::preset_all_bf16` and `trainer.init_optimizer()` which depend on
//!    the full `QLoraTrainingConfig` assembled from `LoraTrainingConfig`.
//!
//! Batch 3/4 plan:
//! - Extend `CandleModel` to hold both `Qwen35Model` and `QLoraTrainer`.
//! - Wire `save` to call `trainer.save_adapter(Path::new(dest))`.
//! - Wire `load_from_path` to reconstruct both from the model path + a sidecar
//!   `adapter.safetensors` for warm-start.
//!
//! Source reference: `crates/vox-populi/src/mens/tensor/candle_qlora_train/checkpoint_mid.rs`
//! and `finalize.rs` for the save patterns; `training_loop/checkpoint.rs` for resume logic.

use crate::model::CandleModel;

/// Save a LoRA adapter checkpoint to `dest`.
pub fn save(model: &CandleModel, dest: &str) -> anyhow::Result<()> {
    if let Some(trainer) = &model.trainer {
        trainer.save_adapter(std::path::Path::new(dest))?;
        Ok(())
    } else {
        anyhow::bail!("Cannot save checkpoint: model has no active trainer.")
    }
}
