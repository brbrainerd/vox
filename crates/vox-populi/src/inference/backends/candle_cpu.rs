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

/// CPU inference backend backed by candle `QMatMul` quantized weights.
///
/// State is held in a label-keyed map because [`LoadedModel`] is opaque and cannot hold
/// the (non-`Clone`, mutating) `QwenForward`.
pub struct CandleCpuBackend {
    loaded: Mutex<HashMap<String, Arc<LoadedState>>>,
    counter: AtomicU64,
}

impl Default for CandleCpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CandleCpuBackend {
    pub fn new() -> Self {
        Self {
            loaded: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
        }
    }

    /// Load a local SP-1 quantized artifact directory (`config.json`, `model*.safetensors`,
    /// `tokenizer.json`) into an in-memory model and register it under a fresh label.
    ///
    /// This is the supported entry point for local inference. [`InferenceBackend::load`]
    /// (the `ModelBundle` path) intentionally errors until a content-addressed store
    /// resolver exists — see its doc comment.
    pub fn load_from_dir(&self, dir: &std::path::Path) -> Result<LoadedModel, InferenceError> {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let label = format!("candle-cpu-dir-{id}");
        candle_device::load_from_dir_on_device(
            dir,
            Device::Cpu,
            BackendId::CandleCpu,
            label,
            &self.loaded,
        )
    }
}

#[async_trait]
impl InferenceBackend for CandleCpuBackend {
    fn id(&self) -> BackendId {
        BackendId::CandleCpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            cuda_tier: 0,
            metal_tier: 0,
            vram_gb: 0,
            max_context_len: 4096,
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

    /// `ModelBundle` is hash-only and there is no content-addressed-store resolver wired
    /// yet (Mn-T3), so this backend cannot locate the artifact files from a bundle. This
    /// is an intentional, documented gap: use [`CandleCpuBackend::load_from_dir`] for
    /// local artifacts until the CAS lands.
    fn load(&self, _bundle: &ModelBundle) -> Result<LoadedModel, InferenceError> {
        Err(InferenceError::Unsupported(
            BackendId::CandleCpu,
            "ModelBundle CAS resolution not wired (Mn-T3); use load_from_dir for local artifacts"
                .into(),
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

    /// Write a tiny full-attention quantized artifact + a hand-rolled minimal WordLevel
    /// `tokenizer.json` into one directory and return it.
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

        // Minimal WordLevel tokenizer.json: whitespace pretokenizer + small vocab.
        // All ids are well under the model vocab (512).
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
    async fn load_from_dir_then_predict_returns_nonstub_text() {
        let backend = CandleCpuBackend::new();
        let dir = build_dir();
        let loaded = backend.load_from_dir(dir.path()).expect("load_from_dir");
        assert!(loaded.label.starts_with("candle-cpu-dir-"));
        // registered
        assert!(backend.loaded.lock().unwrap().contains_key(&loaded.label));

        let out = backend
            .predict(
                &loaded,
                PromptInput {
                    text: "hello world".into(),
                    system: None,
                },
                SamplingParams {
                    temperature: 0.0,
                    top_p: 1.0,
                    max_tokens: Some(4),
                },
            )
            .await
            .expect("predict");
        assert!(!out.contains("stub"), "stub string must be gone");
        // Determinism: same prompt + greedy → identical output.
        let out2 = backend
            .predict(
                &loaded,
                PromptInput {
                    text: "hello world".into(),
                    system: None,
                },
                SamplingParams {
                    temperature: 0.0,
                    top_p: 1.0,
                    max_tokens: Some(4),
                },
            )
            .await
            .expect("predict 2");
        assert_eq!(out, out2, "greedy predict must be deterministic");

        // unload removes the label.
        backend.unload(loaded.clone()).unwrap();
        assert!(!backend.loaded.lock().unwrap().contains_key(&loaded.label));
    }

    #[tokio::test]
    async fn predict_unknown_label_errors() {
        let backend = CandleCpuBackend::new();
        let bogus = LoadedModel {
            backend: BackendId::CandleCpu,
            label: "nope".into(),
        };
        let err = backend
            .predict(
                &bogus,
                PromptInput {
                    text: "hi".into(),
                    system: None,
                },
                SamplingParams {
                    temperature: 0.0,
                    top_p: 1.0,
                    max_tokens: Some(1),
                },
            )
            .await
            .expect_err("unknown label must error");
        match err {
            InferenceError::Internal(msg) => assert!(msg.contains("not loaded")),
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn bundle_load_is_unsupported_cas_gap() {
        let backend = CandleCpuBackend::new();
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
