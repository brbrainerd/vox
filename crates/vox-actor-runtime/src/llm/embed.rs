//! Durable embedding HTTP client.

use std::future::Future;
use std::pin::Pin;

use crate::{ActivityOptions, ActivityResult, execute_activity};

use super::types::LlmConfig;

type LlmEmbedActivityFuture =
    Pin<Box<dyn Future<Output = Result<Result<Vec<f32>, String>, String>> + Send>>;

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
            // Resolve the embeddings endpoint and pass it as the base-url override so the
            // egress core posts to /embeddings (resolve_egress's default is chat/completions).
            let embed_base =
                config
                    .base_url
                    .clone()
                    .unwrap_or_else(|| match config.provider.as_str() {
                        "openrouter" => vox_config::openrouter_embeddings_url(),
                        "openai" => vox_config::openai_embeddings_url(),
                        "hf_router" | "huggingface" => {
                            "https://router.huggingface.co/v1/embeddings".to_string()
                        }
                        _ => vox_config::openrouter_embeddings_url(),
                    });
            if matches!(config.provider.as_str(), "hf_endpoint")
                && (embed_base.trim().is_empty() || !embed_base.contains("embeddings"))
            {
                return Ok(Err(
                    "hf_endpoint embeddings require base_url pointing to …/v1/embeddings"
                        .to_string(),
                ));
            }

            let input = vox_config::resolve_egress::EgressResolveInput {
                provider: config.provider.clone(),
                model: config.model.clone(),
                base_url_override: Some(embed_base),
                timeout_ms: config.timeout_ms,
            };
            let ereq = match vox_config::resolve_egress::resolve_egress(&input) {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            let vector = match vox_llm_egress::embed_once(&ereq, &text).await {
                Ok(v) => v,
                Err(e) => return Ok(Err(e.to_string())),
            };

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
