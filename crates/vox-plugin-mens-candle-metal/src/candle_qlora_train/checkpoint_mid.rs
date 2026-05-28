//! Mid-epoch checkpoint save + DB notification.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use std::path::Path;

use anyhow::{Context, Result};
use qlora_rs::training::QLoraTrainer;

use super::TrainingDbEvent;
use crate::{checkpoint_state::CheckpointState, config::LoraTrainingConfig};

#[allow(clippy::too_many_arguments)]
pub(super) fn maybe_save_mid_epoch_checkpoint(
    trainer: &mut QLoraTrainer,
    out: &Path,
    config: &LoraTrainingConfig,
    db_tx: &tokio::sync::mpsc::UnboundedSender<TrainingDbEvent>,
    run_id: &str,
    epoch: usize,
    global_step: u32,
    pair_loop_idx: usize,
    shuffled_indices: &[usize],
    last_loss_val: f32,
    run_start_inst: std::time::Instant,
) -> Result<()> {
    if let Some(every) = config.checkpoint_every
        && every > 0
        && (pair_loop_idx + 1).is_multiple_of(every)
    {
        let ckpt_path = out.join(format!("checkpoint_step_{global_step}.safetensors"));
        trainer
            .save_adapter(&ckpt_path)
            .context("save mid-epoch adapter")?;

        let state = CheckpointState {
            schema: crate::checkpoint_state::CHECKPOINT_SCHEMA.to_string(),
            run_id: run_id.to_string(),
            epoch: epoch as u32,
            global_step,
            pair_offset: pair_loop_idx + 1,
            shuffled_indices: shuffled_indices.to_vec(),
            rng_seed: config.seed,
            adapter_path: ckpt_path.display().to_string(),
            last_loss: last_loss_val,
            wall_seconds_elapsed: run_start_inst.elapsed().as_secs_f64(),
            saved_at_utc: CheckpointState::now_utc(),
        };
        state.save(out).context("save CheckpointState mid-epoch")?;

        let _ = db_tx.send(TrainingDbEvent::Checkpoint {
            run_id: run_id.to_string(),
            epoch: epoch as u32,
            global_step,
            last_loss: Some(last_loss_val),
            adapter_path: ckpt_path.display().to_string(),
        });
    }
    Ok(())
}
