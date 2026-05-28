//! Checkpoint resume logic.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use anyhow::Result;
use qlora_rs::training::QLoraTrainer;
use rand::seq::SliceRandom;
use std::path::Path;
use vox_tensor::data::TrainingPair;

use super::types::QloraTrainingResume;
use crate::{checkpoint_state::CheckpointState, config::LoraTrainingConfig, train_log};

pub fn apply_checkpoint_resume(
    trainer: &mut QLoraTrainer,
    config: &LoraTrainingConfig,
    out: &Path,
    pairs_len: usize,
) -> Result<QloraTrainingResume> {
    let mut start_epoch = 1usize;
    let mut global_step = 0u32;
    let mut resume_pair_offset = 0usize;
    let mut resume_shuffled_indices: Option<Vec<usize>> = None;

    let checkpoint_root = config.resume_from.as_deref().unwrap_or(out);
    if !config.force_restart
        && let Some(ckpt) = CheckpointState::load(checkpoint_root)
    {
        train_log::info(&format!(
            "Checkpoint found in {} — resuming from epoch={} global_step={} pair_offset={}",
            checkpoint_root.display(),
            ckpt.epoch,
            ckpt.global_step,
            ckpt.pair_offset
        ));
        if std::path::Path::new(&ckpt.adapter_path).exists() {
            if let Err(err) = super::super::load_adapter_into_trainer(
                trainer,
                std::path::Path::new(&ckpt.adapter_path),
            ) {
                train_log::warn(&format!(
                    "Resume adapter load failed for {}: {err}",
                    ckpt.adapter_path
                ));
            }
        } else {
            train_log::warn(&format!(
                "Resume checkpoint references missing adapter {}; continuing with fresh adapter weights.",
                ckpt.adapter_path
            ));
        }
        start_epoch = ckpt.epoch as usize;
        global_step = ckpt.global_step;
        resume_pair_offset = ckpt.pair_offset;
        if ckpt.shuffled_indices.is_empty() {
            train_log::warn(
                "Resume checkpoint did not include shuffled_indices (epoch-boundary checkpoint); reshuffling for resume epoch.",
            );
            resume_shuffled_indices = None;
            resume_pair_offset = 0;
        } else {
            let (validated_indices, dropped_bad_indices) =
                sanitize_resume_indices(&ckpt.shuffled_indices, pairs_len);
            if dropped_bad_indices > 0 {
                train_log::warn(&format!(
                    "Resume checkpoint shuffled_indices dropped {} out-of-range/duplicate entries; reshuffling current epoch.",
                    dropped_bad_indices
                ));
                resume_shuffled_indices = None;
                resume_pair_offset = 0;
            } else if validated_indices.len() != pairs_len {
                train_log::warn(&format!(
                    "Resume checkpoint shuffled_indices length {} does not match current dataset size {}; reshuffling current epoch.",
                    validated_indices.len(),
                    pairs_len
                ));
                resume_shuffled_indices = None;
                resume_pair_offset = 0;
            } else {
                resume_shuffled_indices = Some(validated_indices);
            }
        }
    }

    Ok(QloraTrainingResume {
        start_epoch,
        global_step,
        resume_pair_offset,
        resume_shuffled_indices,
    })
}

pub fn build_epoch_shuffled_indices(
    epoch: usize,
    start_epoch: usize,
    pairs: &[TrainingPair],
    resume_shuffled_indices: &Option<Vec<usize>>,
    rng: &mut rand::rngs::StdRng,
    monotonic_difficulty: bool,
) -> Vec<usize> {
    if epoch == start_epoch
        && let Some(idx) = resume_shuffled_indices
        && !idx.is_empty()
    {
        return idx.clone();
    }
    let mut idx: Vec<usize> = (0..pairs.len()).collect();
    if monotonic_difficulty {
        idx.sort_by_key(|&v| pairs[v].difficulty.unwrap_or(5));
    } else {
        idx.shuffle(rng);
    }
    idx
}

pub fn sanitize_resume_indices(indices: &[usize], pair_count: usize) -> (Vec<usize>, usize) {
    if indices.is_empty() {
        return (Vec::new(), 0);
    }
    let mut seen = vec![false; pair_count];
    let mut out = Vec::with_capacity(indices.len());
    let mut dropped = 0usize;
    for &idx in indices {
        if idx >= pair_count || seen[idx] {
            dropped += 1;
            continue;
        }
        seen[idx] = true;
        out.push(idx);
    }
    (out, dropped)
}
