//! SSE streaming chat completions.

use std::pin::Pin;

use futures_util::StreamExt;
use tokio_stream::Stream;

use super::types::{ChatMessage, LlmConfig};

/// Token-by-token streaming implementation.
pub async fn llm_stream(
    messages: Vec<ChatMessage>,
    config: LlmConfig,
) -> Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String> {
    // Resolve once (single-source) and stream via the sanctioned egress core.
    let input = vox_config::resolve_egress::EgressResolveInput {
        provider: config.provider.clone(),
        model: config.model.clone(),
        base_url_override: config.base_url.clone(),
        // Resolved but ignored by stream_once (a whole-request deadline would sever SSE).
        timeout_ms: config.timeout_ms,
    };
    let ereq = vox_config::resolve_egress::resolve_egress(&input)?;
    let wire_msgs: Vec<vox_llm_egress::ChatMessage> = messages
        .iter()
        .map(|m| vox_llm_egress::ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();
    let wire_tools: Option<Vec<vox_llm_egress::ToolDef>> = config.tools.as_ref().map(|ts| {
        ts.iter()
            .map(|t| vox_llm_egress::ToolDef {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            })
            .collect()
    });
    let params = vox_llm_egress::ChatParams {
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        response_format: config.response_format.as_ref(),
        tools: wire_tools.as_deref(),
        tool_choice: config.tool_choice.as_ref(),
    };

    // Streaming cost is gamify's concern (it has the cost_reporter); the facade records
    // cost via its non-streaming telemetry path, so it ignores the surfaced cost here.
    let (inner, _cost_usd) = vox_llm_egress::stream_once(&ereq, &wire_msgs, &params)
        .await
        .map_err(|e| e.to_string())?;
    // Map the core's structured error item type to the facade's String error.
    let mapped = inner.map(|item| item.map_err(|e| e.to_string()));
    Ok(Box::pin(mapped))
}
