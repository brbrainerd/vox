//! LLM settings bridge: the `[llm]` config SSOT (concurrency + retry) and a
//! presence check for the OpenRouter key.
//!
//! Reads/writes go through `VoxConfig` (which persists the `[llm]` table to
//! `~/.vox/config.toml`). The throttle in `vox-actor-runtime::llm::throttle`
//! reads these same values via `VoxConfig::load()`, so the GUI and the egress
//! path share one source of truth.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LlmConfigDto {
    pub max_concurrent_requests: usize,
    pub openrouter_max_concurrent: Option<usize>,
    pub openai_max_concurrent: Option<usize>,
    pub retry_max_attempts: u32,
}

#[tauri::command]
pub async fn get_llm_config() -> Result<LlmConfigDto, String> {
    let cfg = vox_config::VoxConfig::load();
    Ok(LlmConfigDto {
        max_concurrent_requests: cfg.llm_max_concurrent_requests,
        openrouter_max_concurrent: cfg.llm_openrouter_max_concurrent,
        openai_max_concurrent: cfg.llm_openai_max_concurrent,
        retry_max_attempts: cfg.llm_retry_max_attempts,
    })
}

#[tauri::command]
pub async fn set_llm_config(config: serde_json::Value) -> Result<(), String> {
    let mut cfg = vox_config::VoxConfig::load();
    if let Some(v) = config.get("maxConcurrentRequests").and_then(|v| v.as_u64()) {
        cfg.llm_max_concurrent_requests = (v as usize).clamp(1, 256);
    }
    if let Some(v) = config.get("openrouterMaxConcurrent") {
        cfg.llm_openrouter_max_concurrent = v.as_u64().map(|n| (n as usize).clamp(1, 256));
    }
    if let Some(v) = config.get("openaiMaxConcurrent") {
        cfg.llm_openai_max_concurrent = v.as_u64().map(|n| (n as usize).clamp(1, 256));
    }
    if let Some(v) = config.get("retryMaxAttempts").and_then(|v| v.as_u64()) {
        cfg.llm_retry_max_attempts = (v as u32).clamp(0, 10);
    }
    // Persists the [llm] table to ~/.vox/config.toml (merge-write).
    cfg.save().map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct OpenRouterKeyStatusDto {
    pub configured: bool,
}

/// Presence check for the OpenRouter key (does not call the network — the GUI
/// only needs to know whether a key is set so it can prompt the user otherwise).
#[tauri::command]
pub async fn openrouter_key_status() -> Result<OpenRouterKeyStatusDto, String> {
    let configured =
        vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey).is_present();
    Ok(OpenRouterKeyStatusDto { configured })
}

#[derive(Debug, Serialize)]
pub struct ProviderStatusDto {
    /// Debug-format provider name, e.g. "OpenRouter", "Ollama".
    pub provider: String,
    pub key_present: bool,
    pub is_local: bool,
    /// Some(reachable) from the cached local probe; None for cloud providers.
    pub local_reachable: Option<bool>,
    /// Model names the local server reported (empty for cloud providers).
    pub local_models: Vec<String>,
}

/// Per-backend availability (B9): credential presence for every candidate
/// provider + live local-server health from the shared TTL-cached probe.
#[tauri::command]
pub async fn inference_provider_status() -> Result<Vec<ProviderStatusDto>, String> {
    use vox_orchestrator::models::ProviderType;
    let statuses = vox_orchestrator::models::key_guard::inference_provider_statuses();
    let base = vox_config::inference::local_ollama_populi_base_url();
    let probe = vox_actor_runtime::inference_env::probe_populi_capabilities_cached(
        &base,
        std::time::Duration::from_secs(15),
    )
    .await;
    Ok(statuses
        .into_iter()
        .map(|(p, key_present)| {
            let is_local = matches!(
                p,
                ProviderType::Ollama | ProviderType::PopuliMesh | ProviderType::VoxLocal
            );
            ProviderStatusDto {
                provider: format!("{p:?}"),
                key_present,
                is_local,
                local_reachable: is_local.then_some(probe.reachable),
                local_models: if is_local {
                    probe.model_names.clone()
                } else {
                    Vec::new()
                },
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_status_dto_serializes_shape_frontend_expects() {
        let dto = ProviderStatusDto {
            provider: "Anthropic".into(),
            key_present: false,
            is_local: false,
            local_reachable: None,
            local_models: vec![],
        };
        let j = serde_json::to_value(&dto).expect("serialize");
        assert_eq!(j["provider"], "Anthropic");
        assert_eq!(j["key_present"], false);
        assert!(j["local_reachable"].is_null());
    }
}
