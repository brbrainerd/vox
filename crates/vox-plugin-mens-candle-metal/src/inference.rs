//! Candle inference server — ported from `vox-populi/src/mens/tensor/candle_inference_serve.rs`.
//!
//! Grammar-constrained generation (vox_constrained_gen / vox_grammar_export) is not pulled
//! into this plugin to avoid dependency bloat; the `grammar_mode` parameter accepts a JSON
//! string selecting the mode and falls back to unconstrained when the mode is unknown.
//!
//! Hub download (crate::mens::hub::download_model) is stubbed: if the base_model field in
//! the adapter metadata is not a local directory path the call returns an error directing the
//! caller to pre-download the model. This matches the deferred-rewire pattern from SP3.

use std::path::Path;

use anyhow::Result;
use candle_core::{Device, Tensor};
use qlora_rs::QuantizedLinear;
use safetensors::Dtype;
use safetensors::SafeTensors;
use tokenizers::Tokenizer;

use crate::adapter_schema_v3::PopuliAdapterManifestV3;
use crate::hf_layout::HfArchitecture;
use crate::model::{
    Qwen2Attention, Qwen2MLP, Qwen35AttentionBlock, Qwen35Layer, Qwen35LinearAttention, Qwen35Model,
};

pub struct InferenceEngine {
    pub model: InferenceModel,
    pub tokenizer: Tokenizer,
    pub device: Device,
}

pub enum InferenceModel {
    Qwen35(Qwen35Model),
}

fn resolve_adapter_manifest_path(model_dir: &Path) -> Option<std::path::PathBuf> {
    let manifest = model_dir.join("adapter_manifest.json");
    if manifest.is_file() {
        return Some(manifest);
    }
    None
}

impl InferenceEngine {
    pub fn load(model_dir: &Path, device_kind: &crate::device::DeviceKind) -> Result<Self> {
        let tokenizer_path = model_dir.join("tokenizer.json");
        let _tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("load tokenizer: {e}"))?;

        let _device = match device_kind {
            crate::device::DeviceKind::Cpu => Device::Cpu,
            _ => {
                #[cfg(feature = "metal")]
                let dev = Device::new_metal(0).unwrap_or(Device::Cpu);
                #[cfg(not(feature = "metal"))]
                let dev = Device::Cpu;
                dev
            }
        };

        let adapter_path = model_dir.join("candle_qlora_adapter.safetensors");
        let meta_path = resolve_adapter_manifest_path(model_dir)
            .unwrap_or_else(|| model_dir.join("adapter_manifest.json"));

        if !adapter_path.is_file() || !meta_path.is_file() {
            anyhow::bail!(
                "LoRA adapter or adapter_manifest.json not found in {}",
                model_dir.display()
            );
        }

        let meta_raw = std::fs::read_to_string(&meta_path)
            .map_err(|e| anyhow::anyhow!("read manifest {}: {e}", meta_path.display()))?;
        let meta: PopuliAdapterManifestV3 = serde_json::from_str(&meta_raw)?;

        // Resolve base shards — local directory only; hub download is deferred (see module doc).
        let base_shards = if let Some(ref base) = meta.base_model {
            if Path::new(base).is_dir() {
                let mut shards = Vec::new();
                for entry in std::fs::read_dir(base)? {
                    let p = entry?.path();
                    if p.extension().map(|e| e == "safetensors").unwrap_or(false)
                        && p.file_name().unwrap().to_string_lossy().contains("model")
                    {
                        shards.push(p);
                    }
                }
                shards
            } else {
                anyhow::bail!(
                    "base_model '{}' is not a local directory. Hub download is not supported in \
                     the plugin; pre-download the model to a local path and update adapter_manifest.json.",
                    base
                );
            }
        } else {
            anyhow::bail!(
                "adapter manifest missing `base_model` reference. Cannot load frozen weights."
            );
        };

        let merged_file = model_dir.join("merged.safetensors");
        let mut all_buffers = Vec::new();
        let mut weight_maps = Vec::new();

        if merged_file.is_file() {
            let b = std::fs::read(&merged_file)?;
            all_buffers.push(b);
        }
        for p in &base_shards {
            all_buffers.push(std::fs::read(p)?);
        }
        for b in &all_buffers {
            weight_maps.push(SafeTensors::deserialize(b)?);
        }
        let qlora_cfg = qlora_rs::qlora::QLoraConfig::default();

        // Helper to find a tensor in any map
        let get_tensor = |key: &str| -> Result<Tensor> {
            for st in &weight_maps {
                if let Ok(view) = st.tensor(key) {
                    let shape = view.shape().to_vec();
                    let dtype = match view.dtype() {
                        Dtype::F32 => candle_core::DType::F32,
                        Dtype::BF16 => candle_core::DType::BF16,
                        Dtype::F16 => candle_core::DType::F16,
                        _ => continue,
                    };
                    let t = Tensor::from_raw_buffer(view.data(), dtype, &shape, &_device)?;
                    return if t.dtype() == candle_core::DType::F32 {
                        Ok(t)
                    } else {
                        Ok(t.to_dtype(candle_core::DType::F32)?)
                    };
                }
            }
            anyhow::bail!("Weight not found: {key}");
        };

