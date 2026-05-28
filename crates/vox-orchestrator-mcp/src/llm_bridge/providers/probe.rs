use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::metadata::ollama_base_url;
use crate::llm_bridge::error::HttpInferError;
use crate::llm_bridge::limits::{
    OLLAMA_PROBE_CACHE_TTL_SECS, OLLAMA_PROBE_TIMEOUT_SECS, VOX_LOCAL_PROBE_CACHE_TTL_SECS,
    VOX_LOCAL_PROBE_TIMEOUT_SECS,
};

fn vox_local_probe_ok_at() -> &'static Mutex<Option<Instant>> {
    static CACHE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Returns the base URL for the VoxLocal inference server (default: `http://127.0.0.1:7863`).
fn vox_local_base_url() -> String {
    std::env::var("VOX_LOCAL_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:7863".to_string())
}

/// Cheap `GET /health` probe so routing to VoxLocal fails fast with a clear message.
///
/// Successful probes are cached per-process for `VOX_LOCAL_PROBE_CACHE_TTL_SECS`.
pub(crate) async fn probe_vox_local_health(client: &reqwest::Client) -> Result<(), HttpInferError> {
    let ttl = Duration::from_secs(VOX_LOCAL_PROBE_CACHE_TTL_SECS);
    {
        let cache = vox_local_probe_ok_at()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(t0) = *cache {
            if t0.elapsed() < ttl {
                return Ok(());
            }
        }
    }

    let base = vox_local_base_url();
    let url = format!("{}/health", base.trim_end_matches('/'));
    let res = client
        .get(&url)
        .timeout(Duration::from_secs(VOX_LOCAL_PROBE_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| HttpInferError {
            status: 0,
            message: format!(
                "VoxLocal unreachable at {base} ({e}); run `python scripts/vox_populi::inference.py --serve` or set VOX_LOCAL_ENDPOINT."
            ),
            is_capability_gap: false,
        })?;
    let code = res.status().as_u16();
    if !res.status().is_success() {
        let t = res.text().await.unwrap_or_default();
        let err = HttpInferError {
            status: code,
            message: format!("VoxLocal /health error: {t}"),
            is_capability_gap: false,
        };
        let mut cache = vox_local_probe_ok_at()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *cache = None;
        return Err(err);
    }
    let mut cache = vox_local_probe_ok_at()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *cache = Some(Instant::now());
    Ok(())
}

fn ollama_probe_ok_at() -> &'static Mutex<Option<Instant>> {
    static CACHE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Cheap `GET /api/tags` probe so routing to Ollama fails fast with a clear message.
///
/// Successful probes are cached per-process for `OLLAMA_PROBE_CACHE_TTL_SECS`.
pub(crate) async fn probe_ollama_tags(client: &reqwest::Client) -> Result<(), HttpInferError> {
    let ttl = Duration::from_secs(OLLAMA_PROBE_CACHE_TTL_SECS);
    {
        let cache = ollama_probe_ok_at()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(t0) = *cache {
            if t0.elapsed() < ttl {
                return Ok(());
            }
        }
    }

    let base = ollama_base_url();
    let url = format!("{}/api/tags", base.trim_end_matches('/'));
    let res = client
        .get(&url)
        .timeout(Duration::from_secs(OLLAMA_PROBE_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| HttpInferError {
            status: 0,
            message: format!(
                "Ollama unreachable at {base} ({e}); set OLLAMA_HOST or start Ollama / Mens."
            ),
            is_capability_gap: false,
        })?;
    let code = res.status().as_u16();
    if !res.status().is_success() {
        let t = res.text().await.unwrap_or_default();
        let err = HttpInferError {
            status: code,
            message: format!("Ollama /api/tags error: {t}"),
            is_capability_gap: false,
        };
        let mut cache = ollama_probe_ok_at()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *cache = None;
        return Err(err);
    }
    let mut cache = ollama_probe_ok_at()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *cache = Some(Instant::now());
    Ok(())
}
