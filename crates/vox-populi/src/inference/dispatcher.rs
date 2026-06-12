use std::sync::Arc;

use super::backend::{InferenceBackend, InferenceError, PromptInput, SamplingParams, Verdict};
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
    use super::backends::CandleCpuBackend;
    use super::*;

    // The CandleCpu backend picks this bundle (hash verifies) but `load(&ModelBundle)`
    // now returns the documented CAS-not-wired error — there is no resolver to map a
    // hash-only bundle to local artifact files yet (Mn-T3). Real inference goes through
    // `CandleCpuBackend::load_from_dir`. So `predict_auto` surfaces that error.
    #[tokio::test]
    async fn auto_dispatch_picks_cpu_but_bundle_load_unsupported() {
        let d = InferenceDispatcher::new(vec![Arc::new(CandleCpuBackend::new())]);
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
        let err = d
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
            .expect_err("ModelBundle load is unsupported until CAS lands");
        match err {
            InferenceError::Unsupported(_, msg) => {
                assert!(
                    msg.contains("CAS"),
                    "expected CAS-not-wired message, got: {msg}"
                );
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }
}
