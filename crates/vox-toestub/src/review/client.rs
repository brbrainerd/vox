use std::time::Duration;
use crate::rules::{Finding, Language, SourceFile};
use super::providers::{ReviewProvider, auto_discover_providers};
use super::types::ReviewFinding;
use super::prompts::{build_review_prompt, build_diff_review_prompt, REVIEW_SYSTEM_PROMPT};
use super::formatters::parse_review_response;

/// Performs AI-powered code review using the configured provider cascade.
pub struct ReviewClient {
    providers: Vec<ReviewProvider>,
    http: reqwest::Client,
}

impl ReviewClient {
    /// Create a client with an explicit provider list.
    pub fn new(providers: Vec<ReviewProvider>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent("vox-review/0.1")
            .build()
            .expect("Failed to build HTTP client for vox review");
        Self { providers, http }
    }

    /// Auto-discover providers from environment variables.
    pub fn auto() -> Self {
        Self::new(auto_discover_providers())
    }

    /// Auto-discover, but only free providers.
    pub fn free_only() -> Self {
        let providers = auto_discover_providers()
            .into_iter()
            .filter(|p| !p.requires_key())
            .collect();
        Self::new(providers)
    }

    /// The name of the first available provider.
    pub fn primary_provider_name(&self) -> &str {
        self.providers.first().map(|p| p.name()).unwrap_or("none")
    }

    /// Review a source file and return structured findings.
    pub async fn review_file(
        &self,
        file: &SourceFile,
        static_findings: &[Finding],
        lang_hint: Language,
        max_context_tokens: usize,
    ) -> Result<(Vec<ReviewFinding>, String, usize), String> {
        self.review_file_with_diff(file, static_findings, lang_hint, max_context_tokens, None)
            .await
    }

    /// Like `review_file` but scoped to changed lines in `diff_hunk`.
    pub async fn review_file_with_diff(
        &self,
        file: &SourceFile,
        static_findings: &[Finding],
        lang_hint: Language,
        max_context_tokens: usize,
        diff_hunk: Option<&str>,
    ) -> Result<(Vec<ReviewFinding>, String, usize), String> {
        let prompt = match diff_hunk {
            Some(hunk) => build_diff_review_prompt(file, static_findings, max_context_tokens, hunk),
            None => build_review_prompt(file, static_findings, lang_hint, max_context_tokens),
        };

        for provider in &self.providers {
            match self.call_provider(provider, &prompt).await {
                Ok((response, tokens)) => {
                    let mut findings = parse_review_response(&response, &file.path);
                    // Verification pass: remove impossible line numbers
                    findings.retain(|f| f.line == 0 || f.line <= file.lines.len());
                    // Remove very low confidence findings
                    findings.retain(|f| f.confidence >= 40);
                    return Ok((findings, provider.name().to_string(), tokens));
                }
                Err(e) => {
                    eprintln!(
                        "  [review] Provider '{}' failed: {} — trying next…",
                        provider.name(),
                        e
                    );
                }
            }
        }

        Err("All review providers failed. Try setting OPENROUTER_API_KEY, OPENAI_API_KEY, or GEMINI_API_KEY.".to_string())
    }

    pub async fn call_provider(
        &self,
        provider: &ReviewProvider,
        prompt: &str,
    ) -> Result<(String, usize), String> {
        match provider {
            ReviewProvider::OpenRouter {
                api_key,
                model,
                site_url,
            } => {
                let key = resolve_key(api_key, "OPENROUTER_API_KEY");
                if key.is_empty() {
                    return Err("No OpenRouter API key".to_string());
                }
                self.call_chat_completions(
                    "https://openrouter.ai/api/v1/chat/completions",
                    &key,
                    model,
                    prompt,
                    Some(site_url),
                )
                .await
            }
            ReviewProvider::OpenAi {
                api_key,
                model,
                base_url,
            } => {
                let key = resolve_key(api_key, "OPENAI_API_KEY");
                if key.is_empty() {
                    return Err("No OpenAI API key".to_string());
                }
                let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
                self.call_chat_completions(&url, &key, model, prompt, None)
                    .await
            }
            ReviewProvider::Gemini { api_key, model } => {
                let key = resolve_key(api_key, "GEMINI_API_KEY");
                if key.is_empty() {
                    return Err("No Gemini API key".to_string());
                }
                self.call_gemini(&key, model, prompt).await
            }
            ReviewProvider::Ollama { url, model } => self.call_ollama(url, model, prompt).await,
            ReviewProvider::Pollinations { model } => self.call_pollinations(model, prompt).await,
        }
    }

