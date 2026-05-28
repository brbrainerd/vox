//! End-of-epoch summary, checkpoint, and telemetry.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use std::path::Path;

use anyhow::{Context, Result};
use qlora_rs::training::QLoraTrainer;

use super::TrainingDbEvent;
use crate::{checkpoint_state::CheckpointState, config::LoraTrainingConfig, telemetry, train_log};

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_epoch(
    trainer: &mut QLoraTrainer,
    out: &Path,
    config: &LoraTrainingConfig,
    db_tx: &tokio::sync::mpsc::UnboundedSender<TrainingDbEvent>,
    run_id: &str,
    epoch: usize,
    global_step: u32,
    epoch_steps: u32,
    epoch_loss_sum: f64,
    val_loss_sum: f64,
    val_steps: u32,
    last_loss_val: f32,
    progress_anchor_time: std::time::Instant,
) -> Result<()> {
    let avg_loss = if epoch_steps > 0 {
        epoch_loss_sum / epoch_steps as f64
    } else {
        0.0
    };
    let avg_val_loss = if val_steps > 0 {
        val_loss_sum / val_steps as f64
    } else {
        0.0
    };
    train_log::info(&format!(
        "Epoch {}/{} complete — avg_loss={:.4} val_loss={:.4} ({} steps, {} val steps)",
        epoch, config.epochs, avg_loss, avg_val_loss, epoch_steps, val_steps
    ));

    let epoch_ckpt = out.join(format!("checkpoint_epoch_{epoch}.safetensors"));
    trainer
        .save_adapter(&epoch_ckpt)
        .context("save epoch adapter")?;

    let epoch_state = CheckpointState {
        schema: crate::checkpoint_state::CHECKPOINT_SCHEMA.to_string(),
        run_id: run_id.to_string(),
        epoch: (epoch + 1) as u32,
        global_step,
        pair_offset: 0,
        shuffled_indices: vec![],
        rng_seed: config.seed,
        adapter_path: epoch_ckpt.display().to_string(),
        last_loss: last_loss_val,
        wall_seconds_elapsed: progress_anchor_time.elapsed().as_secs_f64(),
        saved_at_utc: CheckpointState::now_utc(),
    };
    epoch_state
        .save(out)
        .context("save CheckpointState at epoch boundary")?;

    let _ = db_tx.send(TrainingDbEvent::Checkpoint {
        run_id: run_id.to_string(),
        epoch: epoch as u32,
        global_step,
        last_loss: Some(last_loss_val),
        adapter_path: epoch_ckpt.display().to_string(),
    });
    let _ = db_tx.send(TrainingDbEvent::EpochSummary {
        run_id: run_id.to_string(),
        epoch: epoch as u32,
        global_step,
        avg_loss,
        avg_val_loss,
        val_steps,
    });

    telemetry::append(
        out,
        "epoch_complete",
        serde_json::json!({
            "epoch": epoch,
            "avg_loss": avg_loss,
            "val_loss": avg_val_loss,
            "steps": epoch_steps,
            "val_steps": val_steps,
            "global_step": global_step,
            "checkpoint": epoch_ckpt.display().to_string(),
        }),
    )?;
    Ok(())
}
