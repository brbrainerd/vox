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

        // Retain only the most-recent N checkpoints to bound disk use. A single
        // long run otherwise accumulates one ~137 MB adapter per checkpoint and
        // can fill the disk (observed: ~70 files / ~13 GB → "no space" error).
        // The just-written checkpoint is the newest, so it is always retained
        // (and remains the one CheckpointState references for resume).
        let keep = std::env::var("VOX_MENS_KEEP_CHECKPOINTS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| *n >= 1)
            .unwrap_or(3);
        prune_old_checkpoints(out, keep);

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

/// Delete all but the `keep` most-recent `checkpoint_step_<N>.safetensors` files in
/// `out`. "Most recent" is by the numeric step `<N>`, so the highest-step (latest,
/// resume-referenced) checkpoint is always retained. Best-effort: I/O errors on
/// individual files are ignored so pruning never aborts training.
fn prune_old_checkpoints(out: &Path, keep: usize) {
    let keep = keep.max(1);
    let mut ckpts: Vec<(u64, std::path::PathBuf)> = match std::fs::read_dir(out) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter_map(|p| {
                let step = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_prefix("checkpoint_step_"))
                    .and_then(|n| n.strip_suffix(".safetensors"))
                    .and_then(|n| n.parse::<u64>().ok())?;
                Some((step, p))
            })
            .collect(),
        Err(_) => return,
    };
    if ckpts.len() <= keep {
        return;
    }
    ckpts.sort_by_key(|(step, _)| *step);
    let to_delete = ckpts.len() - keep;
    for (_, path) in ckpts.into_iter().take(to_delete) {
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::prune_old_checkpoints;

    #[test]
    fn keeps_newest_n_by_step() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        // 10, 20, ... 60 — note 100 sorts numerically (not lexically) after 60.
        for step in [10u64, 20, 30, 40, 50, 60, 100] {
            std::fs::write(p.join(format!("checkpoint_step_{step}.safetensors")), b"x").unwrap();
        }
        // Unrelated files must be untouched.
        std::fs::write(p.join("checkpoint_state.json"), b"{}").unwrap();
        std::fs::write(p.join("training_manifest.json"), b"{}").unwrap();

        prune_old_checkpoints(p, 3);

        let mut kept: Vec<String> = std::fs::read_dir(p)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("checkpoint_step_"))
            .collect();
        kept.sort();
        // The three highest steps (40, 50, 60, 100 → newest 3 = 50,60,100) retained.
        assert_eq!(
            kept,
            vec![
                "checkpoint_step_100.safetensors",
                "checkpoint_step_50.safetensors",
                "checkpoint_step_60.safetensors",
            ]
        );
        // Non-checkpoint files survive.
        assert!(p.join("checkpoint_state.json").exists());
        assert!(p.join("training_manifest.json").exists());
    }

    #[test]
    fn noop_when_under_limit() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        std::fs::write(p.join("checkpoint_step_5.safetensors"), b"x").unwrap();
        prune_old_checkpoints(p, 3);
        assert!(p.join("checkpoint_step_5.safetensors").exists());
    }
}
