//! MCP chat / inline-edit LLM routing: model resolution from the orchestrator registry and
//! HTTP calls (OpenRouter-compatible, Google Gemini `generateContent`, local Ollama).

use vox_orchestrator::types::AgentId;

mod error;
mod infer;
pub mod infer_test_stub;
mod limits;
mod model_route_policy;
mod provider_adapter;
mod provider_auth;
mod provider_endpoints;
mod providers;

/// Single agent id for MCP-hosted LLM usage accounting (not per-tool agents).
pub(crate) const MCP_GLOBAL_LLM_AGENT: AgentId = AgentId(0);

pub use infer::{McpInferRouting, call_llm, mcp_infer_completion};
pub use model_route_policy::{
    McpChatModelResolution, mcp_global_llm_context_fill_ratio, mcp_provider_telemetry_labels,
    resolve_mcp_chat_model, resolve_mcp_chat_model_sync,
};

/// Clamp requested max output tokens for HTTP chat APIs (OpenRouter / Gemini caps).
#[must_use]
pub fn clamp_http_max_output_tokens(n: u64) -> u64 {
    n.clamp(1, limits::HTTP_MAX_OUTPUT_TOKENS_CAP)
}

/// Result from the VoxLocal inference server.
#[derive(Debug)]
pub struct VoxLocalGenerateResult {
    /// Generated Vox source code.
    pub code: String,
    /// Whether the server validated the code as syntactically correct.
    pub valid: Option<bool>,
    /// Validation errors reported by the server, if any.
    pub errors: Vec<String>,
    /// Validation warnings reported by the server, if any.
    pub warnings: Vec<String>,
    /// Number of generation attempts made.
    pub attempts: u64,
}

/// Generate Vox code via the local MENS inference server.
///
/// Benefits over a raw HTTP call: the health probe result is TTL-cached (30 s),
/// and the endpoint is resolved from `VOX_LOCAL_ENDPOINT` (default 127.0.0.1:7863).
pub async fn vox_local_generate(
    client: &reqwest::Client,
    prompt: &str,
    validate: bool,
    max_retries: u32,
) -> Result<VoxLocalGenerateResult, String> {
    use error::HttpInferError;
    use providers::probe_vox_local_health;

    probe_vox_local_health(client)
        .await
        .map_err(|e: HttpInferError| e.message)?;

    let base =
        std::env::var("VOX_LOCAL_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:7863".to_string());
    let endpoint = format!("{}/generate", base.trim_end_matches('/'));

    #[derive(serde::Serialize)]
    struct Req<'a> {
        prompt: &'a str,
        validate: bool,
        max_retries: u32,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        code: String,
        valid: Option<bool>,
        #[serde(default)]
        attempts: u64,
        #[serde(default)]
        errors: Vec<String>,
        #[serde(default)]
        warnings: Vec<String>,
    }

    let resp = client
        .post(&endpoint)
        .json(&Req {
            prompt,
            validate,
            max_retries,
        })
        .send()
        .await
        .map_err(|e| format!("VoxLocal /generate request failed: {e}"))?;

    let status = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("VoxLocal server error {status}: {body}"));
    }

    let parsed: Resp = resp
        .json()
        .await
        .map_err(|e| format!("VoxLocal response parse error: {e}"))?;

    Ok(VoxLocalGenerateResult {
        code: parsed.code,
        valid: parsed.valid,
        errors: parsed.errors,
        warnings: parsed.warnings,
        attempts: parsed.attempts,
    })
}
