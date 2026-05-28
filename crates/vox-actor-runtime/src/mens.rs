//! # Mens LLM Client
//!
//! `PopuliClient` communicates with the Mens LLM (local or remote) for
//! code generation, text embedding, classification, and fine-tuning data submission.
//! The type keeps the historical `Populi*` naming because environment and control-plane
//! compatibility still use `POPULI_*`; the module path (`mens`) is the domain-facing name.
//!
//! ## Modes
//! - **Local**: `http://localhost:11434` (Ollama-compatible API)
//! - **Remote**: `https://raw.githubusercontent.com/vox-foundation/vox/main/mens` (configurable)

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned by [`PopuliClient`] HTTP calls and response parsing.
#[derive(Debug, Error)]
pub enum MensError {
    /// Underlying `reqwest` failure (DNS, TLS, timeout, etc.).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// Server responded but the requested model is not loaded or unknown.
    #[error("Model not available: {0}")]
    ModelNotAvailable(String),
    /// HTTP 429 or similar; caller may backoff using `retry_after_ms`.
    #[error("Rate limited, retry after {retry_after_ms}ms")]
    RateLimited {
        /// Suggested delay before retry (milliseconds); heuristic when server omits header.
        retry_after_ms: u64,
    },
    /// JSON shape did not match expected Mens/Ollama response.
    #[error("Malformed response: {0}")]
    MalformedResponse(String),
}

/// Configuration for connecting to Mens.
#[derive(Debug, Clone)]
pub struct MensConfig {
    /// Base URL of the Mens API (e.g., `http://localhost:11434` or `https://raw.githubusercontent.com/vox-foundation/vox/main/mens`).
    pub base_url: String,
    /// Optional API key for remote authentication.
    pub api_key: Option<String>,
    /// Model identifier (e.g., `mens-v1`, `codellama:7b`).
    pub model: String,
    /// Temperature for generation (0.0–2.0).
    pub temperature: f64,
    /// Max tokens for generation.
    pub max_tokens: u32,
}

impl Default for MensConfig {
    fn default() -> Self {
        Self {
            base_url: vox_config::LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT.to_string(),
            api_key: None,
            model: "default-model".to_string(),
            temperature: 0.7,
            max_tokens: 2048,
        }
    }
}

impl MensConfig {
    /// Construct from environment variables.
    pub fn from_env() -> Self {
        Self {
            base_url: vox_config::inference::local_ollama_populi_base_url(),
            api_key: vox_secrets::resolve_secret(vox_secrets::SecretId::PopuliApiKey)
                .expose()
                .map(|s| s.to_string()),
            model: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxPopuliModel)
                .expose()
                .unwrap_or("default-model")
                .to_string(),
            temperature: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxPopuliTemperature)
                .expose()
                .and_then(|s: &str| s.parse().ok())
                .unwrap_or(0.7),
            max_tokens: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxPopuliMaxTokens)
                .expose()
                .and_then(|s: &str| s.parse().ok())
                .unwrap_or(2048),
        }
    }
}

/// A generation request.
#[derive(Debug, Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    options: GenerateOptions,
}

#[derive(Debug, Serialize)]
struct GenerateOptions {
    temperature: f64,
    num_predict: u32,
}

/// A generation response.
#[derive(Debug, Deserialize)]
pub struct GenerateResponse {
    /// Generated completion text from Mens/Ollama.
    pub response: String,
    /// Model name echoed by the server.
    pub model: String,
    /// Token or evaluation count when the API provides it.
    #[serde(default)]
    pub eval_count: u64,
    /// Evaluation duration in nanoseconds when reported.
    #[serde(default)]
    pub eval_duration: u64,
}

/// An embedding request.
#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

/// An embedding response.
#[derive(Debug, Deserialize)]
pub struct EmbedResponse {
    /// Embedding vector for the input text.
    pub embedding: Vec<f64>,
}

/// Classification result.
#[derive(Debug, Clone)]
pub struct Classification {
    /// Chosen category label from the model output.
    pub label: String,
    /// Confidence score in `[0, 1]` (heuristic for single-shot classify).
    pub confidence: f64,
}

/// The main Mens LLM client.
pub struct PopuliClient {
    config: MensConfig,
    http: reqwest::Client,
}

