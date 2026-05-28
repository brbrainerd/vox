//! Durable embedding HTTP client.

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use crate::{ActivityOptions, ActivityResult, execute_activity};

use super::types::LlmConfig;
use super::wire::{OpenRouterUsage, chat_requires_nonempty_api_key, resolve_chat_api_key};

type LlmEmbedActivityFuture =
    Pin<Box<dyn Future<Output = Result<Result<Vec<f32>, String>, String>> + Send>>;

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
            let api_key = resolve_chat_api_key(&config);

            if chat_requires_nonempty_api_key(&config.provider) && api_key.is_empty() {
                return Ok(Err("No API key available for LLM provider".to_string()));
            }

            let base_url =
                config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| match config.provider.as_str() {
                        "openrouter" => vox_config::OPENROUTER_EMBEDDINGS_URL.to_string(),
                        "openai" => vox_config::OPENAI_EMBEDDINGS_URL.to_string(),
                        "hf_router" | "huggingface" => {
                            "https://router.huggingface.co/v1/embeddings".to_string()
                        }
                        _ => vox_config::OPENROUTER_EMBEDDINGS_URL.to_string(),
                    });
            if matches!(config.provider.as_str(), "hf_endpoint")
                && (base_url.trim().is_empty() || !base_url.contains("embeddings"))
            {
                return Ok(Err(
                    "hf_endpoint embeddings require base_url pointing to …/v1/embeddings"
                        .to_string(),
                ));
            }

            let client = vox_http_client::client();
            let req_body = OpenRouterEmbedRequest {
                model: &config.model,
                input: &text,
            };

            let mut req = client.post(&base_url).json(&req_body);
            if !api_key.is_empty() {
                req = req.bearer_auth(api_key);
            }
            let res = req
                .send()
                .await
                .map_err(|e| format!("HTTP request failed: {}", e))?;

            if !res.status().is_success() {
                let err_text = res
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<no body>"));
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
        let fut_typed: LlmEmbedActivityFuture = Box::pin(fut);
        fut_typed
    })
    .await
}
