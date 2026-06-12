//! Autoregressive token-generation loop over [`QwenForward`] (SP-2 final piece).
//!
//! `QwenForward` is **prefill-only** (no KV cache), so this loop re-feeds the full
//! running sequence on every step. That is O(seq^2) work per generated token and is a
//! known performance limitation — acceptable for the small CPU models this crate
//! currently targets, to be replaced by an incremental KV-cache decode later (Mn-T2).
//!
//! Sampling note: [`crate::SamplingParams`] carries no RNG seed, so the "sampling" path
//! here is **deterministic**: temperature/top-p shape the nucleus, then we pick the
//! argmax of the renormalized in-nucleus distribution. This makes generation
//! reproducible without pulling in an RNG crate. True stochastic sampling needs a seed
//! field added to `SamplingParams` first.

use candle_core::{DType, Device, Tensor};

use super::qwen_forward::{ForwardError, QwenForward};

/// Decode configuration for [`generate`].
#[derive(Debug, Clone)]
pub struct GenConfig {
    pub max_new_tokens: usize,
    /// `<= 0.0` selects greedy/argmax decoding (deterministic).
    pub temperature: f64,
    /// Nucleus (top-p) cutoff in `(0.0, 1.0]`; ignored when greedy.
    pub top_p: f64,
    /// Optional stop token; generation halts before appending it.
    pub eos_token_id: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error("forward: {0}")]
    Forward(#[from] ForwardError),
    #[error("candle: {0}")]
    Candle(#[from] candle_core::Error),
    #[error("empty prompt: at least one token id is required")]
    EmptyPrompt,
}

/// Greedy/deterministic autoregressive decode. Returns ONLY the newly generated token
/// ids (not the prompt). Stops at `eos_token_id` or after `max_new_tokens`.
pub fn generate(
    model: &mut QwenForward,
    prompt_ids: &[u32],
    cfg: &GenConfig,
    dev: &Device,
) -> Result<Vec<u32>, GenerateError> {
    if prompt_ids.is_empty() {
        return Err(GenerateError::EmptyPrompt);
    }

    let mut seq: Vec<u32> = prompt_ids.to_vec();
    let mut new_tokens: Vec<u32> = Vec::with_capacity(cfg.max_new_tokens);

    for _ in 0..cfg.max_new_tokens {
        let seq_len = seq.len();
        // Prefill-only: re-feed the entire running sequence (no KV cache — see module docs).
        let input = Tensor::from_vec(seq.clone(), (1, seq_len), dev)?;
        let logits = model.forward(&input, 0)?; // [1, seq, vocab]

        // Last-position logits: [vocab].
        let last = logits
            .narrow(1, seq_len - 1, 1)?
            .squeeze(1)?
            .squeeze(0)?
            .to_dtype(DType::F32)?;

        let next = if cfg.temperature <= 0.0 {
            argmax(&last)?
        } else {
            sample_nucleus(&last, cfg.temperature, cfg.top_p)?
        };

        if Some(next) == cfg.eos_token_id {
            break;
        }
        new_tokens.push(next);
        seq.push(next);
    }

    Ok(new_tokens)
}

/// Index of the maximum logit (greedy decode).
fn argmax(logits: &Tensor) -> Result<u32, GenerateError> {
    let v = logits.to_vec1::<f32>()?;
    let mut best = 0usize;
    let mut best_val = f32::NEG_INFINITY;
    for (i, &x) in v.iter().enumerate() {
        if x > best_val {
            best_val = x;
            best = i;
        }
    }
    Ok(best as u32)
}

/// Deterministic top-p reduction: apply temperature, softmax, keep the smallest set of
/// tokens whose cumulative probability reaches `top_p`, then pick the argmax inside that
/// nucleus. No RNG — see module docs for the seed caveat.
fn sample_nucleus(logits: &Tensor, temperature: f64, top_p: f64) -> Result<u32, GenerateError> {
    let raw = logits.to_vec1::<f32>()?;
    let temp = temperature.max(1e-6) as f32;

    // Softmax over temperature-scaled logits (numerically stable).
    let max = raw.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = raw.iter().map(|&x| ((x - max) / temp).exp()).collect();
    let sum: f32 = exps.iter().sum();
    let probs: Vec<f32> = exps.iter().map(|&e| e / sum).collect();

    // Sort token indices by descending probability.
    let mut order: Vec<usize> = (0..probs.len()).collect();
    order.sort_by(|&a, &b| probs[b].total_cmp(&probs[a]));

    // Walk the nucleus; the first token is the argmax and is always in-nucleus.
    let p_cut = top_p.clamp(0.0, 1.0) as f32;
    let mut cum = 0.0f32;
    let mut best = order[0];
    let mut best_prob = probs[order[0]];
    for &idx in &order {
        cum += probs[idx];
        if probs[idx] > best_prob {
            best_prob = probs[idx];
            best = idx;
        }
        if cum >= p_cut {
            break;
        }
    }
    Ok(best as u32)
}

#[cfg(test)]
mod tests {
    use super::qwen_weights::QwenWeights;
    use super::*;
    use candle_core::{DType, Tensor};
    use vox_hf_layout::HfTransformerLayout;

    // ── Fixture helpers (mirrors qwen_forward.rs tests) ─────────────────────────
    fn build_artifact(
        config_json: &str,
        tensors: &std::collections::HashMap<String, Tensor>,
    ) -> tempfile::TempDir {
        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        candle_core::safetensors::save(tensors, indir.path().join("model.safetensors")).unwrap();
        std::fs::write(indir.path().join("config.json"), config_json).unwrap();
        vox_quantize::quantize(&vox_quantize::QuantizeRequest {
            input_dir: indir.path().to_path_buf(),
            output_dir: outdir.path().to_path_buf(),
            mixture: vox_quantize::QuantMixture::Q4KM,
            verify: false,
            device: vox_quantize::DevicePref::Cpu,
        })
        .unwrap();
        outdir
    }

    fn rand2(out: usize, inp: usize, dev: &Device) -> Tensor {
        (Tensor::randn(0f32, 1f32, (out, inp), dev).unwrap() * 0.02).unwrap()
    }
    fn ones1(n: usize, dev: &Device) -> Tensor {
        Tensor::ones((n,), DType::F32, dev).unwrap()
    }

    /// Build a tiny full-attention quantized model on CPU.
    fn tiny_model(dev: &Device) -> (HfTransformerLayout, QwenForward, usize) {
        let hidden = 256usize;
        let heads = 8usize;
        let head_dim = hidden / heads;
        let inter = 256usize;
        let vocab = 512usize;
        let p = "model.language_model.layers";

        let cfg = format!(
            r#"{{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],
                "text_config":{{"hidden_size":{hidden},"num_attention_heads":{heads},
                "num_key_value_heads":{heads},"num_hidden_layers":1,"vocab_size":{vocab},
                "intermediate_size":{inter},"head_dim":{head_dim},
                "rope_parameters":{{"rope_theta":10000,"partial_rotary_factor":1.0}},
                "layer_types":["full_attention"]}}}}"#
        );

        let mut t = std::collections::HashMap::new();
        t.insert(
            "model.language_model.embed_tokens.weight".into(),
            rand2(vocab, hidden, dev),
        );
        t.insert(
            format!("{p}.0.self_attn.q_proj.weight"),
            rand2(hidden, hidden, dev),
        );
        t.insert(
            format!("{p}.0.self_attn.k_proj.weight"),
            rand2(hidden, hidden, dev),
        );
        t.insert(
            format!("{p}.0.self_attn.v_proj.weight"),
            rand2(hidden, hidden, dev),
        );
        t.insert(
            format!("{p}.0.self_attn.o_proj.weight"),
            rand2(hidden, hidden, dev),
        );
        t.insert(
            format!("{p}.0.mlp.gate_proj.weight"),
            rand2(inter, hidden, dev),
        );
        t.insert(
            format!("{p}.0.mlp.up_proj.weight"),
            rand2(inter, hidden, dev),
        );
        t.insert(
            format!("{p}.0.mlp.down_proj.weight"),
            rand2(hidden, inter, dev),
        );
        t.insert(format!("{p}.0.input_layernorm.weight"), ones1(hidden, dev));
        t.insert(
            format!("{p}.0.post_attention_layernorm.weight"),
            ones1(hidden, dev),
        );
        t.insert(
            "model.language_model.norm.weight".into(),
            ones1(hidden, dev),
        );
        t.insert("lm_head.weight".into(), rand2(vocab, hidden, dev));

        let outdir = build_artifact(&cfg, &t);
        let layout = HfTransformerLayout::from_config_json_str(&cfg).unwrap();
        let weights = QwenWeights::load(outdir.path(), dev).unwrap();
        let model = QwenForward::new(&layout, weights, dev).unwrap();
        (layout, model, vocab)
    }

    #[test]
    fn greedy_generate_is_bounded_and_deterministic() {
        let dev = Device::Cpu;
        let (_layout, mut model, vocab) = tiny_model(&dev);
        let cfg = GenConfig {
            max_new_tokens: 3,
            temperature: 0.0, // greedy → deterministic
            top_p: 1.0,
            eos_token_id: None,
        };
        let prompt = [1u32, 2u32];

        let out1 = generate(&mut model, &prompt, &cfg, &dev).unwrap();
        assert!(out1.len() <= 3, "must not exceed max_new_tokens");
        assert!(
            out1.iter().all(|&id| (id as usize) < vocab),
            "ids must be in vocab"
        );

        // Determinism: greedy decode on the same model across repeated calls is identical.
        let out2 = generate(&mut model, &prompt, &cfg, &dev).unwrap();
        assert_eq!(
            out1, out2,
            "greedy decode must be deterministic across runs"
        );
    }
}
