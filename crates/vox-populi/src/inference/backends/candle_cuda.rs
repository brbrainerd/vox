use crate::inference::backend::{
    BackendCapabilities, BackendId, InferenceBackend, InferenceError, LoadedModel, PromptInput,
    Quantization, SamplingParams, Verdict,
};
use crate::inference::backends::candle_device::{self, LoadedState};
use crate::inference::generate::{GenConfig, generate};
use async_trait::async_trait;
use candle_core::Device;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use vox_package::ModelBundle;

/// CUDA inference backend backed by candle `QMatMul` quantized weights.
///
/// Falls back to CPU when CUDA is unavailable (feature off or no device present).
pub struct CandleCudaBackend {
    loaded: Mutex<HashMap<String, Arc<LoadedState>>>,
    counter: AtomicU64,
}

impl Default for CandleCudaBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CandleCudaBackend {
    pub fn new() -> Self {
        Self {
            loaded: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
        }
    }

    /// Load a local SP-1 quantized artifact directory into an in-memory model.
    ///
    /// Attempts to use CUDA device 0; falls back to CPU with a warning when CUDA is
    /// unavailable (feature off or no device present).
    pub fn load_from_dir(&self, dir: &std::path::Path) -> Result<LoadedModel, InferenceError> {
        let dev = Device::new_cuda(0).unwrap_or_else(|_| {
            tracing::warn!("CUDA unavailable, falling back to CPU for CandleCudaBackend");
            Device::Cpu
        });
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let label = format!("candle-cuda-dir-{id}");
        candle_device::load_from_dir_on_device(dir, dev, BackendId::CandleCuda, label, &self.loaded)
    }
}

