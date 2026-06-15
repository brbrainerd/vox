//! Environment resolution for **inference providers** (local Mens/Ollama and cloud keys).
//!
//! This module is the **SSOT** for reading env vars used across CLI, MCP, and runtime. Callers that
//! need HTTP probes (health, model lists) use `vox_actor_runtime::inference_env::probe_populi_capabilities`.

use crate::snapshot::SnapshotCache;

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

static INFERENCE_PROFILE_CACHE: SnapshotCache<InferenceProfile> = SnapshotCache::new();

/// Read [`InferenceProfile`] from **`vox_populi::inference_PROFILE`** (case-insensitive).
#[must_use]
pub fn inference_profile_from_env() -> InferenceProfile {
    INFERENCE_PROFILE_CACHE.get_or_init(|| {
        let raw =
            crate::env_parse::resolve_config_str("vox_populi::inference_PROFILE", "desktop_ollama");
        let raw = raw.trim().to_ascii_lowercase();
        match raw.as_str() {
            "cloud_openai_compatible" | "cloud" => InferenceProfile::CloudOpenAiCompatible,
            "mobile_litert" | "litert" => InferenceProfile::MobileLitert,
            "mobile_coreml" | "coreml" => InferenceProfile::MobileCoreml,
            "lan_gateway" | "lan" => InferenceProfile::LanGateway,
            _ => InferenceProfile::DesktopOllama,
        }
    })
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

/// OpenRouter API base URL (config-aware: `OPENROUTER_BASE_URL` → config.toml → default).
///
/// Default `https://openrouter.ai/api`; the `/v1/...` suffixes are appended by the
/// endpoint accessors so that defaults match the legacy `OPENROUTER_*_URL` consts byte-for-byte.
static OPENROUTER_BASE_CACHE: SnapshotCache<String> = SnapshotCache::new();

#[must_use]
pub fn openrouter_base_url() -> String {
    OPENROUTER_BASE_CACHE.get_or_init(|| {
        let resolved = crate::env_parse::resolve_config_str(
            "OPENROUTER_BASE_URL",
            "https://openrouter.ai/api",
        );
        sanitize_base_url(
            &resolved,
            "https://openrouter.ai/api",
            "OPENROUTER_BASE_URL",
        )
    })
}

/// Validate a user-supplied base URL: trim trailing slashes and, if the result is
/// empty or has no `scheme://`, fall back to `default` (warning once).
fn sanitize_base_url(resolved: &str, default: &str, key: &str) -> String {
    let trimmed = resolved.trim_end_matches('/');
    if trimmed.is_empty() || !trimmed.contains("://") {
        static WARNED: std::sync::Once = std::sync::Once::new();
        WARNED.call_once(|| {
            tracing::warn!(
                config_key = key,
                value = resolved,
                "ignoring malformed base URL (empty or missing scheme); using default"
            );
        });
        return default.to_string();
    }
    trimmed.to_string()
}

/// OpenAI-compatible API base URL.
///
/// Precedence: `VOX_OPENAI_BASE_URL` → legacy `OPENAI_BASE_URL` → config.toml → default
/// `https://api.openai.com/v1`. Endpoint accessors append the path suffix.
static OPENAI_COMPAT_BASE_CACHE: SnapshotCache<String> = SnapshotCache::new();

#[must_use]
pub fn openai_compatible_base_url() -> String {
    OPENAI_COMPAT_BASE_CACHE.get_or_init(|| {
        let legacy =
            crate::env_parse::resolve_config_str("OPENAI_BASE_URL", "https://api.openai.com/v1");
        let resolved = crate::env_parse::resolve_config_str("VOX_OPENAI_BASE_URL", &legacy);
        sanitize_base_url(
            &resolved,
            "https://api.openai.com/v1",
            "VOX_OPENAI_BASE_URL",
        )
    })
}

/// OpenRouter chat completions endpoint (config-aware). Default equals
/// [`OPENROUTER_CHAT_COMPLETIONS_URL`].
#[must_use]
pub fn openrouter_chat_completions_url() -> String {
    format!("{}/v1/chat/completions", openrouter_base_url())
}

/// OpenRouter models list endpoint (config-aware). Default equals
/// [`OPENROUTER_MODELS_LIST_URL`].
#[must_use]
pub fn openrouter_models_list_url() -> String {
    format!("{}/v1/models", openrouter_base_url())
}

/// OpenRouter embeddings endpoint (config-aware). Default equals
/// [`OPENROUTER_EMBEDDINGS_URL`].
#[must_use]
pub fn openrouter_embeddings_url() -> String {
    format!("{}/v1/embeddings", openrouter_base_url())
}

/// OpenAI chat completions endpoint (config-aware). Default equals
/// [`OPENAI_CHAT_COMPLETIONS_URL`].
#[must_use]
pub fn openai_chat_completions_url() -> String {
    format!("{}/chat/completions", openai_compatible_base_url())
}

/// OpenAI embeddings endpoint (config-aware). Default equals
/// [`OPENAI_EMBEDDINGS_URL`].
#[must_use]
pub fn openai_embeddings_url() -> String {
    format!("{}/embeddings", openai_compatible_base_url())
}

/// Local Ollama-compatible API base URL.
///
/// Precedence: **`VOX_POPULI_LOCAL_OLLAMA_URL`** → **`POPULI_URL`** → **`OLLAMA_URL`** → `http://localhost:11434`.
static LOCAL_OLLAMA_BASE_CACHE: SnapshotCache<String> = SnapshotCache::new();

pub fn local_ollama_populi_base_url() -> String {
    LOCAL_OLLAMA_BASE_CACHE.get_or_init(|| {
        if let Some(secret) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxPopuliLocalOllamaUrl)
                .expose()
                .map(std::string::ToString::to_string)
        {
            return secret;
        }
        const UNSET: &str = "\u{0}__vox_unset__";
        let populi = crate::env_parse::resolve_config_str("POPULI_URL", UNSET);
        if populi != UNSET {
            return populi;
        }
        let ollama = crate::env_parse::resolve_config_str("OLLAMA_URL", UNSET);
        if ollama != UNSET {
            return ollama;
        }
        LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT.to_string()
    })
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
static HF_CHAT_MODEL_PREF_CACHE: SnapshotCache<Option<String>> = SnapshotCache::new();

pub fn hf_chat_model_preference() -> Option<String> {
    HF_CHAT_MODEL_PREF_CACHE
        .get_or_init(|| crate::secrets::secrets_str(vox_secrets::SecretId::VoxHfChatModel))
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
static HF_DEDICATED_CHAT_URL_CACHE: SnapshotCache<Option<String>> = SnapshotCache::new();

pub fn hf_dedicated_chat_completions_url() -> Option<String> {
    HF_DEDICATED_CHAT_URL_CACHE
        .get_or_init(|| crate::secrets::secrets_str(vox_secrets::SecretId::VoxHfDedicatedChatUrl))
}

/// Model id sent in the JSON body for [`hf_dedicated_chat_completions_url`] (`HF_DEDICATED_CHAT_MODEL`).
static HF_DEDICATED_CHAT_MODEL_CACHE: SnapshotCache<Option<String>> = SnapshotCache::new();

pub fn hf_dedicated_chat_model() -> Option<String> {
    HF_DEDICATED_CHAT_MODEL_CACHE
        .get_or_init(|| crate::secrets::secrets_str(vox_secrets::SecretId::VoxHfDedicatedChatModel))
}

/// Canonical HF Inference Providers router chat completions URL (override via secrets `VOX_HF_ROUTER_CHAT_COMPLETIONS_URL`).
static HF_ROUTER_URL_CACHE: SnapshotCache<String> = SnapshotCache::new();

#[must_use]
pub fn hf_router_chat_completions_url() -> String {
    HF_ROUTER_URL_CACHE.get_or_init(|| {
        crate::secrets::secrets_str(vox_secrets::SecretId::VoxHfRouterChatCompletionsUrl)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://router.huggingface.co/v1/chat/completions".to_string())
    })
}

/// Sanitize a string for ChatML formatting by replacing control tokens that could
/// trigger prompt injection (e.g., `<|im_start|>`, `<|im_end|>`).
#[must_use]
pub fn sanitize_chatml(input: &str) -> String {
    input
        .replace("<|im_start|>", "[im_start]")
        .replace("<|im_end|>", "[im_end]")
}

/// Resolve a tuning f32: secret first (request/env via `vox_secrets`), then `~/.vox/config.toml`
/// under the canonical env name, else `None`. Preserves the Optional semantics.
fn tuning_f32(secret: vox_secrets::SecretId, canonical_env: &str) -> Option<f32> {
    if let Some(v) = vox_secrets::resolve_secret(secret)
        .expose()
        .and_then(|s| s.parse::<f32>().ok())
    {
        return Some(v);
    }
    crate::env_parse::resolve_config_opt_f32(canonical_env)
}

/// Resolve a tuning i32: secret first, then `~/.vox/config.toml`, else `None`.
fn tuning_i32(secret: vox_secrets::SecretId, canonical_env: &str) -> Option<i32> {
    if let Some(v) = vox_secrets::resolve_secret(secret)
        .expose()
        .and_then(|s| s.parse::<i32>().ok())
    {
        return Some(v);
    }
    crate::env_parse::resolve_config_opt_i32(canonical_env)
}

static TOGETHER_TUNING_TEMP_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static TOGETHER_TUNING_TOP_P_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static GEMINI_TUNING_TEMP_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static GEMINI_TUNING_TOP_P_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static OLLAMA_TUNING_TEMP_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static OLLAMA_TUNING_TOP_P_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static OPENAI_TUNING_TEMP_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static OPENAI_TUNING_TOP_P_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static ANTHROPIC_TUNING_TEMP_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static ANTHROPIC_TUNING_TOP_P_CACHE: SnapshotCache<Option<f32>> = SnapshotCache::new();
static OLLAMA_TUNING_NUM_CTX_CACHE: SnapshotCache<Option<i32>> = SnapshotCache::new();

/// Temperature for Together AI inference.
pub fn together_tuning_temperature() -> Option<f32> {
    TOGETHER_TUNING_TEMP_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::TogetherTuningTemperature,
            "TOGETHER_TUNING_TEMPERATURE",
        )
    })
}

