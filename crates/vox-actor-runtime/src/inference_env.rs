//! Async probes and **OpenAI-compatible endpoint descriptors** for Hugging Face + local Mens/Ollama.
//!
//! Environment **keys and base URL precedence** live in [`vox_config::inference`]; this module adds
//! HTTP capability discovery and constants for the HF Inference Providers router.

use std::time::Duration;

pub use vox_config::inference::{
    huggingface_hub_token, local_ollama_populi_base_url, openrouter_api_key,
};

pub use vox_orchestrator_types::{HuggingFaceDedicatedEndpoint, HuggingFaceRouterEndpoint};
/// Canonical router URL; override via [`vox_config::inference::hf_router_chat_completions_url`].
pub const HF_ROUTER_CHAT_COMPLETIONS_URL: &str =
    "https://router.huggingface.co/v1/chat/completions";

/// Build a HF router chat endpoint for `model`, filling the token from env when present.
pub fn resolve_huggingface_router(model: impl Into<String>) -> HuggingFaceRouterEndpoint {
    let model = model.into();
    HuggingFaceRouterEndpoint {
        chat_completions_url: vox_config::inference::hf_router_chat_completions_url(),
        bearer_token: huggingface_hub_token(),
        model,
    }
}

/// Resolve a dedicated endpoint: uses the same HF token env vars as the router.
pub fn resolve_huggingface_dedicated(
    chat_completions_url: impl Into<String>,
    model: impl Into<String>,
) -> HuggingFaceDedicatedEndpoint {
    HuggingFaceDedicatedEndpoint {
        model: model.into(),
        chat_completions_url: chat_completions_url.into(),
        bearer_token: huggingface_hub_token(),
    }
}

/// Normalized row from the Hugging Face Hub [`/api/models`](https://huggingface.co/docs/hub/api) listing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HfHubTextGenModelBrief {
    /// Hub model id (`org/name`).
    pub id: String,
    /// Hub `pipeline_tag` when present (e.g. `text-generation`).
    pub pipeline_tag: Option<String>,
    /// Reported download count from the Hub listing, if available.
    pub downloads: Option<u64>,
}

/// Parse Hub `/api/models` JSON **array** into brief records (accepts `modelId` or `id`).
pub fn parse_hf_hub_models_array(json: &str) -> Result<Vec<HfHubTextGenModelBrief>, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid Hub models JSON: {e}"))?;
    let arr = v
        .as_array()
        .ok_or_else(|| "Hub models response must be a JSON array".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for row in arr {
        let Some(id) = row
            .get("modelId")
            .or_else(|| row.get("id"))
            .and_then(|x| x.as_str())
            .map(str::to_string)
        else {
            continue;
        };
        let pipeline_tag = row
            .get("pipeline_tag")
            .and_then(|x| x.as_str())
            .map(str::to_string);
        let downloads = row.get("downloads").and_then(|x| x.as_u64());
        out.push(HfHubTextGenModelBrief {
            id,
            pipeline_tag,
            downloads,
        });
    }
    if out.is_empty() && !arr.is_empty() {
        return Err("Hub models array contained no rows with modelId/id".to_string());
    }
    Ok(out)
}

/// Fetch public (or token-visible) **text-generation** models from the Hub, sorted by downloads.
pub async fn fetch_hf_hub_text_generation_models(
    limit: u32,
) -> Result<Vec<HfHubTextGenModelBrief>, String> {
    let limit = limit.clamp(1, 100);
    let client = vox_http_client::client_builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("reqwest client build failed: {e}"))?;
    let url = format!(
        "https://huggingface.co/api/models?pipeline_tag=text-generation&sort=downloads&direction=-1&limit={limit}"
    );
    let mut req = client.get(url);
    if let Some(ref t) = huggingface_hub_token() {
        req = req.bearer_auth(t);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let tail: String = body.chars().take(240).collect();
        return Err(format!("Hub models API {status}: {tail}"));
    }
    parse_hf_hub_models_array(&body)
}

