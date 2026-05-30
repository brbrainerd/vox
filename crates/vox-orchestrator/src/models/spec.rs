//! LLM model specifications, capabilities, and routing keys.
//!
//! [`ModelRegistry`](crate::models::ModelRegistry) (in `registry.rs`) uses these types for task-category routing.

use crate::types::TaskCategory;
use crate::usage::LlmUsageKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Records which data source last set the pricing fields on a [`ModelSpec`].
///
/// Priority (highest → lowest): `Telemetry` > `LiteLLM` > `OpenRouter` | `AnthropicDirect` > `UserConfig` > `Bootstrap`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PricingSource {
    /// Compile-time bootstrap JSON (may be stale). Used only as a cold-start fallback.
    #[default]
    Bootstrap,
    /// We know this model exists but have no pricing data.
    Unknown,
    /// Fetched from OpenRouter `/api/v1/models` at runtime.
    OpenRouter,
    /// Fetched directly from the Anthropic `/v1/models` API (key-gated).
    /// Model discovery only; pricing is still filled in by the LiteLLM oracle.
    AnthropicDirect,
    /// Supplemented from the LiteLLM `model_prices_and_context_window.json` oracle.
    /// Fills gaps OpenRouter doesn't expose (cache-hit pricing, Anthropic, Google).
    LiteLLM,
    /// User's local `models.toml` override.
    UserConfig,
    /// Calibrated from observed telemetry rollup (highest trust — observed > catalog).
    Telemetry,
}

use super::generated::{ModelTier, StrengthTag};

/// Rich capabilities for a model, imported from DeI and the OpenRouter /models catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ModelCapabilities {
    pub supports_json: bool,
    pub supports_vision: bool,
    #[serde(default = "vox_config::serde_defaults::default_true")]
    pub supports_native_tools: bool,
    #[serde(default)]
    pub supports_tool_use: bool,
    #[serde(default)]
    pub supports_reasoning: bool,
    #[serde(default)]
    pub supports_web_search: bool,
    #[serde(default)]
    pub supports_image_generation: bool,
    #[serde(default)]
    pub supports_audio_input: bool,
    #[serde(default)]
    pub supports_audio_output: bool,
    /// Whether the model accepts file attachments (PDFs, documents, etc.) as input.
    #[serde(default)]
    pub supports_file_input: bool,
    /// Whether the model can consume and produce JSON-Lines streams.
    #[serde(default)]
    pub supports_jsonl: bool,
    /// Whether this model is trusted to author Vox language source
    /// (`VoxScript` artifact forms). Registry-authoring flag, NOT an
    /// OpenRouter-advertised routing capability — seeded `true` only for
    /// MENS models, defaults `false` for everything else. Operators can
    /// additionally opt models in via the `[audit.route] vox_capable_models`
    /// allowlist override (see `vox-cli` `audit_route::resolve_vox_capability`).
    #[serde(default)]
    pub writes_vox: bool,
    pub max_context: u64,
    pub tier: ModelTier,
    /// Provider-reported RPM limit (e.g. from OpenRouter `per_request_limits`).
    pub rate_limit_rpm: Option<u32>,
    /// Provider-reported RPD limit (e.g. from OpenRouter `per_request_limits`).
    pub rate_limit_rpd: Option<u32>,
    /// Median response latency in milliseconds from catalog metadata (p50).
    #[serde(default)]
    pub latency_p50_ms: Option<u32>,
    /// Whether the provider applies content moderation to outputs.
    #[serde(default)]
    pub is_moderated: bool,
    /// Provider-reported uptime score 0.0–1.0 (1.0 = fully available).
    #[serde(default)]
    pub uptime_score: Option<f32>,
}

impl ModelCapabilities {
    /// Whether this catalog row advertises the given routing [`Capability`](super::generated::Capability).
    #[must_use]
    pub fn supports(&self, cap: super::generated::Capability) -> bool {
        use super::generated::Capability::*;
        match cap {
            SupportsJson => self.supports_json,
            SupportsVision => self.supports_vision,
            SupportsToolUse => self.supports_tool_use || self.supports_native_tools,
            SupportsReasoning => self.supports_reasoning,
            SupportsWebSearch => self.supports_web_search,
            SupportsImageGeneration => self.supports_image_generation,
            SupportsAudioInput => self.supports_audio_input,
            SupportsAudioOutput => self.supports_audio_output,
        }
    }

    /// OR-merge OpenRouter / contract-inferred capability flags into this struct.
    pub fn merge_capability_flags(&mut self, flags: &super::generated::CapabilityFlags) {
        self.supports_json |= flags.supports_json;
        self.supports_vision |= flags.supports_vision;
        self.supports_tool_use |= flags.supports_tool_use;
        self.supports_reasoning |= flags.supports_reasoning;
        self.supports_web_search |= flags.supports_web_search;
        self.supports_image_generation |= flags.supports_image_generation;
        self.supports_audio_input |= flags.supports_audio_input;
        self.supports_audio_output |= flags.supports_audio_output;
    }
}

