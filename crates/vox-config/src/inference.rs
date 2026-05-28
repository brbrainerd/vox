//! Environment resolution for **inference providers** (local Mens/Ollama and cloud keys).
//!
//! This module is the **SSOT** for reading env vars used across CLI, MCP, and runtime. Callers that
//! need HTTP probes (health, model lists) use `vox_actor_runtime::inference_env::probe_populi_capabilities`.

/// Where chat / completion traffic is expected to run (desktop daemon vs cloud vs on-device).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InferenceProfile {
    /// Default: local Ollama-compatible HTTP (`OLLAMA_HOST` / `POPULI_URL` / localhost).
    #[default]
    DesktopOllama,
    /// OpenRouter / HF / other OpenAI-compatible cloud endpoints from config.
    CloudOpenAiCompatible,
    /// On-device LiteRT-LM (app-owned runtime).
    MobileLitert,
    /// Apple Core ML (app-owned).
    MobileCoreml,
    /// Ollama or compatible gateway on LAN (explicit base URL).
    LanGateway,
}

impl InferenceProfile {
    /// Whether tooling may probe and call **local** Ollama-compatible HTTP (loopback or `OLLAMA_HOST`).
    #[must_use]
    pub const fn allows_local_ollama_http(self) -> bool {
        matches!(self, Self::DesktopOllama | Self::LanGateway)
    }
}

/// Read [`InferenceProfile`] from **`vox_populi::inference_PROFILE`** (case-insensitive).
#[must_use]
pub fn inference_profile_from_env() -> InferenceProfile {
    let raw = std::env::var("vox_populi::inference_PROFILE")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase());
    match raw.as_deref() {
        Some("cloud_openai_compatible") | Some("cloud") => InferenceProfile::CloudOpenAiCompatible,
        Some("mobile_litert") | Some("litert") => InferenceProfile::MobileLitert,
        Some("mobile_coreml") | Some("coreml") => InferenceProfile::MobileCoreml,
        Some("lan_gateway") | Some("lan") => InferenceProfile::LanGateway,
        Some("desktop_ollama") | Some("ollama") | None => InferenceProfile::DesktopOllama,
        _ => InferenceProfile::DesktopOllama,
    }
}

/// Whether MCP / other HTTP clients may use **local** Ollama (`vox_populi::inference_PROFILE`).
#[must_use]
pub fn inference_profile_allows_local_ollama_http() -> bool {
    inference_profile_from_env().allows_local_ollama_http()
}

/// OpenRouter chat completions endpoint (OpenAI-compatible).
pub const OPENROUTER_CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
/// OpenRouter models list endpoint used for catalog discovery.
pub const OPENROUTER_MODELS_LIST_URL: &str = "https://openrouter.ai/api/v1/models";
/// OpenRouter embeddings endpoint (OpenAI-compatible).
pub const OPENROUTER_EMBEDDINGS_URL: &str = "https://openrouter.ai/api/v1/embeddings";
/// OpenAI chat completions endpoint.
pub const OPENAI_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";
/// OpenAI embeddings endpoint.
pub const OPENAI_EMBEDDINGS_URL: &str = "https://api.openai.com/v1/embeddings";
/// Local Ollama/Populi base URL fallback.
pub const LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT: &str = "http://localhost:11434";

/// Local Ollama-compatible API base URL.
///
/// Precedence: **`VOX_POPULI_LOCAL_OLLAMA_URL`** → **`POPULI_URL`** → **`OLLAMA_URL`** → `http://localhost:11434`.
pub fn local_ollama_populi_base_url() -> String {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxPopuliLocalOllamaUrl)
        .expose()
        .map(std::string::ToString::to_string)
        .or_else(|| std::env::var("POPULI_URL").ok())
        .or_else(|| std::env::var("OLLAMA_URL").ok())
        .unwrap_or_else(|| LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT.to_string())
}

/// Hugging Face Hub / Inference token for router and Hub APIs.
///
/// Precedence: **`HF_TOKEN`** → **`HUGGING_FACE_HUB_TOKEN`**.
pub fn huggingface_hub_token() -> Option<String> {
    vox_secrets::resolve_env_only(vox_secrets::SecretId::HuggingFaceToken)
        .expose()
        .map(std::string::ToString::to_string)
}

/// OpenRouter API key (`OPENROUTER_API_KEY`).
pub fn openrouter_api_key() -> Option<String> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey)
        .expose()
        .map(std::string::ToString::to_string)
}