    /// Call an OpenAI-compatible `/chat/completions` endpoint.
    async fn call_chat_completions(
        &self,
        url: &str,
        api_key: &str,
        model: &str,
        prompt: &str,
        referer: Option<&str>,
    ) -> Result<(String, usize), String> {
        let body = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": REVIEW_SYSTEM_PROMPT
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.1,
            "max_tokens": 4096
        });

        let mut req = self
            .http
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json");

        if let Some(r) = referer {
            req = req.header("HTTP-Referer", r);
            req = req.header("X-Title", "Vox Code Review");
        }

        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("HTTP {status}: {text}"));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON parse error: {e}"))?;

        let text = json
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let tokens = json
            .pointer("/usage/total_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        if text.is_empty() {
            return Err("Empty response from provider".to_string());
        }

        Ok((text, tokens))
    }

    /// Call the Gemini generateContent endpoint.
    async fn call_gemini(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
    ) -> Result<(String, usize), String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );
        let full_prompt = format!("{}\n\n{}", REVIEW_SYSTEM_PROMPT, prompt);
        let body = serde_json::json!({
            "contents": [{ "parts": [{ "text": full_prompt }] }],
            "generationConfig": { "temperature": 0.1, "maxOutputTokens": 4096 }
        });

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Gemini HTTP {status}: {text}"));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| format!("JSON: {e}"))?;
        let text = json
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if text.is_empty() {
            return Err("Empty Gemini response".to_string());
        }
        // Gemini doesn't return token counts in the same way — estimate
        let tokens = text.len() / 4;
        Ok((text, tokens))
    }

    /// Call local Ollama /api/generate.
    async fn call_ollama(
        &self,
        url: &str,
        model: &str,
        prompt: &str,
    ) -> Result<(String, usize), String> {
        let full_prompt = format!("{}\n\n{}", REVIEW_SYSTEM_PROMPT, prompt);
        let body = serde_json::json!({
            "model": model,
            "prompt": full_prompt,
            "stream": false,
            "options": { "temperature": 0.1, "num_predict": 4096 }
        });

        let resp = self
            .http
            .post(format!("{}/api/generate", url))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Ollama HTTP error: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Ollama HTTP {}", resp.status()));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| format!("JSON: {e}"))?;
        let text = json
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tokens = json.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        if text.is_empty() {
            return Err("Empty Ollama response".to_string());
        }
        Ok((text, tokens))
    }

    /// Call Pollinations.ai text endpoint.
    async fn call_pollinations(
        &self,
        model: &str,
        prompt: &str,
    ) -> Result<(String, usize), String> {
        let url = "https://text.pollinations.ai/";
        let body = serde_json::json!({
            "messages": [
                { "role": "system", "content": REVIEW_SYSTEM_PROMPT },
                { "role": "user", "content": prompt }
            ],
            "model": model,
            "private": true,
            "nologo": true
        });

        let resp = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Pollinations HTTP error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Pollinations HTTP {status}: {text}"));
        }

        let text = resp.text().await.map_err(|e| format!("Text read: {e}"))?;
        if text.trim().is_empty() {
            return Err("Empty Pollinations response".to_string());
        }
        // Pollinations doesn't return token counts — estimate
        let tokens = text.len() / 4;
        Ok((text, tokens))
    }
}

fn resolve_key(stored: &str, env_var: &str) -> String {
    if stored.is_empty() {
        std::env::var(env_var).unwrap_or_default()
    } else {
        stored.to_string()
    }
}