/// Top-P for Together AI inference.
pub fn together_tuning_top_p() -> Option<f32> {
    TOGETHER_TUNING_TOP_P_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::TogetherTuningTopP,
            "TOGETHER_TUNING_TOP_P",
        )
    })
}

/// Temperature for Gemini inference.
pub fn gemini_tuning_temperature() -> Option<f32> {
    GEMINI_TUNING_TEMP_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::GeminiTuningTemperature,
            "GEMINI_TUNING_TEMPERATURE",
        )
    })
}

/// Top-P for Gemini inference.
pub fn gemini_tuning_top_p() -> Option<f32> {
    GEMINI_TUNING_TOP_P_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::GeminiTuningTopP,
            "GEMINI_TUNING_TOP_P",
        )
    })
}

/// Temperature for Ollama inference.
pub fn ollama_tuning_temperature() -> Option<f32> {
    OLLAMA_TUNING_TEMP_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::OllamaTuningTemperature,
            "OLLAMA_TUNING_TEMPERATURE",
        )
    })
}

/// Top-P for Ollama inference.
pub fn ollama_tuning_top_p() -> Option<f32> {
    OLLAMA_TUNING_TOP_P_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::OllamaTuningTopP,
            "OLLAMA_TUNING_TOP_P",
        )
    })
}

