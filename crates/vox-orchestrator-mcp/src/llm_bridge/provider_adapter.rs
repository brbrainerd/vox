use std::future::Future;
use std::pin::Pin;
use vox_orchestrator::models::{ModelSpec, ProviderType};

use super::error::HttpInferError;
use super::provider_auth::{bearer_for, extra_headers_for};
use super::provider_endpoints::endpoint_for;
use super::providers::{
    HttpCallMetadata, http_gemini_with_metadata, http_ollama_with_metadata,
    http_openai_compatible_with_headers, probe_ollama_tags, probe_vox_local_health,
};

#[derive(Debug, Clone)]
pub(crate) struct ProviderInferResult {
    pub text: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub provider_request_id: Option<String>,
    pub provider_reported_cost_usd: Option<f64>,
    /// Anthropic-style: tokens served from prompt cache (cheap reads).
    pub cache_read_input_tokens: Option<u32>,
    /// Anthropic-style: tokens written to populate the prompt cache (creation premium).
    pub cache_creation_input_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct InferRequest<'a> {
    pub system_prompt: &'a str,
    pub user_prompt: vox_openai::ChatMessageContent<'a>,
    pub max_t: u64,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub json_mode: bool,
    pub tools: Option<serde_json::Value>,
    pub tool_choice: Option<serde_json::Value>,
}

trait ProviderAdapter: Send + Sync {
    fn supports(&self, provider_type: &ProviderType) -> bool;
    fn infer<'a>(
        &'a self,
        client: &'a reqwest::Client,
        model: &'a ModelSpec,
        req: InferRequest<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInferResult, HttpInferError>> + Send + 'a>>;
}

fn anthropic_tools_guard(req: &InferRequest<'_>) -> Result<(), HttpInferError> {
    if req.tools.is_some() || req.tool_choice.is_some() {
        return Err(HttpInferError::capability_gap(
            "AnthropicNative does not support tool calls; \
             retrying via OpenAI-compat adapter",
        ));
    }
    Ok(())
}

struct GoogleDirectAdapter;
struct OllamaAdapter;
struct AnthropicNativeAdapter;
struct OpenAiCompatAdapter;

fn adapt_result(
    text: String,
    prompt_tokens: u32,
    completion_tokens: u32,
    meta: HttpCallMetadata,
) -> ProviderInferResult {
    ProviderInferResult {
        text,
        prompt_tokens,
        completion_tokens,
        provider_request_id: meta.provider_request_id,
        provider_reported_cost_usd: meta.provider_reported_cost_usd,
        cache_read_input_tokens: meta.cache_read_input_tokens,
        cache_creation_input_tokens: meta.cache_creation_input_tokens,
    }
}

impl ProviderAdapter for GoogleDirectAdapter {
    fn supports(&self, provider_type: &ProviderType) -> bool {
        matches!(provider_type, ProviderType::GoogleDirect)
    }

    fn infer<'a>(
        &'a self,
        client: &'a reqwest::Client,
        model: &'a ModelSpec,
        req: InferRequest<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInferResult, HttpInferError>> + Send + 'a>>
    {
        Box::pin(async move {
            let resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::GeminiApiKey);
            let key = resolved.expose().ok_or_else(|| HttpInferError {
                status: 0,
                message: "GEMINI_API_KEY is not set (required for Google-direct models)".into(),
                is_capability_gap: false,
            })?;
            let (text, prompt_tokens, completion_tokens, meta) = http_gemini_with_metadata(
                client,
                &model.id,
                key,
                model,
                req.system_prompt,
                req.user_prompt,
                req.max_t,
                req.temperature,
                req.top_p,
                req.json_mode,
            )
            .await?;
            Ok(adapt_result(text, prompt_tokens, completion_tokens, meta))
        })
    }
}

impl ProviderAdapter for OllamaAdapter {
    fn supports(&self, provider_type: &ProviderType) -> bool {
        matches!(provider_type, ProviderType::Ollama)
    }

