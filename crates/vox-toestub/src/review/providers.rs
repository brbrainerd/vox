use serde::{Deserialize, Serialize};

/// AI provider for code review — superset of `AiProvider`, with OpenRouter
/// and OpenAI-compatible endpoints added.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum ReviewProvider {
    /// OpenRouter — aggregator supporting Claude, GPT-4o, Gemini Pro, and free models.
    /// Uses the OpenAI-compatible `/chat/completions` API.
    /// Set `OPENROUTER_API_KEY` env var or provide `api_key` directly.
    OpenRouter {
        #[serde(default)]
        api_key: String,
        /// Model identifier, e.g. `"anthropic/claude-3.5-sonnet"`.
        #[serde(default = "default_openrouter_model")]
        model: String,
        /// Your site URL (required by OpenRouter TOS, used in `HTTP-Referer`).
        #[serde(default = "default_site_url")]
        site_url: String,
    },
    /// OpenAI-compatible endpoint — defaults to api.openai.com but can be
    /// overridden for local servers (LMStudio, vLLM, etc.).
    OpenAi {
        #[serde(default)]
        api_key: String,
        #[serde(default = "default_openai_model")]
        model: String,
        /// Base URL, e.g. `"https://api.openai.com/v1"`.
        #[serde(default = "default_openai_base_url")]
        base_url: String,
    },
    /// Google Gemini Flash free tier.
    Gemini {
        #[serde(default)]
        api_key: String,
        #[serde(default = "default_gemini_model")]
        model: String,
    },
    /// Local Ollama instance — zero auth, zero cost.
    Ollama {
        #[serde(default = "default_ollama_url")]
        url: String,
        #[serde(default = "default_ollama_model")]
        model: String,
    },
    /// Pollinations.ai — always available, no auth.
    Pollinations {
        #[serde(default = "default_pollinations_model")]
        model: String,
    },
}

pub fn default_openrouter_model() -> String {
    "anthropic/claude-3.5-sonnet".to_string()
}
pub fn default_site_url() -> String {
    "https://github.com/brbrainerd/vox".to_string()
}
pub fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}
pub fn default_openai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}
pub fn default_gemini_model() -> String {
    "gemini-2.5-flash".to_string()
}
pub fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
pub fn default_ollama_model() -> String {
    "codellama".to_string()
}
pub fn default_pollinations_model() -> String {
    "openai".to_string()
}

impl ReviewProvider {
    /// Human-readable name.
    pub fn name(&self) -> &str {
        match self {
            ReviewProvider::OpenRouter { model, .. } => model.as_str(),
            ReviewProvider::OpenAi { model, .. } => model.as_str(),
            ReviewProvider::Gemini { .. } => "Gemini Flash (free)",
            ReviewProvider::Ollama { model, .. } => model.as_str(),
            ReviewProvider::Pollinations { .. } => "Pollinations.ai (free)",
        }
    }

    /// Whether this provider requires an API key.
    pub fn requires_key(&self) -> bool {
        matches!(
            self,
            ReviewProvider::OpenRouter { .. }
                | ReviewProvider::OpenAi { .. }
                | ReviewProvider::Gemini { .. }
        )
    }
}

/// Build the provider cascade from environment variables and local probing.
/// Returns providers in priority order.
pub fn auto_discover_providers() -> Vec<ReviewProvider> {
    let mut providers = Vec::new();

    // 1. OpenRouter (supports Claude, free models, etc.)
    let or_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    if !or_key.is_empty() {
        let model =
            std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| default_openrouter_model());
        providers.push(ReviewProvider::OpenRouter {
            api_key: or_key,
            model,
            site_url: default_site_url(),
        });
    }

    // 2. OpenAI-compatible
    let oai_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    if !oai_key.is_empty() {
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| default_openai_model());
        let base_url =
            std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| default_openai_base_url());
        providers.push(ReviewProvider::OpenAi {
            api_key: oai_key,
            model,
            base_url,
        });
    }

    // 3. Gemini
    let gem_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
    if !gem_key.is_empty() {
        let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| default_gemini_model());
        providers.push(ReviewProvider::Gemini {
            api_key: gem_key,
            model,
        });
    }

    // 4. Ollama (probe with a short timeout)
    let ollama_url = std::env::var("OLLAMA_URL").unwrap_or_else(|_| default_ollama_url());
    if probe_ollama(&ollama_url) {
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| default_ollama_model());
        providers.push(ReviewProvider::Ollama {
            url: ollama_url,
            model,
        });
    }

    // 5. Pollinations — always available
    providers.push(ReviewProvider::Pollinations {
        model: default_pollinations_model(),
    });

    providers
}

pub fn probe_ollama(url: &str) -> bool {
    let probe_url = format!("{}/api/tags", url);
    std::process::Command::new("curl")
        .args([
            "--silent",
            "--max-time",
            "2",
            "-o",
            "NUL", // Use NUL on Windows
            "-w",
            "%{http_code}",
            &probe_url,
        ])
        .output()
        .map(|o| {
            let code = String::from_utf8_lossy(&o.stdout);
            code.trim() == "200"
        })
        .unwrap_or(false)
}