/// Result of probing an Ollama-compatible server (Mens local lane).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PopuliCapabilitySnapshot {
    /// Ollama-compatible server base URL that was probed.
    pub base_url: String,
    /// Whether `/api/tags` returned HTTP success and parsed models.
    pub reachable: bool,
    /// Model names reported by `/api/tags`.
    pub model_names: Vec<String>,
    /// `Some(true)` if version metadata suggests GPU-capable runtime; `None` if unknown.
    pub gpu_capable: Option<bool>,
    /// Human-readable probe notes or error summary when unreachable.
    pub notes: String,
}

/// Query `/api/tags` and `/api/version` on an Ollama-compatible `base_url`.
pub async fn probe_populi_capabilities(base_url: &str) -> PopuliCapabilitySnapshot {
    let base = base_url.trim_end_matches('/');
    let client = match vox_http_client::client_builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return PopuliCapabilitySnapshot {
                base_url: base.to_string(),
                reachable: false,
                model_names: Vec::new(),
                gpu_capable: None,
                notes: format!("reqwest client build failed: {e}"),
            };
        }
    };

    let tags_url = format!("{base}/api/tags");
    let tags_resp = match client.get(&tags_url).send().await {
        Ok(r) => r,
        Err(e) => {
            return PopuliCapabilitySnapshot {
                base_url: base.to_string(),
                reachable: false,
                model_names: Vec::new(),
                gpu_capable: None,
                notes: e.to_string(),
            };
        }
    };

    if !tags_resp.status().is_success() {
        let status = tags_resp.status();
        let body = tags_resp
            .text()
            .await
            .unwrap_or_else(|_| String::from("<body read error>"));
        return PopuliCapabilitySnapshot {
            base_url: base.to_string(),
            reachable: false,
            model_names: Vec::new(),
            gpu_capable: None,
            notes: format!("GET /api/tags -> {status}: {body}"),
        };
    }

    let tags_text = match tags_resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return PopuliCapabilitySnapshot {
                base_url: base.to_string(),
                reachable: false,
                model_names: Vec::new(),
                gpu_capable: None,
                notes: format!("read /api/tags body: {e}"),
            };
        }
    };

    let model_names = parse_ollama_tags_models(&tags_text);
    let gpu_capable = probe_gpu_hint_version(&client, base).await;

    let snapshot = PopuliCapabilitySnapshot {
        base_url: base.to_string(),
        reachable: true,
        model_names,
        gpu_capable,
        notes: String::new(),
    };

    tracing::info!(
        target: "vox_orchestrator::model_route",
        base_url = %snapshot.base_url,
        reachable = snapshot.reachable,
        model_count = snapshot.model_names.len(),
        gpu_capable = ?snapshot.gpu_capable,
        event = "populi_capability_probe",
        "mens probe complete"
    );

    snapshot
}

fn parse_ollama_tags_models(json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = v.get("models").and_then(|m| m.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for m in arr {
        if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
            out.push(name.to_string());
        }
    }
    out
}

async fn probe_gpu_hint_version(client: &reqwest::Client, base: &str) -> Option<bool> {
    let url = format!("{base}/api/version");
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().await.ok()?.to_lowercase();
    if text.contains("cuda") || text.contains("rocm") || text.contains("metal") {
        return Some(true);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tags_extracts_names() {
        let j = r#"{"models":[{"name":"a"},{"name":"b"}]}"#;
        assert_eq!(
            parse_ollama_tags_models(j),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[tokio::test]
    async fn probe_unbound_port_unreachable() {
        let s = probe_populi_capabilities("http://127.0.0.1:1").await;
        assert!(!s.reachable);
        assert!(s.model_names.is_empty());
    }

    #[test]
    fn hf_router_endpoint_includes_default_url() {
        let e = resolve_huggingface_router("meta-llama/Llama-3.2-3B-Instruct");
        assert_eq!(
            e.chat_completions_url,
            HF_ROUTER_CHAT_COMPLETIONS_URL.to_string()
        );
        assert_eq!(e.model, "meta-llama/Llama-3.2-3B-Instruct");
    }

    #[test]
    fn parse_hf_hub_models_accepts_model_id() {
        let j = r#"[{"modelId":"a/b","pipeline_tag":"text-generation","downloads":42}]"#;
        let m = parse_hf_hub_models_array(j).expect("parse");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, "a/b");
        assert_eq!(m[0].downloads, Some(42));
    }
}
