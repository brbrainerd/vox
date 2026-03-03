use crate::{execute_activity, ActivityOptions, ActivityResult};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::future::Future;
use std::pin::Pin;
use tokio_stream::Stream;

/// Message format for the chat API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// A configuration block for an LLM provider integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String, // e.g. "openrouter", "openai", "anthropic"
    pub model: String,    // e.g. "anthropic/claude-3.5-sonnet"
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u64>,
    pub response_format: Option<serde_json::Value>, // Structured JSON output
}

impl LlmConfig {
    /// Convenience constructor for OpenRouter.
    pub fn openrouter(model: impl Into<String>) -> Self {
        Self {
            provider: "openrouter".into(),
            model: model.into(),
            base_url: Some("https://openrouter.ai/api/v1/chat/completions".into()),
            api_key: std::env::var("OPENROUTER_API_KEY").ok(),
            temperature: None,
            max_tokens: None,
            response_format: None,
        }
    }

    /// Convenience constructor for OpenAI-compatible endpoints.
    pub fn openai(model: impl Into<String>) -> Self {
        Self {
            provider: "openai".into(),
            model: model.into(),
            base_url: Some("https://api.openai.com/v1/chat/completions".into()),
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            temperature: None,
            max_tokens: None,
            response_format: None,
        }
    }

    /// Resolve from a model registry alias.
    ///
    /// `registry` maps alias names (e.g. `"fast"`, `"smart"`) to
    /// `(provider, model_id, temperature, api_key_env)` tuples.
    pub fn from_registry(
        alias: &str,
        registry: &std::collections::HashMap<String, ModelRegistryEntry>,
    ) -> Result<Self, String> {
        let entry = registry
            .get(alias)
            .ok_or_else(|| format!("Unknown model alias: {}", alias))?;
        let api_key = entry
            .api_key_env
            .as_deref()
            .and_then(|env_name| std::env::var(env_name).ok())
            .or_else(|| match entry.provider.as_str() {
                "openrouter" => std::env::var("OPENROUTER_API_KEY").ok(),
                "openai" => std::env::var("OPENAI_API_KEY").ok(),
                "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
                _ => None,
            });
        let base_url = entry
            .base_url
            .clone()
            .or_else(|| match entry.provider.as_str() {
                "openrouter" => Some("https://openrouter.ai/api/v1/chat/completions".into()),
                "openai" => Some("https://api.openai.com/v1/chat/completions".into()),
                _ => None,
            });
        Ok(Self {
            provider: entry.provider.clone(),
            model: entry.model.clone(),
            base_url,
            api_key,
            temperature: entry.temperature,
            max_tokens: entry.max_tokens,
            response_format: None,
        })
    }
}

/// An entry in a Vox `@config model_registry:` block, deserialized at compile time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistryEntry {
    pub provider: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u64>,
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
}

/// Tracks token usage and cost per LLM call — stored in @table ModelMetric.
/// Serializable so it can be persisted to VoxDB directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetric {
    /// Millisecond-timestamp of the completion.
    pub ts: u64,
    pub model: String,
    pub provider: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    /// Estimated cost in USD (computed from a model registry lookup if available).
    pub estimated_cost_usd: f64,
}

impl ModelMetric {
    /// Build from an LlmResponse, computing cost at `cost_per_1k` rate.
    pub fn from_response(res: &LlmResponse, provider: &str, cost_per_1k: f64) -> Self {
        let total_tokens = res.prompt_tokens + res.completion_tokens;
        Self {
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            model: res.model.clone(),
            provider: provider.to_string(),
            prompt_tokens: res.prompt_tokens,
            completion_tokens: res.completion_tokens,
            estimated_cost_usd: (total_tokens as f64 / 1000.0) * cost_per_1k,
        }
    }
}

/// The standard parsed response from an LLM chat operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub model: String,
}

#[derive(Serialize)]
struct OpenRouterRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<&'a serde_json::Value>,
    stream: bool,
}

#[derive(Deserialize, Debug)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
    usage: Option<OpenRouterUsage>,
    model: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OpenRouterChoice {
    message: Option<OpenRouterMessage>,
}

#[derive(Deserialize, Debug)]
struct OpenRouterMessage {
    content: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OpenRouterUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

/// Core durable wrapper for LLM chat (single complete response).
pub async fn llm_chat(
    options: &ActivityOptions,
    messages: Vec<ChatMessage>,
    config: LlmConfig,
) -> ActivityResult<Result<LlmResponse, String>> {
    let activity_name = format!("llm_chat_{}_{}", config.provider, config.model);

    execute_activity(&activity_name, options, || {
        let messages = messages.clone();
        let config = config.clone();

        let fut = async move {
            let api_key = config
                .api_key
                .unwrap_or_else(|| match config.provider.as_str() {
                    "openrouter" => env::var("OPENROUTER_API_KEY").unwrap_or_default(),
                    "openai" => env::var("OPENAI_API_KEY").unwrap_or_default(),
                    _ => String::new(),
                });

            if api_key.is_empty() {
                return Ok(Err("No API key available for LLM provider".to_string()));
            }

            let base_url = config
                .base_url
                .unwrap_or_else(|| match config.provider.as_str() {
                    "openrouter" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
                    "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
                    _ => "https://openrouter.ai/api/v1/chat/completions".to_string(),
                });

            let client = Client::new();
            let req_body = OpenRouterRequest {
                model: &config.model,
                messages: &messages,
                temperature: config.temperature,
                max_tokens: config.max_tokens,
                response_format: config.response_format.as_ref(),
                stream: false,
            };

            let res = client
                .post(&base_url)
                .bearer_auth(api_key)
                .json(&req_body)
                .send()
                .await
                .map_err(|e| format!("HTTP request failed: {}", e))?;

            if !res.status().is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Ok(Err(format!("LLM API returned error: {}", err_text)));
            }

            let llm_res: OpenRouterResponse = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse response JSON: {}", e))?;

            let content = llm_res
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message)
                .and_then(|m| m.content)
                .unwrap_or_default();

            let usage = llm_res.usage.unwrap_or(OpenRouterUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
            });

            Ok(Ok(LlmResponse {
                content,
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                model: llm_res.model.unwrap_or_else(|| config.model.clone()),
            }))
        };
        let fut_typed: Pin<
            Box<dyn Future<Output = Result<Result<LlmResponse, String>, String>> + Send>,
        > = Box::pin(fut);
        fut_typed
    })
    .await
}

