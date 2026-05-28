use super::metadata::HttpCallMetadata;
use super::types::{
    GeminiCandidate, GeminiContent, GeminiGenCfg, GeminiGenerateBody, GeminiPart, GeminiPartOut,
    GeminiResponse, GeminiSys, GeminiTurn,
};
use crate::llm_bridge::error::HttpInferError;
use crate::llm_bridge::providers::types::GeminiInlineData;

pub(crate) async fn http_gemini_with_metadata(
    client: &reqwest::Client,
    model_id: &str,
    api_key: &str,
    spec: &vox_orchestrator::models::ModelSpec,
    system: &str,
    user: vox_openai::ChatMessageContent<'_>,
    max_tokens: u64,
    temperature: Option<f32>,
    top_p: Option<f32>,
    json_mode: bool,
) -> Result<(String, u32, u32, HttpCallMetadata), HttpInferError> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model_id}:generateContent?key={api_key}"
    );

    let system_instruction = if system.is_empty() {
        None
    } else {
        Some(GeminiSys {
            parts: vec![GeminiPartOut {
                text: Some(system),
                inline_data: None,
            }],
        })
    };

    let response_mime_type = if json_mode {
        Some("application/json")
    } else {
        None
    };

    let mut parts = Vec::new();
    match user {
        vox_openai::ChatMessageContent::Text(t) => {
            parts.push(GeminiPartOut {
                text: Some(t),
                inline_data: None,
            });
        }
        vox_openai::ChatMessageContent::Parts(p) => {
            for part in p {
                match part {
                    vox_openai::ChatMessagePart::Text { text } => {
                        parts.push(GeminiPartOut {
                            text: Some(text),
                            inline_data: None,
                        });
                    }
                    vox_openai::ChatMessagePart::ImageUrl { image_url } => {
                        // Gemini expects data format. We assume data:mime;base64,data
                        if image_url.url.starts_with("data:") {
                            if let Some(comma_pos) = image_url.url.find(',') {
                                let header = &image_url.url[5..comma_pos];
                                let data = &image_url.url[comma_pos + 1..];
                                if let Some(semi_pos) = header.find(';') {
                                    let mime = &header[..semi_pos];
                                    parts.push(GeminiPartOut {
                                        text: None,
                                        inline_data: Some(GeminiInlineData {
                                            mime_type: mime,
                                            data,
                                        }),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let body = GeminiGenerateBody {
        system_instruction,
        contents: vec![GeminiTurn {
            role: "user",
            parts,
        }],
        generation_config: GeminiGenCfg {
            temperature,
            top_p,
            max_output_tokens: max_tokens.min(u32::MAX as u64) as u32,
            response_mime_type,
        },
    };

    let res = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| HttpInferError {
            status: 0,
            message: format!("Gemini HTTP: {e}"),
            is_capability_gap: false,
        })?;
    let status = res.status();
    let code = status.as_u16();
    let provider_request_id = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);

    if !status.is_success() {
        let t = res.text().await.unwrap_or_default();
        return Err(HttpInferError {
            status: code,
            message: t,
            is_capability_gap: false,
        });
    }

    let parsed: GeminiResponse = res.json().await.map_err(|e| HttpInferError {
        status: code,
        message: format!("Gemini JSON: {e}"),
        is_capability_gap: false,
    })?;

    let text = parsed
        .candidates
        .unwrap_or_default()
        .into_iter()
        .next()
        .and_then(|c: GeminiCandidate| c.content)
        .and_then(|c: GeminiContent| c.parts)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p: GeminiPart| p.text)
        .collect::<Vec<_>>()
        .join("");

    let prompt_t = parsed
        .usage_metadata
        .as_ref()
        .and_then(|u| u.prompt_token_count)
        .unwrap_or(0);
    let out_t = parsed
        .usage_metadata
        .as_ref()
        .and_then(|u| u.candidates_token_count)
        .unwrap_or(0);

    let estimated_usd = (prompt_t as f64 / 1000.0) * spec.cost_per_1k_input
        + (out_t as f64 / 1000.0) * spec.cost_per_1k_output;

    Ok((
        text,
        prompt_t,
        out_t,
        HttpCallMetadata {
            provider_request_id,
            provider_reported_cost_usd: Some(estimated_usd),
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
        },
    ))
}