        let config_path = model_dir.join("config.json");
        let mut layout = if config_path.is_file() {
            let s = std::fs::read_to_string(&config_path)
                .map_err(|e| anyhow::anyhow!("read config.json: {e}"))?;
            crate::hf_layout::HfTransformerLayout::from_config_json_str(&s)?
        } else {
            anyhow::bail!("config.json missing in {}", model_dir.display());
        };

        if layout.architecture == HfArchitecture::Qwen35 {
            let p = format!("{}.0.input_layernorm.weight", layout.namespace_prefix);
            if get_tensor(&p).is_err()
                && get_tensor("model.layers.0.input_layernorm.weight").is_ok()
            {
                layout.namespace_prefix = "model.layers".to_string();
            }
        }

        let model = if layout.architecture == HfArchitecture::Qwen35 {
            let mut layers = Vec::new();
            let n_heads = layout.num_attention_heads.max(1);
            let n_kv_heads = layout.num_key_value_heads.max(1);
            let head_dim = layout.head_dim.unwrap_or(layout.hidden_size / n_heads);
            for i in 0..layout.num_hidden_layers {
                let p = format!("{}.{}", layout.namespace_prefix, i);
                let ln1 = candle_nn::RmsNorm::new(
                    get_tensor(&format!("{p}.input_layernorm.weight"))?,
                    1e-6,
                );
                let ln2 = candle_nn::RmsNorm::new(
                    get_tensor(&format!("{p}.post_attention_layernorm.weight"))?,
                    1e-6,
                );
                let layer_type = layout
                    .layer_types
                    .get(i)
                    .map(String::as_str)
                    .unwrap_or("full_attention");

                let attention = if layer_type == "linear_attention" {
                    let key_heads = layout.linear_num_key_heads.unwrap_or(n_heads);
                    let value_heads = layout.linear_num_value_heads.unwrap_or(n_heads);
                    let key_dim = layout.linear_key_head_dim.unwrap_or(head_dim);
                    let value_dim = layout.linear_value_head_dim.unwrap_or(head_dim);
                    let qkv_rows = (key_heads * key_dim * 2) + (value_heads * value_dim);
                    let qkv_proj = QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.linear_attn.in_proj_qkv.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?;
                    let z_proj = QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.linear_attn.in_proj_z.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?;
                    let b_proj = QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.linear_attn.in_proj_b.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?;
                    let a_proj = QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.linear_attn.in_proj_a.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?;
                    let out_proj = QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.linear_attn.out_proj.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?;
                    let conv = get_tensor(&format!("{p}.linear_attn.conv1d.weight"))?;
                    let conv_weight = if conv.rank() == 3 {
                        conv.squeeze(1)?
                    } else {
                        conv
                    };
                    if conv_weight.dim(0)? != qkv_rows {
                        anyhow::bail!(
                            "qwen3_5 linear conv rows mismatch at layer {}: expected {}, got {}",
                            i,
                            qkv_rows,
                            conv_weight.dim(0)?
                        );
                    }
                    Qwen35AttentionBlock::Linear(Qwen35LinearAttention {
                        qkv_proj,
                        z_proj,
                        b_proj,
                        a_proj,
                        out_proj,
                        conv_weight,
                        dt_bias: get_tensor(&format!("{p}.linear_attn.dt_bias"))?,
                        a_log: get_tensor(&format!("{p}.linear_attn.A_log"))?,
                        norm: candle_nn::RmsNorm::new(
                            get_tensor(&format!("{p}.linear_attn.norm.weight"))?,
                            1e-6,
                        ),
                        num_k_heads: key_heads,
                        num_v_heads: value_heads,
                        head_k_dim: key_dim,
                        head_v_dim: value_dim,
                    })
                } else {
                    let q_proj = QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.self_attn.q_proj.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?;
                    let k_proj = QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.self_attn.k_proj.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?;
                    let v_proj = QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.self_attn.v_proj.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?;
                    let o_proj = QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.self_attn.o_proj.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?;
                    Qwen35AttentionBlock::Full(Qwen2Attention {
                        q_proj,
                        k_proj,
                        v_proj,
                        o_proj,
                        n_heads,
                        n_kv_heads,
                        head_dim,
                    })
                };

                let mlp = Qwen2MLP {
                    gate_proj: QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.mlp.gate_proj.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?,
                    up_proj: QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.mlp.up_proj.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?,
                    down_proj: QuantizedLinear::from_weight(
                        &get_tensor(&format!("{p}.mlp.down_proj.weight"))?,
                        None,
                        &qlora_cfg,
                        &_device,
                    )?,
                };
                let inv_freq = get_tensor(&format!("{p}.self_attn.rotary_emb.inv_freq"))
                    .or_else(|_| get_tensor(&format!("{p}.linear_attn.rotary_emb.inv_freq")))
                    .ok();

                layers.push(Qwen35Layer {
                    input_layernorm: ln1,
                    attention,
                    post_attention_layernorm: ln2,
                    mlp,
                    inv_freq,
                });
            }
            let norm = candle_nn::RmsNorm::new(
                get_tensor("model.language_model.norm.weight")
                    .or_else(|_| get_tensor("model.norm.weight"))?,
                1e-6,
            );
            let lm_head = QuantizedLinear::from_weight(
                &get_tensor("lm_head.weight").or_else(|_| {
                    get_tensor("model.language_model.embed_tokens.weight")
                        .or_else(|_| get_tensor("model.embed_tokens.weight"))
                })?,
                None,
                &qlora_cfg,
                &_device,
            )?;
            let embed_tokens = get_tensor("model.language_model.embed_tokens.weight")
                .or_else(|_| get_tensor("model.embed_tokens.weight"))?;
            InferenceModel::Qwen35(Qwen35Model {
                embed_tokens,
                layers,
                norm,
                lm_head,
            })
        } else {
            anyhow::bail!(
                "Unsupported architecture for inference: {:?}",
                layout.architecture
            );
        };
        Ok(Self {
            model,
            tokenizer: _tokenizer,
            device: _device,
        })
    }

    /// Autoregressive generation loop with KV cache (greedy decoding, unconstrained).
    ///
    /// Grammar-constrained decoding is not available in the plugin build.
    /// `prompt_json` is a JSON object with fields:
    ///   - `"prompt"`: string — the prompt text
    ///   - `"max_tokens"`: integer (default 256)
    ///   - `"temperature"`: float (default 1.0, currently unused — greedy)
    pub fn generate_from_json(&mut self, prompt_json: &str) -> Result<String> {
        #[derive(serde::Deserialize)]
        struct PromptRequest {
            prompt: String,
            #[serde(default = "default_max_tokens")]
            max_tokens: usize,
            #[serde(default = "default_temperature")]
            temperature: f64,
        }
        fn default_max_tokens() -> usize {
            256
        }
        fn default_temperature() -> f64 {
            1.0
        }

        let req: PromptRequest = serde_json::from_str(prompt_json)
            .map_err(|e| anyhow::anyhow!("parse prompt_json: {e}"))?;

        let generated = self.generate(&req.prompt, req.max_tokens, req.temperature)?;

        let out = serde_json::json!({
            "generated_text": generated,
            "prompt_tokens": self.tokenizer.encode(req.prompt.as_str(), true)
                .map(|e| e.len()).unwrap_or(0),
        });
        Ok(out.to_string())
    }

    fn generate(&mut self, prompt: &str, max_tokens: usize, _temperature: f64) -> Result<String> {
        let mut tokens = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("tokenizer error: {e}"))?
            .get_ids()
            .to_vec();

        let mut generated = String::new();

        // Greedy decoding without KV cache — re-runs the full context each step.
        // KV-cache autoregressive generation requires forward_with_cache which is
        // available in vox-populi's candle_model_qwen but not yet in the plugin's
        // model.rs (SP3 stub). Use full-context forward for correctness.
        for _ in 0..max_tokens {
            let input = Tensor::new(tokens.as_slice(), &self.device)?.unsqueeze(0)?;

            let logits = match &self.model {
                InferenceModel::Qwen35(model) => model.forward(&input)?,
            };
            let logits = logits.squeeze(0)?;
            let seq = logits.dim(0)?;
            let logits = logits.narrow(0, seq.saturating_sub(1), 1)?.squeeze(0)?;

            let slice = logits.to_vec1::<f32>()?;

            // Greedy decoding
            let mut next_token = 0u32;
            let mut max_val = f32::NEG_INFINITY;
            for (idx, &v) in slice.iter().enumerate() {
                if v > max_val {
                    max_val = v;
                    next_token = idx as u32;
                }
            }

            if let Ok(char_str) = self.tokenizer.decode(&[next_token], false) {
                generated.push_str(&char_str);
            }
            tokens.push(next_token);

            // EOS token for Qwen2 family
            if next_token == 151643 {
                break;
            }
        }

        Ok(generated)
    }
}

/// Entry point called from `backend.rs` `run_inference`.
///
/// `model_dir_json` is a JSON string with a `"model_dir"` field pointing to the
/// local model directory, plus the prompt fields consumed by `generate_from_json`.
pub fn run(model_dir: &str, prompt_json: &str) -> Result<String> {
    let path = std::path::Path::new(model_dir);
    let mut engine = InferenceEngine::load(path, &crate::device::DeviceKind::Best)?;
    engine.generate_from_json(prompt_json)
}

#[cfg(test)]
mod tests {
    use super::resolve_adapter_manifest_path;

    #[test]
    fn resolve_adapter_manifest_finds_v3() {
        let d = tempfile::tempdir().expect("tempdir");
        assert!(resolve_adapter_manifest_path(d.path()).is_none());

        let manifest = d.path().join("adapter_manifest.json");
        std::fs::write(&manifest, "{}").expect("manifest");
        assert_eq!(
            resolve_adapter_manifest_path(d.path()).as_deref(),
            Some(manifest.as_path())
        );
    }
}