/// Temperature for OpenAI inference.
pub fn openai_tuning_temperature() -> Option<f32> {
    OPENAI_TUNING_TEMP_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::OpenaiTuningTemperature,
            "OPENAI_TUNING_TEMPERATURE",
        )
    })
}

/// Top-P for OpenAI inference.
pub fn openai_tuning_top_p() -> Option<f32> {
    OPENAI_TUNING_TOP_P_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::OpenaiTuningTopP,
            "OPENAI_TUNING_TOP_P",
        )
    })
}

/// Temperature for Anthropic inference.
pub fn anthropic_tuning_temperature() -> Option<f32> {
    ANTHROPIC_TUNING_TEMP_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::AnthropicTuningTemperature,
            "ANTHROPIC_TUNING_TEMPERATURE",
        )
    })
}

/// Top-P for Anthropic inference.
pub fn anthropic_tuning_top_p() -> Option<f32> {
    ANTHROPIC_TUNING_TOP_P_CACHE.get_or_init(|| {
        tuning_f32(
            vox_secrets::SecretId::AnthropicTuningTopP,
            "ANTHROPIC_TUNING_TOP_P",
        )
    })
}

/// Context size for Ollama inference.
pub fn ollama_tuning_num_ctx() -> Option<i32> {
    OLLAMA_TUNING_NUM_CTX_CACHE.get_or_init(|| {
        tuning_i32(
            vox_secrets::SecretId::OllamaTuningNumCtx,
            "OLLAMA_TUNING_NUM_CTX",
        )
    })
}

