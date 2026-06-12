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
