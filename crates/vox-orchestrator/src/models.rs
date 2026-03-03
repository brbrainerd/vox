use crate::config::CostPreference;
use crate::types::TaskCategory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Specification for an LLM model in the registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub id: String,
    pub provider: String,
    /// Which API endpoint to use: "google_direct", "openrouter", or "ollama".
    pub provider_type: ProviderType,
    pub max_tokens: u64,
    /// Simplified cost metric representing aggregate cost per 1000 tokens.
    pub cost_per_1k: f64,
    /// Whether this model is free (no per-token cost).
    pub is_free: bool,
    pub strengths: Vec<String>,
}

/// Provider routing type — determines which API endpoint to call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// Google AI Studio direct (generativelanguage.googleapis.com)
    GoogleDirect,
    /// OpenRouter API (openrouter.ai/api/v1)
    OpenRouter,
    /// Local Ollama instance (localhost:11434)
    Ollama,
}

/// Configuration wrapper for models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub models: Vec<ModelSpec>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            models: vec![
                // ── Free Tier (Google AI Studio direct, no credit card) ──
                ModelSpec {
                    id: "gemini-2.0-flash-lite".to_string(),
                    provider: "google".to_string(),
                    provider_type: ProviderType::GoogleDirect,
                    max_tokens: 1_000_000,
                    cost_per_1k: 0.0,
                    is_free: true,
                    strengths: vec!["codegen".to_string(), "parsing".to_string()],
                },
                ModelSpec {
                    id: "gemini-2.5-flash-preview".to_string(),
                    provider: "google".to_string(),
                    provider_type: ProviderType::GoogleDirect,
                    max_tokens: 1_000_000,
                    cost_per_1k: 0.0,
                    is_free: true,
                    strengths: vec!["codegen".to_string(), "review".to_string(), "parsing".to_string()],
                },
                ModelSpec {
                    id: "gemini-2.5-pro".to_string(),
                    provider: "google".to_string(),
                    provider_type: ProviderType::GoogleDirect,
                    max_tokens: 2_000_000,
                    cost_per_1k: 0.0,
                    is_free: true,
                    strengths: vec!["codegen".to_string(), "debugging".to_string(), "review".to_string(), "research".to_string()],
                },
                // ── Free Tier (OpenRouter :free, requires free API key) ──
                ModelSpec {
                    id: "mistral/devstral-2-2512:free".to_string(),
                    provider: "mistral".to_string(),
                    provider_type: ProviderType::OpenRouter,
                    max_tokens: 262_000,
                    cost_per_1k: 0.0,
                    is_free: true,
                    strengths: vec!["codegen".to_string(), "refactoring".to_string()],
                },
                ModelSpec {
                    id: "qwen/qwen3-coder:free".to_string(),
                    provider: "qwen".to_string(),
                    provider_type: ProviderType::OpenRouter,
                    max_tokens: 262_000,
                    cost_per_1k: 0.0,
                    is_free: true,
                    strengths: vec!["codegen".to_string()],
                },
                ModelSpec {
                    id: "meta-llama/llama-4-scout:free".to_string(),
                    provider: "meta".to_string(),
                    provider_type: ProviderType::OpenRouter,
                    max_tokens: 512_000,
                    cost_per_1k: 0.0,
                    is_free: true,
                    strengths: vec!["review".to_string(), "parsing".to_string()],
                },
                ModelSpec {
                    id: "moonshotai/kimi-k2:free".to_string(),
                    provider: "moonshot".to_string(),
                    provider_type: ProviderType::OpenRouter,
                    max_tokens: 200_000,
                    cost_per_1k: 0.0,
                    is_free: true,
                    strengths: vec!["codegen".to_string(), "research".to_string()],
                },
                // ── Paid Tier (OpenRouter, auto-selected when budget allows) ──
                ModelSpec {
                    id: "deepseek/deepseek-v3.2".to_string(),
                    provider: "deepseek".to_string(),
                    provider_type: ProviderType::OpenRouter,
                    max_tokens: 128_000,
                    cost_per_1k: 0.00027,
                    is_free: false,
                    strengths: vec!["codegen".to_string(), "debugging".to_string(), "logic".to_string()],
                },
                ModelSpec {
                    id: "anthropic/claude-sonnet-4.5".to_string(),
                    provider: "anthropic".to_string(),
                    provider_type: ProviderType::OpenRouter,
                    max_tokens: 200_000,
                    cost_per_1k: 0.003,
                    is_free: false,
                    strengths: vec!["codegen".to_string(), "refactoring".to_string(), "review".to_string()],
                },
                ModelSpec {
                    id: "openai/gpt-5".to_string(),
                    provider: "openai".to_string(),
                    provider_type: ProviderType::OpenRouter,
                    max_tokens: 256_000,
                    cost_per_1k: 0.005,
                    is_free: false,
                    strengths: vec!["review".to_string(), "parsing".to_string(), "research".to_string()],
                },
                ModelSpec {
                    id: "openai/o3".to_string(),
                    provider: "openai".to_string(),
                    provider_type: ProviderType::OpenRouter,
                    max_tokens: 200_000,
                    cost_per_1k: 0.010,
                    is_free: false,
                    strengths: vec!["debugging".to_string(), "logic".to_string()],
                },
            ],
        }
    }
}


/// A registry managing available agent models and model routing.
#[derive(Debug, Clone, Default)]
pub struct ModelRegistry {
    models: HashMap<String, ModelSpec>,
    agent_overrides: HashMap<u64, String>,
}

