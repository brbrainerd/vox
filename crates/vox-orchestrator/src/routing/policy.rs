//! Embedded `contracts/orchestration/model-routing.v1.yaml` with Clavis overrides.
//!
//! Contract version is carried as `x-vox-version` in the YAML (see file on disk).

use serde::Deserialize;

/// High-level routing objective (aligned with contract `routing_objective.kind`).
#[derive(Debug, Clone, Deserialize)]
pub struct RoutingObjective {
    #[serde(default = "default_objective_kind")]
    pub kind: String,
}

fn default_objective_kind() -> String {
    "quality_first".to_string()
}

impl Default for RoutingObjective {
    fn default() -> Self {
        Self {
            kind: default_objective_kind(),
        }
    }
}

/// Weights for merging Socrates + scoreboard signals (sums need not be 1.0; engine normalizes).
#[derive(Debug, Clone, Deserialize)]
pub struct QualityWeights {
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

impl Default for QualityWeights {
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

/// Thompson / epsilon exploration knobs (overridable via Clavis `VOX_ROUTING_EXPLORATION_EPSILON`).
#[derive(Debug, Clone, Deserialize)]
pub struct ExplorationPolicy {
    #[serde(default = "explore_algo_default")]
    pub algorithm: String,
    #[serde(default = "eps_floor_def")]
    pub epsilon_floor: f64,
    #[serde(default = "eps_ceil_def")]
    pub epsilon_ceiling: f64,
    #[serde(default = "min_samples_def")]
    pub min_samples_per_arm: u32,
    #[serde(default = "max_conc_def")]
    pub max_concurrent_explorations: u32,
    #[serde(default = "budget_def")]
    pub budget_usd_per_day: f64,
    #[serde(default = "novel_boost_def")]
    pub exploration_boost_for_novel: f64,
}

fn explore_algo_default() -> String {
    "thompson_beta".to_string()
}
fn eps_floor_def() -> f64 {
    0.02
}
fn eps_ceil_def() -> f64 {
    0.12
}
fn min_samples_def() -> u32 {
    3
}
fn max_conc_def() -> u32 {
    2
}
fn budget_def() -> f64 {
    50.0
}
fn novel_boost_def() -> f64 {
    0.5
}

impl Default for ExplorationPolicy {
    fn default() -> Self {
        Self {
            algorithm: explore_algo_default(),
            epsilon_floor: eps_floor_def(),
            epsilon_ceiling: eps_ceil_def(),
            min_samples_per_arm: min_samples_def(),
            max_concurrent_explorations: max_conc_def(),
            budget_usd_per_day: budget_def(),
            exploration_boost_for_novel: novel_boost_def(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SafetyCaps {
    #[serde(default = "max_cost_req_def")]
    pub max_cost_usd_per_request: f64,
}

fn max_cost_req_def() -> f64 {
    5.0
}

impl Default for SafetyCaps {
    fn default() -> Self {
        Self {
            max_cost_usd_per_request: max_cost_req_def(),
        }
    }
}

/// Parsed extension block from `model-routing.v1.yaml` (x-vox-version ≥ 2).
#[derive(Debug, Clone, Deserialize)]
pub struct RoutingPolicyExtension {
    #[serde(default)]
    pub routing_objective: RoutingObjective,
    #[serde(default)]
    pub quality_weights: QualityWeights,
    #[serde(default)]
    pub exploration: ExplorationPolicy,
    #[serde(default)]
    pub safety: SafetyCaps,
    #[serde(default)]
    pub fallback_graph: Vec<String>,
}

impl Default for RoutingPolicyExtension {
    fn default() -> Self {
        Self {
            routing_objective: RoutingObjective::default(),
            quality_weights: QualityWeights::default(),
            exploration: ExplorationPolicy::default(),
            safety: SafetyCaps::default(),
            fallback_graph: default_fallback_graph(),
        }
    }
}

fn default_fallback_graph() -> Vec<String> {
    vec![
        "OpenRouter".to_string(),
        "PopuliMesh".to_string(),
        "Ollama".to_string(),
    ]
}

/// Full contract parse: unknown top-level keys are ignored.
#[derive(Debug, Clone, Deserialize)]
struct ModelRoutingYaml {
    #[serde(default)]
    routing_objective: RoutingObjective,
    #[serde(default)]
    quality_weights: QualityWeights,
    #[serde(default)]
    exploration: ExplorationPolicy,
    #[serde(default)]
    safety: SafetyCaps,
    #[serde(default)]
    fallback_graph: Vec<String>,
}

/// Live routing policy: YAML SSOT + Clavis overrides.
#[derive(Debug, Clone)]
pub struct RoutingPolicy {
    pub routing_objective: RoutingObjective,
    pub quality_weights: QualityWeights,
    pub exploration: ExplorationPolicy,
    pub safety: SafetyCaps,
    pub fallback_graph: Vec<String>,
    /// Non-empty → model `provider` must match one of these substrings (lowercase).
    pub provider_allowlist: Vec<String>,
    /// Model `provider` must not match any of these substrings (lowercase).
    pub provider_denylist: Vec<String>,
    /// Optional global hard-pin model id (Clavis `VOX_ROUTING_HARD_PIN_MODEL`).
    pub hard_pin_model_id: Option<String>,
    /// Optional USD ceiling per session (`VOX_ROUTING_MAX_SPEND_USD_PER_SESSION`); when set and
    /// in-process MCP LLM spend meets or exceeds this value, routing forces free-tier models only.
    pub max_spend_usd_per_session: Option<f64>,
}

impl RoutingPolicy {
    /// Provider allow/deny rules from routing policy (Clavis-driven).
    #[must_use]
    pub fn provider_filter_allows(&self, model: &crate::models::ModelSpec) -> bool {
        let prov = model.provider.to_ascii_lowercase();
        let id_lc = model.id.to_ascii_lowercase();
        if self
            .provider_denylist
            .iter()
            .any(|d| prov.contains(d) || id_lc.contains(d))
        {
            return false;
        }
        if self.provider_allowlist.is_empty() {
            return true;
        }
        self.provider_allowlist
            .iter()
            .any(|a| prov.contains(a) || id_lc.contains(a))
    }
    /// Load embedded repo contract and apply Clavis routing overrides.
    #[must_use]
    pub fn load() -> Self {
        const YAML: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/orchestration/model-routing.v1.yaml"
        ));
        let parsed: ModelRoutingYaml =
            serde_yaml::from_str(YAML).expect("parse embedded model-routing.v1.yaml");
        let mut exploration = parsed.exploration;
        if let Some(eps) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRoutingExplorationEpsilon)
                .expose()
        {
            if let Ok(v) = eps.parse::<f64>() {
                if (0.0..=1.0).contains(&v) {
                    exploration.epsilon_ceiling = v;
                }
            }
        }
        let provider_allowlist = parse_csv_lower(
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRoutingProviderAllowlist)
                .expose(),
        );
        let provider_denylist = parse_csv_lower(
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRoutingProviderDenylist).expose(),
        );
        let hard_pin_model_id =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRoutingHardPinModel)
                .expose()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(std::string::ToString::to_string);
        let max_spend_usd_per_session =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRoutingMaxSpendUsdPerSession)
                .expose()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|v| *v > 0.0);

        Self {
            routing_objective: parsed.routing_objective,
            quality_weights: parsed.quality_weights,
            exploration,
            safety: parsed.safety,
            fallback_graph: if parsed.fallback_graph.is_empty() {
                default_fallback_graph()
            } else {
                parsed.fallback_graph
            },
            provider_allowlist,
            provider_denylist,
            hard_pin_model_id,
            max_spend_usd_per_session,
        }
    }
}

fn parse_csv_lower(raw: Option<&str>) -> Vec<String> {
    let Some(s) = raw else {
        return Vec::new();
    };
    s.split(',')
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_embedded_policy_parses() {
        let p = RoutingPolicy::load();
        assert_eq!(p.routing_objective.kind, "quality_first");
        assert!(p.exploration.epsilon_ceiling > 0.0);
        assert!(!p.fallback_graph.is_empty());
    }
}