    fn infer<'a>(
        &'a self,
        client: &'a reqwest::Client,
        model: &'a ModelSpec,
        req: InferRequest<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInferResult, HttpInferError>> + Send + 'a>>
    {
        Box::pin(async move {
            probe_ollama_tags(client).await?;
            let (text, prompt_tokens, completion_tokens, meta) = http_ollama_with_metadata(
                client,
                &model.id,
                req.system_prompt,
                req.user_prompt,
                req.max_t,
                req.temperature,
                req.top_p,
                req.json_mode,
            )
            .await?;
            Ok(adapt_result(text, prompt_tokens, completion_tokens, meta))
        })
    }
}
impl ProviderAdapter for AnthropicNativeAdapter {
    fn supports(&self, provider_type: &ProviderType) -> bool {
        matches!(provider_type, ProviderType::Anthropic)
            && vox_secrets::resolve_secret(vox_secrets::SecretId::VoxAnthropicDirect)
                .expose()
                .unwrap_or("")
                == "1"
    }

    fn infer<'a>(
        &'a self,
        client: &'a reqwest::Client,
        model: &'a ModelSpec,
        req: InferRequest<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInferResult, HttpInferError>> + Send + 'a>>
    {
        Box::pin(async move {
            anthropic_tools_guard(&req)?;
            use super::providers::http_anthropic_direct;
            let url = endpoint_for(model)?;
            let bearer = super::provider_auth::bearer_for(model)?;
            let api_key = bearer.strip_prefix("Bearer ").unwrap_or(&bearer);

            let (text, prompt_tokens, completion_tokens, meta) = http_anthropic_direct(
                client,
                &url,
                api_key,
                model,
                &model.id,
                req.system_prompt,
                req.user_prompt,
                req.max_t,
                req.temperature,
                req.top_p,
            )
            .await?;
            Ok(adapt_result(text, prompt_tokens, completion_tokens, meta))
        })
    }
}

impl ProviderAdapter for OpenAiCompatAdapter {
    fn supports(&self, provider_type: &ProviderType) -> bool {
        !matches!(
            provider_type,
            ProviderType::GoogleDirect | ProviderType::Ollama
        )
    }

    fn infer<'a>(
        &'a self,
        client: &'a reqwest::Client,
        model: &'a ModelSpec,
        req: InferRequest<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInferResult, HttpInferError>> + Send + 'a>>
    {
        Box::pin(async move {
            let url = endpoint_for(model)?;
            let bearer = bearer_for(model)?;
            let headers = extra_headers_for(model);
            let (text, prompt_tokens, completion_tokens, meta) =
                http_openai_compatible_with_headers(
                    client,
                    &url,
                    &bearer,
                    &model.id,
                    req.system_prompt,
                    req.user_prompt,
                    req.max_t,
                    req.temperature,
                    req.top_p,
                    req.json_mode,
                    req.tools.clone(),
                    req.tool_choice.clone(),
                    &headers,
                )
                .await?;
            Ok(adapt_result(text, prompt_tokens, completion_tokens, meta))
        })
    }
}

struct VoxLocalAdapter;

#[derive(serde::Serialize)]
struct VoxLocalGenerateRequest {
    prompt: String,
    validate: bool,
    max_retries: u32,
}

#[derive(serde::Deserialize)]
struct VoxLocalGenerateResponse {
    code: String,
    valid: Option<bool>,
    #[serde(default)]
    errors: Vec<String>,
}

