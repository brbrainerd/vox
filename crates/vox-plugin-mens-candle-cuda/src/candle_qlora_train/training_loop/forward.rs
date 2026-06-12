//! `forward_masked_ce` — masked cross-entropy forward pass.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use super::types::MaskedCeForward;
use crate::config::LoraTrainingConfig;
use anyhow::Result;
use candle_core::Device;

pub fn forward_masked_ce(
    model: &crate::candle_qlora_train::TrainGraphModel,
    ids: &[u32],
    prefix_len: usize,
    trunc_offset: usize,
    sample_weight: f64,
    token_weights: Option<&[f32]>,
    config: &LoraTrainingConfig,
    device: &Device,
) -> Result<MaskedCeForward> {
    let ids_len = ids.len();
    if ids_len < 2 {
        return Ok(MaskedCeForward::NoSupervision);
    }
    let input_ids = candle_core::Tensor::new(&ids[..ids_len - 1], device)?.unsqueeze(0)?;
    let targets = candle_core::Tensor::new(&ids[1..], device)?.unsqueeze(0)?;

    // Activation/gradient checkpointing: when enabled, the forward severs the
    // autograd tape at segment boundaries so only ~1 segment's activations are
    // retained for backward (bounds the single-backward peak that OOMs 3B on
    // 16GB). The loop runs the segmented recompute-backward; here we just build
    // logits (tape over the last segment + head) and carry the segments out.
    if config.gradient_checkpointing {
        let n_segments = gradient_checkpoint_segments(config);
        let ckpt = model.forward_checkpointed(&input_ids, n_segments)?;
        let logits = ckpt.logits.flatten_to(1)?;
        let targets_flat = targets.flatten_all()?;
        return finalize_masked_ce(
            logits,
            targets_flat,
            ids_len,
            prefix_len,
            trunc_offset,
            sample_weight,
            token_weights,
            config,
            device,
            Some(ckpt.segments),
        );
    }

    let logits = model.forward(&input_ids)?;
    let logits = logits.flatten_to(1)?;
    let targets_flat = targets.flatten_all()?;
    finalize_masked_ce(
        logits,
        targets_flat,
        ids_len,
        prefix_len,
        trunc_offset,
        sample_weight,
        token_weights,
        config,
        device,
        None,
    )
}

/// Number of checkpoint segments to split the layer stack into.
///
/// More segments → lower peak VRAM but more recompute (forward run ~2x). Default
/// 4 is a good knee for a 36-layer 3B model on 16GB; overridable via
/// `VOX_MENS_GC_SEGMENTS` for tuning.
fn gradient_checkpoint_segments(_config: &LoraTrainingConfig) -> usize {
    std::env::var("VOX_MENS_GC_SEGMENTS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or(4)
}

/// Build the masked cross-entropy loss from `logits` (shape `[seq-1, vocab]`).
///
/// Shared by the eager and checkpointed forward paths so the loss numerics are
/// byte-identical regardless of checkpointing — the only difference is that the
/// checkpointed `logits` tape spans just the final segment, and `segments`
/// carries the boundary activations for the loop's recompute-backward.
#[allow(clippy::too_many_arguments)]
fn finalize_masked_ce(
    logits: candle_core::Tensor,
    targets_flat: candle_core::Tensor,
    ids_len: usize,
    prefix_len: usize,
    trunc_offset: usize,
    sample_weight: f64,
    token_weights: Option<&[f32]>,
    config: &LoraTrainingConfig,
    device: &Device,
    segments: Option<Vec<crate::model::CheckpointSegment>>,
) -> Result<MaskedCeForward> {
    let vocab_dim = logits.dim(1)?;
    let max_tgt = targets_flat.max(0)?.to_scalar::<u32>()? as usize;
    if max_tgt >= vocab_dim {
        anyhow::bail!(
            "forward_masked_ce: max target token id {max_tgt} >= logits vocab dimension {vocab_dim}"
        );
    }

    let prompt_len = prefix_len.saturating_sub(trunc_offset);
    let ce_last_k = if config.qlora_ce_last_k == 0 {
        ids_len
    } else {
        config.qlora_ce_last_k
    };
    let last_k_start = ids_len.saturating_sub(ce_last_k);

    let mut mask_vec: Vec<f32> = (0..ids_len - 1)
        .map(|i| {
            let target_idx = i + 1;
            if target_idx >= prompt_len && target_idx >= last_k_start {
                1.0f32
            } else {
                0.0
            }
        })
        .collect();

    if let Some(tw) = token_weights {
        for (i, &w) in tw.iter().enumerate() {
            let target_idx = i + 1;
            if target_idx < ids_len && i < mask_vec.len() {
                mask_vec[i] *= w;
            }
        }
    }

    let mask = candle_core::Tensor::from_vec(mask_vec, ids_len - 1, device)?;

    let mask_sum = mask.sum_all()?.to_scalar::<f32>()?;
    if mask_sum <= 0.0 || !mask_sum.is_finite() {
        return Ok(MaskedCeForward::NoSupervision);
    }

    let log_sm = candle_nn::ops::log_softmax(&logits, 1)?;
    let logprobs = log_sm
        .gather(&targets_flat.unsqueeze(1)?, 1)?
        .flatten_all()?;
    let loss = (logprobs.broadcast_mul(&mask)?.sum_all()? / mask.sum_all()?)?;
    let w = -sample_weight as f32;
    let w_t = candle_core::Tensor::new(&[w], device)?;
    let loss = loss.broadcast_mul(&w_t)?;

    let loss_scalar = match loss.rank() {
        0 => loss.to_scalar::<f32>()?,
        1 if loss.dim(0)? == 1 => loss.squeeze(0)?.to_scalar::<f32>()?,
        r => {
            anyhow::bail!("unexpected loss rank: expected scalar or [1], got rank={r}")
        }
    };
    if !loss_scalar.is_finite() {
        let kind = if loss_scalar.is_nan() { "nan" } else { "inf" };
        return Ok(MaskedCeForward::NonFinite { kind, mask_sum });
    }

    Ok(MaskedCeForward::Finite {
        loss,
        loss_scalar,
        supervised_tokens: mask_sum.max(0.0) as u64,
        theoretical_tokens: (ids_len.saturating_sub(1)) as u64,
        syntax_weight_sum: mask_sum,
        segments,
    })
}