/// Preferred Hugging Face **router** model id for chat when policy selects HF (`HF_CHAT_MODEL`).
pub fn hf_chat_model_preference() -> Option<String> {
    crate::secrets::secrets_str(vox_secrets::SecretId::VoxHfChatModel)
}

/// Preferred OpenRouter model id when policy selects OpenRouter (`OPENROUTER_CHAT_MODEL`).
///
/// Falls back to [`crate::bootstrap_inference::OPENROUTER_AUTO`] when unset.
pub fn openrouter_chat_model_preference() -> String {
    crate::routing_migration::trace_openrouter_chat_env_migration_once();
    let preferred = crate::secrets::secrets_str(vox_secrets::SecretId::VoxOpenRouterChatModel)
        .or_else(|| crate::secrets::secrets_str(vox_secrets::SecretId::OpenRouterGeminiModel));
    crate::routing_policy::resolve_openrouter_model(preferred)
}

/// OpenAI-compatible chat completions URL for a **pinned** Hugging Face Inference Endpoint
/// (`HF_DEDICATED_CHAT_URL`), when policy should prefer dedicated over the shared router.
pub fn hf_dedicated_chat_completions_url() -> Option<String> {
    crate::secrets::secrets_str(vox_secrets::SecretId::VoxHfDedicatedChatUrl)
}

/// Model id sent in the JSON body for [`hf_dedicated_chat_completions_url`] (`HF_DEDICATED_CHAT_MODEL`).
pub fn hf_dedicated_chat_model() -> Option<String> {
    crate::secrets::secrets_str(vox_secrets::SecretId::VoxHfDedicatedChatModel)
}

/// Canonical HF Inference Providers router chat completions URL (override via secrets `VOX_HF_ROUTER_CHAT_COMPLETIONS_URL`).
#[must_use]
pub fn hf_router_chat_completions_url() -> String {
    crate::secrets::secrets_str(vox_secrets::SecretId::VoxHfRouterChatCompletionsUrl)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://router.huggingface.co/v1/chat/completions".to_string())
}

/// Sanitize a string for ChatML formatting by replacing control tokens that could
/// trigger prompt injection (e.g., `<|im_start|>`, `<|im_end|>`).
#[must_use]
pub fn sanitize_chatml(input: &str) -> String {
    input
        .replace("<|im_start|>", "[im_start]")
        .replace("<|im_end|>", "[im_end]")
}

/// Temperature for Together AI inference.
pub fn together_tuning_temperature() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::TogetherTuningTemperature)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Top-P for Together AI inference.
pub fn together_tuning_top_p() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::TogetherTuningTopP)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Temperature for Gemini inference.
pub fn gemini_tuning_temperature() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::GeminiTuningTemperature)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Top-P for Gemini inference.
pub fn gemini_tuning_top_p() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::GeminiTuningTopP)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Temperature for Ollama inference.
pub fn ollama_tuning_temperature() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::OllamaTuningTemperature)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Top-P for Ollama inference.
pub fn ollama_tuning_top_p() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::OllamaTuningTopP)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Temperature for OpenAI inference.
pub fn openai_tuning_temperature() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::OpenaiTuningTemperature)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Top-P for OpenAI inference.
pub fn openai_tuning_top_p() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::OpenaiTuningTopP)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Temperature for Anthropic inference.
pub fn anthropic_tuning_temperature() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::AnthropicTuningTemperature)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Top-P for Anthropic inference.
pub fn anthropic_tuning_top_p() -> Option<f32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::AnthropicTuningTopP)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
}

/// Context size for Ollama inference.
pub fn ollama_tuning_num_ctx() -> Option<i32> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::OllamaTuningNumCtx)
        .expose()
        .and_then(|s| s.parse::<i32>().ok())
}

#[cfg(test)]
#[allow(unsafe_code)] // serialized with TEST_ENV_LOCK
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn local_base_prefers_populi_then_ollama() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        unsafe {
            std::env::remove_var("POPULI_URL");
            std::env::remove_var("OLLAMA_URL");
        }
        assert_eq!(
            local_ollama_populi_base_url(),
            LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT
        );

        unsafe {
            std::env::set_var("OLLAMA_URL", "http://localhost:9999");
        }
        assert_eq!(local_ollama_populi_base_url(), "http://localhost:9999");

        unsafe {
            std::env::set_var("POPULI_URL", LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT);
        }
        assert_eq!(
            local_ollama_populi_base_url(),
            LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT
        );

        unsafe {
            std::env::remove_var("POPULI_URL");
            std::env::remove_var("OLLAMA_URL");
        }
    }
}