impl PopuliClient {
    /// Create a new client from config.
    pub fn new(config: MensConfig) -> Self {
        Self {
            http: vox_http_client::client(),
            config,
        }
    }

    /// Create from environment variables.
    pub fn from_env() -> Self {
        Self::new(MensConfig::from_env())
    }

    /// Probe `/api/tags` (and `/api/version` for GPU hints) on [`MensConfig::base_url`].
    pub async fn probe_capabilities(&self) -> crate::inference_env::PopuliCapabilitySnapshot {
        crate::inference_env::probe_populi_capabilities(&self.config.base_url).await
    }

    /// Generate text completion from a prompt.
    /// Prompts are canonicalized (normalized, order-invariant) before sending to reduce order bias.
    pub async fn generate(&self, prompt: &str) -> Result<GenerateResponse, MensError> {
        let canonical = crate::prompt_canonical::canonicalize_simple(prompt);
        let prompt = canonical.as_str();
        let req = GenerateRequest {
            model: &self.config.model,
            prompt,
            stream: false,
            options: GenerateOptions {
                temperature: self.config.temperature,
                num_predict: self.config.max_tokens,
            },
        };

        let resp = self
            .http
            .post(format!("{}/api/generate", self.config.base_url))
            .json(&req)
            .send()
            .await?;

        if resp.status().as_u16() == 429 {
            return Err(MensError::RateLimited {
                retry_after_ms: 1000,
            });
        }

        let body = resp.json::<GenerateResponse>().await?;
        Ok(body)
    }

    /// Generate a code completion specifically for Vox code.
    pub async fn generate_code(
        &self,
        context: &str,
        instruction: &str,
    ) -> Result<String, MensError> {
        let instruction = crate::prompt_canonical::canonicalize_simple(instruction);
        let prompt = format!(
            "You are a Vox programming language expert. Vox is an AI-native language.\n\n\
             Context:\n```vox\n{context}\n```\n\n\
             Instruction: {instruction}\n\n\
             Respond with ONLY the Vox code, no explanation:"
        );
        let resp = self.generate(&prompt).await?;
        Ok(resp.response.trim().to_string())
    }

    /// Compute text embedding for RAG (Retrieval-Augmented Generation).
    pub async fn embed(&self, text: &str) -> Result<Vec<f64>, MensError> {
        let req = EmbedRequest {
            model: &self.config.model,
            prompt: text,
        };

        let resp = self
            .http
            .post(format!("{}/api/embeddings", self.config.base_url))
            .json(&req)
            .send()
            .await?;

        let body = resp.json::<EmbedResponse>().await?;
        Ok(body.embedding)
    }

    /// Classify input text into categories.
    pub async fn classify(
        &self,
        text: &str,
        categories: &[&str],
    ) -> Result<Classification, MensError> {
        let cats = categories.join(", ");
        let prompt = format!(
            "Classify the following text into exactly ONE of these categories: [{cats}]\n\n\
             Text: \"{text}\"\n\n\
             Reply with ONLY the category name, nothing else:"
        );
        let resp = self.generate(&prompt).await?;
        let label = resp.response.trim().to_string();
        Ok(Classification {
            label,
            confidence: 1.0, // single-shot classification
        })
    }

    /// Submit training data for fine-tuning (RLHF pair).
    pub async fn fine_tune_submit(
        &self,
        prompt: &str,
        chosen: &str,
        rejected: Option<&str>,
    ) -> Result<(), MensError> {
        let body = serde_json::json!({
            "prompt": prompt,
            "chosen": chosen,
            "rejected": rejected,
            "model": self.config.model,
        });

        self.http
            .post(format!("{}/api/training/submit", self.config.base_url))
            .json(&body)
            .send()
            .await?;

        Ok(())
    }

    /// Perform a RAG query: embed the query, find similar snippets, augment prompt.
    pub async fn rag_query(
        &self,
        query: &str,
        context_snippets: &[String],
    ) -> Result<String, MensError> {
        let context = context_snippets
            .iter()
            .enumerate()
            .map(|(i, s)| format!("--- Snippet {} ---\n{}", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n\n");

        let augmented_prompt = format!(
            "Using the following code snippets as reference:\n\n{context}\n\n\
             Answer the following question about Vox:\n{query}"
        );

        let resp = self.generate(&augmented_prompt).await?;
        Ok(resp.response)
    }
}
