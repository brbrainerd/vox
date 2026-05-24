use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vox_package::ModelBundle;

/// Declared quantization families for routing — extend as catalog grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quantization {
    Fp16,
    Bf16,
    #[serde(rename = "q8_0")]
    Q8Zero,
    #[serde(rename = "q4_k")]
    Q4K,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendCapabilities {
    pub cuda_tier: u8,
    pub metal_tier: u8,
    pub vram_gb: u32,
    pub max_context_len: u32,
    pub streaming: bool,
    pub quantizations: Vec<Quantization>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendId {
    CandleCuda,
    CandleMetal,
    CandleCpu,
    LlamaCppRpc,
    OllamaSubprocess,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Yes,
    No { reason: String },
}

/// Opaque loaded-model handle — backends own internal state (Mn-T2).
#[derive(Debug, Clone)]
pub struct LoadedModel {
    pub backend: BackendId,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptInput {
    pub text: String,
    #[serde(default)]
    pub system: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingParams {
    pub temperature: f32,
    pub top_p: f32,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    #[error("backend {0:?}: {1}")]
    Unsupported(BackendId, String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("internal: {0}")]
    Internal(String),
}

#[async_trait]
pub trait InferenceBackend: Send + Sync {
    fn id(&self) -> BackendId;

    fn capabilities(&self) -> BackendCapabilities;

    fn can_serve(&self, bundle: &ModelBundle) -> Verdict;

    fn load(&self, bundle: &ModelBundle) -> Result<LoadedModel, InferenceError>;

    async fn predict(
        &self,
        model: &LoadedModel,
        prompt: PromptInput,
        sampling: SamplingParams,
    ) -> Result<String, InferenceError>;

    fn unload(&self, model: LoadedModel) -> Result<(), InferenceError>;
}
