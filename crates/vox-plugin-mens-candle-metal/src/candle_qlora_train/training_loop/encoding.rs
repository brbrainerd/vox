//! Token encoding for training steps.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use anyhow::Result;
use tokenizers::Tokenizer;
use vox_tensor::data::TrainingPair;

use super::types::{EncodedTrainStep, TryEncodeOutcome};
use crate::config::LoraTrainingConfig;

pub fn try_encode_training_step(
    pair: &TrainingPair,
    system_prompt: &str,
    tokenizer: &Tokenizer,
    config: &LoraTrainingConfig,
    max_difficulty: u8,
) -> Result<TryEncodeOutcome> {
    if config.curriculum && pair.difficulty.unwrap_or(5) > max_difficulty {
        return Ok(TryEncodeOutcome::SkipCurriculum);
    }
    let text = if let Some(ref turns) = pair.messages {
        crate::training_text::chatml_turns_text(turns, &config.chatml)
    } else if let (Some(p), Some(r)) = (pair.effective_prompt(), pair.effective_response()) {
        crate::training_text::chatml_supervised_text(system_prompt, p, r, &config.chatml)
    } else {
        return Ok(TryEncodeOutcome::SkipShortSeq);
    };
    let prefix_text = if let Some(ref turns) = pair.messages {
        crate::training_text::chatml_turns_prefix_open_assistant(turns, &config.chatml)
    } else if let Some(p) = pair.effective_prompt() {
        crate::training_text::chatml_prefix_open_assistant(system_prompt, p, &config.chatml)
    } else {
        return Ok(TryEncodeOutcome::SkipShortSeq);
    };
    let prefix_len = crate::candle_qlora_train::ce_mask_align::aligned_prefix_token_len(
        tokenizer,
        prefix_text.as_str(),
        text.as_str(),
    )?;
    let enc = tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut ids = enc.get_ids().to_vec();
    let raw_token_len = ids.len();
    let mut trunc_offset = 0usize;
    if ids.len() > config.seq_len {
        trunc_offset = ids.len() - config.seq_len;
        ids = ids[trunc_offset..].to_vec();
    }
    if ids.len() < 2 {
        return Ok(TryEncodeOutcome::SkipShortSeq);
    }
    let (sample_weight, _) = super::logic::trajectory_weight_for_pair(pair, config);
    let token_weights = pair.syntax_spans.as_ref().map(|spans| {
        crate::candle_qlora_train::ce_mask_align::align_syntax_spans_to_tokens(
            &enc,
            spans,
            trunc_offset,
        )
    });

    Ok(TryEncodeOutcome::Encoded(EncodedTrainStep {
        raw_token_len,
        ids,
        prefix_len,
        trunc_offset,
        sample_weight,
        token_weights,
    }))
}

pub fn token_ids_in_model_vocab(ids: &[u32], vocab: usize) -> bool {
    if vocab == 0 {
        return false;
    }
    ids.iter().all(|&id| (id as usize) < vocab)
}