/// Specification for an LLM model in the registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    /// Stable model slug used in APIs and config (e.g. `gemini-2.0-flash-lite`).
    pub id: String,
    /// The unique system-wide slug
    #[serde(default)]
    pub canonical_slug: String,
    /// Provider namespace for billing and routing (`google`, `openrouter`, …).
    pub provider: String,
    /// Which API endpoint to use: "google_direct", "openrouter", or "ollama".
    pub provider_type: ProviderType,
    /// Advertised context window / max output budget in tokens.
    pub max_tokens: u64,
    /// Simplified cost metric representing aggregate cost per 1000 tokens.
    pub cost_per_1k: f64,
    #[serde(default)]
    pub cost_per_1k_input: f64,
    #[serde(default)]
    pub cost_per_1k_output: f64,
    /// Optional ground-truth cost observed from provider telemetry (blended).
    #[serde(default)]
    pub observed_cost_per_1k: Option<f64>,
    /// Per-1k-token cost for writing a new prompt-cache prefix (e.g. 1.25× normal on Anthropic).
    /// Zero means caching is not supported or the cost is not known.
    #[serde(default)]
    pub cache_creation_cost_per_1k: f64,
    /// Per-1k-token cost for a prompt-cache hit read (typically ~10% of input cost).
    /// Zero means caching is not supported or the cost is not known.
    #[serde(default)]
    pub cache_read_cost_per_1k: f64,
    /// Whether the provider supports prompt-prefix caching for this model.
    #[serde(default)]
    pub supports_prompt_caching: bool,
    /// Which data source last set the pricing fields on this spec.
    #[serde(default)]
    pub pricing_source: PricingSource,
    /// Whether this model is free (no per-token cost).
    pub is_free: bool,
    /// Tags describing fit (speed, reasoning, codegen) for heuristic routing.
    pub strengths: Vec<StrengthTag>,
    #[serde(default)]
    pub capabilities: ModelCapabilities,
    #[serde(default)]
    pub supported_parameters: Vec<String>,
}

pub use vox_orchestrator_types::ProviderType;

pub use vox_orchestrator_types::ChatRouteBackend as ModelRouteBackend;

/// Resolve the transport/backend lane for a concrete model spec.
#[must_use]
pub fn route_backend_for_model(spec: &ModelSpec) -> ModelRouteBackend {
    match spec.provider_type {
        ProviderType::Ollama => ModelRouteBackend::Ollama,
        ProviderType::VoxLocal => ModelRouteBackend::VoxLocal,
        ProviderType::PopuliMesh => ModelRouteBackend::PopuliMesh,
        ProviderType::GoogleDirect => ModelRouteBackend::GeminiDirect,
        ProviderType::OpenRouter => ModelRouteBackend::OpenRouter,
        ProviderType::Groq
        | ProviderType::Mistral
        | ProviderType::DeepSeek
        | ProviderType::Cerebras
        | ProviderType::SambaNova
        | ProviderType::Anthropic
        | ProviderType::HuggingFaceRouter
        | ProviderType::Custom(_) => {
            // P0 Fix: Map arbitrarily typed third-party providers (even those lacking '/') to
            // OpenRouter or a non-cascading endpoint. CascadeFallback on unknown IDs loops infinitely.
            ModelRouteBackend::OpenRouter
        }
    }
}

impl ModelSpec {
    /// Whether the model supports native web-search grounding.
    ///
    /// This is a convenience forwarder to [`ModelCapabilities::supports_web_search`] so callers
    /// can query the capability directly on the spec without drilling into `.capabilities`.
    #[must_use]
    #[inline]
    pub fn supports_web_search(&self) -> bool {
        self.capabilities.supports_web_search
    }

    /// Whether the model accepts file attachments as context.
    #[must_use]
    #[inline]
    pub fn supports_file_input(&self) -> bool {
        self.capabilities.supports_file_input
    }

    /// Whether the model supports JSON-Lines streaming output.
    #[must_use]
    #[inline]
    pub fn supports_jsonl(&self) -> bool {
        self.capabilities.supports_jsonl
    }

