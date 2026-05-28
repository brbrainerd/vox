use crate::backend::{
    BackendCapabilities, BackendId, InferenceBackend, InferenceError, LoadedModel, PromptInput,
    Quantization, SamplingParams, Verdict,
};
use async_trait::async_trait;
use vox_package::ModelBundle;

pub struct CandleCpuBackend;

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
            quantizations: vec![Quantization::Q4K],
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

    fn load(&self, bundle: &ModelBundle) -> Result<LoadedModel, InferenceError> {
        Ok(LoadedModel {
            backend: self.id(),
            label: format!("candle-cpu-{}", hex_prefix(&bundle.bundle_hash)),
        })
    }

    async fn predict(
        &self,
        _model: &LoadedModel,
        prompt: PromptInput,
        _sampling: SamplingParams,
    ) -> Result<String, InferenceError> {
        Ok(format!("[candle-cpu stub] {}", prompt.text))
    }

    fn unload(&self, _model: LoadedModel) -> Result<(), InferenceError> {
        Ok(())
    }
}

fn hex_prefix(d: &[u8; 64]) -> String {
    d.iter().take(4).map(|b| format!("{b:02x}")).collect()
}