#[async_trait]
impl InferenceBackend for CandleCudaBackend {
    fn id(&self) -> BackendId {
        BackendId::CandleCuda
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            cuda_tier: 1,
            metal_tier: 0,
            vram_gb: 0, // TODO: query from vox-plugin-nvml-probe when available (Mn-T3)
            max_context_len: 32768,
            streaming: false,
            quantizations: vec![
                Quantization::Q4K,
                Quantization::Q5K,
                Quantization::Q6K,
                Quantization::Q8Zero,
            ],
        }
    }

    fn can_serve(&self, bundle: &ModelBundle) -> Verdict {
        if bundle.verify_bundle_hash() {
            Verdict::Yes
        } else {
            Verdict::No {
                reason: "bundle_hash mismatch".into(),
            }
        }
    }

    /// `ModelBundle` CAS resolution not wired yet (Mn-T3). Use
    /// [`CandleCudaBackend::load_from_dir`] for local artifacts.
    fn load(&self, _bundle: &ModelBundle) -> Result<LoadedModel, InferenceError> {
        Err(InferenceError::Unsupported(
            BackendId::CandleCuda,
            "ModelBundle CAS resolution not wired (Mn-T3); use load_from_dir".into(),
        ))
    }

    async fn predict(
        &self,
        model: &LoadedModel,
        prompt: PromptInput,
        sampling: SamplingParams,
    ) -> Result<String, InferenceError> {
        let state = {
            let map = self.loaded.lock().expect("loaded map poisoned");
            map.get(&model.label)
                .cloned()
                .ok_or_else(|| InferenceError::Internal("model not loaded".into()))?
        };

        // Prepend system prompt if present.
        let text = match &prompt.system {
            Some(sys) if !sys.is_empty() => format!("{sys}\n{}", prompt.text),
            _ => prompt.text.clone(),
        };

        let encoding = state
            .tokenizer
            .encode(text, true)
            .map_err(|e| InferenceError::Internal(format!("tokenizer encode: {e}")))?;
        let prompt_ids: Vec<u32> = encoding.get_ids().to_vec();
        if prompt_ids.is_empty() {
            return Err(InferenceError::Internal(
                "tokenizer produced no tokens for prompt".into(),
            ));
        }

        let cfg = GenConfig {
            max_new_tokens: sampling.max_tokens.unwrap_or(256) as usize,
            temperature: sampling.temperature as f64,
            top_p: sampling.top_p as f64,
            eos_token_id: state.eos,
        };

        // The device is already baked into the QwenForward weights at load time.
        // generate() uses Device::Cpu only for input_ids tensor construction; the
        // forward pass itself runs on whatever device the weights live on.
        let dev = Device::Cpu;
        let new_ids = {
            let mut fwd = state.forward.lock().expect("forward poisoned");
            generate(&mut fwd, &prompt_ids, &cfg, &dev)
                .map_err(|e| InferenceError::Internal(format!("generate: {e}")))?
        };

        let decoded = state
            .tokenizer
            .decode(&new_ids, true)
            .map_err(|e| InferenceError::Internal(format!("tokenizer decode: {e}")))?;
        Ok(decoded)
    }

    fn unload(&self, model: LoadedModel) -> Result<(), InferenceError> {
        self.loaded
            .lock()
            .expect("loaded map poisoned")
            .remove(&model.label);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Tensor};

    fn rand2(out: usize, inp: usize, dev: &Device) -> Tensor {
        (Tensor::randn(0f32, 1f32, (out, inp), dev).unwrap() * 0.02).unwrap()
    }
    fn ones1(n: usize, dev: &Device) -> Tensor {
        Tensor::ones((n,), DType::F32, dev).unwrap()
    }

    fn build_dir() -> tempfile::TempDir {
        let dev = Device::Cpu;
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
            rand2(vocab, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.self_attn.q_proj.weight"),
            rand2(hidden, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.self_attn.k_proj.weight"),
            rand2(hidden, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.self_attn.v_proj.weight"),
            rand2(hidden, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.self_attn.o_proj.weight"),
            rand2(hidden, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.mlp.gate_proj.weight"),
            rand2(inter, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.mlp.up_proj.weight"),
            rand2(inter, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.mlp.down_proj.weight"),
            rand2(hidden, inter, &dev),
        );
        t.insert(format!("{p}.0.input_layernorm.weight"), ones1(hidden, &dev));
        t.insert(
            format!("{p}.0.post_attention_layernorm.weight"),
            ones1(hidden, &dev),
        );
        t.insert(
            "model.language_model.norm.weight".into(),
            ones1(hidden, &dev),
        );
        t.insert("lm_head.weight".into(), rand2(vocab, hidden, &dev));

        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        candle_core::safetensors::save(&t, indir.path().join("model.safetensors")).unwrap();
        std::fs::write(indir.path().join("config.json"), &cfg).unwrap();
        vox_quantize::quantize(&vox_quantize::QuantizeRequest {
            input_dir: indir.path().to_path_buf(),
            output_dir: outdir.path().to_path_buf(),
            mixture: vox_quantize::QuantMixture::Q4KM,
            verify: false,
            device: vox_quantize::DevicePref::Cpu,
        })
        .unwrap();

        let tok = r#"{
          "version": "1.0",
          "truncation": null,
          "padding": null,
          "added_tokens": [],
          "normalizer": null,
          "pre_tokenizer": { "type": "Whitespace" },
          "post_processor": null,
          "decoder": null,
          "model": {
            "type": "WordLevel",
            "vocab": {
              "<|endoftext|>": 0,
              "[UNK]": 1,
              "hello": 2,
              "world": 3,
              "foo": 4,
              "bar": 5
            },
            "unk_token": "[UNK]"
          }
        }"#;
        std::fs::write(outdir.path().join("tokenizer.json"), tok).unwrap();
        outdir
    }

    #[tokio::test]
    async fn cuda_backend_falls_back_to_cpu_and_predicts() {
        // CUDA device will be absent in CI; load_from_dir falls back to CPU automatically.
        let backend = CandleCudaBackend::new();
        let dir = build_dir();
        let loaded = backend.load_from_dir(dir.path()).expect("load_from_dir");
        assert!(loaded.label.starts_with("candle-cuda-dir-"));

        let out = backend
            .predict(
                &loaded,
                PromptInput {
                    text: "hello".into(),
                    system: None,
                },
                SamplingParams {
                    temperature: 0.0,
                    top_p: 1.0,
                    max_tokens: Some(3),
                },
            )
            .await
            .expect("predict");
        assert!(!out.contains("stub"));
    }

    #[test]
    fn bundle_load_is_unsupported_cas_gap() {
        let backend = CandleCudaBackend::new();
        let mut bundle = ModelBundle {
            weights_hash: [1u8; 64],
            weights_merkle_leaves: None,
            tokenizer_hash: [2u8; 64],
            config_hash: [3u8; 64],
            bundle_hash: [0u8; 64],
            format: vox_package::WeightFormat::SafeTensorsSingle,
            provenance: vox_package::BundleProvenance {
                source_label: "test".into(),
                hf_repo: None,
            },
        };
        bundle.bundle_hash = vox_package::compute_model_bundle_content_hash(&bundle);
        match backend.load(&bundle) {
            Err(InferenceError::Unsupported(_, msg)) => assert!(msg.contains("CAS")),
            other => panic!("expected Unsupported CAS error, got {other:?}"),
        }
    }
}