fn extract_prompt_text(content: &vox_openai::ChatMessageContent<'_>) -> String {
    match content {
        vox_openai::ChatMessageContent::Text(t) => t.to_string(),
        vox_openai::ChatMessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|p| match p {
                vox_openai::ChatMessagePart::Text { text } => Some(*text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
    }
}

impl ProviderAdapter for VoxLocalAdapter {
    fn supports(&self, provider_type: &ProviderType) -> bool {
        matches!(provider_type, ProviderType::VoxLocal)
    }

    fn infer<'a>(
        &'a self,
        client: &'a reqwest::Client,
        model: &'a ModelSpec,
        req: InferRequest<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInferResult, HttpInferError>> + Send + 'a>>
    {
        Box::pin(async move {
            probe_vox_local_health(client).await?;
            let endpoint = endpoint_for(model)?;
            let prompt = extract_prompt_text(&req.user_prompt);
            let body = VoxLocalGenerateRequest {
                prompt,
                validate: true,
                max_retries: 3,
            };
            let resp = client
                .post(&endpoint)
                .json(&body)
                .send()
                .await
                .map_err(|e| HttpInferError {
                    status: 0,
                    message: format!("VoxLocal /generate request failed: {e}"),
                    is_capability_gap: false,
                })?;

            let status = resp.status().as_u16();
            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(HttpInferError {
                    status,
                    message: format!("VoxLocal server error {status}: {body}"),
                    is_capability_gap: false,
                });
            }

            let parsed: VoxLocalGenerateResponse =
                resp.json().await.map_err(|e| HttpInferError {
                    status: 0,
                    message: format!("VoxLocal response parse error: {e}"),
                    is_capability_gap: false,
                })?;

            if parsed.valid == Some(false) && !parsed.errors.is_empty() {
                tracing::warn!(
                    target: "vox.mcp.llm.vox_local",
                    errors = ?parsed.errors,
                    "VoxLocal returned invalid code"
                );
            }

            // Token counts are not reported by the 7863 server; estimate from byte length.
            let approx_tokens = (parsed.code.len() / 4) as u32;
            Ok(ProviderInferResult {
                text: parsed.code,
                prompt_tokens: 0,
                completion_tokens: approx_tokens,
                provider_request_id: None,
                provider_reported_cost_usd: None,
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            })
        })
    }
}

fn adapters() -> Vec<Box<dyn ProviderAdapter>> {
    vec![
        Box::new(GoogleDirectAdapter),
        Box::new(VoxLocalAdapter),
        Box::new(OllamaAdapter),
        Box::new(AnthropicNativeAdapter),
        Box::new(OpenAiCompatAdapter),
    ]
}

pub(crate) async fn infer_via_provider_adapter(
    client: &reqwest::Client,
    model: &ModelSpec,
    system_prompt: &str,
    user_prompt: vox_openai::ChatMessageContent<'_>,
    max_t: u64,
    temperature: Option<f32>,
    top_p: Option<f32>,
    json_mode: bool,
    tools: Option<serde_json::Value>,
    tool_choice: Option<serde_json::Value>,
) -> Result<ProviderInferResult, HttpInferError> {
    let req = InferRequest {
        system_prompt,
        user_prompt,
        max_t,
        temperature,
        top_p,
        json_mode,
        tools,
        tool_choice,
    };
    let mut last_err: Option<HttpInferError> = None;
    for adapter in adapters() {
        if adapter.supports(&model.provider_type) {
            match adapter.infer(client, model, req.clone()).await {
                Ok(result) => return Ok(result),
                Err(e) if e.is_capability_gap => {
                    tracing::debug!(
                        target: "vox.mcp.llm.adapter",
                        message = %e.message,
                        "adapter skipped (capability gap); trying next"
                    );
                    last_err = Some(e);
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        HttpInferError::new(
            0,
            format!("No provider adapter found for {:?}", model.provider_type),
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_infer_request<'a>(
        tools: Option<serde_json::Value>,
        tool_choice: Option<serde_json::Value>,
    ) -> InferRequest<'a> {
        InferRequest {
            system_prompt: "",
            user_prompt: vox_openai::ChatMessageContent::Text("hello"),
            max_t: 256,
            temperature: None,
            top_p: None,
            json_mode: false,
            tools,
            tool_choice,
        }
    }

    #[test]
    fn anthropic_tools_guard_rejects_tools() {
        let req = make_infer_request(
            Some(serde_json::json!([{"type": "function", "function": {"name": "foo"}}])),
            None,
        );
        let err = anthropic_tools_guard(&req).expect_err("should be a capability gap");
        assert!(err.is_capability_gap);
        assert!(
            err.message
                .contains("AnthropicNative does not support tool calls")
        );
    }

    #[test]
    fn anthropic_tools_guard_rejects_tool_choice() {
        let req = make_infer_request(None, Some(serde_json::json!("auto")));
        let err = anthropic_tools_guard(&req).expect_err("should be a capability gap");
        assert!(err.is_capability_gap);
    }

    #[test]
    fn anthropic_tools_guard_passes_without_tools() {
        let req = make_infer_request(None, None);
        assert!(anthropic_tools_guard(&req).is_ok());
    }
}
