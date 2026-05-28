use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::models::ModelSpec;
use crate::models::spec::ProviderType;

/// Sensitivity level of a task or file context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PrivacyLevel {
    /// Public or non-sensitive internal data.
    Public,
    /// Sensitive internal data, no PII.
    Internal,
    /// Contains PII (Personally Identifiable Information).
    Private,
    /// High-stakes or regulated data (HIPAA, GDPR, etc).
    Regulated,
}

/// Routing policy for different privacy levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyRoutingPolicy {
    /// Minimum model tier required for each level.
    pub min_tier: PrivacyLevel,
    /// Explicitly forbidden providers for sensitive data.
    pub forbidden_providers: HashSet<String>,
    /// Whether local inference is mandatory for Private/Regulated levels.
    pub force_local_for_private: bool,
}

impl Default for PrivacyRoutingPolicy {
    fn default() -> Self {
        let mut forbidden = HashSet::new();
        forbidden.insert("untrusted-provider".to_string());
        Self {
            min_tier: PrivacyLevel::Public,
            forbidden_providers: forbidden,
            force_local_for_private: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyRoutingDecision {
    /// Safe to use public/remote models.
    Allowed,
    /// Must redact PII before sending remote.
    Redact,
    /// Must use local inference only.
    LocalOnly,
    /// Privacy policy blocks this operation.
    Blocked,
}

impl std::fmt::Display for PrivacyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "public"),
            Self::Internal => write!(f, "internal"),
            Self::Private => write!(f, "private"),
            Self::Regulated => write!(f, "regulated"),
        }
    }
}

impl std::fmt::Display for PrivacyRoutingDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allowed => write!(f, "allowed"),
            Self::Redact => write!(f, "redact"),
            Self::LocalOnly => write!(f, "local-only"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

#[must_use]
#[inline]
pub fn model_supports_privacy_local_inference(model: &ModelSpec) -> bool {
    matches!(
        model.provider_type,
        ProviderType::Ollama | ProviderType::VoxLocal
    )
}

/// A router that filters model candidates based on privacy constraints.
pub struct PrivacyRouter {
    pub policy: PrivacyRoutingPolicy,
}

impl PrivacyRouter {
    pub fn new(policy: PrivacyRoutingPolicy) -> Self {
        Self { policy }
    }

    /// Determines the routing decision for a given PII detection.
    pub fn route(&self, pii_detected: bool) -> PrivacyRoutingDecision {
        if pii_detected {
            if self.policy.force_local_for_private {
                PrivacyRoutingDecision::LocalOnly
            } else {
                PrivacyRoutingDecision::Redact
            }
        } else {
            PrivacyRoutingDecision::Allowed
        }
    }

    /// Filter available models based on task privacy level.
    pub fn filter_models(
        &self,
        level: PrivacyLevel,
        candidates: Vec<crate::models::ModelSpec>,
    ) -> Vec<crate::models::ModelSpec> {
        candidates
            .into_iter()
            .filter(|m| {
                // 1. Check forbidden providers
                if self.policy.forbidden_providers.contains(&m.provider) {
                    return false;
                }

                // 2. Enforce local-only for Private+ if configured
                if self.policy.force_local_for_private && level >= PrivacyLevel::Private {
                    if !model_supports_privacy_local_inference(m) {
                        return false;
                    }
                }

                true
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModelCapabilities;
    use crate::models::spec::PricingSource;

    fn dummy_llm(id: &str, provider_type: ProviderType, provider: &str) -> ModelSpec {
        ModelSpec {
            id: id.to_string(),
            canonical_slug: id.to_string(),
            provider: provider.to_string(),
            provider_type,
            max_tokens: 8192,
            cost_per_1k: 0.01,
            cost_per_1k_input: 0.01,
            cost_per_1k_output: 0.01,
            observed_cost_per_1k: None,
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::Bootstrap,
            is_free: false,
            strengths: vec![],
            capabilities: ModelCapabilities::default(),
            supported_parameters: vec![],
        }
    }

    #[test]
    fn local_inference_predicate_tracks_provider_type() {
        assert!(model_supports_privacy_local_inference(&dummy_llm(
            "x",
            ProviderType::Ollama,
            "ollama"
        )));
        assert!(!model_supports_privacy_local_inference(&dummy_llm(
            "z",
            ProviderType::OpenRouter,
            "openrouter"
        )));
    }
}
