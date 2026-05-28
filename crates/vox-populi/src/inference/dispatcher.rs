use std::sync::Arc;

use crate::inference::backend::{InferenceBackend, InferenceError, PromptInput, SamplingParams, Verdict};
use vox_package::ModelBundle;

/// Chooses the first backend that returns [`Verdict::Yes`] for `can_serve`.
pub struct InferenceDispatcher {
    backends: Vec<Arc<dyn InferenceBackend>>,
}

impl InferenceDispatcher {
    #[must_use]
    pub fn new(backends: Vec<Arc<dyn InferenceBackend>>) -> Self {
        Self { backends }
    }

    #[must_use]
    pub fn backends(&self) -> &[Arc<dyn InferenceBackend>] {
        &self.backends
    }

    /// Pick a backend for this bundle, load, predict, unload (best-effort).
    pub async fn predict_auto(
        &self,
        bundle: &ModelBundle,
        prompt: PromptInput,
        sampling: SamplingParams,
    ) -> Result<String, InferenceError> {
        let backend = self.pick(bundle)?;
        let loaded = backend.load(bundle)?;
        let out = backend.predict(&loaded, prompt, sampling).await;
        let _ = backend.unload(loaded);
        out
    }

    fn pick(&self, bundle: &ModelBundle) -> Result<&Arc<dyn InferenceBackend>, InferenceError> {
        for b in &self.backends {
            if matches!(b.can_serve(bundle), Verdict::Yes) {
                return Ok(b);
            }
        }
        Err(InferenceError::Internal(
            "no inference backend accepted this ModelBundle".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::stubs::CandleCpuStub;

    #[tokio::test]
    async fn auto_dispatch_hits_cpu_stub() {
        let d = InferenceDispatcher::new(vec![Arc::new(CandleCpuStub)]);
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
        let out = d
            .predict_auto(
                &bundle,
                PromptInput {
                    text: "hi".into(),
                    system: None,
                },
                SamplingParams {
                    temperature: 0.7,
                    top_p: 0.9,
                    max_tokens: Some(8),
                },
            )
            .await
            .unwrap();
        assert!(out.contains("stub"));
    }
}
