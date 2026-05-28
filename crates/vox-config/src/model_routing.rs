use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityWeightsConfig {
    #[serde(default = "qw_socrates")]
    pub socrates_factuality: f64,
    #[serde(default = "qw_contra")]
    pub contradiction_inverse: f64,
    #[serde(default = "qw_success")]
    pub success_rate: f64,
    #[serde(default = "qw_lat")]
    pub p50_latency_inverse: f64,
    #[serde(default = "qw_cost")]
    pub cost_inverse: f64,
}

fn qw_socrates() -> f64 {
    0.25
}
fn qw_contra() -> f64 {
    0.15
}
fn qw_success() -> f64 {
    0.25
}
fn qw_lat() -> f64 {
    0.15
}
fn qw_cost() -> f64 {
    0.2
}

impl Default for QualityWeightsConfig {
    fn default() -> Self {
        Self {
            socrates_factuality: qw_socrates(),
            contradiction_inverse: qw_contra(),
            success_rate: qw_success(),
            p50_latency_inverse: qw_lat(),
            cost_inverse: qw_cost(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoutingConfig {
    #[serde(default)]
    pub quality_weights: QualityWeightsConfig,
    #[serde(default)]
    pub latency_bands: LatencyBands,
    #[serde(default)]
    pub exploration: ExplorationConfig,
    #[serde(default)]
    pub safety: SafetyConfig,
    #[serde(default)]
    pub premium_alias: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBands {
    pub excellent_ms: f64,
    pub poor_ms: f64,
}

impl Default for LatencyBands {
    fn default() -> Self {
        Self {
            excellent_ms: 500.0,
            poor_ms: 8000.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationConfig {
    pub budget_usd_per_day: f64,
}

impl Default for ExplorationConfig {
    fn default() -> Self {
        Self {
            budget_usd_per_day: 50.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    pub max_cost_usd_per_request: f64,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            max_cost_usd_per_request: 5.0,
        }
    }
}

// Load the embedded YAML
pub fn load_model_routing_config() -> ModelRoutingConfig {
    let yaml = include_str!("../../../contracts/orchestration/model-routing.v1.yaml");
    let mut cfg: ModelRoutingConfig = serde_yaml::from_str(yaml).unwrap_or_else(|e| {
        tracing::error!("Failed to parse model-routing.v1.yaml: {}", e);
        // Return default values as fallback
        ModelRoutingConfig {
            quality_weights: QualityWeightsConfig::default(),
            latency_bands: LatencyBands::default(),
            exploration: ExplorationConfig::default(),
            safety: SafetyConfig::default(),
            premium_alias: HashMap::new(),
        }
    });

    // 2026-05-15: pins.v1.yaml is the council-reviewed SSOT for premium_alias.
    // Overlay it on top of routing.yaml so the two stay in sync during migration.
    if let Some(pins) = load_model_pins_config() {
        // Pins win over routing.yaml when both define an alias for the same key.
        for (k, v) in pins.premium_alias {
            cfg.premium_alias.insert(k, v);
        }
    }
    cfg
}

/// Minimal projection of `contracts/orchestration/model-pins.v1.yaml` —
/// only the fields the runtime needs today (premium_alias).
/// Other fields (classifier, version_pins, retired_ids, council_signoff) are
/// consumed by tools at audit time, not the runtime selector.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelPinsConfig {
    #[serde(default)]
    pub premium_alias: HashMap<String, String>,
    #[serde(default)]
    pub retired_ids: Vec<String>,
    #[serde(default)]
    pub classifier: ClassifierPinConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassifierPinConfig {
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub fallback: Option<String>,
    #[serde(default)]
    pub promotion_thresholds: PromotionThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionThresholds {
    #[serde(default = "default_min_successful_calls")]
    pub min_successful_calls: u32,
    #[serde(default = "default_max_p50_latency_multiple")]
    pub max_p50_latency_multiple: f64,
    #[serde(default = "default_min_classifier_confidence")]
    pub min_classifier_confidence: f32,
}

impl Default for PromotionThresholds {
    fn default() -> Self {
        Self {
            min_successful_calls: default_min_successful_calls(),
            max_p50_latency_multiple: default_max_p50_latency_multiple(),
            min_classifier_confidence: default_min_classifier_confidence(),
        }
    }
}

fn default_min_successful_calls() -> u32 {
    30
}
fn default_max_p50_latency_multiple() -> f64 {
    2.0
}
fn default_min_classifier_confidence() -> f32 {
    0.70
}

/// Load `contracts/orchestration/model-pins.v1.yaml` — the council-reviewed
/// SSOT for premium aliases, version pins, and classifier configuration.
/// Returns `None` if the file is missing or unparseable (callers fall back
/// to `model-routing.v1.yaml`).
pub fn load_model_pins_config() -> Option<ModelPinsConfig> {
    let yaml = include_str!("../../../contracts/orchestration/model-pins.v1.yaml");
    match serde_yaml::from_str::<ModelPinsConfig>(yaml) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            tracing::warn!("Failed to parse model-pins.v1.yaml: {e}");
            None
        }
    }
}