#[cfg(test)]
#[allow(unsafe_code)] // serialized with TEST_ENV_LOCK
mod tests {
    use super::*;
    use crate::toml_config::test_support::{CONFIG_TEST_LOCK as TEST_ENV_LOCK, HomeGuard};

    #[test]
    fn local_base_prefers_populi_then_ollama() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        unsafe {
            std::env::remove_var("POPULI_URL");
            std::env::remove_var("OLLAMA_URL");
        }
        crate::snapshot::bump(&["POPULI_URL", "OLLAMA_URL"]);
        assert_eq!(
            local_ollama_populi_base_url(),
            LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT
        );

        unsafe {
            std::env::set_var("OLLAMA_URL", "http://localhost:9999");
        }
        crate::snapshot::bump(&["OLLAMA_URL"]);
        assert_eq!(local_ollama_populi_base_url(), "http://localhost:9999");

        unsafe {
            std::env::set_var("POPULI_URL", LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT);
        }
        crate::snapshot::bump(&["POPULI_URL"]);
        assert_eq!(
            local_ollama_populi_base_url(),
            LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT
        );

        unsafe {
            std::env::remove_var("POPULI_URL");
            std::env::remove_var("OLLAMA_URL");
        }
        crate::snapshot::bump(&["POPULI_URL", "OLLAMA_URL"]);
    }

    #[test]
    fn endpoint_defaults_match_legacy_consts() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        for key in [
            "OPENROUTER_BASE_URL",
            "VOX_OPENAI_BASE_URL",
            "OPENAI_BASE_URL",
        ] {
            unsafe {
                std::env::remove_var(key);
            }
            let _ = crate::toml_config::unset_user_config_value(key);
        }
        crate::snapshot::bump(&[]);
        assert_eq!(
            openrouter_chat_completions_url(),
            OPENROUTER_CHAT_COMPLETIONS_URL
        );
        assert_eq!(openrouter_models_list_url(), OPENROUTER_MODELS_LIST_URL);
        assert_eq!(openrouter_embeddings_url(), OPENROUTER_EMBEDDINGS_URL);
        assert_eq!(openai_chat_completions_url(), OPENAI_CHAT_COMPLETIONS_URL);
        assert_eq!(openai_embeddings_url(), OPENAI_EMBEDDINGS_URL);
    }

    #[test]
    fn endpoint_honors_config_toml_base_url() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        unsafe {
            std::env::remove_var("OPENROUTER_BASE_URL");
        }
        crate::toml_config::set_user_config_value(
            "OPENROUTER_BASE_URL",
            "https://proxy.example/api",
        )
        .expect("set");
        crate::snapshot::bump(&["OPENROUTER_BASE_URL"]);
        assert_eq!(
            openrouter_chat_completions_url(),
            "https://proxy.example/api/v1/chat/completions"
        );
        let _ = crate::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
        crate::snapshot::bump(&["OPENROUTER_BASE_URL"]);
    }

    #[test]
    fn trailing_slash_base_yields_single_slash_join() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        unsafe {
            std::env::remove_var("OPENROUTER_BASE_URL");
            std::env::remove_var("VOX_OPENAI_BASE_URL");
            std::env::remove_var("OPENAI_BASE_URL");
        }
        crate::toml_config::set_user_config_value(
            "OPENROUTER_BASE_URL",
            "https://proxy.example/api/",
        )
        .expect("set");
        crate::toml_config::set_user_config_value(
            "VOX_OPENAI_BASE_URL",
            "https://proxy.example/openai/v1/",
        )
        .expect("set");
        crate::snapshot::bump(&["OPENROUTER_BASE_URL", "VOX_OPENAI_BASE_URL"]);
        assert_eq!(openrouter_base_url(), "https://proxy.example/api");
        assert_eq!(
            openrouter_chat_completions_url(),
            "https://proxy.example/api/v1/chat/completions"
        );
        assert_eq!(
            openai_compatible_base_url(),
            "https://proxy.example/openai/v1"
        );
        assert_eq!(
            openai_chat_completions_url(),
            "https://proxy.example/openai/v1/chat/completions"
        );
        let _ = crate::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
        let _ = crate::toml_config::unset_user_config_value("VOX_OPENAI_BASE_URL");
        crate::snapshot::bump(&["OPENROUTER_BASE_URL", "VOX_OPENAI_BASE_URL"]);
    }

    #[test]
    fn local_base_honors_config_toml_when_env_absent() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        unsafe {
            std::env::remove_var("POPULI_URL");
            std::env::remove_var("OLLAMA_URL");
            std::env::remove_var("VOX_POPULI_LOCAL_OLLAMA_URL");
        }
        let _ = crate::toml_config::unset_user_config_value("POPULI_URL");
        crate::toml_config::set_user_config_value("OLLAMA_URL", "http://cfg-host:1234")
            .expect("set");
        crate::snapshot::bump(&["OLLAMA_URL", "POPULI_URL"]);
        assert_eq!(local_ollama_populi_base_url(), "http://cfg-host:1234");
        let _ = crate::toml_config::unset_user_config_value("OLLAMA_URL");
        crate::snapshot::bump(&["OLLAMA_URL"]);
    }

    #[test]
    fn degenerate_base_url_falls_back_to_default() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        unsafe {
            std::env::remove_var("OPENROUTER_BASE_URL");
        }
        crate::toml_config::set_user_config_value("OPENROUTER_BASE_URL", "").expect("set");
        crate::snapshot::bump(&["OPENROUTER_BASE_URL"]);
        assert_eq!(openrouter_base_url(), "https://openrouter.ai/api");

        crate::toml_config::set_user_config_value("OPENROUTER_BASE_URL", "proxy/no-scheme")
            .expect("set");
        crate::snapshot::bump(&["OPENROUTER_BASE_URL"]);
        assert_eq!(openrouter_base_url(), "https://openrouter.ai/api");

        crate::toml_config::set_user_config_value("OPENROUTER_BASE_URL", "https://proxy/api")
            .expect("set");
        crate::snapshot::bump(&["OPENROUTER_BASE_URL"]);
        assert_eq!(openrouter_base_url(), "https://proxy/api");

        let _ = crate::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
        crate::snapshot::bump(&["OPENROUTER_BASE_URL"]);
    }

    #[test]
    fn tuning_honors_config_toml_when_secret_absent() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        unsafe {
            std::env::remove_var("OLLAMA_TUNING_NUM_CTX");
            std::env::remove_var("OLLAMA_TUNING_TEMPERATURE");
        }
        let _ = crate::toml_config::unset_user_config_value("OLLAMA_TUNING_NUM_CTX");
        let _ = crate::toml_config::unset_user_config_value("OLLAMA_TUNING_TEMPERATURE");
        crate::snapshot::bump(&["OLLAMA_TUNING_NUM_CTX", "OLLAMA_TUNING_TEMPERATURE"]);

        assert_eq!(ollama_tuning_num_ctx(), None);
        assert_eq!(ollama_tuning_temperature(), None);

        crate::toml_config::set_user_config_value("OLLAMA_TUNING_NUM_CTX", "8192").expect("set");
        crate::toml_config::set_user_config_value("OLLAMA_TUNING_TEMPERATURE", "0.3").expect("set");
        crate::snapshot::bump(&["OLLAMA_TUNING_NUM_CTX", "OLLAMA_TUNING_TEMPERATURE"]);
        assert_eq!(ollama_tuning_num_ctx(), Some(8192));
        assert_eq!(ollama_tuning_temperature(), Some(0.3));

        let _ = crate::toml_config::unset_user_config_value("OLLAMA_TUNING_NUM_CTX");
        let _ = crate::toml_config::unset_user_config_value("OLLAMA_TUNING_TEMPERATURE");
        crate::snapshot::bump(&["OLLAMA_TUNING_NUM_CTX", "OLLAMA_TUNING_TEMPERATURE"]);
    }

    // --- Cache invalidation tests (Phase 5.2 TDD) ---

    #[test]
    fn openrouter_base_url_re_reads_after_bump() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        let _ = crate::toml_config::unset_user_config_value("OPENROUTER_BASE_URL");
        // Phase 1: set env to "first", bump, read → get "first".
        unsafe {
            std::env::set_var("OPENROUTER_BASE_URL", "https://first.example/api");
        }
        crate::snapshot::bump(&["OPENROUTER_BASE_URL"]);
        let v1 = openrouter_base_url();
        assert_eq!(v1, "https://first.example/api");

        // Phase 2: change env to "second", bump, read → get "second" (cache invalidated).
        unsafe {
            std::env::set_var("OPENROUTER_BASE_URL", "https://second.example/api");
        }
        crate::snapshot::bump(&["OPENROUTER_BASE_URL"]);
        let v2 = openrouter_base_url();
        assert_eq!(v2, "https://second.example/api", "must re-read after bump");

        unsafe {
            std::env::remove_var("OPENROUTER_BASE_URL");
        }
        crate::snapshot::bump(&["OPENROUTER_BASE_URL"]);
    }

    #[test]
    fn openai_base_url_re_reads_after_bump() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        let _ = crate::toml_config::unset_user_config_value("VOX_OPENAI_BASE_URL");
        let _ = crate::toml_config::unset_user_config_value("OPENAI_BASE_URL");
        unsafe {
            std::env::set_var("VOX_OPENAI_BASE_URL", "https://openai-first.example/v1");
        }
        crate::snapshot::bump(&["VOX_OPENAI_BASE_URL"]);
        let v1 = openai_compatible_base_url();
        assert_eq!(v1, "https://openai-first.example/v1");

        unsafe {
            std::env::set_var("VOX_OPENAI_BASE_URL", "https://openai-second.example/v1");
        }
        crate::snapshot::bump(&["VOX_OPENAI_BASE_URL"]);
        let v2 = openai_compatible_base_url();
        assert_eq!(v2, "https://openai-second.example/v1", "re-read after bump");

        unsafe {
            std::env::remove_var("VOX_OPENAI_BASE_URL");
        }
        crate::snapshot::bump(&["VOX_OPENAI_BASE_URL"]);
    }

    #[test]
    fn tuning_re_reads_after_bump() {
        let _g = TEST_ENV_LOCK.lock().expect("env lock");
        let _home = HomeGuard::new();
        let _ = crate::toml_config::unset_user_config_value("GEMINI_TUNING_TEMPERATURE");
        unsafe {
            std::env::remove_var("GEMINI_TUNING_TEMPERATURE");
        }
        // Phase 1: no config set → None.
        crate::snapshot::bump(&["GEMINI_TUNING_TEMPERATURE"]);
        assert_eq!(gemini_tuning_temperature(), None);

        // Phase 2: set config, bump → Some(0.7).
        crate::toml_config::set_user_config_value("GEMINI_TUNING_TEMPERATURE", "0.7").expect("set");
        crate::snapshot::bump(&["GEMINI_TUNING_TEMPERATURE"]);
        assert_eq!(gemini_tuning_temperature(), Some(0.7), "re-read after bump");

        let _ = crate::toml_config::unset_user_config_value("GEMINI_TUNING_TEMPERATURE");
        crate::snapshot::bump(&["GEMINI_TUNING_TEMPERATURE"]);
    }
}