    /// Keys for daily quota rows in `provider_usage` (aligned with `usage` module limits; OpenRouter `:free` aggregate, Ollama `*`).
    #[must_use]
    pub fn llm_usage_key(&self) -> LlmUsageKey {
        match &self.provider_type {
            ProviderType::GoogleDirect => LlmUsageKey {
                provider: "google".to_string(),
                model: self.id.clone(),
            },
            ProviderType::OpenRouter => {
                let model = if self.is_free || self.id.contains(":free") {
                    ":free".to_string()
                } else {
                    self.id.clone()
                };
                LlmUsageKey {
                    provider: "openrouter".to_string(),
                    model,
                }
            }
            ProviderType::Ollama => LlmUsageKey {
                provider: "ollama".to_string(),
                model: "*".to_string(),
            },
            ProviderType::VoxLocal => LlmUsageKey {
                provider: "vox_local".to_string(),
                model: "*".to_string(),
            },
            ProviderType::PopuliMesh => LlmUsageKey {
                provider: "mens".to_string(),
                model: "*".to_string(),
            },
            ProviderType::Groq => LlmUsageKey {
                provider: "groq".to_string(),
                model: self.id.clone(),
            },
            ProviderType::Cerebras => LlmUsageKey {
                provider: "cerebras".to_string(),
                model: self.id.clone(),
            },
            ProviderType::Mistral => LlmUsageKey {
                provider: "mistral".to_string(),
                model: self.id.clone(),
            },
            ProviderType::DeepSeek => LlmUsageKey {
                provider: "deepseek".to_string(),
                model: self.id.clone(),
            },
            ProviderType::SambaNova => LlmUsageKey {
                provider: "sambanova".to_string(),
                model: self.id.clone(),
            },
            ProviderType::Anthropic => LlmUsageKey {
                provider: "anthropic".to_string(),
                model: self.id.clone(),
            },
            ProviderType::HuggingFaceRouter => LlmUsageKey {
                provider: "huggingface".to_string(),
                model: self.id.clone(),
            },
            ProviderType::Custom(_url) => LlmUsageKey {
                provider: "custom".to_string(),
                model: self.id.clone(),
            },
        }
    }
}

fn premium_alias_toml_default() -> HashMap<String, String> {
    let m = HashMap::new();
    let _ = std::hint::black_box(m.capacity());
    m
}

/// Configuration wrapper for models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelConfig {
    /// All models available to the orchestrator for this deployment.
    pub models: Vec<ModelSpec>,
    /// Optional premium model id per task bucket (`codegen`, `testing`, …). Empty = use ranked paid models.
    #[serde(default = "premium_alias_toml_default")]
    pub premium_alias: HashMap<String, String>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        let local_model = vox_secrets::resolve_secret(vox_secrets::SecretId::PopuliModel)
            .expose()
            .filter(|s: &&str| !s.trim().is_empty())
            .unwrap_or("default-model")
            .to_string();

        let bootstrap_json =
            include_str!("../../../../contracts/orchestration/model-catalog.bootstrap.v1.json");
        let mut models: Vec<ModelSpec> =
            serde_json::from_str(bootstrap_json).expect("Invalid bootstrap catalog");

        for m in &mut models {
            if m.id == "llama3:latest" && m.provider == "ollama" {
                m.id = local_model.clone();
                m.canonical_slug = format!("local/{}", local_model);
            }
            if m.capabilities.max_context == 0 {
                m.capabilities.max_context = m.max_tokens;
            }
        }

        Self {
            models,
            premium_alias: vox_config::load_model_routing_config().premium_alias,
        }
    }
}

use super::routing_table::route_for_category;

/// Maps [`TaskCategory`] to a `premium_alias` / routing strength key.
#[must_use]
pub fn task_category_premium_key(task_type: TaskCategory) -> &'static str {
    route_for_category(task_type).premium_alias_key
}

pub fn task_category_strength(task_type: TaskCategory) -> StrengthTag {
    route_for_category(task_type).strength_tag
}

#[cfg(test)]
mod writes_vox_tests {
    use super::ModelCapabilities;

    #[test]
    fn default_writes_vox_is_false() {
        assert!(!ModelCapabilities::default().writes_vox);
    }

    #[test]
    fn missing_key_deserializes_to_false() {
        // A catalog/bootstrap JSON that omits `writes_vox` must remain
        // back-compat: the bare `#[serde(default)]` yields bool's Default (false).
        let json = r#"{
            "supports_json": true,
            "supports_vision": false,
            "max_context": 8192,
            "tier": "local"
        }"#;
        let caps: ModelCapabilities = serde_json::from_str(json).expect("deserialize");
        assert!(!caps.writes_vox);
    }

    #[test]
    fn explicit_true_deserializes_to_true() {
        let json = r#"{
            "supports_json": true,
            "supports_vision": false,
            "writes_vox": true,
            "max_context": 8192,
            "tier": "local"
        }"#;
        let caps: ModelCapabilities = serde_json::from_str(json).expect("deserialize");
        assert!(caps.writes_vox);
    }

    #[test]
    fn mens_style_seed_reports_writes_vox_true() {
        // Mirrors the MENS seed sites (catalog.rs / registry.rs): a `Local`-tier
        // capability with `writes_vox: true` plus `..Default::default()`.
        let caps = ModelCapabilities {
            tier: super::ModelTier::Local,
            writes_vox: true,
            ..Default::default()
        };
        assert!(caps.writes_vox);
    }
}
