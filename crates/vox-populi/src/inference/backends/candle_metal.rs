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

/// Metal inference backend backed by candle `QMatMul` quantized weights.
///
/// Falls back to CPU when Metal is unavailable (non-Apple platform or feature off).
pub struct CandleMetalBackend {
    loaded: Mutex<HashMap<String, Arc<LoadedState>>>,
    counter: AtomicU64,
}

impl Default for CandleMetalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CandleMetalBackend {
    pub fn new() -> Self {
        Self {
            loaded: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
        }
    }

    /// Load a local SP-1 quantized artifact directory into an in-memory model.
    ///
    /// Attempts to use Metal device 0; falls back to CPU with a warning when Metal is
    /// unavailable (non-Apple platform or feature off).
    pub fn load_from_dir(&self, dir: &std::path::Path) -> Result<LoadedModel, InferenceError> {
        let dev = Device::new_metal(0).unwrap_or_else(|_| {
            tracing::warn!("Metal unavailable, falling back to CPU for CandleMetalBackend");
            Device::Cpu
        });
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let label = format!("candle-metal-dir-{id}");
        candle_device::load_from_dir_on_device(
            dir,
            dev,
            BackendId::CandleMetal,
            label,
            &self.loaded,
        )
    }
}

#[async_trait]
impl InferenceBackend for CandleMetalBackend {
    fn id(&self) -> BackendId {
        BackendId::CandleMetal
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            cuda_tier: 0,
            metal_tier: 1,
            vram_gb: 0,
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
    /// [`CandleMetalBackend::load_from_dir`] for local artifacts.
    fn load(&self, _bundle: &ModelBundle) -> Result<LoadedModel, InferenceError> {
        Err(InferenceError::Unsupported(
            BackendId::CandleMetal,
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
        // generate() uses Device::Cpu only for input_ids tensor construction.
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

    fn build_dir() -> tempfile::TempDir {
        crate::inference::backends::candle_test_helpers::candle_model_build_dir()
    }

    #[tokio::test]
    async fn metal_backend_falls_back_to_cpu_and_predicts() {
        // Metal will be absent on non-Apple platforms; load_from_dir falls back to CPU.
        let backend = CandleMetalBackend::new();
        let dir = build_dir();
        let loaded = backend.load_from_dir(dir.path()).expect("load_from_dir");
        assert!(loaded.label.starts_with("candle-metal-dir-"));

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
        let backend = CandleMetalBackend::new();
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
