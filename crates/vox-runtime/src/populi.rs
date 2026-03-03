//! # Populi LLM Client
//!
//! `PopuliClient` communicates with the Populi LLM (local or remote) for
//! code generation, text embedding, classification, and fine-tuning data submission.
//!
//! ## Modes
//! - **Local**: `http://localhost:11434` (Ollama-compatible API)
//! - **Remote**: `https://raw.githubusercontent.com/brbrainerd/vox/main/populi` (configurable)

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PopuliError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Model not available: {0}")]
    ModelNotAvailable(String),
    #[error("Rate limited, retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },
    #[error("Malformed response: {0}")]
    MalformedResponse(String),
}

/// Configuration for connecting to Populi.
#[derive(Debug, Clone)]
pub struct PopuliConfig {
    /// Base URL of the Populi API (e.g., `http://localhost:11434` or `https://raw.githubusercontent.com/brbrainerd/vox/main/populi`).
    pub base_url: String,
    /// Optional API key for remote authentication.
    pub api_key: Option<String>,
    /// Model identifier (e.g., `populi-v1`, `codellama:7b`).
    pub model: String,
    /// Temperature for generation (0.0–2.0).
    pub temperature: f64,
    /// Max tokens for generation.
    pub max_tokens: u32,
}

impl Default for PopuliConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            api_key: None,
            model: "default-model".to_string(),
            temperature: 0.7,
            max_tokens: 2048,
        }
    }
}

impl PopuliConfig {
    /// Construct from environment variables.
    pub fn from_env() -> Self {
        Self {
            base_url: std::env::var("POPULI_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            api_key: std::env::var("POPULI_API_KEY").ok(),
            model: std::env::var("POPULI_MODEL").unwrap_or_else(|_| "default-model".to_string()),
            temperature: std::env::var("POPULI_TEMPERATURE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.7),
            max_tokens: std::env::var("POPULI_MAX_TOKENS")
                .ok()
                .and_then(|s| s.parse().ok())
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
    pub response: String,
    pub model: String,
    #[serde(default)]
    pub eval_count: u64,
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
    pub embedding: Vec<f64>,
}

/// Classification result.
#[derive(Debug, Clone)]
pub struct Classification {
    pub label: String,
    pub confidence: f64,
}

/// The main Populi LLM client.
pub struct PopuliClient {
    config: PopuliConfig,
    http: reqwest::Client,
}

impl PopuliClient {
    /// Create a new client from config.
    pub fn new(config: PopuliConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
        }
    }

    /// Create from environment variables.
    pub fn from_env() -> Self {
        Self::new(PopuliConfig::from_env())
    }

    /// Generate text completion from a prompt.
    /// Prompts are canonicalized (normalized, order-invariant) before sending to reduce order bias.
    pub async fn generate(&self, prompt: &str) -> Result<GenerateResponse, PopuliError> {
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
            return Err(PopuliError::RateLimited {
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
    ) -> Result<String, PopuliError> {
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
    pub async fn embed(&self, text: &str) -> Result<Vec<f64>, PopuliError> {
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
    ) -> Result<Classification, PopuliError> {
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
    ) -> Result<(), PopuliError> {
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
    ) -> Result<String, PopuliError> {
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
