//! Stub backends — real Candle / RPC wiring lands in follow-up tasks (Mn-T13, plugins).

use async_trait::async_trait;

use crate::inference::backend::{
    BackendCapabilities, BackendId, InferenceBackend, InferenceError, LoadedModel, PromptInput,
    Quantization, SamplingParams, Verdict,
};
use vox_package::ModelBundle;

macro_rules! stub_backend {
    ($name:ident, $id:expr, $label:literal) => {
        pub struct $name;

        #[async_trait]
        impl InferenceBackend for $name {
            fn id(&self) -> BackendId {
                $id
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
                    label: format!("{}-{}", $label, hex_prefix(&bundle.bundle_hash)),
                })
            }

            async fn predict(
                &self,
                _model: &LoadedModel,
                prompt: PromptInput,
                _sampling: SamplingParams,
            ) -> Result<String, InferenceError> {
                Ok(format!("[{} stub] {}", stringify!($name), prompt.text))
            }

            fn unload(&self, _model: LoadedModel) -> Result<(), InferenceError> {
                Ok(())
            }
        }
    };
}

fn hex_prefix(d: &[u8; 64]) -> String {
    d.iter().take(4).map(|b| format!("{b:02x}")).collect()
}

stub_backend!(CandleCudaStub, BackendId::CandleCuda, "candle-cuda");
stub_backend!(CandleMetalStub, BackendId::CandleMetal, "candle-metal");
stub_backend!(CandleCpuStub, BackendId::CandleCpu, "candle-cpu");
stub_backend!(LlamaCppRpcStub, BackendId::LlamaCppRpc, "llamacpp-rpc");
stub_backend!(OllamaSubprocessStub, BackendId::OllamaSubprocess, "ollama");
