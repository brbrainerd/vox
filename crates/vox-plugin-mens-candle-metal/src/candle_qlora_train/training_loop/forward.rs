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

    let logits = model.forward(&input_ids)?;
    let logits = logits.flatten_to(1)?;
    let targets_flat = targets.flatten_all()?;
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
    })
}