/// Token-by-token streaming implementation.
pub async fn llm_stream(
    messages: Vec<ChatMessage>,
    config: LlmConfig,
) -> Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String> {
    let api_key = config
        .api_key
        .unwrap_or_else(|| match config.provider.as_str() {
            "openrouter" => env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            "openai" => env::var("OPENAI_API_KEY").unwrap_or_default(),
            _ => String::new(),
        });

    if api_key.is_empty() {
        return Err("No API key available for LLM provider".to_string());
    }

    let base_url = config
        .base_url
        .unwrap_or_else(|| match config.provider.as_str() {
            "openrouter" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
            "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
            _ => "https://openrouter.ai/api/v1/chat/completions".to_string(),
        });

    let client = Client::new();
    let req_body = OpenRouterRequest {
        model: &config.model,
        messages: &messages,
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        response_format: config.response_format.as_ref(),
        stream: true,
    };

    let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;

    let res = client
        .post(&base_url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !res.status().is_success() {
        let err_text = res.text().await.unwrap_or_default();
        return Err(format!("LLM API returned error: {}", err_text));
    }

    let byte_stream = res.bytes_stream();

    let string_stream = byte_stream.map(|chunk_res| {
        match chunk_res {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                // Simple SSE parsing for just the token content
                // Very basic implementation targeting only OpenRouter / OpenAI SSE patterns
                let mut token_text = String::new();
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            continue;
                        }
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(choices) = parsed.get("choices") {
                                if let Some(choice) = choices.get(0) {
                                    if let Some(delta) = choice.get("delta") {
                                        if let Some(content) =
                                            delta.get("content").and_then(|c| c.as_str())
                                        {
                                            token_text.push_str(content);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(token_text)
            }
            Err(e) => Err(format!("Stream read error: {}", e)),
        }
    });

    Ok(Box::pin(string_stream))
}

#[derive(Serialize)]
struct OpenRouterEmbedRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Deserialize, Debug)]
struct OpenRouterEmbedResponse {
    data: Vec<OpenRouterEmbedData>,
    #[allow(dead_code)]
    usage: Option<OpenRouterUsage>,
    #[allow(dead_code)]
    model: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OpenRouterEmbedData {
    embedding: Vec<f32>,
}

/// Core durable wrapper for LLM embedding generation.
pub async fn llm_embed(
    options: &ActivityOptions,
    text: &str,
    config: LlmConfig,
) -> ActivityResult<Result<Vec<f32>, String>> {
    let activity_name = format!("llm_embed_{}_{}", config.provider, config.model);

    execute_activity(&activity_name, options, || {
        let text = text.to_string();
        let config = config.clone();

        let fut = async move {
            let api_key = config
                .api_key
                .unwrap_or_else(|| match config.provider.as_str() {
                    "openrouter" => env::var("OPENROUTER_API_KEY").unwrap_or_default(),
                    "openai" => env::var("OPENAI_API_KEY").unwrap_or_default(),
                    _ => String::new(),
                });

            if api_key.is_empty() {
                return Ok(Err("No API key available for LLM provider".to_string()));
            }

            let base_url = config
                .base_url
                .unwrap_or_else(|| match config.provider.as_str() {
                    "openrouter" => "https://openrouter.ai/api/v1/embeddings".to_string(),
                    "openai" => "https://api.openai.com/v1/embeddings".to_string(),
                    _ => "https://openrouter.ai/api/v1/embeddings".to_string(),
                });

            let client = Client::new();
            let req_body = OpenRouterEmbedRequest {
                model: &config.model,
                input: &text,
            };

            let res = client
                .post(&base_url)
                .bearer_auth(api_key)
                .json(&req_body)
                .send()
                .await
                .map_err(|e| format!("HTTP request failed: {}", e))?;

            if !res.status().is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Ok(Err(format!("LLM API returned error: {}", err_text)));
            }

            let embed_res: OpenRouterEmbedResponse = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse response JSON: {}", e))?;

            let vector = embed_res
                .data
                .into_iter()
                .next()
                .map(|d| d.embedding)
                .unwrap_or_default();

            if vector.is_empty() {
                return Ok(Err("LLM API returned empty embedding vector".to_string()));
            }

            Ok(Ok(vector))
        };
        let fut_typed: Pin<
            Box<dyn Future<Output = Result<Result<Vec<f32>, String>, String>> + Send>,
        > = Box::pin(fut);
        fut_typed
    })
    .await
}
