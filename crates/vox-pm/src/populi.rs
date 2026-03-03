use serde::{Deserialize, Serialize};

/// Client for communicating with local/remote Populi LLM services.
/// Now uses actual HTTP requests instead of hardcoded mock responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopuliClient {
    pub endpoint: String,
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_model() -> String {
    "populi-v1".to_string()
}
fn default_timeout_secs() -> u64 {
    30
}

/// Request body for the generation endpoint.
#[derive(Debug, Serialize)]
struct GenerateRequest<'a> {
    prompt: &'a str,
    model: &'a str,
    max_tokens: u32,
}

/// Response from the generation endpoint.
#[derive(Debug, Deserialize)]
struct GenerateResponse {
    text: String,
}

/// Request body for the embedding endpoint.
#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    text: &'a str,
    model: &'a str,
}

/// Response from the embedding endpoint.
#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

/// Request body for the classification endpoint.
#[derive(Debug, Serialize)]
struct ClassifyRequest<'a> {
    text: &'a str,
    model: &'a str,
}

/// Response from the classification endpoint.
#[derive(Debug, Deserialize)]
struct ClassifyResponse {
    label: String,
    #[allow(dead_code)]
    confidence: f32,
}

/// Request body for the fine-tune submission endpoint.
#[derive(Debug, Serialize)]
struct FineTuneRequest {
    dataset: Vec<u8>,
    model: String,
}

/// Response from the fine-tune submission endpoint.
#[derive(Debug, Deserialize)]
struct FineTuneResponse {
    job_id: String,
}

impl PopuliClient {
    pub fn new(endpoint: String, api_key: String) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            api_key,
            model: default_model(),
            timeout_secs: default_timeout_secs(),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    fn build_client(&self) -> Result<reqwest::Client, String> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))
    }

    /// Generate an LLM response from a prompt.
    pub async fn generate(&self, prompt: &str) -> Result<String, String> {
        let client = self.build_client()?;
        let url = format!("{}/v1/generate", self.endpoint);

        let body = GenerateRequest {
            prompt,
            model: &self.model,
            max_tokens: 2048,
        };

        let resp = client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Populi API error ({status}): {text}"));
        }

        let result: GenerateResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {e}"))?;

        Ok(result.text)
    }

    /// Generate embeddings for a text snippet.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let client = self.build_client()?;
        let url = format!("{}/v1/embed", self.endpoint);

        let body = EmbedRequest {
            text,
            model: &self.model,
        };

        let resp = client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Populi API error ({status}): {text}"));
        }

        let result: EmbedResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {e}"))?;

        Ok(result.embedding)
    }

    /// Classify a text input (e.g. for safety or intent).
    pub async fn classify(&self, text: &str) -> Result<String, String> {
        let client = self.build_client()?;
        let url = format!("{}/v1/classify", self.endpoint);

        let body = ClassifyRequest {
            text,
            model: &self.model,
        };

        let resp = client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Populi API error ({status}): {text}"));
        }

        let result: ClassifyResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {e}"))?;

        Ok(result.label)
    }

    /// Submit a dataset for fine-tuning.
    pub async fn fine_tune_submit(&self, dataset: &[u8]) -> Result<String, String> {
        let client = self.build_client()?;
        let url = format!("{}/v1/fine-tune", self.endpoint);

        let body = FineTuneRequest {
            dataset: dataset.to_vec(),
            model: self.model.clone(),
        };

        let resp = client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Populi API error ({status}): {text}"));
        }

        let result: FineTuneResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {e}"))?;

        Ok(result.job_id)
    }
}
