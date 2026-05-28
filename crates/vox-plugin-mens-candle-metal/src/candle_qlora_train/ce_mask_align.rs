//! Token-aligned supervised-prefix length for masked CE.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use anyhow::Result;
use tokenizers::Tokenizer;

/// Longest initial run where standalone `prefix_text` tokenization matches `full_text` tokenization.
pub(super) fn aligned_prefix_token_len(
    tokenizer: &Tokenizer,
    prefix_text: &str,
    full_text: &str,
) -> Result<usize> {
    let prefix_ids = tokenizer
        .encode(prefix_text, true)
        .map_err(|e| anyhow::anyhow!("tokenizer encode prefix: {e}"))?
        .get_ids()
        .to_vec();
    let full_ids = tokenizer
        .encode(full_text, true)
        .map_err(|e| anyhow::anyhow!("tokenizer encode full: {e}"))?
        .get_ids()
        .to_vec();

    let mut matched = 0usize;
    let upper = prefix_ids.len().min(full_ids.len());
    for i in 0..upper {
        if prefix_ids[i] == full_ids[i] {
            matched = i + 1;
        } else {
            break;
        }
    }

    if matched < prefix_ids.len() {
        tracing::warn!(
            target: "vox_mens_train",
            standalone_prefix_tokens = prefix_ids.len(),
            lcp_with_full_tokens = matched,
            "ChatML prefix tokenization diverged from full-text tokenization; CE mask uses longest matching initial token run"
        );
    }

    Ok(matched)
}

/// Align byte-level syntax spans (from AST) to token indices and weights.
pub(super) fn align_syntax_spans_to_tokens(
    encoding: &tokenizers::Encoding,
    spans: &[vox_tensor::data::SyntaxSpan],
    trunc_offset: usize,
) -> Vec<f32> {
    let mut weights = vec![1.0; encoding.len()];
    let offsets = encoding.get_offsets();

    if offsets.len() != weights.len() {
        return weights;
    }

    for span in spans {
        for (i, (start, end)) in offsets.iter().enumerate() {
            if *start < span.end && *end > span.start {
                weights[i] = weights[i].max(span.weight);
            }
        }
    }

    if trunc_offset > 0 {
        if trunc_offset < weights.len() {
            weights[trunc_offset..].to_vec()
        } else {
            vec![1.0]
        }
    } else {
        weights
    }
}