impl ModelRegistry {
    /// Create a new model registry, loading from the configuration file or falling back to defaults.
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
            agent_overrides: HashMap::new(),
        };

        // Try to load from models.toml in the config directory
        let model_config = if let Some(mut config_path) = vox_db::paths::config_dir() {
            config_path.push("models.toml");
            if config_path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&config_path) {
                    toml::from_str(&contents).unwrap_or_else(|_| ModelConfig::default())
                } else {
                    ModelConfig::default()
                }
            } else {
                let default_config = ModelConfig::default();
                // Create config dir if needed and write default file
                if let Some(parent) = config_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                    if let Ok(toml_str) = toml::to_string_pretty(&default_config) {
                        let _ = std::fs::write(&config_path, toml_str);
                    }
                }
                default_config
            }
        } else {
            ModelConfig::default()
        };

        for model in model_config.models {
            registry.register(model);
        }

        registry
    }

    /// Register a new model specification.
    pub fn register(&mut self, spec: ModelSpec) {
        self.models.insert(spec.id.clone(), spec);
    }

    /// Return the best model for a given task category and complexity.
    /// If preference is Economy, it will favor models with lower cost_per_1k.
    /// If complexity is low, it will favor cheaper models to save budget.
    pub fn best_for(
        &self,
        task_type: TaskCategory,
        complexity: u8,
        preference: CostPreference,
    ) -> Option<ModelSpec> {
        // Automatic Dynamic Tiering: Low complexity tasks don't need premium models
        let effective_pref = if complexity <= 3 {
            CostPreference::Economy
        } else {
            preference
        };

        if effective_pref == CostPreference::Economy {
            // Find the cheapest model that has the relevant strength for the category
            let strength = match task_type {
                TaskCategory::CodeGen => "codegen",
                TaskCategory::Testing => "codegen",
                TaskCategory::Debugging => "debugging",
                TaskCategory::TypeChecking => "logic",
                TaskCategory::Research => "research",
                TaskCategory::Parsing => "parsing",
                TaskCategory::Review => "review",
            };

            return self
                .models
                .values()
                .filter(|m| m.strengths.iter().any(|s| s == strength))
                .min_by(|a, b| a.cost_per_1k.partial_cmp(&b.cost_per_1k).unwrap())
                .cloned()
                .or_else(|| self.cheapest());
        }

        // Premium routing: best paid model per task type
        match task_type {
            TaskCategory::CodeGen => self.models.get("anthropic/claude-sonnet-4.5").cloned(),
            TaskCategory::Testing => self.models.get("deepseek/deepseek-v3.2").cloned(),
            TaskCategory::Debugging => self.models.get("openai/o3").cloned(),
            TaskCategory::TypeChecking => self.models.get("openai/o3").cloned(),
            TaskCategory::Research => self.models.get("openai/gpt-5").cloned(),
            TaskCategory::Parsing => self.models.get("gemini-2.5-flash-preview").cloned(),
            TaskCategory::Review => self.models.get("openai/gpt-5").cloned(),
        }
    }

    /// Return the best free model for a given task category.
    pub fn best_free_for(&self, task_type: TaskCategory) -> Option<ModelSpec> {
        let strength = match task_type {
            TaskCategory::CodeGen => "codegen",
            TaskCategory::Testing => "codegen",
            TaskCategory::Debugging => "debugging",
            TaskCategory::TypeChecking => "logic",
            TaskCategory::Research => "research",
            TaskCategory::Parsing => "parsing",
            TaskCategory::Review => "review",
        };

        self.models
            .values()
            .filter(|m| m.is_free && m.strengths.iter().any(|s| s == strength))
            .max_by_key(|m| m.max_tokens)
            .cloned()
            .or_else(|| self.cheapest_free())
    }

    /// Return all free models in the registry.
    pub fn free_models(&self) -> Vec<ModelSpec> {
        self.models.values().filter(|m| m.is_free).cloned().collect()
    }

    /// Return the cheapest free model.
    pub fn cheapest_free(&self) -> Option<ModelSpec> {
        self.models.values().filter(|m| m.is_free).next().cloned()
    }

    /// Return the absolute cheapest model in the registry.
    pub fn cheapest(&self) -> Option<ModelSpec> {
        self.models
            .values()
            .min_by(|a, b| a.cost_per_1k.partial_cmp(&b.cost_per_1k).unwrap())
            .cloned()
    }

    /// Calculate the cost estimate for predicting use of a model for a certain amount of tokens.
    pub fn cost_estimate(&self, model_id: &str, estimated_tokens: u64) -> Option<f64> {
        self.models
            .get(model_id)
            .map(|spec| (estimated_tokens as f64 / 1000.0) * spec.cost_per_1k)
    }

    /// List all registered models.
    pub fn list_models(&self) -> Vec<ModelSpec> {
        self.models.values().cloned().collect()
    }

    /// Get a specific model definition by ID.
    pub fn get(&self, model_id: &str) -> Option<ModelSpec> {
        self.models.get(model_id).cloned()
    }

    /// Set an explicit model override for a specific agent.
    pub fn set_override(&mut self, agent_id: u64, model_id: String) {
        self.agent_overrides.insert(agent_id, model_id);
    }

    /// Check if there's an active model override for an agent.
    pub fn get_override(&self, agent_id: u64) -> Option<String> {
        self.agent_overrides.get(&agent_id).cloned()
    }

    #[cfg(feature = "runtime")]
    pub fn get_llm_config(
        &self,
        task_type: TaskCategory,
        complexity: u8,
        preference: CostPreference,
    ) -> Option<vox_runtime::LlmConfig> {
        self.best_for(task_type, complexity, preference)
            .map(|spec| vox_runtime::LlmConfig {
                provider: spec.provider.clone(),
                model: spec.id.clone(),
                base_url: None,
                api_key: None,
                temperature: None,
                max_tokens: Some(spec.max_tokens),
                response_format: None,
            })
    }
}
