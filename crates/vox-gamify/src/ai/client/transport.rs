use bytes::Bytes;
use futures_util::StreamExt;
use std::pin::Pin;

use futures_util::Stream;

use crate::ai::constants::*;
use crate::ai::error::AiError;
use crate::ai::fallback::deterministic_response;
use crate::ai::keys::{resolve_gemini_key, resolve_openrouter_key};
use crate::ai::provider::FreeAiProvider;
use crate::ai::validate::urlencode;

use super::FreeAiClient;

impl FreeAiClient {
    /// POST to Ollama `/api/generate` with stream=true.
    pub(crate) async fn stream_ollama(
        http: &reqwest::Client,
        url: &str,
        model: &str,
        prompt: &str,
    ) -> Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>> {
        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": true,
        });

        let http = http.clone();
        let url = format!("{}/api/generate", url);

        Box::pin(async_stream::try_stream! {
            let resp = http.post(&url).json(&body).send().await.map_err(AiError::Http)?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                Err(AiError::RateLimited {
                    provider: "ollama".to_string(),
                    retry_after_secs: vox_http_client::parse_retry_after(resp.headers()),
                })?;
            }

            use vox_openai::sse::Utf8LineBuffer;
            let mut stream = resp.bytes_stream();
            let mut line_buf = Utf8LineBuffer::new();

            while let Some(item) = stream.next().await {
                let chunk: Bytes = item.map_err(AiError::Http)?;
                let mut lines: Vec<String> = Vec::new();
                line_buf.push_lossy_bytes(&chunk, |line| {
                    if !line.is_empty() {
                        lines.push(line.to_string());
                    }
                });
                for line in lines {
                    let json: serde_json::Value =
                        serde_json::from_str(&line).map_err(AiError::Json)?;
                    if let Some(token) = json["response"].as_str() {
                        yield token.to_string();
                    }
                    if json["done"].as_bool().unwrap_or(false) {
                        return;
                    }
                }
            }
            let mut tail: Vec<String> = Vec::new();
            line_buf.flush_trailing(|line| tail.push(line.to_string()));
            for line in tail {
                let json: serde_json::Value =
                    serde_json::from_str(&line).map_err(AiError::Json)?;
                if let Some(token) = json["response"].as_str() {
                    yield token.to_string();
                }
            }
        })
    }

    /// POST to Gemini `streamGenerateContent`.
    pub(crate) async fn stream_gemini(
        http: &reqwest::Client,
        api_key: &str,
        model: &str,
        prompt: &str,
    ) -> Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>> {
        let resolved_key = resolve_gemini_key(api_key);

        // Direct Gemini (generativelanguage) is NOT OpenAI-compatible — the egress core can't
        // carry it; documented local egress per the egress design spec.
        // vox-arch-check: allow llm-egress
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}",
            model, resolved_key
        );

        let body = serde_json::json!({
            "contents": [{ "parts": [{ "text": prompt }] }]
        });

        let http = http.clone();

        Box::pin(async_stream::try_stream! {
            let resp = http.post(&url).json(&body).send().await.map_err(AiError::Http)?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                Err(AiError::RateLimited {
                    provider: "google".to_string(),
                    retry_after_secs: vox_http_client::parse_retry_after(resp.headers()),
                })?;
            }

            let mut stream = resp.bytes_stream();

            while let Some(item) = stream.next().await {
                let chunk: Bytes = item.map_err(AiError::Http)?;
                // Gemini stream is an array of objects
                let json: serde_json::Value = serde_json::from_slice(&chunk).map_err(AiError::Json)?;
                if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                    yield text.to_string();
                }
            }
        })
    }

    /// OpenRouter chat completions with `stream: true` (SSE `data:` lines).
    ///
    /// Kept as a documented local egress (not routed through `vox-llm-egress`) because the
    /// core's `stream_once` does not surface the per-call `x-response-cost` header that this
    /// path feeds to `cost_reporter`. Listed as a facade-coverage gap in the egress design
    /// spec; exempted in the Phase 6 arch-check seal pending an egress streaming-cost extension.
    // vox-arch-check: allow llm-egress
    pub(crate) fn stream_openrouter(
        http: &reqwest::Client,
        api_key: &str,
        model: &str,
        prompt: &str,
        cost_reporter: Option<super::CostReportFn>,
    ) -> Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>> {
        let http = http.clone();
        let model = model.to_string();
        let prompt = prompt.to_string();
        let api_key = api_key.to_string();
        Box::pin(async_stream::try_stream! {
            let resolved_key = if api_key.is_empty() {
                vox_config::openrouter_api_key().unwrap_or_default()
            } else {
                api_key
            };
            if resolved_key.is_empty() {
                Err(AiError::AllProvidersFailed(
                    "OPENROUTER_API_KEY not set".to_string(),
                ))?;
            }
            let body = serde_json::json!({
                "model": &model,
                "messages": [{ "role": "user", "content": &prompt }],
                "max_tokens": 512u32,
                "stream": true,
            });
            let resp = http
                .post(openrouter_base())
                .header("Authorization", format!("Bearer {}", resolved_key))
                .header("HTTP-Referer", "https://github.com/vox-foundation/vox")
                .header("X-Title", "Vox Gamify")
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .json(&body)
                .send()
                .await
                .map_err(AiError::Http)?;
            let status = resp.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                Err(AiError::RateLimited {
                    provider: format!("openrouter:{}", model),
                    retry_after_secs: vox_http_client::parse_retry_after(resp.headers()),
                })?;
            }
            if let Some(ref reporter) = cost_reporter {
                if let Some(cost_val) = resp.headers()
                    .get("x-response-cost")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok()) {
                    reporter(cost_val);
                }
            }
            let mut bytes_stream = if status.is_success() {
                resp.bytes_stream()
            } else {
                let body_txt = resp.text().await.unwrap_or_default();
                Err(AiError::AllProvidersFailed(format!(
                    "OpenRouter stream HTTP {} {}",
                    status, body_txt
                )))?
            };
            use vox_openai::sse::{Utf8LineBuffer, sse_data_line_delta};
            let mut line_buf = Utf8LineBuffer::new();
            while let Some(item) = bytes_stream.next().await {
                let chunk: Bytes = item.map_err(AiError::Http)?;
                let mut emitted: Vec<String> = Vec::new();
                line_buf.push_lossy_bytes(&chunk, |line| {
                    if let Some(t) = sse_data_line_delta(line) {
                        emitted.push(t);
                    }
                });
                for t in emitted {
                    yield t;
                }
            }
            let mut tail_emit: Vec<String> = Vec::new();
            line_buf.flush_trailing(|line| {
                if let Some(t) = sse_data_line_delta(line) {
                    tail_emit.push(t);
                }
            });
            for t in tail_emit {
                yield t;
            }
        })
    }

    pub(crate) async fn call_provider_static(
        http: &reqwest::Client,
        provider: &FreeAiProvider,
        prompt: &str,
    ) -> Result<String, AiError> {
        match provider {
            FreeAiProvider::Ollama { url, model } => {
                Self::call_ollama_static(http, url, model, prompt).await
            }
            FreeAiProvider::Pollinations => Self::call_pollinations_static(http, prompt).await,
            FreeAiProvider::Gemini { api_key, model } => {
                Self::call_gemini_static(http, api_key, model, prompt).await
            }
            FreeAiProvider::OpenRouter { api_key, models } => {
                Self::call_openrouter_static(http, api_key, models, prompt).await
            }
            FreeAiProvider::Deterministic => Ok(deterministic_response(prompt)),
        }
    }

    /// Call OpenRouter with model-level fallback through the free-tier list.
    ///
    /// Tries each model until one returns a non-empty response.
    /// On rate limit (429) or quota errors, advances to the next model.
    pub(crate) async fn call_openrouter_static(
        // The wire now goes through vox_llm_egress::chat_once (which owns its HTTP client),
        // so the passed client is no longer used; kept for signature compatibility.
        _http: &reqwest::Client,
        api_key: &str,
        models: &[String],
        prompt: &str,
    ) -> Result<String, AiError> {
        let resolved_key = resolve_openrouter_key(api_key);
        if resolved_key.is_empty() {
            return Err(AiError::AllProvidersFailed(
                "OpenRouter API key not set (configure Clavis or OPENROUTER_API_KEY)".to_string(),
            ));
        }
        let model_list: Vec<&str> = if models.is_empty() {
            OPENROUTER_FREE_MODELS.to_vec()
        } else {
            models.iter().map(String::as_str).collect()
        };

        let mut last_err = String::new();
        let mut first_rate_limit: Option<(String, Option<u64>)> = None;

        for model in &model_list {
            // Route the wire through the sanctioned egress core, but keep gamify's own
            // key + attribution headers (X-Title differs from the orchestrator's).
            let ereq = vox_llm_egress::EgressRequest {
                base_url: vox_config::openrouter_chat_completions_url(),
                api_key: resolved_key.clone(),
                model: (*model).to_string(),
                headers: vec![
                    (
                        "HTTP-Referer".to_string(),
                        "https://github.com/vox-foundation/vox".to_string(),
                    ),
                    ("X-Title".to_string(), "Vox Gamify".to_string()),
                ],
                throttle_key: "openrouter".to_string(),
                max_concurrent: 8,
                timeout_ms: Some(30_000),
            };
            let msgs = [vox_llm_egress::ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }];
            let params = vox_llm_egress::ChatParams {
                max_tokens: Some(512),
                ..Default::default()
            };
            match vox_llm_egress::chat_once(&ereq, &msgs, &params).await {
                Ok(resp) => {
                    let trimmed = resp.content.trim().to_string();
                    if !trimmed.is_empty() {
                        tracing::debug!("OpenRouter model '{}' succeeded", model);
                        return Ok(trimmed);
                    }
                    last_err = format!("model '{}': empty content in response", model);
                }
                Err(vox_llm_egress::EgressError::RateLimited { retry_after }) => {
                    if first_rate_limit.is_none() {
                        first_rate_limit =
                            Some((model.to_string(), retry_after.map(|d| d.as_secs())));
                    }
                    last_err = format!("model '{}': rate limited", model);
                    tracing::debug!("OpenRouter rate-limited for '{}', trying next", model);
                    continue;
                }
                // Quota exceeded (402) — treat like a rate-limit and try the next model.
                Err(vox_llm_egress::EgressError::Status { code: 402, .. }) => {
                    if first_rate_limit.is_none() {
                        first_rate_limit = Some((model.to_string(), None));
                    }
                    last_err = format!("model '{}': HTTP 402", model);
                    tracing::debug!("OpenRouter 402 for '{}', trying next", model);
                    continue;
                }
                Err(e) => {
                    last_err = format!("model '{}': {}", model, e);
                }
            }
            tracing::debug!(
                "OpenRouter model '{}' failed, trying next: {}",
                model,
                last_err
            );
        }

        if let Some((model, retry_after)) = first_rate_limit {
            return Err(AiError::RateLimited {
                provider: format!("openrouter:{}", model),
                retry_after_secs: retry_after,
            });
        }

        Err(AiError::AllProvidersFailed(format!(
            "OpenRouter exhausted all free models: {}",
            last_err
        )))
    }

    pub(crate) async fn call_ollama_static(
        http: &reqwest::Client,
        url: &str,
        model: &str,
        prompt: &str,
    ) -> Result<String, AiError> {
        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
        });
        let resp = http
            .post(format!("{}/api/generate", url))
            .json(&body)
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AiError::RateLimited {
                provider: "ollama".to_string(),
                retry_after_secs: vox_http_client::parse_retry_after(resp.headers()),
            });
        }
        let json: serde_json::Value = resp.json().await?;
        json["response"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or(AiError::EmptyResponse)
    }

    pub(crate) async fn call_pollinations_static(
        http: &reqwest::Client,
        prompt: &str,
    ) -> Result<String, AiError> {
        let encoded = urlencode(prompt);
        let url = format!("{}{}?model=openai&nologo=true", POLLINATIONS_BASE, encoded);
        let resp = http.get(&url).send().await?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AiError::RateLimited {
                provider: "pollinations".to_string(),
                retry_after_secs: vox_http_client::parse_retry_after(resp.headers()),
            });
        }

        let text = resp.text().await?;
        if text.trim().is_empty() {
            return Err(AiError::EmptyResponse);
        }
        Ok(text)
    }

    pub(crate) async fn call_gemini_static(
        http: &reqwest::Client,
        api_key: &str,
        model: &str,
        prompt: &str,
    ) -> Result<String, AiError> {
        let resolved_key = resolve_gemini_key(api_key);
        let url = GEMINI_ENDPOINT_TEMPLATE
            .replace("{MODEL}", model)
            .replace("{KEY}", &resolved_key);
        let body = serde_json::json!({ "contents": [{ "parts": [{ "text": prompt }] }] });
        let resp = http.post(&url).json(&body).send().await?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AiError::RateLimited {
                provider: "google".to_string(),
                retry_after_secs: vox_http_client::parse_retry_after(resp.headers()),
            });
        }

        let json: serde_json::Value = resp.json().await?;
        json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or(AiError::EmptyResponse)
    }

    /// Call a single provider.
    pub(crate) async fn call_provider(
        &self,
        provider: &FreeAiProvider,
        prompt: &str,
    ) -> Result<String, AiError> {
        Self::call_provider_static(&self.http, provider, prompt).await
    }

    /// Return the list of configured providers (for status display).
    pub fn providers(&self) -> &[FreeAiProvider] {
        &self.providers
    }
}

#[cfg(test)]
mod ollama_ndjson_line_tests {
    use vox_openai::sse::Utf8LineBuffer;

    /// Mirrors the line handling in [`FreeAiClient::stream_ollama`]
    /// (framing itself is covered in vox-openai's sse tests).
    fn collect_responses_from_chunks(chunks: &[&[u8]]) -> Vec<String> {
        let mut line_buf = Utf8LineBuffer::new();
        let mut out: Vec<String> = Vec::new();
        let mut on_line = |line: &str| {
            if line.is_empty() {
                return;
            }
            let json: serde_json::Value = serde_json::from_str(line).unwrap();
            if let Some(s) = json["response"].as_str() {
                out.push(s.to_string());
            }
        };
        for chunk in chunks {
            line_buf.push_lossy_bytes(chunk, &mut on_line);
        }
        line_buf.flush_trailing(&mut on_line);
        out
    }

    #[test]
    fn ndjson_split_across_tcp_like_chunks() {
        let out = collect_responses_from_chunks(&[
            b"{\"response\":\"hel",
            b"lo\",\"done\":false}\n{\"response\":\"\",\"done\":true}\n",
        ]);
        assert_eq!(out, vec!["hello", ""]);
    }
}
